// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::{collections::HashMap, path::Path};
use tokio::{
    sync::{mpsc, oneshot},
    task::{spawn, JoinHandle},
};
use yggdrasil::{ConcretePath, TreeBuilder, Value};

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

            loop {
                if let Some(message) = mailbox_receiver.recv().await {
                    match message {
                        TreeProtocol::FindSources(name, tx) => {
                            tx.send(tree.find_sources(&name)).expect("nothing bad")
                        }
                        TreeProtocol::FindSinks(name, tx) => {
                            tx.send(tree.find_sinks(&name)).expect("nothing bad")
                        }
                        TreeProtocol::Compute(path, tx) => tx
                            .send(tree.lookup_path(&path)?.compute(&tree)?)
                            .expect("nothing bad"),
                        TreeProtocol::HandleEvent(path, value, tx) => tx
                            .send(tree.handle_event(&path, value)?)
                            .expect("nothing bad"),
                        TreeProtocol::Finish => break,
                    }
                }
            }

            Ok(())
        });

        Ok(Self {
            task,
            mailbox: TreeMailbox { mailbox },
        })
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
    mailbox: mpsc::Sender<TreeProtocol>,
}

#[derive(Debug)]
enum TreeProtocol {
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
            .send(TreeProtocol::FindSources(name.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn find_sinks(&mut self, name: &str) -> Fallible<Vec<ConcretePath>> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeProtocol::FindSinks(name.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn compute(&mut self, path: &ConcretePath) -> Fallible<Value> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeProtocol::Compute(path.to_owned(), tx))
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
            .send(TreeProtocol::HandleEvent(path.to_owned(), event, tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(TreeProtocol::Finish).await?;
        Ok(())
    }
}
