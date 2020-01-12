// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{err_msg, Fallible};
use log::trace;
use std::{collections::HashMap, net::IpAddr};
use yggdrasil::{SubTree, TreeSource, Value, ValueType};

pub struct LegacyMCU {
    pub path_map: HashMap<IpAddr, String>,
    value_map: HashMap<String, Value>,
}

impl LegacyMCU {
    pub fn new() -> Fallible<Box<Self>> {
        Ok(Box::new(Self {
            path_map: HashMap::new(),
            value_map: HashMap::new(),
        }))
    }
}

impl TreeSource for LegacyMCU {
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()> {
        let ip = tree
            .lookup("/ip")?
            .compute(tree.tree())?
            .as_string()?
            .parse::<IpAddr>()?;
        self.path_map.insert(ip, path.to_string());
        self.value_map
            .insert(path.to_string(), Value::String("off".to_owned()));
        Ok(())
    }

    fn nodetype(&self, _path: &str, _tree: &SubTree) -> Fallible<ValueType> {
        Ok(ValueType::STRING)
    }

    fn get_all_possible_values(&self, _path: &str, _tree: &SubTree) -> Fallible<Vec<Value>> {
        Ok(vec!["on", "off", "moonlight", "low", "default"]
            .iter()
            .map(|&v| Value::String(v.to_owned()))
            .collect::<Vec<Value>>())
    }

    fn handle_event(&mut self, path: &str, value: Value, _tree: &SubTree) -> Fallible<()> {
        let entry = self
            .value_map
            .get_mut(path)
            .ok_or_else(|| err_msg("recvd event for unknown path"))?;
        *entry = value;
        Ok(())
    }

    fn get_value(&self, path: &str, _tree: &SubTree) -> Option<Value> {
        trace!("LegacyMCU: get_value @ {}", path);
        self.value_map.get(path).map(|v| v.to_owned())
    }
}
