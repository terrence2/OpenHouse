// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{clock::Clock, hue::Hue, legacy_mcu::LegacyMCU};
use actix::prelude::*;
use failure::Fallible;
use std::path::Path;
use tracing::{error, trace};
use yggdrasil::{Tree, TreeBuilder, TreeSource, Value};

pub struct DBServer {
    tree: Tree,
    pub clock: Clock,
    pub legacy_mcu: LegacyMCU,
    pub hue: Hue,
}

impl DBServer {
    pub fn new_from_file(filename: &Path) -> Fallible<Self> {
        let tree = TreeBuilder::default().build_from_file(filename)?;

        let mut legacy_mcu = LegacyMCU::new()?;
        let mcu_paths = tree.find_sources("legacy-mcu");
        for path in &mcu_paths {
            legacy_mcu.add_path(&path, &tree.subtree_at(&tree.lookup(&path)?)?)?;
        }

        let mut clock = Clock::new()?;
        let clock_paths = tree.find_sources("clock");
        for path in &clock_paths {
            clock.add_path(&path, &tree.subtree_at(&tree.lookup(&path)?)?)?;
        }

        let hue = Hue::new(&tree)?;

        let db_server = Self {
            tree,
            clock,
            legacy_mcu,
            hue,
        };
        Ok(db_server)
    }
}

impl Actor for DBServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {}
}

pub struct HandleEvent {
    pub path: String,
    pub value: Value,
}

impl Message for HandleEvent {
    type Result = Fallible<()>;
}

impl Handler<HandleEvent> for DBServer {
    type Result = Fallible<()>;

    fn handle(&mut self, msg: HandleEvent, _ctx: &mut Context<Self>) -> Self::Result {
        trace!("db server: recvd event {} <- {}", msg.path, msg.value);
        match self.tree.handle_event(&msg.path, msg.value) {
            Ok(groups) => {
                if let Some(parts) = groups.get("hue") {
                    self.hue.values_updated(parts)?;
                }
            }
            Err(e) => error!("db server: failed to handle event: {}", e),
        }
        Ok(())
    }
}

pub struct TickEvent {}
impl Message for TickEvent {
    type Result = Fallible<()>;
}

impl Handler<TickEvent> for DBServer {
    type Result = Fallible<()>;

    fn handle(&mut self, _msg: TickEvent, _ctx: &mut Context<Self>) -> Self::Result {
        let updates = self.clock.handle_tick();
        for (path, value) in &updates {
            self.tree.handle_event(&path, Value::from_integer(*value))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new() -> Fallible<()> {
        let _sys = System::new("open_house");
        let db = DBServer::new_from_file(Path::new("examples/eyrie.ygg"))?;
        let _button_path_map = db.legacy_mcu.path_map.clone();
        Ok(())
    }
}
