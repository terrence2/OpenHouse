// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use downcast_rs::{impl_downcast, Downcast};
use failure::Fallible;
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};
use tree::SubTree;
use value::Value;

/// This Trait allows a Sink to provide required metadata to the Tree.
pub trait TreeSink: Downcast {
    /// Note the following path listed as a sink using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()>;

    /// Parsing is finished and we are ready to start the system.
    fn on_ready(&mut self, _tree: &SubTree) -> Fallible<()> {
        return Ok(());
    }

    /// Update the given paths to the new values.
    fn values_updated(&mut self, values: &[(String, Value)]) -> Fallible<()>;
}
impl_downcast!(TreeSink);

/// SinkRef holds a shared, ref-counted, heap-allocated, internall-mutable
/// reference to a sink that can be shared by the Tree and the surrounding
/// context.
#[derive(Clone)]
pub struct SinkRef {
    sink: Rc<RefCell<Box<TreeSink>>>,
}

impl SinkRef {
    /// Create a new SinkRef from a heap-allocated TreeSink implementation.
    pub fn new(sink: Box<TreeSink>) -> Self {
        SinkRef {
            sink: Rc::new(RefCell::new(sink)),
        }
    }

    /// A helper function to make it easy to downcast to a mutable, concrete type
    /// so that the sink object can be mutated.
    pub fn mutate_as<T>(&self, f: &mut FnMut(&mut T)) -> Fallible<()>
    where
        T: TreeSink,
    {
        RefMut::map(self.sink.borrow_mut(), |ts| {
            if let Some(real) = ts.downcast_mut::<T>() {
                f(real);
            }
            ts
        });
        return Ok(());
    }

    pub fn inspect_as<T, V>(&self, f: &Fn(&T) -> &V) -> Fallible<Ref<V>>
    where
        T: TreeSink,
    {
        let inner: Ref<V> = Ref::map(self.sink.borrow(), |ts| {
            return f(ts.downcast_ref::<T>().unwrap());
        });
        return Ok(inner);
    }

    pub(super) fn on_ready(&self, tree: &SubTree) -> Fallible<()> {
        self.sink.borrow_mut().on_ready(tree)
    }

    pub(super) fn add_path(&self, path: &str, tree: &SubTree) -> Fallible<()> {
        self.sink.borrow_mut().add_path(path, tree)
    }

    pub(super) fn values_updated(&self, values: &[(String, Value)]) -> Fallible<()> {
        self.sink.borrow_mut().values_updated(values)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tree::{SubTree, TreeBuilder};

    struct TestSink {}

    impl TestSink {
        fn new() -> Fallible<SinkRef> {
            return Ok(SinkRef::new(Box::new(Self {})));
        }

        fn frob(&mut self) {}
    }

    impl TreeSink for TestSink {
        fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Fallible<()> {
            return Ok(());
        }

        fn values_updated(&mut self, _values: &[(String, Value)]) -> Fallible<()> {
            return Ok(());
        }
    }

    #[test]
    fn test_sink_methods() -> Fallible<()> {
        let sink = TestSink::new()?;
        let tree = TreeBuilder::empty();
        let subtree = tree.subtree_at(&tree.root())?;
        sink.add_path("", &subtree)?;
        sink.values_updated(&vec![])?;
        return Ok(());
    }

    #[test]
    fn test_sink_mutate() -> Fallible<()> {
        let sink = TestSink::new()?;
        sink.mutate_as::<TestSink>(&mut |s| s.frob())?;
        return Ok(());
    }
}
