// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{
    color::{Color, Mired, BHS},
    json_helpers::{ObjectHelper, ValueHelper},
    TreeMailbox,
};
use bytes::BytesMut;
use failure::{ensure, Fallible};
use hyper::{
    body::HttpBody,
    client::{Client, HttpConnector},
    Body, Request, Response, Uri,
};
use json::{object, parse, stringify, JsonValue};
use std::collections::HashMap;
use tokio::{
    sync::mpsc::{channel, Sender},
    task::{spawn, JoinHandle},
};
use tracing::{info, trace};
use yggdrasil::{ConcretePath, Value};

pub struct HueServer {
    task: JoinHandle<Fallible<()>>,
    mailbox: HueMailbox,
}

impl HueServer {
    pub async fn launch(mut tree: TreeMailbox) -> Fallible<Self> {
        let mut bridge_paths = tree.find_sinks("hue-bridge").await?;
        ensure!(
            bridge_paths.len() == 1,
            "Exactly one Hue hub supported at this time."
        );
        let bridge_path = &bridge_paths.remove(0);
        let mut bridge = HueBridge::setup(bridge_path, tree).await?;

        let (mailbox, mut mailbox_receiver) = channel(16);
        let task = spawn(async move {
            loop {
                if let Some(message) = mailbox_receiver.recv().await {
                    match message {
                        HueServerProtocol::ValuesUpdated(values) => {
                            trace!("hue system handling {} updates", values.len());
                            bridge.handle_values_updated(values).await?;
                        }
                        HueServerProtocol::Finish => mailbox_receiver.close(),
                    }
                } else {
                    break;
                }
            }
            Ok(())
        });
        Ok(Self {
            task,
            mailbox: HueMailbox { mailbox },
        })
    }

    pub async fn join(self) -> Fallible<()> {
        self.task.await??;
        Ok(())
    }

    pub fn mailbox(&self) -> HueMailbox {
        self.mailbox.clone()
    }
}

#[derive(Debug)]
enum HueServerProtocol {
    ValuesUpdated(Vec<(ConcretePath, Value)>),
    Finish,
}

#[derive(Clone, Debug)]
pub struct HueMailbox {
    mailbox: Sender<HueServerProtocol>,
}

impl HueMailbox {
    pub async fn finish(&mut self) -> Fallible<()> {
        self.mailbox.send(HueServerProtocol::Finish).await?;
        Ok(())
    }

    pub async fn values_updated(&mut self, values: &[(ConcretePath, Value)]) -> Fallible<()> {
        self.mailbox
            .send(HueServerProtocol::ValuesUpdated(values.to_vec()))
            .await?;
        Ok(())
    }
}

struct HueBridge {
    client: HueBridgeClient,

    // Map from paths to light names.
    path_map: HashMap<ConcretePath, String>,

    // Map from light names to the light number needed to talk to the hub.
    light_map: HashMap<String, u32>,

    // Map from light sets to group numbers.
    group_map: HashMap<Vec<u32>, u32>,
}

impl HueBridge {
    async fn setup(bridge_path: &ConcretePath, mut tree: TreeMailbox) -> Fallible<Self> {
        let bridge_address = tree
            .compute(&(bridge_path / "address"))
            .await?
            .as_string()?;
        let bridge_username = tree
            .compute(&(bridge_path / "username"))
            .await?
            .as_string()?;
        let client = HueBridgeClient::new(&bridge_address, &bridge_username)?;
        let resp = client.get("").await?;
        let body = parse(&resp)?;

        Self::show_configuration(&body)?;
        let light_map = Self::collect_lights(&body)?;
        let group_map = Self::collect_groups(&body)?;

        let mut path_map = HashMap::new();
        for path in &tree.find_sinks("hue").await? {
            path_map.insert(path.to_owned(), path.basename().to_owned());
        }

        Ok(Self {
            client,
            path_map,
            light_map,
            group_map,
        })
    }

    fn show_configuration(body: &JsonValue) -> Fallible<()> {
        let config = body.to_object()?.fetch("config")?.to_object()?;
        let props = vec![
            "name",
            "zigbeechannel",
            "bridgeid",
            "mac",
            "dhcp",
            "ipaddress",
            "netmask",
            "gateway",
            "proxyaddress",
            "proxyport",
            "modelid",
            "datastoreversion",
            "swversion",
            "apiversion",
        ];
        info!("hue hub info ->");
        for prop in &props {
            info!("{:>20}: {}", prop, config.fetch(prop)?);
        }
        Ok(())
    }

    // Build the light map so that we can bridge from light names to the numbers
    // that the hub works with internally.
    fn collect_lights(body: &JsonValue) -> Fallible<HashMap<String, u32>> {
        info!("hue light info ->");
        let mut light_map = HashMap::new();
        let lights = body.to_object()?.fetch("lights")?.to_object()?;
        for (number, light) in lights.iter() {
            let name = light.to_object()?.fetch("name")?.to_str()?;
            info!(
                "{:>3} : {:<20} : {} : {}",
                number,
                name,
                light.to_object()?.fetch("modelid")?.to_str()?,
                light.to_object()?.fetch("swversion")?.to_str()?
            );
            light_map.insert(name.to_owned(), number.parse()?);
        }
        Ok(light_map)
    }

    // Groups are not limited in recent releases of the firmware, so just
    // collect all currently existing groups on startup instead of trying to
    // clean up.
    fn collect_groups(body: &JsonValue) -> Fallible<HashMap<Vec<u32>, u32>> {
        info!("hue group info ->");
        let mut group_map = HashMap::new();
        let groups = body.to_object()?.fetch("groups")?.to_object()?;
        for (number, group) in groups.iter() {
            let mut lights = Vec::new();
            let lights_node = group.to_object()?.fetch("lights")?.to_array()?;
            for s in lights_node {
                lights.push(s.to_str()?.parse()?);
            }
            lights.sort();

            info!("{:>3} => {:?}", number, lights);
            group_map.insert(lights, number.parse()?);
        }
        Ok(group_map)
    }

    fn group_by_value(
        values: &[(ConcretePath, Value)],
    ) -> Fallible<HashMap<String, Vec<ConcretePath>>> {
        let mut by_value = HashMap::new();
        for (path, value) in values {
            by_value
                .entry(value.as_string()?)
                .or_insert_with(|| vec![])
                .push(path.to_owned());
        }
        Ok(by_value)
    }

    async fn handle_values_updated(&mut self, values: Vec<(ConcretePath, Value)>) -> Fallible<()> {
        // Group lights by value.
        let groups = Self::group_by_value(&values)?;
        trace!("handle {} groups of value updates", groups.len());
        for (value, group) in &groups {
            let mut lights = group
                .iter()
                .map(|path| {
                    let light_name = &self.path_map[path];
                    self.light_map[light_name]
                })
                .collect::<Vec<u32>>();
            lights.sort();
            info!("hue worker: group {:?} -> {}", lights, value);

            if !self.group_map.contains_key(&lights) {
                self.make_new_group(&lights).await?;
            }
            let group_name = self.group_map[&lights];

            self.update_group(group_name, &value).await?;
        }
        Ok(())
    }

    async fn make_new_group(&mut self, lights: &[u32]) -> Fallible<()> {
        let mut arr = JsonValue::new_array();
        for light in lights {
            arr.push(light.to_string())?;
        }
        let obj = object! {"lights" => arr};
        let resp = self.client.post("/groups/", stringify(obj)).await?;
        let name = resp[0]["success"]["id"].to_str()?.parse()?;
        self.group_map.insert(lights.to_vec(), name);
        Ok(())
    }

    async fn update_group(&self, group: u32, light_value: &str) -> Fallible<()> {
        let url = format!("/groups/{}/action", group);
        let obj = HueBridgeClient::light_state_for_value(light_value)?;
        let put_data = stringify(obj);
        let _resp = self.client.put(&url, put_data).await?;
        Ok(())
    }
}

struct HueBridgeClient {
    address: String,
    username: String,
    client: Client<HttpConnector>,
}

impl HueBridgeClient {
    fn new(address: &str, username: &str) -> Fallible<Self> {
        Ok(Self {
            address: address.to_owned(),
            username: username.to_owned(),
            client: Client::builder()
                .keep_alive(true)
                .http1_writev(false) // always flatten so we send fewer packets
                .retry_canceled_requests(true)
                .set_host(true)
                //.gzip(true)
                //.timeout(Duration::from_secs(30))
                .build_http(),
        })
    }

    fn light_state_for_value(value: &str) -> Fallible<JsonValue> {
        if value == "none" {
            return Ok(object! {"on" => false});
        }
        let color = Color::parse(value)?;
        let mut obj = match color {
            Color::Mired(Mired { color_temp: ct }) => object! {"ct" => ct},
            Color::RGB(rgb) => {
                let bhs = BHS::from_rgb(&rgb)?;
                object! {"bri" => bhs.brightness, "hue" => bhs.hue, "sat" => bhs.saturation}
            }
            Color::BHS(BHS {
                brightness,
                hue,
                saturation,
            }) => object! {"bri" => brightness, "hue" => hue, "sat" => saturation},
        };
        obj["on"] = true.into();
        // FIXME: support transition time
        obj["transitiontime"] = 10.into();
        Ok(obj)
    }

    fn url(&self, path: &str) -> Fallible<Uri> {
        let path = format!("/api/{}{}", self.username, path);
        Ok(Uri::builder()
            .scheme("http")
            .authority(self.address.as_str())
            .path_and_query(path.as_str())
            .build()?)
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

    async fn get(&self, path: &str) -> Fallible<String> {
        let url = self.url(path)?;
        trace!("GET {}", url);
        let body = Self::read_body(self.client.get(url).await?).await?;
        Ok(body)
    }

    async fn post(&self, path: &str, data: String) -> Fallible<JsonValue> {
        let url = self.url(path)?;
        trace!("POST {} -> {}", url, data);
        let req = Request::builder()
            .method("POST")
            .uri(url)
            .body(Body::from(data))?;
        let body = Self::read_body(self.client.request(req).await?).await?;
        Ok(parse(&body)?)
    }

    async fn put(&self, path: &str, data: String) -> Fallible<JsonValue> {
        let url = self.url(path)?;
        trace!("PUT {} -> {}", url, data);
        let req = Request::builder()
            .method("PUT")
            .uri(url)
            .body(Body::from(data))?;
        let body = Self::read_body(self.client.request(req).await?).await?;
        Ok(parse(&body)?)
    }
}
