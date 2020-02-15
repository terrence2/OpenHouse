// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{TreeMailbox, UpdateMailbox};
use failure::{bail, ensure, Fallible};
use futures::{
    future::{select, Either},
    sink::SinkExt,
    StreamExt,
};
use json::JsonValue;
//use hyper::{
//    body::HttpBody,
//    Body, Request, Response
//};
use std::collections::HashMap;
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
    time::{delay_for, Duration},
};
use tokio_tungstenite::connect_async;
use tracing::{error, info, trace, warn};
use tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};
use url::Url;
use yggdrasil::{ConcretePath, Float, Value};

#[derive(Debug, Clone)]
struct RedstoneDevice {
    path: ConcretePath,
    url: Url,
    property: String,
}

struct DeviceServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: DeviceMailbox,
}

fn value_from_json(json: &JsonValue) -> Fallible<Value> {
    Ok(match json {
        JsonValue::Boolean(b) => Value::from_boolean(*b),
        JsonValue::Short(s) => Value::new_str(&s),
        JsonValue::String(s) => Value::new_str(&s),
        JsonValue::Number(n) => {
            let (sign, mantissa, exponent) = n.as_parts();
            match (sign, exponent) {
                (true, 0) => {
                    if mantissa < i64::max_value() as u64 {
                        Value::from_integer(mantissa as i64)
                    } else {
                        Value::from_float(Float::new((*n).into())?)
                    }
                }
                (false, 0) => {
                    if mantissa < (-i64::min_value()) as u64 {
                        Value::from_integer(-(mantissa as i64))
                    } else {
                        Value::from_float(Float::new((*n).into())?)
                    }
                }
                _ => Value::from_float(Float::new((*n).into())?),
            }
        }
        _ => bail!("non-value float in value_from_json"),
    })
}

impl DeviceServer {
    async fn track_device(
        device: RedstoneDevice,
        mut update: UpdateMailbox,
        mut tree: TreeMailbox,
    ) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            info!("Managing device: {}", device.url);

            // Outer loop is reconnection loop
            'connect: loop {
                trace!("Connecting to {}", device.url);
                match connect_async(device.url.clone()).await {
                    Ok((mut ws_stream, _response)) => {
                        'open_message_loop: loop {
                            match select(ws_stream.next(), Box::pin(mailbox_receiver.recv())).await
                            {
                                Either::Right((maybe_message, _stream_next)) => {
                                    match maybe_message {
                                        Some(DeviceProtocol::SetProperty(v)) => {
                                            let send =
                                                ws_stream.send(Message::text(format!("{}", v)));
                                            send.await?;
                                            info!(
                                                "sent value `{}` to closed device: {}",
                                                v, device.url
                                            );
                                        }
                                        Some(DeviceProtocol::Finish) => break 'open_message_loop,
                                        None => break 'open_message_loop,
                                    };
                                }
                                Either::Left((maybe_stream, _mailbox_unrecv)) => {
                                    if let Some(maybe_message) = maybe_stream {
                                        match maybe_message {
                                            Ok(Message::Ping(data)) => {
                                                trace!("ping message: {:?}", data)
                                            }
                                            Ok(Message::Pong(data)) => {
                                                trace!("pong message: {:?}", data)
                                            }
                                            Ok(Message::Binary(data)) => trace!(
                                                "ignoring binary message from {}: {:?}",
                                                device.url,
                                                data
                                            ),
                                            Ok(Message::Text(json_text)) => {
                                                trace!(
                                                    "recv message from {}: {} bytes",
                                                    device.url,
                                                    json_text.len()
                                                );
                                                let body = json::parse(&json_text)?;
                                                ensure!(body.has_key("messageType"));
                                                ensure!(body.has_key("data"));
                                                let data = &body["data"];
                                                ensure!(data.has_key(&device.property));
                                                let value = &data[&device.property];
                                                println!("data: {:?}", data);
                                                let updates = tree
                                                    .handle_event(
                                                        &device.path,
                                                        value_from_json(value)?,
                                                    )
                                                    .await?;
                                                update.apply_updates(updates).await?;
                                            }
                                            Ok(Message::Close(status)) => {
                                                warn!(
                                                    "connection closed from {}, status: {:?}",
                                                    device.url, status
                                                );
                                                break 'open_message_loop;
                                            }
                                            Err(e) => {
                                                error!("failed to receive message: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ws_stream
                            .close(Some(CloseFrame {
                                code: CloseCode::Normal,
                                reason: "gateway shutdown".into(),
                            }))
                            .await?;
                        break 'connect;
                    }
                    Err(e) => {
                        warn!("Failed to connect to {}: {}", device.url, e);
                    }
                }

                'closed_message_loop: loop {
                    match select(
                        delay_for(Duration::from_secs(15)),
                        Box::pin(mailbox_receiver.recv()),
                    )
                    .await
                    {
                        Either::Right((maybe_message, _delay)) => {
                            match maybe_message {
                                Some(DeviceProtocol::SetProperty(v)) => {
                                    error!(
                                        "cannot send value `{}` to closed device: {}",
                                        v, device.url
                                    );
                                }
                                Some(DeviceProtocol::Finish) => break 'connect,
                                None => break 'connect,
                            };
                        }
                        Either::Left(((), _mailbox_unrecv)) => {
                            // TODO: Dropping the unreceived recv does not drop messages ever?
                            break 'closed_message_loop;
                        }
                    }
                }
            }
            mailbox_receiver.close();

            Ok(())
        });
        Ok(DeviceServer {
            task,
            mailbox: DeviceMailbox { mailbox },
        })
    }

    async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    fn mailbox(&self) -> DeviceMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum DeviceProtocol {
    SetProperty(Value),
    Finish,
}

#[derive(Clone, Debug)]
pub struct DeviceMailbox {
    mailbox: Sender<DeviceProtocol>,
}

impl DeviceMailbox {
    pub async fn set_property(&mut self, value: Value) -> Fallible<()> {
        self.mailbox
            .send(DeviceProtocol::SetProperty(value))
            .await?;
        Ok(())
    }

    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(DeviceProtocol::Finish).await?;
        Ok(())
    }
}

pub struct RedstoneServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: RedstoneMailbox,
}

impl RedstoneServer {
    pub async fn launch(update: UpdateMailbox, mut tree: TreeMailbox) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            info!("redstone webthings gateway starting up");
            let mut devices = Vec::new();
            for source_path in &tree.find_sources("redstone").await? {
                let host_path = source_path / "host";
                let host = if tree.path_exists(&host_path).await? {
                    tree.compute(&host_path).await?.as_string()?
                } else {
                    source_path.basename().to_owned()
                };
                let port_path = source_path / "port";
                let port = if tree.path_exists(&port_path).await? {
                    tree.compute(&port_path).await?.as_integer()?
                } else {
                    80
                };
                let property_path = source_path / "property";
                let property = tree.compute(&property_path).await?.as_string()?;
                let address = format!("ws://{}:{}/thing", host, port);
                let url = Url::parse(&address)?;
                info!("Adding device {} => {}:{}", source_path, url, property);
                devices.push(RedstoneDevice {
                    path: source_path.to_owned(),
                    url,
                    property,
                });
            }

            let mut device_servers = HashMap::new();
            for device in devices.drain(..) {
                let path = device.path.clone();
                let device_server =
                    DeviceServer::track_device(device, update.clone(), tree.clone()).await?;
                device_servers.insert(path, device_server);
            }

            while let Some(message) = mailbox_receiver.recv().await {
                match message {
                    RedstoneProtocol::SetProperty(path, value) => {
                        if let Some(device_server) = device_servers.get_mut(&path) {
                            device_server.mailbox().set_property(value).await?;
                        } else {
                            warn!("attempted to set value on non-existing path: {}", path);
                        }
                    }
                    RedstoneProtocol::Finish => {
                        for (_, server) in device_servers.drain() {
                            server.mailbox().finish().await?;
                            server.join().await?;
                        }
                        mailbox_receiver.close()
                    }
                }
            }

            Ok(())
        });
        Ok(Self {
            task,
            mailbox: RedstoneMailbox { mailbox },
        })
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    pub fn mailbox(&self) -> RedstoneMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum RedstoneProtocol {
    SetProperty(ConcretePath, Value),
    Finish,
}

#[derive(Clone, Debug)]
pub struct RedstoneMailbox {
    mailbox: Sender<RedstoneProtocol>,
}

impl RedstoneMailbox {
    pub async fn set_property(&mut self, path: ConcretePath, value: Value) -> Fallible<()> {
        self.mailbox
            .send(RedstoneProtocol::SetProperty(path, value))
            .await?;
        Ok(())
    }

    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(RedstoneProtocol::Finish).await?;
        Ok(())
    }
}
