// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{bail, Fallible};
use std::{collections::HashMap, path::Path};
use tokio::{
    sync::{mpsc, mpsc::Receiver, oneshot},
    task::{spawn, JoinHandle},
};
use tracing::error;
use yggdrasil::{ConcretePath, Tree, TreeBuilder, Value};

#[derive(Debug)]
pub struct TreeServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: TreeMailbox,
}

impl TreeServer {
    pub async fn launch(filename: &Path) -> Fallible<Self> {
        let filename = filename.to_path_buf();
        let (mailbox, mut mailbox_receiver) = mpsc::channel(16);
        let task = spawn(async move {
            let mut tree = match TreeBuilder::default().build_from_file(&filename) {
                Ok(tree) => tree,
                Err(e) => {
                    error!("Failed to parse configuration:");
                    error!("{:?}", e.backtrace());
                    bail!("failed to parse configuration")
                }
            };

            while let Some(message) = mailbox_receiver.recv().await {
                let result = Self::handle_message(message, &mut mailbox_receiver, &mut tree);
                if let Err(e) = result {
                    error!("Error: {}", e);
                    error!("{}", e.backtrace());
                }
            }

            Ok(())
        });

        Ok(Self {
            task,
            mailbox: TreeMailbox { mailbox },
        })
    }

    fn handle_message<T>(
        message: TreeServerProtocol,
        mailbox_receiver: &mut Receiver<T>,
        tree: &mut Tree,
    ) -> Fallible<()> {
        match message {
            TreeServerProtocol::FindSources(name, tx) => {
                tx.send(tree.find_sources(&name)).ok();
            }
            TreeServerProtocol::FindSinks(name, tx) => {
                tx.send(tree.find_sinks(&name)).ok();
            }
            TreeServerProtocol::PathExists(path, tx) => {
                tx.send(tree.lookup_path(&path).is_ok()).ok();
            }
            TreeServerProtocol::Compute(path, tx) => {
                tx.send(tree.lookup_path(&path)?.compute(&tree)?).ok();
            }
            TreeServerProtocol::HandleEvent(path, value, tx) => {
                match tree.handle_event(&path, value) {
                    Ok(result) => {
                        tx.send(result).ok();
                    }
                    Err(e) => {
                        error!("failed to handle_event: {}", e);
                        tx.send(HashMap::new()).ok();
                    }
                }
            }
            TreeServerProtocol::Finish => {
                mailbox_receiver.close();
            }
        }
        Ok(())
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    pub fn mailbox(&self) -> TreeMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug, Clone)]
pub struct TreeMailbox {
    mailbox: mpsc::Sender<TreeServerProtocol>,
}

#[derive(Debug)]
enum TreeServerProtocol {
    FindSources(String, oneshot::Sender<Vec<ConcretePath>>),
    FindSinks(String, oneshot::Sender<Vec<ConcretePath>>),
    PathExists(ConcretePath, oneshot::Sender<bool>),
    Compute(ConcretePath, oneshot::Sender<Value>),
    HandleEvent(
        ConcretePath,
        Value,
        oneshot::Sender<HashMap<String, Vec<(ConcretePath, Value)>>>,
    ),
    Finish,
}

impl TreeMailbox {
    pub async fn find_sources(&mut self, name: &str) -> Fallible<Vec<ConcretePath>> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeServerProtocol::FindSources(name.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn find_sinks(&mut self, name: &str) -> Fallible<Vec<ConcretePath>> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeServerProtocol::FindSinks(name.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn path_exists(&mut self, path: &ConcretePath) -> Fallible<bool> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeServerProtocol::PathExists(path.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn compute(&mut self, path: &ConcretePath) -> Fallible<Value> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeServerProtocol::Compute(path.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn handle_event(
        &mut self,
        path: &ConcretePath,
        event: Value,
    ) -> Fallible<HashMap<String, Vec<(ConcretePath, Value)>>> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeServerProtocol::HandleEvent(path.to_owned(), event, tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(TreeServerProtocol::Finish).await?;
        Ok(())
    }
}
