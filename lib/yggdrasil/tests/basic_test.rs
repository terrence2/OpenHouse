// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate failure;
extern crate yggdrasil;

use failure::Fallible;
use yggdrasil::{Error, SinkRef, SourceRef, SubTree, Tree, TreeSink, TreeSource, Value, ValueType};

struct Light {
    value: Option<Value>,
}
impl TreeSink for Light {
    fn nodetype(&self, _path: &str, _tree: &SubTree) -> Result<ValueType, Error> {
        return Ok(ValueType::STRING);
    }
    fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Result<(), Error> {
        return Ok(());
    }
    fn values_updated(&mut self, values: &Vec<(String, Value)>) -> Result<(), Error> {
        for (path, value) in values.iter() {
            assert_eq!(*path, "/room/light");
            self.value = Some(value.to_owned());
        }
        return Ok(());
    }
}

struct Switch {}
impl TreeSource for Switch {
    fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Result<(), Error> {
        return Ok(());
    }
    fn nodetype(&self, _path: &str, _tree: &SubTree) -> Result<ValueType, Error> {
        return Ok(ValueType::STRING);
    }
    fn get_all_possible_values(&self, _path: &str, _tree: &SubTree) -> Result<Vec<Value>, Error> {
        return Ok(vec![]);
    }
    fn handle_event(&mut self, _path: &str, _value: Value, _tree: &SubTree) -> Fallible<()> {
        return Ok(());
    }
    fn get_value(&self, _path: &str, _tree: &SubTree) -> Option<Value> {
        return Some(Value::String("foo".to_owned()));
    }
}

#[test]
fn test_main() -> Fallible<()> {
    let program = r#"
room
    light
        $light
        <-/room/switch
    switch
        ^switch
"#;
    let src = SourceRef::new(Box::new(Switch {}));
    let sink = SinkRef::new(Box::new(Light { value: None }));
    let tree = Tree::new_empty()
        .add_source_handler("switch", &src)?
        .add_sink_handler("light", &sink)?
        .build_from_str(program)?;

    tree.handle_event("/room/switch", Value::String("foo".to_owned()))?;

    //assert_eq!(sink.0.borrow().value, Some(Value::String("foo".to_owned())));
    let v = sink.inspect_as::<Light, Option<Value>>(&|l| &l.value)?;
    assert_eq!((*v).clone().unwrap(), Value::String("foo".to_owned()));

    return Ok(());
}
