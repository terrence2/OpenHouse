// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
pub(super) mod tostr;

use failure::Fallible;
use path::ConcretePath;
use std::fmt;
use tree::Tree;
use value::{Value, ValueType};

pub trait NativeFunc {
    fn compute(&self, value: Value, tree: &Tree) -> Fallible<Value>;
    fn virtually_compute_for_path(&self, values: Vec<Value>, tree: &Tree) -> Fallible<Vec<Value>>;
    fn find_all_possible_inputs(
        &self,
        value_type: ValueType,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<ValueType>;
    fn box_clone(&self) -> Box<NativeFunc>;
}

impl Clone for Box<NativeFunc> {
    fn clone(&self) -> Box<NativeFunc> {
        self.box_clone()
    }
}

impl fmt::Debug for Box<NativeFunc> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TEST")
    }
}
