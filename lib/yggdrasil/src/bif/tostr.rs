// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use bif::NativeFunc;
use failure::Fallible;
use path::ConcretePath;
use tree::Tree;
use value::{Value, ValueType};

#[derive(Clone, Debug)]
pub(crate) struct ToStr;

impl NativeFunc for ToStr {
    fn compute(&self, value: Value, tree: &Tree) -> Fallible<Value> {
        Ok(Value::String(match value {
            Value::String(s) => s,
            Value::Integer(i) => format!("{}", i),
            Value::Float(f) => format!("{}", f),
            Value::Boolean(b) => format!("{}", b),
            Value::Path(p) => {
                let noderef = tree.lookup_dynamic_path(&p)?;
                self.compute(noderef.compute(tree)?, tree)?.as_string()?
            }
        }))
    }

    fn virtually_compute_for_path(&self, values: Vec<Value>, tree: &Tree) -> Fallible<Vec<Value>> {
        let mut results = Vec::new();
        for v in values {
            results.push(self.compute(v, tree)?);
        }
        return Ok(results);
    }

    fn find_all_possible_inputs(
        &self,
        _value_type: ValueType,
        _tree: &Tree,
        _out: &mut Vec<ConcretePath>,
    ) -> Fallible<ValueType> {
        Ok(ValueType::STRING)
    }

    fn box_clone(&self) -> Box<NativeFunc> {
        Box::new((*self).clone())
    }
}
