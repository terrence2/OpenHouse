// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{tree::SubTree, value::Value};
use failure::Fallible;

/// This Trait allows a Sink to provide required metadata to the Tree.
pub trait TreeSink {
    /// Note the following path listed as a sink using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()>;

    /// Parsing is finished and we are ready to start the system.
    fn on_ready(&mut self, _tree: &SubTree) -> Fallible<()> {
        Ok(())
    }

    /// Update the given paths to the new values.
    fn values_updated(&mut self, values: &[(String, Value)]) -> Fallible<()>;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree::{SubTree, TreeBuilder};

    struct TestSink;
    impl TreeSink for TestSink {
        fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Fallible<()> {
            Ok(())
        }

        fn values_updated(&mut self, _values: &[(String, Value)]) -> Fallible<()> {
            Ok(())
        }
    }

    #[test]
    fn test_sink_methods() -> Fallible<()> {
        let mut sink = TestSink {};
        let tree = TreeBuilder::empty();
        let subtree = tree.subtree_at(&tree.root())?;
        sink.add_path("", &subtree)?;
        sink.values_updated(&[])?;
        Ok(())
    }
}
