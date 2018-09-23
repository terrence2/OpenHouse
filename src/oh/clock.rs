// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::{Actor, Addr, AsyncContext, Context, Message};
use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use failure::{err_msg, Fallible};
use oh::{DBServer, TickEvent};
use std::{collections::HashMap, time::Duration as StdDuration};
use yggdrasil::{SubTree, Tree, TreeSource, Value, ValueType};

pub(crate) const CLOCK_PRELUDE: &'static str = r#"
sys
    time
        seconds
            unix
                ^clock
                interval <- "second"
                wrap <- "never"
            yearly
                ^clock
                interval <- "second"
                wrap <- "yearly"
            monthly
                ^clock
                interval <- "second"
                wrap <- "monthly"
            weekly
                ^clock
                interval <- "second"
                wrap <- "weekly"
            daily
                ^clock
                interval <- "second"
                wrap <- "daily"
            hourly
                ^clock
                interval <- "second"
                wrap <- "hourly"
            minutly
                ^clock
                interval <- "second"
                wrap <- "minutly"
        minutes
            unix
                ^clock
                interval <- "minute"
                wrap <- "never"
            yearly
                ^clock
                interval <- "minute"
                wrap <- "yearly"
            monthly
                ^clock
                interval <- "minute"
                wrap <- "monthly"
            weekly
                ^clock
                interval <- "minute"
                wrap <- "weekly"
            daily
                ^clock
                interval <- "minute"
                wrap <- "daily"
            hourly
                ^clock
                interval <- "minute"
                wrap <- "hourly"
        hours
            unix
                ^clock
                interval <- "hour"
                wrap <- "never"
            yearly
                ^clock
                interval <- "hour"
                wrap <- "yearly"
            monthly
                ^clock
                interval <- "hour"
                wrap <- "monthly"
            weekly
                ^clock
                interval <- "hour"
                wrap <- "weekly"
            daily
                ^clock
                interval <- "hour"
                wrap <- "daily"
"#;

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
            ClockWrap::Minutly => now.time().second() as i64,
            ClockWrap::Hourly => (now.time().minute() * 60 + now.time().second()) as i64,
            ClockWrap::Daily => now.time().num_seconds_from_midnight() as i64,
            ClockWrap::Weekly => {
                let day = now.date().weekday().num_days_from_sunday(); // day of week, 0 on sunday
                (day * SECS_PER_DAY + now.time().num_seconds_from_midnight()) as i64
            }
            ClockWrap::Monthly => {
                let day = now.date().day0(); // day of month, 0 based
                (day * SECS_PER_DAY + now.time().num_seconds_from_midnight()) as i64
            }
            ClockWrap::Yearly => {
                let day = now.date().ordinal() - 1; // day of year, 1 based
                (day * SECS_PER_DAY + now.time().num_seconds_from_midnight()) as i64
            }
            ClockWrap::Never => now.timestamp(),
        }
    }
}

struct ClockDef {
    interval: ClockInterval,
    wrap: ClockWrap,
    last_value: i64,
}

impl ClockDef {
    fn new(interval: ClockInterval, wrap: ClockWrap) -> Self {
        let now = Local::now();
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
        return None;
    }
}

pub struct Clock {
    clocks: HashMap<String, ClockDef>,
}

impl Clock {
    pub fn new() -> Fallible<Box<Self>> {
        Ok(Box::new(Self {
            clocks: HashMap::new(),
        }))
    }

    pub fn handle_tick(&mut self) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        let now = Local::now();
        for (path, clock) in self.clocks.iter_mut() {
            if let Some(value) = clock.tick(&now) {
                out.push((path.to_owned(), value));
            }
        }
        return out;
    }
}

impl TreeSource for Clock {
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()> {
        let interval = tree.lookup("/interval")?.compute(tree.tree())?.as_string()?;
        let wrap = tree.lookup("/wrap")?.compute(tree.tree())?.as_string()?;
        let def = ClockDef::new(
            ClockInterval::from_str(&interval)?,
            ClockWrap::from_str(&wrap)?,
        );
        self.clocks.insert(path.to_owned(), def);
        return Ok(());
    }

    fn nodetype(&self, _path: &str, _tree: &SubTree) -> Fallible<ValueType> {
        return Ok(ValueType::INTEGER);
    }

    fn get_all_possible_values(&self, _path: &str, _tree: &SubTree) -> Fallible<Vec<Value>> {
        // FIXME: this should be possible -- need to implement integer ranges
        bail!("compilation error: a time value flowed into a path")
    }

    fn handle_event(&mut self, path: &str, value: Value, _tree: &SubTree) -> Fallible<()> {
        ensure!(
            self.clocks[path].last_value == value.as_integer()?,
            "runtime error: clock event value does not match cached value"
        );
        return Ok(());
    }

    fn get_value(&self, path: &str, _tree: &SubTree) -> Option<Value> {
        trace!("CLOCK: get_value @ {}", path);
        return Some(Value::Integer(self.clocks[path].last_value));
    }
}

pub struct TickWorker {
    db_addr: Addr<DBServer>,
}

impl Actor for TickWorker {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.run_interval(StdDuration::from_millis(500), move |act, _| {
            act.handle_tick()
        });
    }
}

impl TickWorker {
    pub(crate) fn new(db_addr: &Addr<DBServer>) -> Self {
        TickWorker {
            db_addr: db_addr.to_owned(),
        }
    }

    fn handle_tick(&self) {
        self.db_addr.do_send(TickEvent {});
    }
}