// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::{collections::HashMap, net::IpAddr};
use yggdrasil::{SubTree, TreeSource, Value, ValueType};

pub struct LegacyMCU {
    pub path_map: HashMap<IpAddr, String>,
}

impl LegacyMCU {
    pub fn new() -> Fallible<Box<Self>> {
        Ok(Box::new(Self {
            path_map: HashMap::new(),
        }))
    }

    // pub fn get_path_map(&self) -> Fallible<HashMap<IpAddr, String>> {
    //     let mut out = HashMap::new();
    //     for mcu in self.info.iter() {
    //         out.insert(mcu.ip, mcu.path.clone());
    //     }
    //     return Ok(out);
    // }
}

impl TreeSource for LegacyMCU {
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()> {
        let ip = tree
            .lookup("/ip")?
            .compute(tree.tree())?
            .as_string()?
            .parse::<IpAddr>()?;
        // let mcu = MCUInfo {
        //     path: path.to_string(),
        //     ip:
        // };
        // self.info.push(mcu);
        self.path_map.insert(ip, path.to_string());
        return Ok(());
    }

    fn nodetype(&self, _path: &str, _tree: &SubTree) -> Fallible<ValueType> {
        return Ok(ValueType::STRING);
    }

    fn get_all_possible_values(&self, _path: &str, _tree: &SubTree) -> Fallible<Vec<Value>> {
        return Ok(vec!["on", "off", "moonlight", "low", "default"]
            .iter()
            .map(|v| Value::String(v.to_string()))
            .collect::<Vec<Value>>());
    }

    fn get_value(&self, _path: &str, _tree: &SubTree) -> Option<Value> {
        return Some(Value::String("foo".to_owned()));
    }
}
