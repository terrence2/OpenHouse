// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::{Actor, Addr, AsyncContext, Context, Handler};
use chrono::Duration;
use failure::{bail, ensure, Fallible};
use log::trace;
use oh::{DBServer, ServerStarted};
use std::collections::HashMap;
use yggdrasil::{SubTree, TreeSource, Value, ValueType};

pub struct MotionDetector {
    // Lags raw_state by the configured delay.
    filtered_state: bool,

    // What the device has given us. PIR sensors may or may not be configurable on
    // the device itself with a delay period, but even if so, it's better to be able
    // to tweak the setting in config.
    raw_state: bool,

    // How long to wait after activity disappears from the detector before we actually
    // set the device state to 'off'.
    delay: Duration,
}

impl Default for MotionDetector {
    fn default() -> Self {
        Self {
            filtered_state: false,
            raw_state: false,
            delay: Duration::seconds(5 * 60),
        }
    }
}

pub enum Device {
    MotionDetector(MotionDetector),
}

#[derive(Debug, Clone)]
pub struct DeviceLocation {
    host: String,
    port: Option<u16>,
}

impl DeviceLocation {
    pub fn url(&self) -> String {
        match self.port {
            Some(port) => format!("https://{}:{}/redstone", self.host, port),
            None => format!("https://{}/redstone", self.host),
        }
    }
}

pub struct Redstone {
    worker: Option<Addr<RedstoneWorker>>,
    devices: HashMap<String, Device>,
    path_map: HashMap<String, DeviceLocation>,
}

impl Redstone {
    pub fn new() -> Fallible<Box<Self>> {
        Ok(Box::new(Self {
            worker: None,
            devices: HashMap::new(),
            path_map: HashMap::new(),
        }))
    }

    pub fn connect_to_db(&mut self, db_addr: &Addr<DBServer>) -> Fallible<()> {
        self.worker
            .as_ref()
            .expect("got connect_to_db before startup")
            .send(ServerStarted {
                db_addr: db_addr.to_owned(),
            })
            .wait()?
    }

    pub fn handle_event(&mut self) -> Vec<(String, i64)> {
        let out = Vec::new();
        //        let now = Local::now();
        //        for (path, clock) in &mut self.clocks {
        //            if let Some(value) = clock.tick(&now) {
        //                out.push((path.to_owned(), value));
        //            }
        //        }
        return out;
    }
}

impl TreeSource for Redstone {
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()> {
        // Look up the host and port in our subtree so that we can talk to it.
        let host = tree.lookup("/host")?.compute(tree.tree())?.as_string()?;
        let port = match tree.lookup("/port") {
            Ok(node) => Some(node.compute(tree.tree())?.as_integer()? as u16),
            Err(_) => None,
        };
        self.path_map
            .insert(path.to_owned(), DeviceLocation { host, port });

        // We need to know the kind of device in order to allow use in paths,
        // but we also want to be able to start if our devices are out of range,
        // battery, service, etc. Thus we have to take the kind here.
        let kind = tree.lookup("/kind")?.compute(tree.tree())?.as_string()?;
        let device = match kind.as_str() {
            "motion-detector" => Device::MotionDetector(MotionDetector::default()),
            _ => bail!("redstone: unknown device type {}", kind),
        };

        self.devices.insert(path.to_owned(), device);
        return Ok(());
    }

    fn nodetype(&self, path: &str, _tree: &SubTree) -> Fallible<ValueType> {
        Ok(match self.devices[path] {
            Device::MotionDetector(_) => ValueType::BOOLEAN,
        })
    }

    fn get_all_possible_values(&self, path: &str, _tree: &SubTree) -> Fallible<Vec<Value>> {
        Ok(match self.devices[path] {
            Device::MotionDetector(_) => vec![Value::Boolean(true), Value::Boolean(false)],
        })
    }

    fn handle_event(&mut self, path: &str, value: Value, _tree: &SubTree) -> Fallible<()> {
        //        ensure!(
        //            self.clocks[path].last_value == value.as_integer()?,
        //            "runtime error: clock event value does not match cached value"
        //        );
        return Ok(());
    }

    fn get_value(&self, path: &str, _tree: &SubTree) -> Option<Value> {
        trace!("DEVICE: get_value @ {}", path);
        Some(match &self.devices[path] {
            Device::MotionDetector(md) => Value::Boolean(md.filtered_state),
        })
    }

    fn on_ready(&mut self, _tree: &SubTree) -> Fallible<()> {
        let worker = RedstoneWorker::new(&self.path_map);
        self.worker = Some(worker.start());
        return Ok(());
    }
}

// The worker gets created and started as a side-effect of creating the tree.
// Unfortunately, we need to send things to the tree-process, which only gets
// created after this point, so we have to wait until we get the ServerStarted
// message before we can actually send events.
pub struct RedstoneWorker {
    db_addr: Option<Addr<DBServer>>,
    clients: Vec<Addr<RedstoneClient>>,
    path_map: HashMap<String, DeviceLocation>,
}

impl RedstoneWorker {
    pub(crate) fn new(path_map: &HashMap<String, DeviceLocation>) -> Self {
        RedstoneWorker {
            db_addr: None,
            clients: Vec::new(),
            path_map: path_map.to_owned(),
        }
    }
}

use actix::Arbiter;
use actix_web::ws::Client;
use futures::{Future, Stream};

//extern crate openssl;
use actix_web::{actix, client::ClientConnector, client::Connect};
use openssl::ssl::{SslConnector, SslFiletype, SslMethod, SslVerifyMode};

impl Actor for RedstoneWorker {
    type Context = Context<Self>;
    fn started(&mut self, _ctx: &mut Context<Self>) {
        // Kick off a connector that knows about ssl with our certs.
        let mut ssl_builder = SslConnector::builder(SslMethod::tls()).unwrap();
        ssl_builder.set_verify(SslVerifyMode::NONE);
        ssl_builder
            .set_certificate_chain_file("../../esp/redstone/CA/intermediate/certs/chain.cert.pem")
            .unwrap();
        //ssl_builder.set_ca_file("../../esp/redstone/CA/intermediate/certs/chain.cert.pem").unwrap();
        ssl_builder
            .set_certificate_file(
                "../../esp/redstone/CA/intermediate/certs/redstone_orchestrator.cert.pem",
                SslFiletype::PEM,
            )
            .unwrap();
        ssl_builder
            .set_private_key_file(
                "../../esp/redstone/CA/intermediate/private/redstone_orchestrator.key.pem",
                SslFiletype::PEM,
            )
            .unwrap();
        let ssl_conn = ssl_builder.build();
        let conn = ClientConnector::with_connector(ssl_conn).start();

        // Connect to all devices and then busy loop waiting for events
        // self.db_addr.send
        let device_locs = self.path_map.values().cloned().collect::<Vec<_>>();
        for loc in device_locs {
            println!("going to connect to: {}", loc.url());
            let coroutine = Client::with_connector(loc.url(), conn.clone())
                .connect()
                .map_err(|e| {
                    println!("Error: {}", e);
                    ()
                })
                .map(|(reader, writer)| {
                    println!("connected");
                    let foo = RedstoneClient::create(|ctx| {
                        RedstoneClient::add_stream(reader, ctx);
                        RedstoneClient { writer }
                    });
                    //self.clients.push(foo);

                    //                    let addr = ChatClient::create(|ctx| {
                    //                        ChatClient::add_stream(reader, ctx);
                    //                        ChatClient(writer)
                    //                    });
                    //
                    //                    // start console loop
                    //                    thread::spawn(move || loop {
                    //                        let mut cmd = String::new();
                    //                        if io::stdin().read_line(&mut cmd).is_err() {
                    //                            println!("error");
                    //                            return;
                    //                        }
                    //                        addr.do_send(ClientCommand(cmd));
                    //                    });
                    ()
                });
            Arbiter::spawn(coroutine);
        }
    }
}

use actix::ActorContext;
use actix::StreamHandler;
use actix_web::ws::{ClientWriter, Message, ProtocolError};
use std::time::Duration as StdDuration;

struct RedstoneClient {
    writer: ClientWriter,
}

impl RedstoneClient {
    fn heartbeat(&self, ctx: &mut Context<Self>) {
        ctx.run_later(StdDuration::new(1, 0), |client, ctx| {
            println!("going to send heartbeat");
            client.writer.ping("hi");
            client.heartbeat(ctx);

            // client should also check for a timeout here, similar to the
            // server code
        });
    }
}

impl Actor for RedstoneClient {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        println!("RedstoneClient started");

        // start heartbeats otherwise server will disconnect after 10 seconds
        self.heartbeat(ctx)
    }

    fn stopped(&mut self, _: &mut Context<Self>) {
        println!("RedstoneClient stopped");

        // Stop application on disconnect
        //System::current().stop();
    }
}

impl StreamHandler<Message, ProtocolError> for RedstoneClient {
    fn handle(&mut self, msg: Message, _ctx: &mut Context<Self>) {
        match msg {
            Message::Text(txt) => println!("Server: {:?}", txt),
            _ => (),
        }
    }

    fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Stream Handler Connected");
    }

    fn finished(&mut self, ctx: &mut Context<Self>) {
        println!("Stream Handler Disconnected");
        ctx.stop()
    }
}

impl Handler<ServerStarted> for RedstoneWorker {
    type Result = Fallible<()>;

    fn handle(&mut self, msg: ServerStarted, _ctx: &mut Context<Self>) -> Self::Result {
        self.db_addr = Some(msg.db_addr);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_connect() -> Fallible<()> {
        let sys = actix::System::new("ws-example");

        let mut map = HashMap::new();
        map.insert(
            "/foo".to_owned(),
            DeviceLocation {
                host: "progenitor.eyrie".to_owned(),
                port: None,
            },
        );
        let rs = RedstoneWorker::new(&map);
        let addr = rs.start();

        sys.run();
        Ok(())
    }
}
