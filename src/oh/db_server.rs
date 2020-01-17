// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::oh::{clock::Clock, hue::Hue, legacy_mcu::LegacyMCU};
use actix::prelude::*;
use failure::Fallible;
use log::{error, trace};
use std::path::Path;
use yggdrasil::{SinkRef, SourceRef, Tree, TreeBuilder, Value};

pub struct DBServer {
    tree: Tree,
    pub clock: SourceRef,
    pub legacy_mcu: SourceRef,
    pub hue: SinkRef,
}

impl DBServer {
    pub fn new_from_file(filename: &Path) -> Fallible<Self> {
        let hue = SinkRef::new(Hue::new()?);
        let legacy_mcu = SourceRef::new(LegacyMCU::new()?);
        let clock = SourceRef::new(Clock::new()?);
        let tree = TreeBuilder::default()
            .add_source_handler("clock", &clock)?
            .add_source_handler("legacy-mcu", &legacy_mcu)?
            .add_sink_handler("hue", &hue)?
            .build_from_file(filename)?;
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
            Ok(_) => (),
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
        let updates = self.clock.mutate_as(&mut |c: &mut Clock| c.handle_tick())?;
        //println!("woudl apply updated: {:?}", updates);
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
        let _button_path_map = db
            .legacy_mcu
            .inspect_as(&|mcu: &LegacyMCU| &mcu.path_map)?
            .clone();
        Ok(())
    }
}
