// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{TreeMailbox, UpdateMailbox};
use chrono::{DateTime, Datelike, Local, Timelike};
use failure::{bail, Fallible};
use futures::future::{select, Either};
use std::collections::HashMap;
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
    time::{delay_for, Duration},
};
use tracing::trace;
use yggdrasil::Value;

/**
 * Example usage:
 * sys
 *    time
 *        seconds
 *            yearly
 *                ^clock
 *                interval <- "second"
 *                wrap <- "yearly"
 */

#[derive(Clone, Debug)]
enum ClockInterval {
    Second,
    Minute,
    Hour,
}

impl ClockInterval {
    fn from_str(s: &str) -> Fallible<ClockInterval> {
        Ok(match s {
            "second" => ClockInterval::Second,
            "minute" => ClockInterval::Minute,
            "hour" => ClockInterval::Hour,
            _ => bail!("unknown interval for clock: {}", s),
        })
    }

    fn convert_seconds(&self, seconds: i64) -> i64 {
        match self {
            ClockInterval::Second => seconds,
            ClockInterval::Minute => seconds / 60,
            ClockInterval::Hour => seconds / (60 * 60),
        }
    }
}

#[derive(Clone, Debug)]
enum ClockWrap {
    Never,
    Yearly,
    Monthly,
    Weekly,
    Daily,
    Hourly,
    Minutly,
}

impl ClockWrap {
    fn from_str(s: &str) -> Fallible<ClockWrap> {
        Ok(match s {
            "never" => ClockWrap::Never,
            "yearly" => ClockWrap::Yearly,
            "monthly" => ClockWrap::Monthly,
            "weekly" => ClockWrap::Weekly,
            "daily" => ClockWrap::Daily,
            "hourly" => ClockWrap::Hourly,
            "minutly" => ClockWrap::Minutly,
            _ => bail!("unknown wrap mode for clock: {}", s),
        })
    }

    fn seconds(&self, now: &DateTime<Local>) -> i64 {
        const SECS_PER_DAY: u32 = 60 * 60 * 24;
        match self {
            ClockWrap::Minutly => i64::from(now.time().second()),
            ClockWrap::Hourly => i64::from(now.time().minute() * 60 + now.time().second()),
            ClockWrap::Daily => i64::from(now.time().num_seconds_from_midnight()),
            ClockWrap::Weekly => {
                let day = now.date().weekday().num_days_from_sunday(); // day of week, 0 on sunday
                i64::from(day * SECS_PER_DAY + now.time().num_seconds_from_midnight())
            }
            ClockWrap::Monthly => {
                let day = now.date().day0(); // day of month, 0 based
                i64::from(day * SECS_PER_DAY + now.time().num_seconds_from_midnight())
            }
            ClockWrap::Yearly => {
                let day = now.date().ordinal() - 1; // day of year, 1 based
                i64::from(day * SECS_PER_DAY + now.time().num_seconds_from_midnight())
            }
            ClockWrap::Never => now.timestamp(),
        }
    }
}

#[derive(Clone, Debug)]
struct ClockDef {
    interval: ClockInterval,
    wrap: ClockWrap,
    last_value: i64,
}

impl ClockDef {
    fn new(interval: ClockInterval, wrap: ClockWrap) -> Self {
        ClockDef {
            interval,
            wrap,
            last_value: 0,
        }
    }

    fn value(&self, now: &DateTime<Local>) -> i64 {
        self.interval.convert_seconds(self.wrap.seconds(now))
    }

    fn tick(&mut self, now: &DateTime<Local>) -> Option<i64> {
        let next_value = self.value(now);
        if next_value != self.last_value {
            self.last_value = next_value;
            return Some(next_value);
        }
        None
    }
}

pub struct ClockServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: ClockMailbox,
}

impl ClockServer {
    pub async fn launch(mut update: UpdateMailbox, mut tree: TreeMailbox) -> Fallible<Self> {
        let mut clock_map = HashMap::new();
        for path in &tree.find_sources("clock").await? {
            let interval = tree.compute(&(path / "interval")).await?.as_string()?;
            let wrap = tree.compute(&(path / "wrap")).await?.as_string()?;
            let clock_def = ClockDef::new(
                ClockInterval::from_str(&interval)?,
                ClockWrap::from_str(&wrap)?,
            );
            clock_map.insert(path.to_owned(), clock_def);
        }

        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            let mut mailbox_recv = Box::pin(mailbox_receiver.recv());
            loop {
                match select(delay_for(Duration::from_secs(1)), mailbox_recv).await {
                    Either::Right((maybe_message, _delay)) => {
                        if let Some(message) = maybe_message {
                            match message {
                                ClockServerProtocol::Finish => {
                                    // Note: we borrowed the mailbox above by storing the recv across the loop,
                                    // so we can't reborrow here to close it. Luckily this system can only take
                                    // a single Finish message, so there's not much point closing cleanly.
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    Either::Left(((), mailbox_unrecv)) => {
                        mailbox_recv = mailbox_unrecv;
                        let now = Local::now();
                        for (path, clock_def) in clock_map.iter_mut() {
                            if let Some(v) = clock_def.tick(&now) {
                                trace!("{} timed out", path.to_string());
                                let updates =
                                    tree.handle_event(path, Value::from_integer(v)).await?;
                                update.apply_updates(updates).await?;
                            }
                        }
                    }
                }
            }
            Ok(())
        });
        Ok(Self {
            task,
            mailbox: ClockMailbox { mailbox },
        })
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    pub fn mailbox(&self) -> ClockMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum ClockServerProtocol {
    Finish,
}

#[derive(Clone, Debug)]
pub struct ClockMailbox {
    mailbox: Sender<ClockServerProtocol>,
}

impl ClockMailbox {
    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(ClockServerProtocol::Finish).await?;
        Ok(())
    }
}
