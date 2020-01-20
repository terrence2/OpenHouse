// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::path::Path;
use tokio::{
    sync::{mpsc, oneshot},
    task::{spawn, JoinHandle},
};
use tracing::error;
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
            let tree = TreeBuilder::default().build_from_file(&filename)?;

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
        self.mailbox.send(TreeProtocol::FindSinks(name.to_owned(), tx)).await?;
        Ok(rx.await?)
    }

    pub async fn compute(&mut self, path: &ConcretePath) -> Fallible<Value> {
        let (tx, rx) = oneshot::channel();
        self.mailbox
            .send(TreeProtocol::Compute(path.to_owned(), tx))
            .await?;
        Ok(rx.await?)
    }

    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(TreeProtocol::Finish).await?;
        Ok(())
    }
}
