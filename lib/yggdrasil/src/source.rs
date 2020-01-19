// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::tree::SubTree;
use failure::{Fallible};

/// This Trait allows a Source to provide required metadata to the Tree.
pub trait TreeSource {
    /// Note the following path listed as a source using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()>;
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::value::Value;
    use std::collections::HashMap;

    pub struct SimpleSource {
        inputs: HashMap<String, Value>,
    }

    impl SimpleSource {
        pub fn new() -> Self {
            Self {
                inputs: HashMap::new(),
            }
        }
    }

    impl TreeSource for SimpleSource {
        fn add_path(&mut self, path: &str, _tree: &SubTree) -> Fallible<()> {
            self.inputs.insert(path.into(), Value::new_str("foo"));
            Ok(())
        }
    }

    #[test]
    fn test_source_new() {
        SimpleSource::new();
    }
}
