// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::{collections::HashMap, path::Path};
use tokio::{
    sync::{mpsc, mpsc::Receiver, oneshot},
    task::{spawn, JoinHandle},
};
use tracing::error;
use yggdrasil::{ConcretePath, TreeBuilder, Tree, Value};

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
            let mut tree = TreeBuilder::default().build_from_file(&filename)?;

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

    fn handle_message<T>(message: TreeServerProtocol, mailbox_receiver: &mut Receiver<T>, tree: &mut Tree) -> Fallible<()> {
        match message {
            TreeServerProtocol::FindSources(name, tx) => {
                tx.send(tree.find_sources(&name)).ok();
            }
            TreeServerProtocol::FindSinks(name, tx) => {
                tx.send(tree.find_sinks(&name)).ok();
            },
            TreeServerProtocol::Compute(path, tx) => {
                tx.send(tree.lookup_path(&path)?.compute(&tree)?).ok();
            }
            TreeServerProtocol::HandleEvent(path, value, tx) => {
                tx.send(tree.handle_event(&path, value)?).ok();
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
