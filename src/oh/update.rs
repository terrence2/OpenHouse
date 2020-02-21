// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{HueMailbox, RedstoneMailbox};
use failure::Fallible;
use std::collections::HashMap;
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
};
use tracing::{error, trace};
use yggdrasil::{ConcretePath, Value};

pub struct UpdateServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: UpdateMailbox,
}

impl UpdateServer {
    pub async fn launch() -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            let mut maybe_hue = None;
            let mut maybe_redstone = None;
            while let Some(message) = mailbox_receiver.recv().await {
                match message {
                    UpdateServerProtocol::SetHueMailbox(hue_mailbox) => {
                        maybe_hue = Some(hue_mailbox);
                    }
                    UpdateServerProtocol::SetRedstoneMailbox(redstone_mailbox) => {
                        maybe_redstone = Some(redstone_mailbox);
                    }
                    UpdateServerProtocol::ApplyUpdates(updates) => {
                        trace!("updating sinks from {} subsystems", updates.len());
                        if let Some(ref hue) = maybe_hue {
                            if let Some(values) = updates.get("hue") {
                                trace!("updating {} values in hue subsystem", values.len());
                                let mut hue = hue.to_owned();
                                match hue.values_updated(values).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!(
                                            "failed to update hue values: {}\n{}",
                                            e,
                                            e.backtrace()
                                        );
                                    }
                                }
                            }
                        }
                        if let Some(ref redstone) = maybe_redstone {
                            if let Some(values) = updates.get("redstone") {
                                trace!("updating {} values in redstone subsystem", values.len());
                                let mut redstone = redstone.to_owned();
                                for (path, value) in values {
                                    match redstone
                                        .set_property(path.to_owned(), value.to_owned())
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(e) => {
                                            error!(
                                                "failed to update redstone property: {}\n{}",
                                                e,
                                                e.backtrace()
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    UpdateServerProtocol::Finish => mailbox_receiver.close(),
                }
            }
            Ok(())
        });
        Ok(Self {
            task,
            mailbox: UpdateMailbox { mailbox },
        })
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    pub fn mailbox(&self) -> UpdateMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum UpdateServerProtocol {
    ApplyUpdates(HashMap<String, Vec<(ConcretePath, Value)>>),
    SetHueMailbox(HueMailbox),
    SetRedstoneMailbox(RedstoneMailbox),
    Finish,
}

#[derive(Clone, Debug)]
pub struct UpdateMailbox {
    mailbox: Sender<UpdateServerProtocol>,
}

impl UpdateMailbox {
    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(UpdateServerProtocol::Finish).await?;
        Ok(())
    }

    pub async fn set_redstone(&mut self, redstone: RedstoneMailbox) -> Fallible<()> {
        self.mailbox
            .send(UpdateServerProtocol::SetRedstoneMailbox(redstone))
            .await?;
        Ok(())
    }

    pub async fn set_hue(&mut self, hue: HueMailbox) -> Fallible<()> {
        self.mailbox
            .send(UpdateServerProtocol::SetHueMailbox(hue))
            .await?;
        Ok(())
    }

    pub async fn apply_updates(
        &mut self,
        updates: HashMap<String, Vec<(ConcretePath, Value)>>,
    ) -> Fallible<()> {
        self.mailbox
            .send(UpdateServerProtocol::ApplyUpdates(updates))
            .await?;
        Ok(())
    }
}
