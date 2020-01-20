// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use bytes::BytesMut;
use failure::Fallible;
use std::{borrow::Borrow, net::{IpAddr, SocketAddr}, collections::HashMap, convert::Infallible, error::Error};
use crate::oh::TreeMailbox;
use hyper::{service::{make_service_fn, service_fn}, body::HttpBody, Body, Request, Response, Server, server::conn::AddrStream};
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
};
use tracing::trace;

/*
pub struct LegacyMCU {
    pub path_map: HashMap<IpAddr, ConcretePath>,
}

impl LegacyMCU {
    pub fn new(tree: &Tree) -> Fallible<Self> {
        let mut path_map = HashMap::new();
        for path in &tree.find_sources("legacy-mcu") {
            let ip = tree
                .lookup(path)?
                .child("ip")?
                .compute(tree)?
                .as_string()?
                .parse::<IpAddr>()?;
            path_map.insert(ip, path.to_owned());
        }
        Ok(Self { path_map })
    }
}
*/

pub struct LegacyMcu {
    task: JoinHandle<Fallible<()>>,
    mailbox: LegacyMcuMailbox,
}

//async fn handle(_: Request<Body>) -> Result<Response<Body>, Infallible> {
//    Ok(Response::new("Hello, World!".into()))
//}

impl LegacyMcu {
    pub async fn launch(host: IpAddr, port: u16, mut tree: TreeMailbox) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            let mut path_map = HashMap::new();
            for source_path in &tree.find_sources("legacy-mcu").await? {
                let ip_addr = tree.compute(&(source_path / "ip")).await?.as_string()?.parse::<IpAddr>()?;
                trace!("Mapping {} => {}", ip_addr, source_path);
                path_map.insert(ip_addr, source_path.to_owned());
            }

            let addr = SocketAddr::from((host, port));
//            let make_svc = make_service_fn(|_conn| async {
//                Ok::<_, Infallible>(service_fn(handle))
//            });
//            let make_svc = make_service_fn(|conn: &AddrStream| async {
//                println!("REMOTE: {:?}", conn.remote_addr());
//                async move {
//                    Ok::<_, Infallible>(service_fn(|req| async {
//                        Ok::<_, Infallible>(Response::new(Body::from("Hello World")))
//                    }))
//                }
//            });

            let make_svc = make_service_fn(move |socket: &AddrStream| {
                let remote_addr = socket.remote_addr();
                println!("PATH MAP: {:?}", path_map);
                println!("ADDR: {}", remote_addr.ip());
                let path = path_map[&remote_addr.ip()].clone();
                println!("APTH: {}", path);
                async move {
                    Ok::<_, Infallible>(service_fn(move |mut req: Request<Body>| async move {
                        println!("REQ: {:?}", req);
                        let mut data = BytesMut::new();
                        while !req.body().is_end_stream() {
                            if let Some(Ok(content)) = req.body_mut().data().await {
                                data.extend_from_slice(&content.slice(..));
                            }
                        }
                        let command = String::from_utf8_lossy(data.as_ref());

                        Ok::<_, Infallible>(
                            Response::new(Body::from(format!("Hello, {}!", remote_addr)))
                        )
                    }))
                }
            });
            let server = Server::bind(&addr).serve(make_svc);
            let handle = spawn(server);


            /*
            if let Err(e) = server.await {
                eprintln!("server error: {}", e);
            }
            */

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
