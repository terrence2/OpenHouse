// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::TreeMailbox;
use bytes::BytesMut;
use failure::Fallible;
use hyper::{
    body::HttpBody,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use std::{
    borrow::Borrow,
    collections::HashMap,
    convert::Infallible,
    error::Error,
    net::{IpAddr, SocketAddr},
};
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
};
use tracing::{info, trace, warn};
use yggdrasil::Value;

pub struct LegacyMcu {
    task: JoinHandle<Fallible<()>>,
    mailbox: LegacyMcuMailbox,
}

async fn read_body(mut req: Request<Body>) -> String {
    let mut data = BytesMut::new();
    while !req.body().is_end_stream() {
        if let Some(Ok(content)) = req.body_mut().data().await {
            data.extend_from_slice(&content.slice(..));
        }
    }
    String::from_utf8_lossy(data.as_ref()).to_string()
}

impl LegacyMcu {
    pub async fn launch(host: IpAddr, port: u16, mut tree: TreeMailbox) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            let mut path_map = HashMap::new();
            for source_path in &tree.find_sources("legacy-mcu").await? {
                let ip_addr = tree
                    .compute(&(source_path / "ip"))
                    .await?
                    .as_string()?
                    .parse::<IpAddr>()?;
                trace!("Mapping {} => {}", ip_addr, source_path);
                path_map.insert(ip_addr, source_path.to_owned());
            }

            let make_svc = make_service_fn(move |socket: &AddrStream| {
                let tree = tree.clone();
                let remote_addr = socket.remote_addr();
                let maybe_path = path_map.get(&remote_addr.ip()).cloned();
                if maybe_path.is_none() {
                    warn!("Missing path info on connection: {:?}", socket);
                }
                async move {
                    Ok::<_, Infallible>(service_fn(move |mut req: Request<Body>| {
                        let mut tree = tree.clone();
                        let maybe_path = maybe_path.clone();
                        return async move {
                            if let Some(ref path) = maybe_path {
                                let command = read_body(req).await;
                                if let Ok(updates) =
                                    tree.handle_event(&path, Value::from_string(command)).await
                                {
                                    // FIXME: forward here
                                    println!("UPDATES: {:?}", updates);
                                }
                            } else {
                                warn!("Skipping LegacyMCU request: {:?}", req);
                            }
                            Ok::<_, Infallible>(Response::new(Body::empty()))
                        };
                    }))
                }
            });
            let addr = SocketAddr::from((host, port));
            info!("LegacyMCU listening on {}", addr);
            let server = Server::bind(&addr).serve(make_svc);
            let handle = spawn(server);

            loop {
                if let Some(message) = mailbox_receiver.recv().await {
                    match message {
                        LegacyMcuProtocol::Finish => break,
                    }
                }
            }

            Ok(())
        });
        Ok(Self {
            task,
            mailbox: LegacyMcuMailbox { mailbox },
        })
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    pub fn mailbox(&self) -> LegacyMcuMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum LegacyMcuProtocol {
    Finish,
}

#[derive(Clone, Debug)]
pub struct LegacyMcuMailbox {
    mailbox: Sender<LegacyMcuProtocol>,
}

impl LegacyMcuMailbox {
    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(LegacyMcuProtocol::Finish).await?;
        Ok(())
    }
}
