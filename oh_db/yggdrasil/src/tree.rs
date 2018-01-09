// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use path::{PathBuilder, Glob, Path, PathIter};
use std::collections::{HashMap, HashSet};
use ketos::{Builder, FromValue, Interpreter, Name};

mod errors {
    error_chain! {
        types {
            TreeError, TreeErrorKind, TreeResultExt, TreeResult;
        }

        links {
            PathError(::path::errors::PathError, ::path::errors::PathErrorKind);
        }

        errors {
            DirectoryNotEmpty(d: String) {
                description("directory not empty")
                display("directory not empty: '{}'", d)
            }
            NotDirectory(p: ::path::Path) {
                description("not a directory")
                display("not a directory: '{}'", p)
            }
            NotFile(p: ::path::Path) {
                description("not a file")
                display("not a file: '{}'", p)
            }
            NoSuchNode(name: String) {
                description("no such node")
                display("no such node: '{}'", name)
            }
            NodeAlreadyExists(name: String) {
                description("path already exists")
                display("path already exists: '{}'", name)
            }
            FormulaRemovalDisallowed
            FormulaExecutionFailure(error: String) {
                description("formula execution failure")
                display("formula execution failure: '{}'", error)
            }
            FormulaTypeError(error: String) {
                description("formula type error")
                display("formula type error: '{}'", error)
            }
            FormulaSyntaxError(error: String) {
                description("formula syntax error")
                display("formula syntax error: '{}'", error)
            }
            FormulaInputNotFound(error: String) {
                description("formula input not found")
                display("formula input not found: '{}'", error)
            }
        }
    }
}
use tree::errors::{TreeErrorKind, TreeResult};

pub type TreeChanges = HashMap<String, Vec<Path>>;

/// Each node contains a Directory of more nodes or some leaf data.
enum Node {
    Directory(DirectoryData),
    Formula(FormulaData),
    File(FileData),
}
impl Node {
    // Follow the parts of the path iterator until we reach the terminal
    // node, returning it.
    fn lookup(&self, parts: &mut PathIter) -> TreeResult<&Node> {
        let name = match parts.next() {
            Some(name) => name,
            None => return Ok(self),
        };
        return match self {
            &Node::Directory(ref d) => d.lookup(name)?.lookup(parts),
            _ => Err(TreeErrorKind::NoSuchNode(name.into()).into())
        };
    }

    // Like lookup, but takes and returns mutable references.
    fn lookup_mut(&mut self, parts: &mut PathIter) -> TreeResult<&mut Node> {
        let name = match parts.next() {
            Some(name) => name,
            None => return Ok(self),
        };
        return match self {
            &mut Node::Directory(ref mut d) => d.lookup_mut(name)?.lookup_mut(parts),
            _ => Err(TreeErrorKind::NoSuchNode(name.into()).into())
        };
    }

    // Return all files that match glob, given that this node is at |own_path|.
    fn find(&self, own_path: &Path, glob: &Glob) -> TreeResult<Vec<(Path, &Node)>> {
        let mut acc: Vec<(Path, &Node)> = Vec::new();
        match self {
            &Node::Directory(ref d) => {
                for (child_name, child_node) in d.children.iter() {
                    let child_path = own_path.slash(child_name)?;
                    let matching = child_node.find(&child_path, glob)?;
                    acc.extend(matching);
                }
            }
            &Node::File(_) => {
                if glob.matches(&own_path) {
                    acc.push((own_path.clone(), self));
                }
            }
            &Node::Formula(_) => {
                if glob.matches(&own_path) {
                    acc.push((own_path.clone(), self));
                }
            }
        }
        return Ok(acc);
    }

    // As with find but taking and returning mutable references.
    fn find_mut(&mut self, own_path: &Path, glob: &Glob) -> TreeResult<Vec<(Path, &mut Node)>> {
        let mut acc: Vec<(Path, &mut Node)> = Vec::new();
        match self {
            &mut Node::Directory(ref mut d) => {
                for (child_name, child_node) in d.children.iter_mut() {
                    let child_path = own_path.slash(child_name)?;
                    let matching = child_node.find_mut(&child_path, glob)?;
                    acc.extend(matching);
                }
            }
            &mut Node::File(_) => {
                if glob.matches(&own_path) {
                    acc.push((own_path.clone(), self));
                }
            }
            &mut Node::Formula(_) => {}
        }
        return Ok(acc);
    }
}

/// A file is a basic data holder.
pub struct FileData {
    data: String,
}
impl FileData {
    fn new() -> FileData {
        FileData { data: "".to_owned() }
    }
    pub fn set_data(&mut self, new_data: &str) {
        self.data = new_data.to_owned();
    }
    pub fn get_data(&self) -> String {
        return self.data.clone();
    }
}


/// A formula is a value that is recomputed when its inputs change. Setting data
/// on a formula node does nothing, although is allowed to simplify the implementation.
pub struct FormulaData {
    inputs: HashMap<Name, Path>,
    interp: Interpreter,
}
impl FormulaData {
    pub fn new(raw_inputs: &HashMap<String, Path>, formula: &str) -> TreeResult<FormulaData> {
        let builder = Builder::new().name("__formula__");
        let interp = builder.finish();
        let mut inputs = HashMap::<Name, Path>::new();
        for (raw_name, path) in raw_inputs.iter() {
            let name = interp.scope().add_name(raw_name);
            inputs.insert(name, path.clone());
        }
        let program = format!("(define (__compiled__) {})", formula);
        if let Err(e) = interp.run_code(&program, None) {
            return Err(
                TreeErrorKind::FormulaSyntaxError(e.description().into()).into(),
            );
        }
        return Ok(FormulaData {
            inputs: inputs.clone(),
            interp: interp,
        });
    }
    fn get_data(&self, tree: &Tree) -> TreeResult<String> {
        for (name, path) in self.inputs.iter() {
            let data = match tree.get_data_at(path) {
                Ok(d) => d,
                Err(e) => {
                    return Err(
                        TreeErrorKind::FormulaInputNotFound(format!("{} - from: {}", path, e))
                            .into(),
                    );
                }
            };
            self.interp.scope().add_value(*name, data.clone().into());
        }

        let result = self.interp.call("__compiled__", vec![]);
        return match result {
            Err(e) => Err(
                TreeErrorKind::FormulaExecutionFailure(format!("{:?}", e)).into(),
            ),
            Ok(v) => {
                match String::from_value(v) {
                    Err(e) => Err(TreeErrorKind::FormulaTypeError(format!("{:?}", e)).into()),
                    Ok(s) => Ok(s),
                }
            }
        };
    }
}


/// A directory contains a list of children.
pub struct DirectoryData {
    children: HashMap<String, Node>,
}

impl DirectoryData {
    fn new() -> Self {
        DirectoryData { children: HashMap::new() }
    }

    // Find and return the given node regardless of type.
    fn lookup(&self, name: &str) -> TreeResult<&Node> {
        return self.children.get(name).ok_or_else(|| {
            TreeErrorKind::NoSuchNode(name.into()).into()
        });
    }

    // Find and return the given node regardless of type.
    fn lookup_mut(&mut self, name: &str) -> TreeResult<&mut Node> {
        return self.children.get_mut(name).ok_or_else(|| {
            TreeErrorKind::NoSuchNode(name.into()).into()
        });
    }

    // Internal helper for add_foo.
    fn add_child(&mut self, name: &str, node: Node) -> TreeResult<()> {
        PathBuilder::validate_path_component(name)?;
        if self.children.contains_key(name) {
            return Err(TreeErrorKind::NodeAlreadyExists(name.into()).into());
        }
        let result = self.children.insert(name.to_owned(), node);
        assert!(result.is_none());
        return Ok(());
    }

    /// Returns the directory that was just created.
    pub fn add_directory(&mut self, name: &str) -> TreeResult<&mut DirectoryData> {
        self.add_child(name, Node::Directory(DirectoryData::new()))?;
        return match self.lookup_mut(name)? {
            &mut Node::Directory(ref mut dir) => Ok(dir),
            &mut Node::Formula(_) => bail!("expected directory node"),
            &mut Node::File(_) => bail!("expected directory node")
        };
    }

    /// Adds a file to this directory at |name|.
    pub fn add_file(&mut self, name: &str) -> TreeResult<&mut FileData> {
        self.add_child(name, Node::File(FileData::new()))?;
        return match self.lookup_mut(name)? {
            &mut Node::File(ref mut file) => Ok(file),
            &mut Node::Formula(_) => bail!("expected file node"),
            &mut Node::Directory(_) => bail!("expected file node")
        };
    }

    /// Add an existing formula at the given name.
    pub fn graft_formula(&mut self, name: &str, formula: FormulaData) -> TreeResult<()> {
        self.add_child(name, Node::Formula(formula))
    }

    /// Remove the given name from the tree.
    pub fn remove_child(&mut self, name: &str) -> TreeResult<()> {
        PathBuilder::validate_path_component(name)?;
        {
            let child = self.children.get(name).ok_or(TreeErrorKind::NoSuchNode(
                name.into(),
            ))?;
            if let &Node::Directory(ref d) = child {
                if !d.children.is_empty() {
                    bail!(TreeErrorKind::DirectoryNotEmpty(name.into()));
                }
            } else if let &Node::Formula(_) = child {
                // FIXME: move removal up a level so we can fixup the inputs hash.
                bail!(TreeErrorKind::FormulaRemovalDisallowed);
            }
        }
        let result = self.children.remove(name);
        assert!(result.is_some());
        return Ok(());
    }

    pub fn list_directory(&mut self) -> Vec<String> {
        self.children.keys().map(|name| { name.clone() }).collect::<Vec<String>>()
    }
}

/// A tree of Node.
pub struct Tree {
    root: Node,
    formula_inputs: HashMap<Path, HashSet<Path>>,
}
impl Tree {
    /// Creates a new, empty Tree.
    pub fn new() -> Tree {
        Tree {
            root: Node::Directory(DirectoryData::new()),
            formula_inputs: HashMap::new(),
        }
    }

    /// Returns the directory at the given path or an error.
    pub fn lookup_directory(&mut self, path: &Path) -> TreeResult<&mut DirectoryData> {
        let node = self.root.lookup_mut(&mut path.iter())?;
        return match node {
            &mut Node::File(_) => Err(TreeErrorKind::NotDirectory(path.clone()).into()),
            &mut Node::Formula(_) => Err(TreeErrorKind::NotDirectory(path.clone()).into()),
            &mut Node::Directory(ref mut d) => Ok(d),
        };
    }

    /// Create a new formula node.
    pub fn create_formula(
        &mut self,
        parent: &Path,
        name: &str,
        inputs: &HashMap<String, Path>,
        formula: &str,
    ) -> TreeResult<()> {
        // Add formula inputs to the hash for quick lookup.
        let formula_path = parent.slash(name)?;
        for path in inputs.values() {
            if !self.formula_inputs.contains_key(path) {
                self.formula_inputs.insert(path.clone(), HashSet::new());
            }
            self.formula_inputs
                .get_mut(path)
                .expect("just inserted new map")
                .insert(formula_path.clone());
        }

        // Add the formula to the tree.
        let parent = self.lookup_directory(parent)?;
        let formula = Node::Formula(FormulaData::new(inputs, formula)?);
        return parent.add_child(name, formula);
    }

    /// Returns the data at the given node.
    pub fn get_data_at(&self, path: &Path) -> TreeResult<String> {
        let node = self.root.lookup(&mut path.iter())?;
        return match node {
            &Node::File(ref f) => Ok(f.get_data()),
            &Node::Formula(ref f) => f.get_data(self),
            &Node::Directory(_) => Err(TreeErrorKind::NotFile(path.clone()).into()),
        };
    }

    /// Set the data at the given path.
    pub fn set_data_at(&mut self, path: &Path, new_data: &str) -> TreeResult<TreeChanges> {
        {
            let node = self.root.lookup_mut(&mut path.iter())?;
            match node {
                &mut Node::File(ref mut f) => f.set_data(new_data),
                _ => bail!(TreeErrorKind::NotFile(path.clone())),
            };
        }
        let mut paths = HashSet::new();
        paths.insert(path.clone());
        return self.collect_dep_graph(&paths, new_data);
    }

    /// Get all nodes that match the given glob and return their data.
    pub fn get_data_matching(&self, glob: &Glob) -> TreeResult<Vec<(Path, String)>> {
        let matching = self.root.find(&Path::root(), glob)?;
        let mut pairs = Vec::new();
        for (path, node) in matching {
            match node {
                &Node::File(ref f) => pairs.push((path, f.get_data())),
                &Node::Formula(ref f) => pairs.push((path, f.get_data(self)?)),
                &Node::Directory(_) => bail!(TreeErrorKind::NotFile(path.clone())),
            }
        }
        return Ok(pairs);
    }

    /// Set the data at all matching paths. Returns all paths that were modified.
    pub fn set_data_matching(&mut self, glob: &Glob, new_data: &str) -> TreeResult<TreeChanges> {
        let mut paths = HashSet::new();
        {
            let matching = self.root.find_mut(&Path::root(), glob)?;
            for (path, node) in matching {
                match node {
                    &mut Node::File(ref mut f) => f.set_data(new_data),
                    _ => bail!(TreeErrorKind::NotFile(path.clone())),
                }
                paths.insert(path);
            }
        }

        return self.collect_dep_graph(&paths, new_data);
    }

    // Apply the formula dependency list to the given paths iteratively until
    // we have discovered all formula paths that might possibly have changed
    // based on the initial set of data changes. Formula values can depend on
    // other formulas, so this needs to iterate to a fixed-point.
    fn collect_dep_graph(&self, paths: &HashSet<Path>, new_data: &str) -> TreeResult<TreeChanges> {
        let mut worklist: Vec<&Path> = Vec::new();
        for path in paths {
            worklist.push(path);
        }

        let mut affected: HashSet<Path> = HashSet::new();
        let mut processed: HashSet<Path> = HashSet::new();
        while worklist.len() > 0 {
            // Grab the next item in the worklist.
            let path = worklist.pop().expect("len > 0, so pop should work");

            // If we've already visited this path we can skip it.
            if processed.contains(path) {
                continue;
            }
            processed.insert(path.clone());

            // Extend affected with any deps we find.
            if self.formula_inputs.contains_key(path) {
                for affected_path in self.formula_inputs[path].iter() {
                    worklist.push(affected_path);
                    affected.insert(affected_path.clone());
                }
            }
        }

        // Our initial path seeds are things that can be modified, which means
        // that they cannot be formulas themselves. Thus, we should not have
        // found anything affected that was already in the initial paths set.
        // Assert that these sets are actually totally disjoint.
        debug_assert!(paths.intersection(&affected).collect::<Vec<_>>().len() == 0);

        // Now we can build the complete change set to send to subscribers.
        let mut changes = HashMap::new();
        changes.insert(
            new_data.to_owned(),
            paths.to_owned().into_iter().collect::<Vec<_>>(),
        );
        for path in &affected {
            let data = self.get_data_at(path)?;
            if !changes.contains_key(&data) {
                changes.insert(data.clone(), Vec::new());
            }
            changes.get_mut(&data).expect("just inserted").push(
                path.clone(),
            );
        }
        return Ok(changes);
    }
}

#[cfg(test)]
mod tests {
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
                    let results = tree.get_data_matching(&glob).unwrap();
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
    make_glob_matching_tests!(
        [
            (
                test_match_one_char,
                "/?",
                ["/d"],
                ["/a", "/b", "/c", "/aa", "/bb", "/cc", "/d/a"],
                ["/a", "/b", "/c"]
            ),
            (
                test_match_one_char_subdir,
                "/?/a",
                ["/d", "/e", "/f", "/f/g"],
                [
                    "/a",
                    "/b",
                    "/c",
                    "/d/a",
                    "/d/X",
                    "/e/a",
                    "/e/X",
                    "/f/a",
                    "/f/X",
                    "/f/g/a",
                    "/f/g/X"
                ],
                ["/d/a", "/e/a", "/f/a"]
            ),
            (
                test_match_star,
                "/*",
                ["/d"],
                [
                    "/a",
                    "/b",
                    "/c",
                    "/aa",
                    "/bb",
                    "/cc",
                    "/d/a",
                    "/d/b",
                    "/d/c"
                ],
                ["/a", "/b", "/c", "/aa", "/bb", "/cc"]
            ),
            (
                test_match_complex,
                "/room/*/hue-*/*/color",
                [
                    "/room",
                    "/room/a",
                    "/room/b",
                    "/room/a/hue-light",
                    "/room/a/hue-livingcolor",
                    "/room/b/hue-light",
                    "/room/b/hue-livingcolor",
                    "/room/a/hue-light/a-desk",
                    "/room/a/hue-light/a-table",
                    "/room/a/hue-livingcolor/a-desk",
                    "/room/a/hue-livingcolor/a-table",
                    "/room/b/hue-light/b-desk",
                    "/room/b/hue-light/b-table",
                    "/room/b/hue-livingcolor/b-desk",
                    "/room/b/hue-livingcolor/b-table"
                ],
                [
                    "/room/a/hue-light/a-desk/color",
                    "/room/a/hue-light/a-table/color",
                    "/room/a/hue-livingcolor/a-desk/color",
                    "/room/a/hue-livingcolor/a-table/color",
                    "/room/b/hue-light/b-desk/color",
                    "/room/b/hue-light/b-table/color",
                    "/room/b/hue-livingcolor/b-desk/color",
                    "/room/b/hue-livingcolor/b-table/color"
                ],
                [
                    "/room/a/hue-light/a-desk/color",
                    "/room/a/hue-light/a-table/color",
                    "/room/a/hue-livingcolor/a-desk/color",
                    "/room/a/hue-livingcolor/a-table/color",
                    "/room/b/hue-light/b-desk/color",
                    "/room/b/hue-light/b-table/color",
                    "/room/b/hue-livingcolor/b-desk/color",
                    "/room/b/hue-livingcolor/b-table/color"
                ]
            )
        ]
    );
}
