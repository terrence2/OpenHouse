// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::prelude::*;
use failure::Fallible;
use oh::hue::Hue;
use oh::legacy_mcu::LegacyMCU;
use std::path::Path;
use yggdrasil::{SinkRef, SourceRef, Tree, Value};

pub struct DBServer {
    tree: Tree,
    pub legacy_mcu: SourceRef,
    pub hue: SinkRef,
}

impl DBServer {
    pub fn new_from_file(filename: &Path) -> Fallible<Self> {
        let hue = SinkRef::new(Hue::new()?);
        let legacy_mcu = SourceRef::new(LegacyMCU::new()?);
        let tree = Tree::new_empty()
            .add_source_handler("legacy-mcu", &legacy_mcu)?
            .add_sink_handler("hue", &hue)?
            .build_from_file(filename)?;
        Ok(Self {
            tree,
            legacy_mcu,
            hue,
        })
    }
}

impl Actor for DBServer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {}
}

pub struct HandleEvent {
    pub path: String,
    pub value: String,
}

impl Message for HandleEvent {
    type Result = Fallible<()>;
}

impl Handler<HandleEvent> for DBServer {
    type Result = Fallible<()>;

    fn handle(&mut self, msg: HandleEvent, ctx: &mut Context<Self>) -> Self::Result {
        self.tree.handle_event(&msg.path, Value::String(msg.value));
        return Ok(());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new() -> Fallible<()> {
        let db = DBServer::new_from_file(Path::new("test/test.oh"))?;
        let button_path_map = db
            .legacy_mcu
            .inspect_as(&|mcu: &LegacyMCU| &mcu.path_map)?
            .clone();
        return Ok(());
    }
}
