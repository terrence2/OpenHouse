// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{TreeMailbox, UpdateMailbox};
use bytes::BytesMut;
use failure::{bail, ensure, Fallible};
use futures::{
    future::{select, Either},
    sink::SinkExt,
    StreamExt,
};
use hyper::{
    body::HttpBody,
    client::{Client, HttpConnector},
    Body, Response, Uri,
};
use json::JsonValue;
use std::collections::HashMap;
use tokio::{
    net::TcpStream,
    sync::mpsc::{channel, Receiver, Sender},
    task::{spawn, JoinHandle},
    time::{delay_for, Duration},
};
use tokio_tungstenite::{connect_async, WebSocketStream};
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

// There is no Socket message to get a property value, so use http during initialization.
struct RedstoneHttpClient {
    base_url: Url,
    client: Client<HttpConnector>,
}
impl RedstoneHttpClient {
    fn new(base_url: &Url) -> Fallible<Self> {
        Ok(Self {
            base_url: base_url.to_owned(),
            client: Client::builder()
                .keep_alive(false)
                .http1_writev(false) // always flatten so we send fewer packets
                .retry_canceled_requests(true)
                .set_host(true)
                .build_http(),
        })
    }

    async fn read_body(mut resp: Response<Body>) -> Fallible<String> {
        let mut data = BytesMut::new();
        while !resp.body().is_end_stream() {
            match resp.body_mut().data().await {
                None => break,
                Some(result) => data.extend_from_slice(&result?.slice(..)),
            }
        }
        Ok(String::from_utf8_lossy(data.as_ref()).to_string())
    }

    fn url(&self, path: &str) -> Fallible<Uri> {
        let path = format!("{}{}", self.base_url.path(), path);
        Ok(Uri::builder()
            .scheme("http")
            .authority(self.base_url.host_str().unwrap())
            .path_and_query(path.as_str())
            .build()?)
    }

    async fn get(&self, path: &str) -> Fallible<String> {
        let url = self.url(path)?;
        trace!("GET {}", url);
        let body = Self::read_body(self.client.get(self.url(path)?).await?).await?;
        Ok(body)
    }
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum LoopStatus {
    Okay,
    Finished,
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

            // Note: make sure our client doesn't outlive it's welcome.
            {
                trace!("Querying device for initial state");
                let client = RedstoneHttpClient::new(&device.url)?;
                let body = client
                    .get(&format!("/properties/{}", device.property))
                    .await?;
                let json = json::parse(&body)?;
                let value = value_from_json(&json[&device.property])?;
                trace!("setting initial state of {} to {}", device.path, value);
                let updates = tree.handle_event(&device.path, value).await?;
                update.apply_updates(updates).await?;
            }

            // Outer loop is reconnection loop
            'connect: loop {
                trace!("Connecting to {}", device.url);
                match connect_async(device.url.clone()).await {
                    Ok((mut ws_stream, _response)) => {
                        trace!("Connect to {}", device.url);
                        Self::handle_redstone_stream(
                            &mut ws_stream,
                            &mut mailbox_receiver,
                            &device,
                            &mut update,
                            &mut tree,
                        )
                        .await?;
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

                let status = Self::wait_for_retry_connect(&device, &mut mailbox_receiver).await;
                if status == LoopStatus::Finished {
                    break 'connect;
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

    async fn handle_redstone_stream(
        ws_stream: &mut WebSocketStream<TcpStream>,
        mailbox_receiver: &mut Receiver<DeviceProtocol>,
        device: &RedstoneDevice,
        update: &mut UpdateMailbox,
        tree: &mut TreeMailbox,
    ) -> Fallible<()> {
        loop {
            match select(ws_stream.next(), Box::pin(mailbox_receiver.recv())).await {
                Either::Right((maybe_message, _stream_next)) => {
                    match maybe_message {
                        Some(DeviceProtocol::SetProperty(v)) => {
                            let send = ws_stream.send(Message::text(format!("{}", v)));
                            send.await?;
                            trace!("sent value `{}` to device: {}", v, device.url);
                        }
                        Some(DeviceProtocol::Finish) => return Ok(()),
                        None => return Ok(()),
                    };
                }
                Either::Left((maybe_stream, _mailbox_unrecv)) => {
                    let status =
                        Self::handle_redstone_message(maybe_stream, &device, update, tree).await?;
                    if status == LoopStatus::Finished {
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn handle_redstone_message(
        maybe_maybe_message: Option<Result<Message, tungstenite::Error>>,
        device: &RedstoneDevice,
        update: &mut UpdateMailbox,
        tree: &mut TreeMailbox,
    ) -> Fallible<LoopStatus> {
        if maybe_maybe_message.is_none() {
            return Ok(LoopStatus::Okay);
        }
        let maybe_message = maybe_maybe_message.unwrap();
        if let Err(e) = maybe_message {
            error!("failed to receive message: {}", e);
            return Ok(LoopStatus::Okay);
        }
        let message = maybe_message.unwrap();
        match message {
            Message::Ping(data) => trace!("ping message: {:?}", data),
            Message::Pong(data) => trace!("pong message: {:?}", data),
            Message::Binary(data) => {
                trace!("ignoring binary message from {}: {:?}", device.url, data)
            }
            Message::Text(json_text) => {
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
                trace!("setting {} to {}", device.path, value);
                let updates = tree
                    .handle_event(&device.path, value_from_json(value)?)
                    .await?;
                update.apply_updates(updates).await?;
            }
            Message::Close(status) => {
                warn!(
                    "connection closed from {}, status: {:?}",
                    device.url, status
                );
                return Ok(LoopStatus::Finished);
            }
        }
        return Ok(LoopStatus::Okay);
    }

    async fn wait_for_retry_connect(
        device: &RedstoneDevice,
        mailbox_receiver: &mut Receiver<DeviceProtocol>,
    ) -> LoopStatus {
        loop {
            match select(
                delay_for(Duration::from_secs(15)),
                Box::pin(mailbox_receiver.recv()),
            )
            .await
            {
                Either::Right((maybe_message, _delay)) => {
                    match maybe_message {
                        Some(DeviceProtocol::SetProperty(v)) => {
                            error!("cannot send value `{}` to closed device: {}", v, device.url);
                        }
                        Some(DeviceProtocol::Finish) => return LoopStatus::Finished,
                        None => return LoopStatus::Finished,
                    };
                }
                Either::Left(((), _mailbox_unrecv)) => {
                    // TODO: Dropping the unreceived recv does not drop messages ever?
                    return LoopStatus::Okay;
                }
            }
        }
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
