// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use downcast_rs::Downcast;
use failure::Error;
use std::{cell::{Ref, RefCell, RefMut}, rc::Rc};
use tree::SubTree;
use value::{Value, ValueType};

/// This Trait allows a Sink to provide required metadata to the Tree.
pub trait TreeSink: Downcast {
    /// Return the type of values that the sink takes.
    fn nodetype(&self, path: &str, tree: &SubTree) -> Result<ValueType, Error>;

    /// Note the following path listed as a sink using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Result<(), Error>;

    /// Update the given paths to the new values.
    fn values_updated(&mut self, values: &Vec<(&str, Value)>) -> Result<(), Error>;
}
impl_downcast!(TreeSink);

/// SinkRef holds a shared, ref-counted, heap-allocated, internall-mutable
/// reference to a sink that can be shared by the Tree and the surrounding
/// context.
#[derive(Clone)]
pub struct SinkRef(Rc<RefCell<Box<TreeSink>>>);

impl SinkRef {
    /// Create a new SinkRef from a heap-allocated TreeSink implementation.
    pub fn new(sink: Box<TreeSink>) -> Self {
        SinkRef(Rc::new(RefCell::new(sink)))
    }

    /// A helper function to make it easy to downcast to a mutable, concrete type
    /// so that the sink object can be mutated.
    pub fn mutate_as<T>(&self, f: &mut FnMut(&mut T)) -> Result<(), Error>
    where
        T: TreeSink,
    {
        RefMut::map(self.0.borrow_mut(), |ts| {
            if let Some(real) = ts.downcast_mut::<T>() {
                f(real);
            }
            ts
        });
        return Ok(());
    }

    pub fn inspect_as<T, V>(&self, f: &Fn(&T) -> &V) -> Result<Ref<V>, Error>
    where
        T: TreeSink,
    {
        let foo: Ref<V> = Ref::map(self.0.borrow(), |ts| {
            return f(ts.downcast_ref::<T>().unwrap());
        });
        return Ok(foo);
    }

    pub(super) fn nodetype(&self, path: &str, tree: &SubTree) -> Result<ValueType, Error> {
        self.0.borrow().nodetype(path, tree)
    }

    pub(super) fn add_path(&self, path: &str, tree: &SubTree) -> Result<(), Error> {
        self.0.borrow_mut().add_path(path, tree)
    }

    pub(super) fn values_updated(&self, values: &Vec<(&str, Value)>) -> Result<(), Error> {
        self.0.borrow_mut().values_updated(values)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tree::{SubTree, Tree};

    struct TestSink {}

    impl TestSink {
        fn new() -> Result<SinkRef, Error> {
            return Ok(SinkRef::new(Box::new(Self {})));
        }

        fn frob(&mut self) {}
    }

    impl TreeSink for TestSink {
        fn nodetype(&self, _path: &str, _tree: &SubTree) -> Result<ValueType, Error> {
            Ok(ValueType::STRING)
        }

        fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Result<(), Error> {
            return Ok(());
        }

        fn values_updated(&mut self, values: &Vec<(&str, Value)>) -> Result<(), Error> {
            return Ok(());
        }
    }

    #[test]
    fn test_sink_methods() {
        let sink = TestSink::new().unwrap();
        let tree = Tree::new_empty();
        let subtree = tree.subtree_at(&tree.root()).unwrap();
        assert_eq!(sink.nodetype("", &subtree).unwrap(), ValueType::STRING);
        sink.add_path("", &subtree).unwrap();
        sink.values_updated(&vec![]);
    }

    #[test]
    fn test_sink_mutate() {
        let sink = TestSink::new().unwrap();
        sink.mutate_as::<TestSink>(&mut |s| s.frob()).unwrap();
    }
}
