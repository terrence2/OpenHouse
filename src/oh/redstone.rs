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
use json::{object, JsonValue};
use std::{
    boxed::Box,
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::mpsc::{channel, Receiver, Sender},
    task::{spawn, JoinHandle},
    time::delay_for,
};
use tokio_tungstenite::{connect_async, WebSocketStream};
use tracing::{error, info, trace, warn};
use tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};
use url::Url;
use yggdrasil::{ConcretePath, Float, Value};

#[derive(Debug, Clone)]
enum PropertyKind {
    Source,
    Sink,
}

#[derive(Debug, Clone)]
struct RedstoneDevice {
    path: ConcretePath,
    url: Url,
    source_properties: Vec<String>,
    sink_properties: Vec<String>,
    last_seen: Instant,
}

impl RedstoneDevice {
    fn add_property(&mut self, s: String, kind: PropertyKind) {
        match kind {
            PropertyKind::Source => self.source_properties.push(s),
            PropertyKind::Sink => self.sink_properties.push(s),
        }
    }

    fn touch(&mut self) {
        self.last_seen = Instant::now();
    }
}

struct DeviceServer {
    task: JoinHandle<()>,
    mailbox: DeviceMailbox,
}

// There is no Socket message to get a property value, so use http during initialization.
struct RedstoneHttpClient {
    base_url: Url,
    client: Client<HttpConnector>,
}
impl RedstoneHttpClient {
    fn new(base_url: &Url) -> Fallible<Self> {
        let client = Client::builder()
            .keep_alive(false)
            .http1_writev(false) // always flatten so we send fewer packets
            .retry_canceled_requests(true)
            .set_host(true)
            .build_http();
        Ok(Self {
            base_url: base_url.to_owned(),
            client,
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
        match select(delay_for(Duration::from_secs(15)), self.client.get(url)).await {
            Either::Left(_) => bail!("timeout GET {}", path),
            Either::Right((maybe_resp, _)) => {
                let resp = maybe_resp?;
                let result = Self::read_body(resp).await;
                Ok(result?)
            }
        }
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

fn value_to_json(value: &Value) -> Fallible<JsonValue> {
    if value.is_boolean() {
        return Ok(JsonValue::Boolean(value.as_boolean()?));
    }
    if value.is_integer() {
        return Ok(JsonValue::Number(value.as_integer()?.into()));
    }
    if value.is_float() {
        return Ok(JsonValue::Number(value.as_float()?.value.into()));
    }
    if value.is_string() {
        return Ok(JsonValue::String(value.as_string()?));
    }
    bail!("cannot format value {} as json", value)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum LoopStatus {
    Okay,
    Finished,
    UncleanFinished,
}

impl DeviceServer {
    async fn track_device(
        device: RedstoneDevice,
        update: UpdateMailbox,
        tree: TreeMailbox,
    ) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            loop {
                match Self::track_device_main(
                    &mut mailbox_receiver,
                    device.clone(),
                    update.clone(),
                    tree.clone(),
                )
                .await
                {
                    Ok(_) => {
                        info!("DeviceServer({}) exited cleanly", device.url);
                        mailbox_receiver.close();
                        return;
                    }
                    Err(e) => {
                        error!("DeviceServer({}) crashed: {}", device.url, e,);
                        let status =
                            Self::wait_for_retry_connect(&device, &mut mailbox_receiver).await;
                        if status == LoopStatus::Finished {
                            return;
                        }
                    }
                }
            }
        });
        Ok(DeviceServer {
            task,
            mailbox: DeviceMailbox { mailbox },
        })
    }

    async fn track_device_main(
        mailbox_receiver: &mut Receiver<DeviceProtocol>,
        mut device: RedstoneDevice,
        mut update: UpdateMailbox,
        mut tree: TreeMailbox,
    ) -> Fallible<()> {
        info!("device: managing {}", device.url);

        // Note: make sure our client closes its connection when we're done.
        {
            for property in &device.source_properties {
                trace!(
                    "device: querying {} for initial state on {}",
                    device.url,
                    property
                );
                let client = RedstoneHttpClient::new(&device.url)?;
                let body = client.get(&format!("/properties/{}", property)).await?;
                let json = json::parse(&body)?;
                let value = value_from_json(&json[property])?;
                trace!(
                    "device: setting initial state of {} to {}",
                    device.path,
                    value
                );
                let updates = tree
                    .handle_event(&(device.path.clone() / property), value)
                    .await?;
                update.apply_updates(updates).await?;
            }
        }

        // Outer loop is reconnection loop
        info!("device: connecting to {}", device.url);
        let (mut ws_stream, _response) = connect_async(device.url.clone()).await?;
        info!("device: opening websocket to {}", device.url);
        let status = Self::handle_redstone_stream(
            &mut ws_stream,
            mailbox_receiver,
            &mut device,
            &mut update,
            &mut tree,
        )
        .await?;
        info!("device: redstone stream finished: {:?}", status);
        if status != LoopStatus::UncleanFinished {
            ws_stream
                .close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: "gateway shutdown".into(),
                }))
                .await?;
            info!("device: shutting down {}", device.url);
            return Ok(());
        }

        Ok(())
    }

    async fn handle_redstone_stream(
        ws_stream: &mut WebSocketStream<TcpStream>,
        mailbox_receiver: &mut Receiver<DeviceProtocol>,
        device: &mut RedstoneDevice,
        update: &mut UpdateMailbox,
        tree: &mut TreeMailbox,
    ) -> Fallible<LoopStatus> {
        loop {
            trace!("device: waiting for message");
            match select(ws_stream.next(), Box::pin(mailbox_receiver.recv())).await {
                Either::Right((maybe_message, _stream_next)) => {
                    trace!("device: received mailbox messages");
                    match maybe_message {
                        Some(DeviceProtocol::SetProperty(prop, val)) => {
                            info!("device: set_property {}: {} -> {:?}", device.url, prop, val);
                            let mut message = object!(
                                "messageType" => "setProperty",
                                "data" => object!()
                            );
                            message["data"].insert(&prop, value_to_json(&val)?)?;
                            let send = ws_stream.send(Message::text(message.to_string()));
                            send.await?;
                        }
                        Some(DeviceProtocol::PingTimeout) => {
                            trace!("device: pinging {}", device.url);
                            if device.last_seen.elapsed() > Duration::from_secs(45) {
                                ws_stream.close(None).await?;
                                bail!("device: detected time-out, closing and restarting");
                            }
                            ws_stream
                                .send(Message::Ping("ping".as_bytes().to_owned()))
                                .await?;
                        }
                        Some(DeviceProtocol::Finish) | None => return Ok(LoopStatus::Finished),
                    };
                }
                Either::Left((maybe_stream, _mailbox_unrecv)) => {
                    trace!("device: received redstone message");
                    let status =
                        Self::handle_redstone_message(maybe_stream, device, update, tree).await?;
                    if status != LoopStatus::Okay {
                        trace!("device: shutting down redstone stream with: {:?}", status);
                        return Ok(status);
                    }
                }
            }
        }
    }

    async fn handle_redstone_message(
        maybe_maybe_message: Option<Result<Message, tungstenite::Error>>,
        device: &mut RedstoneDevice,
        update: &mut UpdateMailbox,
        tree: &mut TreeMailbox,
    ) -> Fallible<LoopStatus> {
        if maybe_maybe_message.is_none() {
            return Ok(LoopStatus::Okay);
        }
        let maybe_message = maybe_maybe_message.unwrap();
        if let Err(e) = maybe_message {
            error!("failed to receive message: {}", e);
            return Ok(LoopStatus::Finished);
        }
        let message = maybe_message.unwrap();
        match message {
            Message::Ping(data) => {
                trace!("device: ping message: {:?}", data);
                device.touch();
            }
            Message::Pong(data) => {
                trace!("device: pong message: {:?}", data);
                device.touch();
            }
            Message::Binary(data) => trace!(
                "device: ignoring binary message from {}: {:?}",
                device.url,
                data
            ),
            Message::Text(json_text) => {
                trace!(
                    "device: recv message from {}: {} bytes",
                    device.url,
                    json_text.len()
                );
                device.touch();
                let body = json::parse(&json_text)?;
                ensure!(body.has_key("messageType"));
                ensure!(body.has_key("data"));
                let data = &body["data"];
                for property in &device.source_properties {
                    if data.has_key(property) {
                        let value = &data[property];
                        trace!("device: setting {}/{} to {}", device.path, property, value);
                        let updates = tree
                            .handle_event(
                                &(device.path.clone() / property),
                                value_from_json(value)?,
                            )
                            .await?;
                        update.apply_updates(updates).await?;
                    }
                }
            }
            Message::Close(status) => {
                warn!(
                    "device: connection closed from {}, status: {:?}",
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
                delay_for(Duration::from_secs(5)),
                Box::pin(mailbox_receiver.recv()),
            )
            .await
            {
                Either::Right((maybe_message, _delay)) => {
                    match maybe_message {
                        Some(DeviceProtocol::SetProperty(p, v)) => {
                            error!(
                                "device: cannot send update `{}:{}` to closed device: {}",
                                p, v, device.url
                            );
                        }
                        Some(DeviceProtocol::PingTimeout) => {
                            error!("device: cannot send ping to closed device: {}", device.url);
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
        self.task.await?;
        Ok(())
    }

    fn mailbox(&self) -> DeviceMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum DeviceProtocol {
    SetProperty(String, Value),
    PingTimeout,
    Finish,
}

#[derive(Clone, Debug)]
pub struct DeviceMailbox {
    mailbox: Sender<DeviceProtocol>,
}

impl DeviceMailbox {
    pub async fn set_property(&mut self, property: &str, value: Value) -> Fallible<()> {
        self.mailbox
            .send(DeviceProtocol::SetProperty(property.to_owned(), value))
            .await?;
        Ok(())
    }

    pub async fn ping_timeout(&mut self) -> Fallible<()> {
        self.mailbox.send(DeviceProtocol::PingTimeout).await?;
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
    async fn build_device(
        kind: PropertyKind,
        base_path: &ConcretePath,
        mut tree: TreeMailbox,
        devices: &mut HashMap<Url, RedstoneDevice>,
    ) -> Fallible<()> {
        let host_path = base_path.parent() / "host";
        let host = if tree.path_exists(&host_path).await? {
            tree.compute(&host_path).await?.as_string()?
        } else {
            base_path.parent().basename().to_owned()
        };
        let port_path = base_path.parent() / "port";
        let port = if tree.path_exists(&port_path).await? {
            tree.compute(&port_path).await?.as_integer()?
        } else {
            80
        };
        let property_path = base_path / "property";
        let property = if tree.path_exists(&property_path).await? {
            tree.compute(&property_path).await?.as_string()?
        } else {
            base_path.basename().to_owned()
        };
        let address = format!("ws://{}:{}/thing", host, port);
        let url = Url::parse(&address)?;

        info!(
            "server: creating redstone device {} => {}:{}",
            base_path, url, property
        );
        devices
            .entry(url.clone())
            .or_insert(RedstoneDevice {
                path: base_path.parent().to_owned(),
                url,
                source_properties: Vec::new(),
                sink_properties: Vec::new(),
                last_seen: Instant::now(),
            })
            .add_property(property, kind);
        Ok(())
    }

    pub async fn launch(update: UpdateMailbox, mut tree: TreeMailbox) -> Fallible<Self> {
        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            info!("redstone webthings gateway starting up");
            let mut devices = HashMap::new();
            for source_path in &tree.find_sources("redstone").await? {
                Self::build_device(
                    PropertyKind::Source,
                    source_path,
                    tree.clone(),
                    &mut devices,
                )
                .await?;
            }
            for sink_path in &tree.find_sinks("redstone").await? {
                Self::build_device(PropertyKind::Sink, sink_path, tree.clone(), &mut devices)
                    .await?;
            }

            let mut device_servers = HashMap::new();
            for (_, device) in devices.drain() {
                let path = device.path.clone();
                let device_server =
                    DeviceServer::track_device(device, update.clone(), tree.clone()).await?;
                device_servers.insert(path, device_server);
            }

            'message_loop: loop {
                trace!("server: entering mainloop");
                let mailbox_recv = Box::pin(mailbox_receiver.recv());
                match select(delay_for(Duration::from_secs(15)), mailbox_recv).await {
                    Either::Right((maybe_message, _delay)) => {
                        trace!("server: mainloop recv'd message");
                        match maybe_message {
                            Some(RedstoneProtocol::SetProperty(path, value)) => {
                                let device_path = path.parent();
                                let property_name = path.basename();
                                if let Some(device_server) = device_servers.get_mut(&device_path) {
                                    device_server
                                        .mailbox()
                                        .set_property(property_name, value)
                                        .await?;
                                } else {
                                    warn!(
                                        "server: attempted to set value on non-existing path: {}",
                                        path
                                    );
                                }
                            }
                            Some(RedstoneProtocol::Finish) | None => {
                                for (_, server) in device_servers.drain() {
                                    server.mailbox().finish().await?;
                                    server.join().await?;
                                }
                                break 'message_loop;
                            }
                        }
                    }
                    Either::Left(((), _mailbox_unrecv)) => {
                        trace!("server: mainloop recv'd ping timeout");
                        for (_, server) in device_servers.iter_mut() {
                            server.mailbox.ping_timeout().await?;
                        }
                    }
                }
            }
            mailbox_receiver.close();

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
