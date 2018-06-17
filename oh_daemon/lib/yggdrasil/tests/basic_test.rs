// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
extern crate yggdrasil;

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
    fn values_updated(&mut self, _values: &Vec<(&str, Value)>) -> Result<(), Error> {
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
    fn get_value(&self, _path: &str, _tree: &SubTree) -> Option<Value> {
        return Some(Value::String("foo".to_owned()));
    }
}

#[test]
fn test_main() {
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
        .add_source_handler("switch", &src)
        .unwrap()
        .add_sink_handler("light", &sink)
        .unwrap()
        .build_from_str(program)
        .unwrap();

    tree.handle_event("/room/switch", Value::String("foo".to_owned()))
        .unwrap();

    //assert_eq!(sink.0.borrow().value, Some(Value::String("foo".to_owned())));
    let v = sink.inspect_as::<Light, Option<Value>>(&|l| &l.value)
        .unwrap();
    assert_eq!((*v).clone().unwrap(), Value::String("foo".to_owned()));
}
