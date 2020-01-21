// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    bif::NativeFunc,
    path::ConcretePath,
    tree::Tree,
    value::{Value, ValueData},
};
use failure::{bail, Fallible};

#[derive(Clone, Debug)]
pub(crate) struct ToStr;

impl NativeFunc for ToStr {
    fn compute(&self, value: Value, tree: &Tree) -> Fallible<Value> {
        Ok(Value::from_string(match value.data {
            ValueData::String(s) => s,
            ValueData::Integer(i) => format!("{}", i),
            ValueData::Float(f) => format!("{}", f),
            ValueData::Boolean(b) => format!("{}", b),
            ValueData::Path(p) => {
                let noderef = tree.lookup_dynamic_path(&p)?;
                self.compute(noderef.compute(tree)?, tree)?.as_string()?
            }
            ValueData::InputFlag => bail!("runtime error: InputFlag in ToStr"),
        }))
    }

    fn find_all_possible_inputs(
        &self,
        _value_type: (),
        _tree: &Tree,
        _out: &mut Vec<ConcretePath>,
    ) -> Fallible<()> {
        Ok(())
    }

    fn box_clone(&self) -> Box<dyn NativeFunc + Send + Sync> {
        Box::new((*self).clone())
    }
}
