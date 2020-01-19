// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::{collections::HashMap, net::IpAddr};
use yggdrasil::Tree;

pub struct LegacyMCU {
    pub path_map: HashMap<IpAddr, String>,
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
