// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#[macro_use]
extern crate error_chain;
extern crate ketos;

mod path;
mod tree;

pub use path::{Glob, Path, PathBuilder};
pub use tree::{Tree, TreeChanges};

mod errors {
    error_chain! {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
