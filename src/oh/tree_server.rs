// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::path::Path;
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
};
use yggdrasil::TreeBuilder;

#[derive(Debug)]
enum TreeProtocol {
    Poke(i32),
    Finish,
}

#[derive(Debug)]
pub struct TreeServer {
    task: JoinHandle<()>,
    mailbox: TreeMailbox,
}

impl TreeServer {
    pub async fn launch(filename: &Path) -> Fallible<Self> {
        let filename = filename.to_path_buf();
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            let tree = TreeBuilder::default().build_from_file(&filename).unwrap();

            loop {
                if let Some(message) = mailbox_receiver.recv().await {
                    match message {
                        TreeProtocol::Poke(data) => println!("Hi from TreeServer: {}", data),
                        TreeProtocol::Finish => break,
                    }
                }
            }
        });

        Ok(Self {
            task,
            mailbox: TreeMailbox { mailbox },
        })
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await?;
        Ok(())
    }

    pub fn mailbox(&self) -> TreeMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug, Clone)]
pub struct TreeMailbox {
    mailbox: Sender<TreeProtocol>,
}

impl TreeMailbox {
    pub async fn poke(&mut self, data: i32) -> Fallible<()> {
        self.mailbox.send(TreeProtocol::Poke(data)).await?;
        Ok(())
    }

    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(TreeProtocol::Finish).await?;
        Ok(())
    }
}
