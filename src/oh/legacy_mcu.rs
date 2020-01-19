// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use std::{collections::HashMap, net::IpAddr};
use yggdrasil::{SubTree, TreeSource};

pub struct LegacyMCU {
    pub path_map: HashMap<IpAddr, String>,
}

impl LegacyMCU {
    pub fn new() -> Fallible<Self> {
        Ok(Self {
            path_map: HashMap::new(),
        })
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
        Ok(())
    }
}
