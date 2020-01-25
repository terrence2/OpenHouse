// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::HueMailbox;
use failure::Fallible;
use std::collections::HashMap;
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
};
use tracing::trace;
use yggdrasil::{ConcretePath, Value};

pub struct UpdateServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: UpdateMailbox,
}

impl UpdateServer {
    pub async fn launch(mut hue: HueMailbox) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            while let Some(message) = mailbox_receiver.recv().await {
                match message {
                    UpdateServerProtocol::ApplyUpdates(updates) => {
                        trace!("updating sinks with {:?}", updates);
                        if let Some(values) = updates.get("hue") {
                            hue.values_updated(values).await?;
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
