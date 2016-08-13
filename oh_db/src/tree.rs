// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use path::{PathBuilder, Glob, GlobIter, Path, PathIter};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

make_error!(TreeError; {
    DirectoryNotEmpty => String,
    NoSuchNode => String,
    NodeAlreadyExists => String,
    NotDirectory => String,
    NotFile => String
});
pub type TreeResult<T> = Result<T, Box<Error>>;


/// A directory contains a list of children.
type ChildMap = HashMap<String, Node>;
#[derive(Debug)]
pub struct DirectoryData {
    children: ChildMap
}
impl DirectoryData {
    fn new() -> Self {
        DirectoryData { children: HashMap::new() }
    }

    // Find the directory at the bottom of path. If the path crosses
    // a file, return Err(NotDirectory).
    fn lookup_directory_recursive(&mut self, parts: &mut PathIter)
        -> TreeResult<&mut DirectoryData>
    {
        let name = match parts.next() {
            Some(name) => name,
            None => return Ok(self)
        };
        let child = match self.children.get_mut(name) {
            Some(c) => c,
            None => return Err(Box::new(TreeError::NoSuchNode(name.to_owned())))
        };
        return match child {
            &mut Node::File(_) => Err(Box::new(TreeError::NotDirectory(name.to_owned()))),
            &mut Node::Directory(ref mut d) => d.lookup_directory_recursive(parts)
        };
    }

    fn lookup_file_recursive(&mut self, parts: &mut PathIter)
        -> TreeResult<&mut FileData>
    {
        // Look up the next name, path or directory. If we ran out of
        // components before finding a file, then the path exists but does not
        // name a file.
        let name = match parts.next() {
            Some(name) => name,
            None => return Err(Box::new(TreeError::NotFile("".to_owned())))
        };
        let child = match self.children.get_mut(name) {
            Some(c) => c,
            None => return Err(Box::new(TreeError::NoSuchNode(name.to_owned())))
        };
        match child {
            &mut Node::Directory(ref mut d) => d.lookup_file_recursive(parts),
            &mut Node::File(ref mut f) => {
                // If we still have components left, then we need to return
                // NotADirectory to indicate the failed traversal.
                if parts.next().is_some() {
                    Err(Box::new(TreeError::NotDirectory(name.to_owned())))
                } else {
                    Ok(f)
                }
            }
        }
    }

    fn lookup_matching_files_recursive(&mut self, path: &mut Path, parts: &mut GlobIter) ->
        TreeResult<Vec<(Path, &mut FileData)>>
    {
        let glob_component = match parts.next() {
            Some(component) => component,
            None => return Err(Box::new(TreeError::NotFile("".to_owned())))
        };
        let mut out = Vec::new();
        /*
        for (child_name, child) in self.children.iter() {
            if glob_component.matches(&child_name) {
                match child {
                    &mut Node::Directory(ref mut d) => {
                        d.lookup_matching_files_recursive(path, parts);
                    }
                    &mut Node::File(ref mut f) => {
                        if parts.next().is_some() {
                            Err(Box::new(TreeError::NotDirectory(name.to_owned())))
                        } else {
                            Ok(f)
                        }
                        out.push((path, f));
                    }
                }
            }
        }
        */
        return Ok(out);
    }

    pub fn add_directory(&mut self, name: &str) -> TreeResult<()> {
        return self.add_child(name, Node::Directory(DirectoryData::new()));
    }

    pub fn add_file(&mut self, name: &str) -> TreeResult<()> {
        return self.add_child(name, Node::File(FileData::new()))
    }

    fn add_child(&mut self, name: &str, node: Node) -> TreeResult<()> {
        try!(PathBuilder::validate_path_component(name));
        if self.children.contains_key(name) {
            return Err(Box::new(TreeError::NodeAlreadyExists(name.to_owned())));
        }
        let result = self.children.insert(name.to_owned(), node);
        assert!(result.is_none());
        return Ok(());
    }

    pub fn remove_child(&mut self, name: &str) -> TreeResult<()> {
        try!(PathBuilder::validate_path_component(name));
        {
            let child = match self.children.get(name) {
                Some(c) => c,
                None => return Err(Box::new(TreeError::NoSuchNode(name.to_owned())))
            };
            match child {
                &Node::File(_) => {},
                &Node::Directory(ref d) => {
                    if !d.children.is_empty() {
                        return Err(Box::new(TreeError::DirectoryNotEmpty(name.to_owned())));
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
#[derive(Debug)]
pub struct FileData {
    data: String
}
impl FileData {
    fn new() -> FileData {
        FileData { data: "".to_owned() }
    }

    pub fn set_data(&mut self, new_data: &str) {
        self.data = new_data.to_owned();
    }

    pub fn get_data(&self) -> String {
        self.data.clone()
    }
}

/// A node is either a file or a directory.
#[derive(Debug)]
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
    pub fn lookup_directory(&mut self, path: &Path)
        -> TreeResult<&mut DirectoryData>
    {
        return self.root.lookup_directory_recursive(&mut path.iter());
    }

    /// Returns the file at the given directory or an error.
    pub fn lookup_file(&mut self, path: &Path)
        -> TreeResult<&mut FileData>
    {
        return self.root.lookup_file_recursive(&mut path.iter());
    }

    /// Returns pairs of (path, file) that match the given glob.
    pub fn lookup_matching_files<'a>(&'a mut self, glob: &'a Glob)
        -> TreeResult<Vec<(Path, &mut FileData)>>
    {
        let mut path = try!(try!(PathBuilder::new("/")).finish_path());
        return self.root.lookup_matching_files_recursive(&mut path, &mut glob.iter());
    }
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    use super::*;
    use path::{Glob, Path, PathBuilder};

    fn make_path(p: &str) -> Path {
        PathBuilder::new(p).unwrap().finish_path().unwrap()
    }

    fn make_glob(p: &str) -> Glob {
        PathBuilder::new(p).unwrap().finish_glob().unwrap()
    }

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
            let root = tree.lookup_directory(&make_path("/")).unwrap();
            add_children_to_node(root);
        }
        {
            for name in &NAMES {
                let path = make_path(format!("/{}", name).as_str());
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
            let root = tree.lookup_directory(&make_path("/")).unwrap();
            root.add_file("hello").unwrap();
            root.remove_child("hello").unwrap();
        }
    }


    macro_rules! make_glob_matching_tests {
        ( [ $(
            (   $name:ident,
                $glob:expr,
                [
                    $( $dirnames:expr ),*
                ],
                [
                    $( $filenames:expr ),*
                ],
                [
                    $( $results:expr ),*
                ]
            )
        ),* ] ) =>
        {
            $(
                #[test]
                fn $name() {
                    let dirs: Vec<&'static str> = vec![ $($dirnames),* ];
                    let files: Vec<&'static str> = vec![ $($filenames),* ];
                    let mut expect: Vec<&'static str> = vec![ $($results),* ];

                    let mut tree = Tree::new();
                    for dir in dirs {
                        let path = make_path(dir);
                        let parent = path.parent().unwrap();
                        let name = path.basename().unwrap();
                        let parent_node = tree.lookup_directory(&parent).unwrap();
                        parent_node.add_directory(&name).unwrap();
                    }
                    for file in files {
                        let path = make_path(file);
                        let parent = path.parent().unwrap();
                        let name = path.basename().unwrap();
                        let parent_node = tree.lookup_directory(&parent).unwrap();
                        parent_node.add_file(&name).unwrap();
                    }

                    let glob = make_glob($glob);
                    let results = tree.lookup_matching_files(&glob).unwrap();
                    assert!(expect.len() == results.len());
                    for (path, _) in results {
                        let mut found = false;
                        let mut index = 0;
                        for (i, expect_path) in expect.iter().enumerate() {
                            if path.to_str() == *expect_path {
                                found = true;
                                index = i;
                                break;
                            }
                        }
                        assert!(found);
                        expect.swap_remove(index);
                    }
                    assert!(expect.len() == 0);
                }
            )*
        }
    }
    make_glob_matching_tests!([
        (test_match_one_char,
         "/?",
         [], ["/a", "/b", "/c", "/aa", "/bb", "/cc"],
         ["/a", "/b", "/c"])
    ]);

    /*
    #[test]
    fn test_glob_set() {
        let _ = env_logger::init();
        let mut tree = Tree::new();
        {
            let root = tree.lookup_directory(&make_path("/")).unwrap();
            root.add_file("a").unwrap();
            root.add_file("ab").unwrap();
            root.add_file("b").unwrap();
            root.add_file("bb").unwrap();
        }
        let glob = Pattern::new("/a*").unwrap();
        let matching = tree.lookup_matching_files(&glob).unwrap();
        assert_eq!(matching.len(), 2);
        for (path, _) in matching {
            assert!(path.to_str() == "/a" || path.to_str() == "/ab");
        }
    }
    */
}
