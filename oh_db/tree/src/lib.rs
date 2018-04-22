// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#[macro_use]
extern crate approx;
#[macro_use]
extern crate failure;

mod parser;
mod physical;

use failure::Error;
use physical::Dimension2;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub struct Tree {
    root: NodeRef,
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            root: NodeRef::new(Node::new()),
        }
    }

    pub fn root(&self) -> NodeRef {
        self.root.clone()
    }

    pub fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        ensure!(
            path.starts_with('/'),
            "invalid path: tree lookups must start at /"
        );
        let relative: &str = &path[1..];
        if relative.is_empty() {
            return Ok(self.root.clone());
        }
        return self.root.lookup(relative);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataSource {
    Literal(String),
    Indirect(String),
    Sensor(String),
}

#[derive(Clone)]
pub struct NodeRef(Rc<RefCell<Node>>);

impl NodeRef {
    pub fn new(node: Node) -> NodeRef {
        let self_ref = NodeRef(Rc::new(RefCell::new(node)));
        self_ref
            .0
            .borrow_mut()
            .children
            .insert(".".to_owned(), self_ref.clone());
        return self_ref;
    }

    pub fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        self.0.borrow().lookup(path)
    }

    pub fn add_child(&self, name: &str) -> Result<NodeRef, Error> {
        let child = self.0.borrow_mut().add_child(name)?;
        child
            .0
            .borrow_mut()
            .children
            .insert(".".to_owned(), child.clone());
        child
            .0
            .borrow_mut()
            .children
            .insert("..".to_owned(), self.clone());
        return Ok(child);
    }

    pub fn location(&self) -> Option<Dimension2> {
        self.0.borrow().location
    }

    pub fn set_location(&self, loc: Dimension2) {
        self.0.borrow_mut().location = Some(loc);
    }

    pub fn source(&self) -> Option<DataSource> {
        self.0.borrow().source.clone()
    }

    pub fn set_indirect_source(&self, from: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().source = Some(DataSource::Indirect(from.to_owned()));
        return Ok(());
    }

    pub fn set_literal_source(&self, content: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().source = Some(DataSource::Literal(content.to_owned()));
        return Ok(());
    }

    pub fn set_sensor_source(&self, from: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().source = Some(DataSource::Sensor(from.to_owned()));
        return Ok(());
    }

    pub fn target(&self) -> Option<String> {
        self.0.borrow().target.clone()
    }

    pub fn set_target(&self, tgt: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().target.is_none(),
            "target has already been set"
        );
        self.0.borrow_mut().target = Some(tgt.to_owned());
        return Ok(());
    }
}

pub struct Node {
    children: HashMap<String, NodeRef>,
    location: Option<Dimension2>,
    dimensions: Option<Dimension2>,
    source: Option<DataSource>,
    target: Option<String>,
}

impl Node {
    fn new() -> Self {
        Node {
            children: HashMap::new(),
            location: None,
            dimensions: None,
            source: None,
            target: None,
        }
    }

    fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        ensure!(
            !path.starts_with('/'),
            "invalid path: node lookups must not start at /"
        );
        ensure!(!path.is_empty(), "invalid path: empty path component");
        let parts = path.splitn(2, '/').collect::<Vec<_>>();
        ensure!(parts.len() >= 1, "invalid path: did not find a component");
        ensure!(
            self.children.contains_key(parts[0]),
            format!("invalid path: did not find path component: {}", parts[0])
        );
        let child = self.children[parts[0]].clone();
        return match parts.len() {
            1 => Ok(child),
            2 => child.lookup(parts[1]),
            _ => unreachable!(),
        };
    }

    fn add_child(&mut self, name: &str) -> Result<NodeRef, Error> {
        let child = NodeRef::new(Node::new());
        self.children.insert(name.to_owned(), child.clone());
        return Ok(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tree() {
        let tree = Tree::new();
        assert_eq!(None, tree.root().location());

        let d10 = Dimension2::from_str("@10x10").unwrap();
        let d20 = Dimension2::from_str("@20x20").unwrap();

        let child = tree.lookup("/").unwrap().add_child("foopy").unwrap();
        child.set_location(d10.clone());
        assert_eq!(d10, child.location().unwrap());

        let child = tree.lookup("/foopy").unwrap().add_child("barmy").unwrap();
        child.set_location(d20.clone());
        assert_eq!(d20, child.location().unwrap());

        assert_eq!(d10, tree.lookup("/foopy").unwrap().location().unwrap());
        assert_eq!(
            d20,
            tree.lookup("/foopy/barmy").unwrap().location().unwrap()
        );
        assert_eq!(
            d20,
            tree.lookup("/foopy/barmy")
                .unwrap()
                .lookup(".")
                .unwrap()
                .location()
                .unwrap()
        );
        assert_eq!(
            d10,
            tree.lookup("/foopy/barmy")
                .unwrap()
                .lookup("..")
                .unwrap()
                .location()
                .unwrap()
        );
    }
}
