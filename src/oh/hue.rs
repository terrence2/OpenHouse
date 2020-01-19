// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{
    color::{Color, Mired, BHS},
    json_helpers::{ObjectHelper, ValueHelper},
};
use actix::{Actor, Addr, Context, Handler, Message, System};
use failure::{err_msg, Fallible};
use itertools::Itertools;
use json::{object, parse, stringify, JsonValue};
use reqwest::Client;
use std::{collections::HashMap, str::FromStr, time::Duration};
use tracing::{error, info, trace, warn};
use yggdrasil::{ConcretePath, SubTree, TreeSink, Value};

struct ValuesUpdated {
    values: Vec<(String, Value)>,
}
impl Message for ValuesUpdated {
    type Result = Fallible<()>;
}

// The hue plugin to Yggdrasil. All processing is done off-main-thread by the
// HueWorker actor.
pub struct Hue {
    address: Option<String>,
    username: Option<String>,
    path_map: HashMap<String, String>,
    worker: Option<Addr<HueWorker>>,
}

impl Hue {
    pub fn new() -> Fallible<Box<Self>> {
        Ok(Box::new(Hue {
            address: None,
            username: None,
            path_map: HashMap::new(),
            worker: None,
        }))
    }
}

impl TreeSink for Hue {
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()> {
        let concrete = ConcretePath::from_str(path)?;
        let basename = concrete.basename();

        if basename == "hue-bridge" {
            let address = tree.lookup("/address")?.compute(tree.tree())?.as_string()?;
            let username = tree
                .lookup("/username")?
                .compute(tree.tree())?
                .as_string()?;
            self.address = Some(address);
            self.username = Some(username);
            return Ok(());
        }

        // FIXME: allow the user to override the light name, rather than just
        // assuming it's the last path component.
        self.path_map.insert(path.to_owned(), basename.to_owned());
        Ok(())
    }

    fn on_ready(&mut self, _tree: &SubTree) -> Fallible<()> {
        if self.address.is_none() || self.username.is_none() {
            warn!("hue system: no hub defined; not starting");
            return Ok(());
        }
        let address = self
            .address
            .clone()
            .ok_or_else(|| err_msg("hue: no address on bridge"))?;
        let username = self
            .username
            .clone()
            .ok_or_else(|| err_msg("hue: no username on bridge"))?;
        let hub = Hub::new(&address, &username)?;
        let worker = HueWorker::new(hub, &self.path_map);
        self.worker = Some(worker.start());
        Ok(())
    }

    fn values_updated(&mut self, values: &[(String, Value)]) -> Fallible<()> {
        if let Some(ref worker) = self.worker {
            worker.do_send(ValuesUpdated {
                values: values.to_owned(),
            });
        }
        Ok(())
    }
}

struct Hub {
    address: String,
    username: String,
    client: Client,
}

impl Hub {
    fn new(address: &str, username: &str) -> Fallible<Self> {
        Ok(Hub {
            address: address.to_owned(),
            username: username.to_owned(),
            client: Client::builder()
                .gzip(true)
                .timeout(Duration::from_secs(30))
                .build()?,
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

    fn url(&self, path: &str) -> String {
        return format!("http://{}/api/{}{}", self.address, self.username, path);
    }

    fn get(&self, path: &str) -> Fallible<String> {
        let url = self.url(path);
        trace!("GET {}", url);
        let body = self.client.get(&url).send()?.text()?;
        Ok(body)
    }

    fn post(&self, path: &str, data: String) -> Fallible<JsonValue> {
        let url = self.url(path);
        trace!("POST {} -> {}", url, data);
        let body = self.client.post(&url).body(data).send()?.text()?;
        Ok(parse(&body)?)
    }

    fn put(&self, path: &str, data: String) -> Fallible<JsonValue> {
        let url = self.url(path);
        trace!("PUT {} -> {}", url, data);
        let body = self.client.put(&url).body(data).send()?.text()?;
        Ok(parse(&body)?)
    }
}

// The hue worker is the sole, serial thread allowed to talk to the hue hub.
struct HueWorker {
    hub: Hub,

    // Map from paths to light names.
    path_map: HashMap<String, String>,

    // Map from light names to the light number needed to talk to the hub.
    light_map: HashMap<String, u32>,

    // Map from light sets to group numbers.
    group_map: HashMap<Vec<u32>, u32>,
}

impl Actor for HueWorker {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        match self.handle_started(ctx) {
            Ok(_) => (),
            Err(e) => {
                error!("hue system: failed to start: {}", e);
                System::current().stop();
            }
        };
    }
}

impl HueWorker {
    fn new(hub: Hub, path_map: &HashMap<String, String>) -> Self {
        HueWorker {
            hub,
            path_map: path_map.to_owned(),
            light_map: HashMap::new(),
            group_map: HashMap::new(),
        }
    }

    fn handle_started(&mut self, _ctx: &mut Context<Self>) -> Fallible<()> {
        let resp = self.hub.get("")?;
        let body = parse(&resp)?;

        self.show_configuration(&body)?;
        self.collect_lights(&body)?;
        self.collect_groups(&body)?;
        Ok(())
    }

    fn show_configuration(&self, body: &JsonValue) -> Fallible<()> {
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
    fn collect_lights(&mut self, body: &JsonValue) -> Fallible<()> {
        info!("hue light info ->");
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
            self.light_map.insert(name.to_owned(), number.parse()?);
        }
        Ok(())
    }

    // Groups are not limited in recent releases of the firmware, so just
    // collect all currently existing groups on startup instead of trying to
    // clean up.
    fn collect_groups(&mut self, body: &JsonValue) -> Fallible<()> {
        info!("hue group info ->");
        let groups = body.to_object()?.fetch("groups")?.to_object()?;
        for (number, group) in groups.iter() {
            let mut lights = Vec::new();
            let lights_node = group.to_object()?.fetch("lights")?.to_array()?;
            for s in lights_node {
                lights.push(s.to_str()?.parse()?);
            }
            lights.sort();

            info!("{:>3} => {:?}", number, lights);
            self.group_map.insert(lights, number.parse()?);
        }
        Ok(())
    }

    fn handle_values_updated(&mut self, msg: &ValuesUpdated) -> Fallible<()> {
        // Group lights by value.
        let groups = msg.values.iter().group_by(|(_, v)| v);
        for (value, group) in &groups {
            let mut lights = group
                .map(|(path, _)| {
                    let light_name = &self.path_map[path];
                    self.light_map[light_name]
                })
                .collect::<Vec<u32>>();
            lights.sort();
            info!("hue worker: group {:?} -> {}", lights, value);

            if !self.group_map.contains_key(&lights) {
                self.make_new_group(&lights)?;
            }
            let group_name = self.group_map[&lights];

            self.update_group(group_name, value)?;
        }
        Ok(())
    }

    fn make_new_group(&mut self, lights: &[u32]) -> Fallible<()> {
        let mut arr = JsonValue::new_array();
        for light in lights {
            arr.push(light.to_string())?;
        }
        let obj = object! {"lights" => arr};
        let resp = self.hub.post("/groups/", stringify(obj))?;
        let name = resp[0]["success"]["id"].to_str()?.parse()?;
        self.group_map.insert(lights.to_vec(), name);
        Ok(())
    }

    fn update_group(&self, group: u32, light_value: &Value) -> Fallible<()> {
        let url = format!("/groups/{}/action", group);
        let v = &light_value.as_string()?;
        let obj = Hub::light_state_for_value(v)?;
        let put_data = stringify(obj);
        let _resp = self.hub.put(&url, put_data)?;
        Ok(())
    }
}

impl Handler<ValuesUpdated> for HueWorker {
    type Result = Fallible<()>;

    fn handle(&mut self, msg: ValuesUpdated, _: &mut Context<Self>) -> Self::Result {
        match self.handle_values_updated(&msg) {
            Ok(_) => (),
            Err(e) => error!("hue: value update failed: {}", e),
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new_sink() -> Fallible<()> {
        let _hue = Hue::new();
        Ok(())
    }

    #[test]
    fn test_light_state() -> Fallible<()> {
        assert_eq!(Hub::light_state_for_value("none")?, object! {"on" => false});
        assert_eq!(
            Hub::light_state_for_value("mired(40)")?,
            object! {"on" => true, "ct" => 40, "transitiontime" => 10}
        );
        Ok(())
    }
}
