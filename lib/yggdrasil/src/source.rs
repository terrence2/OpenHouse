// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    tree::SubTree,
    value::{Value, ValueType},
};
use downcast_rs::{impl_downcast, Downcast};
use failure::{ensure, Fallible};
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

/// This Trait allows a Source to provide required metadata to the Tree.
pub trait TreeSource: Downcast {
    /// Note the following path listed as a source using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Fallible<()>;

    /// Return the type of the given path.
    fn nodetype(&self, path: &str, tree: &SubTree) -> Fallible<ValueType>;

    /// Return all possible values that the given source can take. This is only
    /// called for sources that are used as a path component in a dynamic path.
    /// In the event this is called for a source that does not have a constrained
    /// set of possible values -- floats, arbitrary strings, etc -- return an
    /// error.
    fn get_all_possible_values(&self, path: &str, tree: &SubTree) -> Fallible<Vec<Value>>;

    /// Called on handle_event, before event processing. The source should be
    /// ready for calls to get_value(path) after this.
    fn handle_event(&mut self, path: &str, value: Value, _tree: &SubTree) -> Fallible<()>;

    /// Return the current value of the given source. Sources are generally
    /// expected to be delivered asyncronously and the latest value will be
    /// cached indefinitely, This is only called when the value is used as a path
    /// component before a change event has occurred.
    fn get_value(&self, path: &str, tree: &SubTree) -> Option<Value>;

    /// Parsing is finished and we are ready to start the system.
    fn on_ready(&mut self, _tree: &SubTree) -> Fallible<()> {
        Ok(())
    }
}
impl_downcast!(TreeSource);

/// SourceRef holds a shared, ref-counted, heap-allocated, internall-mutable
/// reference to a source that can be shared by the Tree and the surrounding
/// context.
#[derive(Clone)]
pub struct SourceRef(Rc<RefCell<Box<dyn TreeSource>>>);

impl SourceRef {
    /// Create a new SourceRef from a heap-allocated TreeSource implementation.
    pub fn new(source: Box<dyn TreeSource>) -> Self {
        SourceRef(Rc::new(RefCell::new(source)))
    }

    /// A helper function to make it easy to downcast to a mutable, concrete type
    /// so that the source object can be mutated.
    pub fn mutate_as<T, U>(&self, f: &mut dyn FnMut(&mut T) -> U) -> Fallible<U>
    where
        T: TreeSource,
    {
        let mut out = Vec::new();
        RefMut::map(self.0.borrow_mut(), |ts| {
            if let Some(real) = ts.downcast_mut::<T>() {
                let result = f(real);
                out.push(result);
            }
            ts
        });
        ensure!(
            !out.is_empty(),
            "runtime error: Source::mutate_as did not return a result"
        );
        let result = out.remove(0);
        Ok(result)
    }

    pub fn inspect_as<T, V>(&self, f: &dyn Fn(&T) -> &V) -> Fallible<Ref<V>>
    where
        T: TreeSource,
    {
        let inner: Ref<V> = Ref::map(self.0.borrow(), |ts| f(ts.downcast_ref::<T>().unwrap()));
        Ok(inner)
    }

    pub(super) fn add_path(&self, path: &str, tree: &SubTree) -> Fallible<()> {
        self.0.borrow_mut().add_path(path, tree)
    }

    pub(super) fn nodetype(&self, path: &str, tree: &SubTree) -> Fallible<ValueType> {
        self.0.borrow().nodetype(path, tree)
    }

    pub(super) fn get_all_possible_values(
        &self,
        path: &str,
        tree: &SubTree,
    ) -> Fallible<Vec<Value>> {
        self.0.borrow().get_all_possible_values(path, tree)
    }

    pub(super) fn handle_event(
        &mut self,
        path: &str,
        value: Value,
        tree: &SubTree,
    ) -> Fallible<()> {
        self.0.borrow_mut().handle_event(path, value, tree)
    }

    pub(super) fn get_value(&self, path: &str, tree: &SubTree) -> Option<Value> {
        self.0.borrow().get_value(path, tree)
    }
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use std::collections::HashMap;

    pub struct SimpleSource {
        values: Vec<Value>,
        inputs: HashMap<String, Value>,
    }

    impl SimpleSource {
        pub fn new_ref(values: Vec<Value>) -> Fallible<SourceRef> {
            let src = Box::new(Self {
                values: values.clone(),
                inputs: HashMap::new(),
            });
            Ok(SourceRef::new(src))
        }
    }

    impl TreeSource for SimpleSource {
        fn get_all_possible_values(&self, _path: &str, _tree: &SubTree) -> Fallible<Vec<Value>> {
            Ok(self.values.clone())
        }

        fn add_path(&mut self, path: &str, _tree: &SubTree) -> Fallible<()> {
            self.inputs
                .insert(path.into(), Value::String("foo".to_owned()));
            Ok(())
        }

        fn handle_event(&mut self, path: &str, value: Value, _tree: &SubTree) -> Fallible<()> {
            let entry = self.inputs.get_mut(path).unwrap();
            *entry = value;
            Ok(())
        }

        fn get_value(&self, path: &str, _tree: &SubTree) -> Option<Value> {
            Some(self.inputs[path].clone())
        }

        fn nodetype(&self, _path: &str, _tree: &SubTree) -> Fallible<ValueType> {
            Ok(ValueType::STRING)
        }
    }

    #[test]
    fn test_source_new() {
        SimpleSource::new_ref(vec![]).unwrap();
    }
}
