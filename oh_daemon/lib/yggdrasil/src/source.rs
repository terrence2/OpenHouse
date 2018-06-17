// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use downcast_rs::Downcast;
use failure::Error;
use std::{cell::{RefCell, RefMut}, rc::Rc};
use tree::SubTree;
use value::{Value, ValueType};

/// This Trait allows a Source to provide required metadata to the Tree.
pub trait TreeSource: Downcast {
    /// Note the following path listed as a source using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Result<(), Error>;

    /// Return the type of the given path.
    fn nodetype(&self, path: &str, tree: &SubTree) -> Result<ValueType, Error>;

    /// Return all possible values that the given source can take. This is only
    /// called for sources that are used as a path component elsewhere. In the
    /// event this is called for a source that does not have a constrained set of
    /// possible values -- floats, arbitrary strings, etc -- return an error.
    fn get_all_possible_values(&self, path: &str, tree: &SubTree) -> Result<Vec<Value>, Error>;

    /// Return the current value of the given source. Sources are generally
    /// expected to be delivered asyncronously and the latest value will be
    /// cached indefinitely, This is only called when the value is used as a path
    /// component before a change event has occurred.
    fn get_value(&self, path: &str, tree: &SubTree) -> Option<Value>;
}
impl_downcast!(TreeSource);

/// SourceRef holds a shared, ref-counted, heap-allocated, internall-mutable
/// reference to a source that can be shared by the Tree and the surrounding
/// context.
#[derive(Clone)]
pub struct SourceRef(Rc<RefCell<Box<TreeSource>>>);

impl SourceRef {
    /// Create a new SourceRef from a heap-allocated TreeSource implementation.
    pub fn new(source: Box<TreeSource>) -> Self {
        SourceRef(Rc::new(RefCell::new(source)))
    }

    /// A helper function to make it easy to downcast to a mutable, concrete type
    /// so that the source object can be mutated.
    pub fn mutate_as<T>(&self, f: &mut FnMut(&mut T)) -> Result<(), Error>
    where
        T: TreeSource,
    {
        RefMut::map(self.0.borrow_mut(), |ts| {
            if let Some(real) = ts.downcast_mut::<T>() {
                f(real);
            }
            ts
        });
        return Ok(());
    }

    pub(super) fn add_path(&self, path: &str, tree: &SubTree) -> Result<(), Error> {
        self.0.borrow_mut().add_path(path, tree)
    }

    pub(super) fn nodetype(&self, path: &str, tree: &SubTree) -> Result<ValueType, Error> {
        self.0.borrow().nodetype(path, tree)
    }

    pub(super) fn get_all_possible_values(
        &self,
        path: &str,
        tree: &SubTree,
    ) -> Result<Vec<Value>, Error> {
        self.0.borrow().get_all_possible_values(path, tree)
    }

    pub(super) fn get_value(&self, path: &str, tree: &SubTree) -> Option<Value> {
        self.0.borrow().get_value(path, tree)
    }
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use tree::Tree;

    pub struct SimpleSource {
        values: Vec<String>,
        input: usize,
    }

    impl SimpleSource {
        pub fn new(values: Vec<String>) -> Result<SourceRef, Error> {
            let src = Box::new(Self { values, input: 0 });
            return Ok(SourceRef::new(src));
        }

        pub fn set_input(&mut self, input: usize, path: &str, tree: &Tree) {
            self.input = input;
            tree.handle_event(path, Value::String(self.values[self.input].clone()))
                .unwrap();
        }
    }

    impl TreeSource for SimpleSource {
        fn get_all_possible_values(
            &self,
            _path: &str,
            _tree: &SubTree,
        ) -> Result<Vec<Value>, Error> {
            Ok(self.values
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect::<Vec<Value>>())
        }

        fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Result<(), Error> {
            return Ok(());
        }

        fn get_value(&self, _path: &str, _tree: &SubTree) -> Option<Value> {
            return Some(Value::String(self.values[self.input].clone()));
        }

        fn nodetype(&self, _path: &str, _tree: &SubTree) -> Result<ValueType, Error> {
            Ok(ValueType::STRING)
        }
    }

    #[test]
    fn test_source_new() {
        SimpleSource::new(vec![]).unwrap();
    }
}
