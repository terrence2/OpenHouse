// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::{
    fut::wrap_future, Actor, ActorContext, Addr, Arbiter, AsyncContext, Context, Handler, Message,
    System,
};
use actix_web::{actix, client, HttpMessage};
use bytes::Bytes;
use failure::{err_msg, Fallible};
use futures::Future;
use std::collections::HashMap;
use yggdrasil::{ConcretePath, SinkRef, SubTree, TreeSink, Value, ValueType};

pub struct Hue {
    address: Option<String>,
    username: Option<String>,
    paths: HashMap<String, String>,
    worker: Option<Addr<HueWorker>>,
}

impl Hue {
    pub fn new() -> Fallible<Box<Self>> {
        //let worker = HueWorker {};
        Ok(Box::new(Hue {
            address: None,
            username: None,
            paths: HashMap::new(),
            worker: None//worker.start(),
        }))
    }
}

impl TreeSink for Hue {
    fn nodetype(&self, path: &str, tree: &SubTree) -> Fallible<ValueType> {
        return Ok(ValueType::STRING);
    }

    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()> {
        let concrete = ConcretePath::from_str(path)?;
        let basename = concrete.basename();

        if basename == "hue-bridge" {
            let address = tree.lookup("/address")?.compute(tree.tree())?.as_string()?;
            let username = tree.lookup("/username")?.compute(tree.tree())?.as_string()?;
            self.address = Some(address);
            self.username = Some(username);
            return Ok(());
        }

        self.paths.insert(path.to_owned(), basename.to_owned());
        return Ok(());
    }

    fn on_ready(&mut self, tree: &SubTree) -> Fallible<()> {
        if self.address.is_none() || self.username.is_none() {
            warn!("hue system: no hub defined; not starting");
            return Ok(());
        }
        let hub = Hub {
            address: self.address.clone().unwrap(),
            username: self.username.clone().unwrap(),
        };
        let worker = HueWorker { hub };
        self.worker = Some(worker.start());
        return Ok(());
    }

    fn values_updated(&mut self, values: &Vec<(String, Value)>) -> Fallible<()> {
        // if let Some(worker) = self.worker {
        //     worker.send(ValuesUpdated { values });
        // }
        return Ok(());
    }
}

struct Hub {
    address: String,
    username: String,
}

impl Hub {
    fn url(&self, path: &str) -> String {
        return format!("http://{}/api/{}{}", self.address, self.username, path);
    }
}

struct HueWorker {
    hub: Hub,
}

impl HueWorker {}

#[derive(Deserialize, Debug)]
struct Foobar {
    name: String,
}

impl Actor for HueWorker {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        Arbiter::spawn(
            client::get(self.hub.url(""))   // <- Create request builder
                .header("User-Agent", "Actix-web")
                .finish()
                .expect("failed to build request")
                .send()
                .map_err(|e| {
                    error!("hue system: cannot reach hue bridge while starting up: {}", e);
                    System::current().stop();
                    ()
                })
                .and_then(|response| {
                    println!("Response: {:?}", response);
                    response.body()
                        .limit(1024 * 1024 * 1024)
                        .map_err(|e| { warn!("hue system: read failed: {}", e); () } )
                        .and_then(|bytes: Bytes| {
                            println!("==== BODY ==== {:?}", bytes);
                            Ok(())
                        })
                }),
        );
    }
}

// struct Initialize {
//     address: String,
//     username: String,
//     paths: HashMap<String, String>,
// }

// impl Message for Initialize {
//     type Result = Fallible<()>;
// }

// impl Handler<Initialize> for HueWorker {
//     type Result = Fallible<()>;

//     fn handle(&mut self, msg: Initialize, ctx: &mut Context<Self>) -> Self::Result {
//         println!("Initializing...");

//         Ok(())
//     }
// }

#[cfg(test)]
mod test {
    use super::*;
    use actix::System;

    #[test]
    fn test_new_sink() -> Fallible<()> {
        let hue = Hue::new();
        return Ok(());
    }

    #[test]
    fn test_new_worker() -> Fallible<()> {
        return Ok(());
    }
}
