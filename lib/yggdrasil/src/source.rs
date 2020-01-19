// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::tree::SubTree;
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

    pub fn add_path(&self, path: &str, tree: &SubTree) -> Fallible<()> {
        self.0.borrow_mut().add_path(path, tree)
    }
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
        pub fn new_ref() -> Fallible<SourceRef> {
            let src = Box::new(Self {
                inputs: HashMap::new(),
            });
            Ok(SourceRef::new(src))
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
        SimpleSource::new_ref().unwrap();
    }
}
