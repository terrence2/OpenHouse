// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use path::{PathBuilder, Glob, Path, PathIter};
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


/// A node is either a file or a directory.
#[derive(Debug)]
enum Node {
    Directory(DirectoryData),
    File(FileData)
}

/// A file contains a string. It implements get_data and set_data.
#[derive(Debug)]
pub struct FileData {
    data: String
}

/// A directory contains a list of children.
#[derive(Debug)]
pub struct DirectoryData {
    children: HashMap<String, Node>
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

    /// Recursively trawl all directories finding matching globs. Note that
    /// doing something smarter here is really hard because any ** will force
    /// us to visit most paths anyway.
    ///
    /// TODO: think of reasonable caching strategies.
    pub fn find_matching_files_recursive(&mut self, own_path: &Path, glob: &Glob)
        -> TreeResult<Vec<(Path, &mut FileData)>>
    {
        let mut acc: Vec<(Path, &mut FileData)> = Vec::new();
        for (child_name, child_node) in &mut self.children {
            let child_path = try!(own_path.slash(child_name));
            match child_node {
                &mut Node::Directory(ref mut d) => {
                    let matching = try!(d.find_matching_files_recursive(&child_path, glob));
                    acc.extend(matching);
                }
                &mut Node::File(ref mut f) => {
                    if glob.matches(&child_path) {
                        acc.push((child_path, f));
                    }
                }
            }
        }
        return Ok(acc);
    }

    // Internal helper for add_foo.
    fn add_child(&mut self, name: &str, node: Node) -> TreeResult<()> {
        try!(PathBuilder::validate_path_component(name));
        if self.children.contains_key(name) {
            return Err(Box::new(TreeError::NodeAlreadyExists(name.to_owned())));
        }
        let result = self.children.insert(name.to_owned(), node);
        assert!(result.is_none());
        return Ok(());
    }

    /// Returns the directory that was just created.
    pub fn add_directory(&mut self, name: &str) -> TreeResult<&mut DirectoryData> {
        try!(self.add_child(name, Node::Directory(DirectoryData::new())));
        return self.get_child_directory(name);
    }

    /// Returns the file that was just created.
    pub fn add_file(&mut self, name: &str) -> TreeResult<&mut FileData> {
        try!(self.add_child(name, Node::File(FileData::new())));
        return match self.children.get_mut(name) {
            None => panic!("Unable to find a file we just added!"),
            Some(n) => match n {
                &mut Node::Directory(_) => panic!("We just added a file!"),
                &mut Node::File(ref mut f) => Ok(f)
            }
        };
    }

    // Returns the indicated child.
    fn get_child_directory(&mut self, name: &str) -> TreeResult<&mut DirectoryData> {
        return match self.children.get_mut(name) {
            None => Err(Box::new(TreeError::NoSuchNode(name.to_owned()))),
            Some(n) => match n {
                &mut Node::File(_) => Err(Box::new(TreeError::NotDirectory(name.to_owned()))),
                &mut Node::Directory(ref mut d) => Ok(d)
            }
        };
    }

    /// Remove the given name from the tree.
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
impl FileData {
    fn new() -> FileData { FileData { data: "".to_owned() } }
    pub fn set_data(&mut self, new_data: &str) { self.data = new_data.to_owned(); }
    pub fn ref_data(&self) -> &str { &self.data }
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
    /// TODO: do we still need the lifetime params?
    pub fn find_matching_files<'a>(&'a mut self, glob: &'a Glob)
        -> TreeResult<Vec<(Path, &mut FileData)>>
    {
        let mut path = try!(try!(PathBuilder::new("/")).finish_path());
        return self.root.find_matching_files_recursive(&mut path, glob);
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
                [ $( $dirnames:expr ),* ],
                [ $( $filenames:expr ),* ],
                [ $( $results:expr ),* ]
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
                    let results = tree.find_matching_files(&glob).unwrap();
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
        (test_match_one_char, "/?",
         ["/d"], ["/a", "/b", "/c", "/aa", "/bb", "/cc", "/d/a"],
         ["/a", "/b", "/c"])
       ,(test_match_one_char_subdir, "/?/a",
         ["/d", "/e", "/f", "/f/g"], ["/a", "/b", "/c", "/d/a", "/d/X", "/e/a", "/e/X",
                                      "/f/a", "/f/X", "/f/g/a", "/f/g/X"],
         ["/d/a", "/e/a", "/f/a"])
       ,(test_match_star, "/*",
         ["/d"], ["/a", "/b", "/c", "/aa", "/bb", "/cc", "/d/a", "/d/b", "/d/c"],
         ["/a", "/b", "/c", "/aa", "/bb", "/cc"])
       ,(test_match_complex, "/room/*/hue-*/*/color",
         ["/room", "/room/a", "/room/b",
          "/room/a/hue-light", "/room/a/hue-livingcolor",
          "/room/b/hue-light", "/room/b/hue-livingcolor",
          "/room/a/hue-light/a-desk", "/room/a/hue-light/a-table",
          "/room/a/hue-livingcolor/a-desk", "/room/a/hue-livingcolor/a-table",
          "/room/b/hue-light/b-desk", "/room/b/hue-light/b-table",
          "/room/b/hue-livingcolor/b-desk", "/room/b/hue-livingcolor/b-table"],
         ["/room/a/hue-light/a-desk/color", "/room/a/hue-light/a-table/color",
          "/room/a/hue-livingcolor/a-desk/color", "/room/a/hue-livingcolor/a-table/color",
          "/room/b/hue-light/b-desk/color", "/room/b/hue-light/b-table/color",
          "/room/b/hue-livingcolor/b-desk/color", "/room/b/hue-livingcolor/b-table/color"],
         ["/room/a/hue-light/a-desk/color", "/room/a/hue-light/a-table/color",
          "/room/a/hue-livingcolor/a-desk/color", "/room/a/hue-livingcolor/a-table/color",
          "/room/b/hue-light/b-desk/color", "/room/b/hue-light/b-table/color",
          "/room/b/hue-livingcolor/b-desk/color", "/room/b/hue-livingcolor/b-table/color"])
    ]);
}
