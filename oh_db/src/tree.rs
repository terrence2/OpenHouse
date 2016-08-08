// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use glob::Pattern;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

make_error!(TreeError; {
    DirectoryNotEmpty => String,
    NoSuchNode => String,
    NodeAlreadyExists => String,
    NotDirectory => String,
    NotFile => String,

    // Path format errors.
    NonAbsolutePath => String,
    Dotfile => String,
    EmptyComponent => String,
    InvalidCharacter => String,
    InvalidWhitespace => String
});
pub type TreeResult<T> = Result<T, TreeError>;

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
    fn lookup_directory_recursive(&mut self, parts: &mut TreePathIter)
        -> TreeResult<&mut DirectoryData>
    {
        let name = match parts.next() {
            Some(name) => name,
            None => return Ok(self)
        };
        let child = match self.children.get_mut(name) {
            Some(c) => c,
            None => return Err(TreeError::NoSuchNode(name.to_owned()))
        };
        return match child {
            &mut Node::File(_) => Err(TreeError::NotDirectory(name.to_owned())),
            &mut Node::Directory(ref mut d) => d.lookup_directory_recursive(parts)
        };
    }

    fn lookup_file_recursive(&mut self, parts: &mut TreePathIter)
        -> TreeResult<&mut FileData>
    {
        // Look up the next name, path or directory. If we ran out of
        // components before finding a file, then the path exists but does not
        // name a file.
        let name = match parts.next() {
            Some(name) => name,
            None => return Err(TreeError::NotFile("".to_owned()))
        };
        let child = match self.children.get_mut(name) {
            Some(c) => c,
            None => return Err(TreeError::NoSuchNode(name.to_owned()))
        };
        match child {
            &mut Node::Directory(ref mut d) => d.lookup_file_recursive(parts),
            &mut Node::File(ref mut f) => {
                // If we still have components left, then we need to return
                // NotADirectory to indicate the failed traversal.
                if parts.next().is_some() {
                    Err(TreeError::NotDirectory(name.to_owned()))
                } else {
                    Ok(f)
                }
            }
        }
    }

    pub fn add_directory(&mut self, name: &str) -> TreeResult<()> {
        return self.add_child(name, Node::Directory(DirectoryData::new()));
    }

    pub fn add_file(&mut self, name: &str) -> TreeResult<()> {
        return self.add_child(name, Node::File(FileData::new()))
    }

    fn add_child(&mut self, name: &str, node: Node) -> TreeResult<()> {
        try!(validate_path_component(name, 0, name, false));
        if self.children.contains_key(name) {
            return Err(TreeError::NodeAlreadyExists(name.to_owned()));
        }
        let result = self.children.insert(name.to_owned(), node);
        assert!(result.is_none());
        return Ok(());
    }

    pub fn remove_child(&mut self, name: &str) -> TreeResult<()> {
        try!(validate_path_component(name, 0, name, false));
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

    /// Returns the directory at the given path or an error.
    pub fn lookup_directory(&mut self, path: &TreePath)
        -> TreeResult<&mut DirectoryData>
    {
        return self.root.lookup_directory_recursive(&mut path.iter());
    }

    /// Returns the file at the given directory or an error.
    pub fn lookup_file(&mut self, path: &TreePath)
        -> TreeResult<&mut FileData>
    {
        return self.root.lookup_file_recursive(&mut path.iter());
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
#[derive(Debug)]
pub struct TreePath {
    raw: String,
    parts: Vec<String>
}

impl TreePath {
    /// Validate and create a new Tree path.
    pub fn new(raw_path: &str) -> TreeResult<TreePath> {
        try!(validate_path_common(raw_path, false));

        let mut path = TreePath {
            raw: raw_path.to_owned(),
            parts: Vec::new()
        };
        if path.raw != "/" {
            for part in path.raw.split('/').skip(1) {
                path.parts.push(part.to_owned());
            }
        }

        assert_eq!(path.raw, "/".to_owned() + &path.parts.join("/"));
        return Ok(path);
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn iter(&self) -> TreePathIter {
        TreePathIter { parts: &self.parts, offset: 0 }
    }
}

impl fmt::Display for TreePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

pub struct TreePathIter<'a> {
    parts: &'a Vec<String>,
    offset: usize
}

impl<'a> Iterator for TreePathIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.parts.len() {
            return None;
        }

        let off = self.offset;
        self.offset += 1;
        return Some(&self.parts[off]);
    }
}

/// Verify that the given glob obeys the same restrictions as those set on tree
/// paths.
pub fn validate_glob(glob: &Pattern) -> TreeResult<()> {
    validate_path_common(glob.as_str(), true)
}

fn validate_path_common(path: &str, allow_glob: bool) -> TreeResult<()>
{
    if !path.starts_with('/') {
        return Err(TreeError::NonAbsolutePath(path.to_owned()));
    }

    // Split produces two empty strings for "/", so just handle it separately
    // instead of trying to do something smart in the loop below.
    if path == "/" {
        return Ok(());
    }

    // Note that since we start with /, we have to skip the first, empty, part.
    for (i, part) in path.split('/').skip(1).enumerate() {
        try!(validate_path_component(path, i, part, allow_glob));
    }
    return Ok(());
}

fn validate_path_component(path: &str, i: usize, part: &str, allow_glob: bool)
    -> TreeResult<()>
{
    if part.len() == 0 {
        return Err(TreeError::EmptyComponent(
                path.to_owned() + " at part " + &i.to_string()));
    }
    if part.starts_with(".") {
        return Err(TreeError::Dotfile(
                path.to_owned() + " at part " + &i.to_string()));
    }

    for c in part.chars() {
        if c == '\\' ||
           c == '/' ||
           c == ':' ||
           c == ',' ||
           (!allow_glob && c == '?') ||
           (!allow_glob && c == '*') ||
           (!allow_glob && c == '[') ||
           (!allow_glob && c == ']') ||
           (!allow_glob && c == '!')
        {
            return Err(TreeError::InvalidCharacter(
                path.to_owned() + " character: " + &c.to_string()));
        }
        if c.is_whitespace() && c != ' ' {
            return Err(TreeError::InvalidWhitespace(
                format!("{} at 0x{:X}", path, c as u32)));
        }
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    use super::*;

    macro_rules! make_badpath_tests {
        ( [ $( ($expect:expr, $name:ident, $string:expr) ),* ] ) =>
        {
            $(
                #[test]
                #[should_panic(expected=$expect)]
                fn $name() {
                    TreePath::new($string).unwrap();
                }
            )*
        }
    }

    make_badpath_tests!([
        ("NonAbsolutePath", test_empty_path, ""),
        ("NonAbsolutePath", test_relative_path, "foo/bar"),
        ("EmptyComponent", test_empty_component_root, "//"),
        ("EmptyComponent", test_empty_component_front, "//foo"),
        ("EmptyComponent", test_empty_component_back, "/foo/"),
        ("EmptyComponent", test_empty_component_middle, "/foo//bar"),
        ("Dotfile", test_dotfile_self, "/foo/."),
        ("Dotfile", test_dotfile_self_middle, "/foo/./bar"),
        ("Dotfile", test_dotfile_parent, "/foo/.."),
        ("Dotfile", test_dotfile_parent_middle, "/foo/../bar"),
        ("Dotfile", test_dotfile_hidden, "/foo/.bar"),
        ("Dotfile", test_dotfile_hidden_middle, "/foo/.bar/baz"),
        ("InvalidWhitespace", test_whitespace_tab, "/foo/a\tb/baz"),
        ("InvalidWhitespace", test_whitespace_vertical_tab, "/foo/a\x0Bb/baz"),
        ("InvalidWhitespace", test_whitespace_newline, "/foo/a\nb/baz"),
        ("InvalidWhitespace", test_whitespace_carriage_return, "/foo/a\rb/baz"),
        ("InvalidWhitespace", test_whitespace_nbsp, "/foo/a\u{A0}b/baz"),
        ("InvalidCharacter", test_invalid_backslash, "/foo/a\\b/baz"),
        ("InvalidCharacter", test_invalid_colon, "/foo/a:b/baz"),
        ("InvalidCharacter", test_invalid_comma, "/foo/a,b/baz"),
        ("InvalidCharacter", test_invalid_star, "/foo/a*b/baz"),
        ("InvalidCharacter", test_invalid_question, "/foo/a?b/baz"),
        ("InvalidCharacter", test_invalid_open_bracket, "/foo/a[b/baz"),
        ("InvalidCharacter", test_invalid_close_bracket, "/foo/a]b/baz"),
        ("InvalidCharacter", test_invalid_exclamation, "/foo/a!b/baz")
    ]);

    static NAMES: [&'static str; 4] = ["a", "b", "c", "d"];

    fn add_children_to_node(node: &mut DirectoryData) {
        for name in &NAMES {
            node.add_directory(name).unwrap();
        }
    }

    #[test]
    fn test_recursive_tree() {
        let _ = env_logger::init();
        let mut tree = Tree::new();
        {
            let root = tree.lookup_directory(&TreePath::new("/").unwrap()).unwrap();
            add_children_to_node(root);
        }
        {
            for name in &NAMES {
                let path = TreePath::new(format!("/{}", name).as_str()).unwrap();
                let node = tree.lookup_directory(&path).unwrap();
                add_children_to_node(node);
            }
        }
    }

    #[test]
    fn test_remove_node() {
        let _ = env_logger::init();
        let mut tree = Tree::new();
        {
            let root = tree.lookup_directory(&TreePath::new("/").unwrap()).unwrap();
            root.add_file("hello").unwrap();
            root.remove_child("hello").unwrap();
        }
    }
}
