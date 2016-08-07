// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use glob::Pattern;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::{Component, Components, Path};

make_error!(TreeError; {
    DirectoryNotEmpty => String,
    NoSuchNode => String,
    NodeAlreadyExists => String,
    NotDirectory => String,
    NotFile => String,

    // Path format errors.
    NonAbsolutePath => String,
    NonUTF8Path => String,
    FoundDotfile => String,
    FoundEmptyComponent => String,
    InvalidCharacter => String,
    InvalidPathComponent => String,
    MalformedPath => String
});
pub type TreeResult<T> = Result<T, TreeError>;

/// Produce a malformed path error.
fn malformed_path(context: &str) -> TreeResult<&mut DirectoryData> {
    Err(TreeError::MalformedPath(context.to_owned()))
}

/// Raise an error if the path component is not safe.
pub fn check_path_component(name: &str) -> TreeResult<()> {
    if name.find('/').is_some() {
        return Err(TreeError::InvalidPathComponent(name.to_owned()));
    }
    return Ok(());
}

/// A directory contains a list of children.
type ChildMap = HashMap<String, Node>;
pub struct DirectoryData {
    children: ChildMap
}
impl DirectoryData {
    fn new() -> Self {
        DirectoryData { children: HashMap::new() }
    }

    /// Find the directory at the bottom of path. If the path crosses
    /// a file, return Err(NotDirectory).
    fn lookup_directory_recursive(&mut self, parts: &mut Components)
        -> TreeResult<&mut DirectoryData>
    {
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
                let part = match os_part.to_str() {
                    Some(s) => s,
                    None => return Err(TreeError::InvalidPathComponent(
                                           os_part.to_string_lossy().into_owned()))
                };
                let child = match self.children.get_mut(part) {
                    Some(c) => c,
                    None => return Err(TreeError::NoSuchNode(part.to_owned()))
                };
                return match child {
                    &mut Node::File(_) => Err(TreeError::NotDirectory(part.to_owned())),
                    &mut Node::Directory(ref mut d) => d.lookup_directory_recursive(parts)
                };
            }
        }
    }

    fn lookup_file(&mut self, name: &str) -> TreeResult<&mut FileData> {
        let child = match self.children.get_mut(name) {
            Some(c) => c,
            None => return Err(TreeError::NoSuchNode(name.to_owned()))
        };
        return match child {
            &mut Node::Directory(_) => Err(TreeError::NotFile(name.to_owned())),
            &mut Node::File(ref mut f) => Ok(f)
        };
    }

    pub fn add_directory(&mut self, name: &str) -> TreeResult<()> {
        return self.add_child(name, Node::Directory(DirectoryData::new()));
    }

    pub fn add_file(&mut self, name: &str) -> TreeResult<()> {
        return self.add_child(name, Node::File(FileData::new()))
    }

    fn add_child(&mut self, name: &str, node: Node) -> TreeResult<()> {
        try!(check_path_component(name));
        if self.children.contains_key(name) {
            return Err(TreeError::NodeAlreadyExists(name.to_owned()));
        }
        let result = self.children.insert(name.to_owned(), node);
        assert!(result.is_none());
        return Ok(());
    }

    pub fn remove_child(&mut self, name: &str) -> TreeResult<()> {
        try!(check_path_component(name));
        {
            let child = match self.children.get(name) {
                Some(c) => c,
                None => return Err(TreeError::NoSuchNode(name.to_owned()))
            };
            match child {
                &Node::File(_) => {},
                &Node::Directory(ref d) => {
                    if !d.children.is_empty() {
                        return Err(TreeError::DirectoryNotEmpty(name.to_owned()));
                    }
                }
            }
        }
        let result = self.children.remove(name);
        assert!(result.is_some());
        return Ok(());
    }

    pub fn list_directory(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for name in self.children.keys() {
            out.push(name.clone());
        }
        return out;
    }
}

/// A file contains some data.
pub struct FileData {
    data: String
}
impl FileData {
    fn new() -> FileData {
        FileData { data: "hello".to_owned() }
    }

    pub fn set_data(&mut self, new_data: &str) {
        self.data = new_data.to_owned();
    }

    pub fn get_data(&self) -> String {
        self.data.clone()
    }
}

/// A node is either a file or a directory.
enum Node {
    Directory(DirectoryData),
    File(FileData)
}

/// A tree of Node.
pub struct Tree {
    root: DirectoryData
}
impl Tree {
    /// Creates a new, empty Tree.
    pub fn new() -> Tree {
        Tree {
            root: DirectoryData::new()
        }
    }

    /// Parse the given string path and traverse the tree.
    /// Returns the node at the given path or an error.
    pub fn lookup_directory(&mut self, path: &Path) -> TreeResult<&mut DirectoryData> {
        let mut parts = path.components();
        if parts.next() != Some(Component::RootDir) {
            return malformed_path("relative");
        }
        return self.root.lookup_directory_recursive(&mut parts);
    }


    pub fn lookup_file(&mut self, path: &Path) -> TreeResult<&mut FileData> {
        let parent_path = match path.parent() {
            Some(p) => p,
            None => return Err(TreeError::NotFile(path.to_string_lossy().into_owned()))
        };
        let file_name = match path.file_name() {
            Some(n) => n,
            None => return Err(TreeError::NotFile(path.to_string_lossy().into_owned()))
        };
        let parent_directory = try!(self.lookup_directory(parent_path));
        return parent_directory.lookup_file(file_name.to_string_lossy().to_mut());
    }
}

/// OpenHouse paths have somewhat stricter rules than a typical filesystem. The
/// rules are:
///   * must be unix style
///   * must be absolute
///   * path components may not start with '.'
///   * path components must not be empty, e.g. //
///   * must only contain printable UTF-8 characters
///   * the following characters are disallowed:
///     - any whitespace character other than 0x20 (plain ol space)
///     - any characters special to yaml:
///       \ : ,
///     - any globbing characters:
///       ? * [ ] !
pub fn validate_path(path: &Path) -> TreeResult<()> {
    try!(validate_path_shared(path));
    // FIXME: we need to also validate against glob chars.
    return Ok(());
}

/// Check that the given glob is suitable for use with Tree.
///
/// A glob must obey the same rules as path, except that glob
/// characters are allowed.
pub fn validate_glob(glob: &Pattern) -> TreeResult<()> {
    let as_path = Path::new(glob.as_str());
    try!(validate_path_shared(as_path));
    return Ok(());
}

fn validate_path_shared(path: &Path) -> TreeResult<()> {
    if !path.is_absolute() {
        return Err(TreeError::NonAbsolutePath(
                   path.to_string_lossy().into_owned()));
    }

    let chars = match path.to_str() {
        Some(s) => s,
        None => {
            return Err(TreeError::NonUTF8Path(
                       path.to_string_lossy().into_owned()));
        }
    };
    assert!(chars == path.to_string_lossy());

    /*
    if chars.char_at(0) == '.' {
        return Err(TreeError::FoundDotfile(chars.to_owned()));
    }
    */

    for c in chars.chars() {
        if c == '\\' ||
           c == ':' ||
           c == ',' ||
           (c.is_whitespace() && c != ' ')
        {
            if c != ' ' {
                return Err(TreeError::InvalidCharacter(
                        chars.to_owned() + " at char: " + &c.to_string()));
            }
        }

    }

    return Ok(());
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    use super::*;
    use std::path::Path;

    static NAMES: [&'static str; 4] = ["a", "b", "c", "d"];

    fn add_children_to_node(node: &mut Node) {
        for name in &NAMES {
            node.add_child(name).unwrap();
        }
    }

    #[test]
    fn test_recursive_tree() {
        let _ = env_logger::init();
        let mut tree = Tree::new();
        {
            let root = tree.lookup(Path::new("/")).unwrap();
            add_children_to_node(root);
        }
        {
            for name in &NAMES {
                let node = tree.lookup(Path::new(format!("/{}", *name).as_str())).unwrap();
                add_children_to_node(node);
            }
        }
    }

    #[test]
    fn test_remove_node() {
        let _ = env_logger::init();
        let mut tree = Tree::new();
        {
            let root = tree.lookup(Path::new("/")).unwrap();
            root.add_child("hello").unwrap();
            root.remove_child("hello").unwrap();
        }
    }
}
