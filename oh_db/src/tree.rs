// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::{Component, Components, Path};

make_error!(TreeError; {
    InvalidPathComponent => String,
    MalformedPath => String,
    NoSuchNode => String,
    NodeAlreadyExists => String
});
pub type TreeResult<T> = Result<T, TreeError>;

// The tree nodes contain children and data.
type ChildMap = HashMap<String, Node>;
type DataMap = HashMap<String, String>;
pub struct Node {
    children: ChildMap,
    data: DataMap
}

fn malformed_path(context: &str) -> TreeResult<&mut Node> {
    Err(TreeError::MalformedPath(String::from(context)))
}

impl Node {
    // Nodes are created via the Tree.
    fn new() -> Self {
        Node {
            children: ChildMap::new(),
            data: DataMap::new()
        }
    }

    // Iterative lookup is hard because of the borrow checker.
    fn lookup_recursive(&mut self, parts: &mut Components) -> TreeResult<&mut Node> {
        let child_name = match parts.next() {
            Some(name) => name,
            None => return Ok(self)
        };
        match child_name {
            Component::RootDir => panic!("path with multiple roots"),
            Component::Prefix(_) => return malformed_path("window paths not supported"),
            Component::CurDir => return malformed_path("current_dir"),
            Component::ParentDir => return malformed_path("parent_dir"),
            Component::Normal(os_part) => {
                let part = os_part.to_string_lossy().into_owned();
                let child = match self.children.get_mut(&part) {
                    Some(c) => c,
                    None => return Err(TreeError::NoSuchNode(String::from(part)))
                };
                return child.lookup_recursive(parts);
            }
        }
    }

    /// Insert a new node under the given name. The child must not exist.
    pub fn add_child(&mut self, name: String) -> TreeResult<()> {
        if name.find('/').is_some() {
            return Err(TreeError::InvalidPathComponent(name));
        }
        if self.children.contains_key(&name) {
            return Err(TreeError::NodeAlreadyExists(name));
        }
        let result = self.children.insert(name.clone(), Node::new());
        assert!(result.is_none());
        return Ok(());
    }
}

// The tree is just a node rooted at /.
pub struct Tree {
    root: Node
}

impl Tree {
    /// Creates a new, empty Tree.
    pub fn new() -> Tree {
        Tree {
            root: Node::new()
        }
    }

    /// Parse the given string path and traverse the tree.
    /// Returns the node at the given path or an error.
    pub fn lookup(&mut self, path: &str) -> TreeResult<&mut Node> {
        let mut parts = Path::new(path).components();
        if parts.next() != Some(Component::RootDir) {
            return malformed_path("relative");
        }
        return self.root.lookup_recursive(&mut parts);
    }
}
