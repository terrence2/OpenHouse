// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
pub(super) mod tostr;

use crate::{path::ConcretePath, tree::Tree, value::Value};
use failure::Fallible;
use std::fmt;

pub trait NativeFunc {
    fn compute(&self, value: Value, tree: &Tree) -> Fallible<Value>;
    fn find_all_possible_inputs(
        &self,
        value_type: (),
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<()>;
    fn box_clone(&self) -> Box<dyn NativeFunc + Send + Sync>;
}

impl Clone for Box<dyn NativeFunc + Send + Sync> {
    fn clone(&self) -> Box<dyn NativeFunc + Send + Sync> {
        self.box_clone()
    }
}

impl fmt::Debug for Box<dyn NativeFunc + Send + Sync> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TEST")
    }
}
