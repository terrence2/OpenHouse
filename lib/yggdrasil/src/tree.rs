// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    bif::{tostr::ToStr, NativeFunc},
    graph::Graph,
    parser::TreeParser,
    path::{ConcretePath, PathComponent, ScriptPath},
    physical::Dimension2,
    script::Script,
    value::Value,
};
use failure::{bail, ensure, Fallible};
use std::{
    cell::RefCell,
    collections::{hash_map::Entry, HashMap},
    default::Default,
    fs,
    path::Path,
    str::FromStr,
    sync::Arc,
};
use tracing::{trace, trace_span, warn};

pub struct TreeBuilder {
    // Extension functions defined by the embedding.
    nifs: HashMap<String, Box<dyn NativeFunc>>,

    // Add builtin functions to `nifs` before loading. (default: true)
    add_builtin_nifs: bool,

    // Handle an import of the given name by supplying a tree rather than
    // searching in the filesystem.
    import_interceptors: HashMap<String, Tree>,
}

impl Default for TreeBuilder {
    fn default() -> TreeBuilder {
        TreeBuilder {
            nifs: HashMap::new(),
            add_builtin_nifs: true,
            import_interceptors: HashMap::new(),
        }
    }
}

impl TreeBuilder {
    pub fn add_native_function(
        mut self,
        name: &str,
        nif: Box<dyn NativeFunc>,
    ) -> Fallible<TreeBuilder> {
        self.nifs.insert(name.to_owned(), nif);
        Ok(self)
    }

    pub fn intercept_import(mut self, name: &str, content: &str) -> Fallible<TreeBuilder> {
        let tree = Tree {
            root: NodeRef::new(Node::new(ConcretePath::new_root())),
            generation: 0,
        };
        let tree = TreeParser::from_str(tree, content, &self.nifs, &HashMap::new())?;
        self.import_interceptors.insert(name.to_owned(), tree);
        Ok(self)
    }

    pub fn without_builtins(mut self) -> Fallible<TreeBuilder> {
        self.add_builtin_nifs = false;
        Ok(self)
    }

    pub fn empty() -> Tree {
        Tree {
            root: NodeRef::new(Node::new(ConcretePath::new_root())),
            generation: 0,
        }
    }

    pub fn build_from_file(self, path: &Path) -> Fallible<Tree> {
        let contents = fs::read_to_string(path)?;
        self.build_from_str(&contents)
    }

    pub fn build_from_str(mut self, s: &str) -> Fallible<Tree> {
        if self.add_builtin_nifs {
            self.nifs.insert("str".to_owned(), Box::new(ToStr));
        }

        let tree = Tree {
            root: NodeRef::new(Node::new(ConcretePath::new_root())),
            generation: 0,
        };

        let tree = TreeParser::from_str(tree, s, &self.nifs, &self.import_interceptors)?
            .link_and_validate_inputs()?
            .map_inputs_to_outputs()?;

        Ok(tree)
    }
}

pub struct Tree {
    root: NodeRef,
    generation: usize,
}

impl Tree {
    pub fn handle_event(
        &mut self,
        path: &str,
        mut value: Value,
    ) -> Fallible<HashMap<String, Vec<(String, Value)>>> {
        self.generation += 1;
        value.set_generation(self.generation);

        let source = self.lookup(path)?;
        source.handle_event(value)?; // cache the value
        let sink_nodes = source.get_sink_nodes_observing()?;

        let mut groups = HashMap::new();
        for node in &sink_nodes {
            let next_value = node.compute(self)?;
            let kind = node.sink_kind()?;
            let value = (node.path_str(), next_value);
            match groups.entry(kind) {
                Entry::Vacant(e) => {
                    e.insert(vec![value]);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().push(value);
                }
            }
        }
        Ok(groups)
    }

    pub fn root(&self) -> NodeRef {
        self.root.clone()
    }

    pub fn lookup(&self, path: &str) -> Fallible<NodeRef> {
        let concrete = ConcretePath::from_str(path)?;
        self.lookup_path(&concrete)
    }

    pub fn lookup_path(&self, path: &ConcretePath) -> Fallible<NodeRef> {
        self.root.lookup_path(&path.components[0..])
    }

    pub fn lookup_dynamic_path(&self, path: &ScriptPath) -> Fallible<NodeRef> {
        self.root.lookup_dynamic_path(&path.components[0..], self)
    }

    // After the tree has been built, visit all nodes looking up references and
    // storing those references directly in the inputs list per script.
    fn link_and_validate_inputs(self) -> Fallible<Tree> {
        self.root.link_and_validate_inputs(&self)?;
        Ok(self)
    }

    fn map_inputs_to_outputs(self) -> Fallible<Tree> {
        let mut graph = Graph::new_empty();
        let mut sinks = Vec::new();
        self.root().populate_flow_graph(&mut graph)?;
        self.root().find_all_sinks(&mut sinks)?;
        self.root().flow_input_to_output(&sinks, &graph)?;
        Ok(self)
    }

    pub fn subtree_at(&self, root: &NodeRef) -> Fallible<SubTree> {
        SubTree::new(self, root)
    }

    pub fn find_sinks(&self, name: &str) -> Vec<String> {
        let mut matching = Vec::new();
        self.root().find_sinks(name, &mut matching);
        matching
    }

    pub fn find_sources(&self, name: &str) -> Vec<String> {
        let mut matching = Vec::new();
        self.root().find_sources(name, &mut matching);
        matching
    }
}

pub struct SubTree<'a> {
    _tree: &'a Tree,
    _root: NodeRef,
}

impl<'a> SubTree<'a> {
    fn new(tree: &'a Tree, root: &NodeRef) -> Fallible<Self> {
        Ok(SubTree {
            _tree: tree,
            _root: root.to_owned(),
        })
    }

    pub fn lookup(&self, path: &str) -> Fallible<NodeRef> {
        let concrete = ConcretePath::from_str(path)?;
        self._root.lookup_path(&concrete.components[0..])
    }

    pub fn tree(&self) -> &'a Tree {
        self._tree
    }
}

#[derive(Clone)]
pub struct NodeRef(Arc<RefCell<Node>>);

impl NodeRef {
    pub fn new(node: Node) -> Self {
        let self_ref = NodeRef(Arc::new(RefCell::new(node)));
        self_ref
            .0
            .borrow_mut()
            .children
            .insert(".".to_owned(), self_ref.clone());
        self_ref
    }

    pub fn lookup_path(&self, parts: &[String]) -> Fallible<NodeRef> {
        if parts.is_empty() {
            return Ok(self.to_owned());
        }
        if let Some(child) = self.child_at(&parts[0]) {
            return child.lookup_path(&parts[1..]);
        }
        bail!(
            "runtime error: lookup on path that does not exist; at {} -> {:?}",
            self.path_str(),
            parts
        )
    }

    pub fn lookup_dynamic_path(&self, parts: &[PathComponent], tree: &Tree) -> Fallible<NodeRef> {
        trace!(
            "Node::lookup_dynamic_path @ {}, looking up {:?}",
            self.path_str(),
            parts
        );
        let child_name = match &parts[0] {
            PathComponent::Name(n) => n.to_owned(),
            PathComponent::Lookup(p) => tree
                .lookup_dynamic_path(p)?
                .compute(tree)?
                .as_path_component()?,
        };
        if let Some(child) = self.child_at(&child_name) {
            if parts.len() == 1 {
                return Ok(child);
            }
            return Ok(child.lookup_dynamic_path(&parts[1..], tree)?);
        }
        bail!(format!(
            "invalid path: did not find path component '{}' @ {}",
            child_name,
            self.path_str()
        ))
    }

    fn find_sinks(&self, sink_name: &str, matching: &mut Vec<String>) {
        if let Some(name) = self.maybe_sink_kind() {
            if name == sink_name {
                matching.push(self.path_str());
            }
        }
        for (name, child) in &self.0.borrow().children {
            if name == "." || name == ".." {
                continue;
            }
            child.find_sinks(sink_name, matching);
        }
    }

    fn find_sources(&self, source_name: &str, matching: &mut Vec<String>) {
        if let Some(name) = self.maybe_source_kind() {
            if name == source_name {
                matching.push(self.path_str());
            }
        }
        for (name, child) in &self.0.borrow().children {
            if name == "." || name == ".." {
                continue;
            }
            child.find_sources(source_name, matching);
        }
    }

    pub fn add_child(&self, name: &str) -> Fallible<NodeRef> {
        let child = self.0.borrow_mut().create_new_child(name)?;
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
        Ok(child)
    }

    pub fn child_names(&self) -> Vec<String> {
        self.0
            .borrow()
            .children
            .keys()
            .filter(|&f| f != "." && f != "..")
            .cloned()
            .collect::<Vec<_>>()
    }

    pub fn name(&self) -> String {
        self.0.borrow().name.clone()
    }

    pub(super) fn link_and_validate_inputs(&self, tree: &Tree) -> Fallible<()> {
        let span = trace_span!("link", "{}", self.path_str());
        let _ = span.enter();
        if self.0.borrow().linked_and_validated {
            return Ok(());
        }
        self.0.borrow_mut().linked_and_validated = true;

        // Note: this pattern is a little funky! Normally we'd match to test
        // these conditions, but if we did that the borrow would last over the
        // body, which would disallow us from re-borrowing mutably inside.
        if self.has_script() {
            // Collect input map while borrowed read-only, so that we can find children.
            let data = if let Some(NodeInput::Script(ref script)) = self.0.borrow().input {
                script.build_input_map(tree)?
            } else {
                unreachable!();
            };

            // Re-borrow read-write to install the input map we built above.
            if let Some(NodeInput::Script(ref mut script)) = self.0.borrow_mut().input {
                script.install_input_map(data)?;
            }
        }

        // Recurse into our children. Use sorted order so results are stable.
        let mut children: Vec<String> = self
            .0
            .borrow()
            .children
            .keys()
            .filter(|s| *s != "." && *s != "..")
            .map(|s| s.to_owned())
            .collect::<_>();
        children.sort();
        for name in &children {
            let child = &self.0.borrow().children[name];
            child.link_and_validate_inputs(tree)?;
        }

        Ok(())
    }

    fn find_all_sinks(&self, sinks: &mut Vec<NodeRef>) -> Fallible<()> {
        for (name, child) in &self.0.borrow().children {
            if name == "." || name == ".." {
                continue;
            }
            child.find_all_sinks(sinks)?;
        }
        if self.0.borrow().sink.is_some() {
            sinks.push(self.to_owned());
        }
        Ok(())
    }

    fn populate_flow_graph(&self, graph: &mut Graph) -> Fallible<()> {
        graph.add_node(self);
        for (name, child) in &self.0.borrow().children {
            if name == "." || name == ".." {
                continue;
            }
            child.populate_flow_graph(graph)?;
        }

        if let Some(NodeInput::Script(ref script)) = self.0.borrow().input {
            script.populate_flow_graph(self, graph)?;
        }

        Ok(())
    }

    fn flow_input_to_output(&self, sinks: &[NodeRef], graph: &Graph) -> Fallible<()> {
        for (name, child) in &self.0.borrow().children {
            if name == "." || name == ".." {
                continue;
            }
            child.flow_input_to_output(sinks, graph)?;
        }

        let mut maybe_connected_sinks = None;
        if let Some(NodeInput::Source(_, _)) = self.0.borrow().input {
            let connected = graph.connected_nodes(self, sinks)?;
            if connected.is_empty() {
                warn!(
                    "dataflow warning: source at {} is not connected to any sinks",
                    self.path_str()
                );
            }
            maybe_connected_sinks = Some(connected);
        };

        if let Some(mut connected_sinks) = maybe_connected_sinks {
            if let Some(NodeInput::Source(_, ref mut sinks)) = self.0.borrow_mut().input {
                assert!(
                    sinks.is_empty(),
                    "dataflow error: found connected sinks at {}, but sinks already set",
                    self.path_str()
                );
                sinks.append(&mut connected_sinks);
            } else {
                panic!("expected source to not mutate")
            }
        }

        Ok(())
    }

    fn has_script(&self) -> bool {
        if let Some(NodeInput::Script(_)) = self.0.borrow().input {
            return true;
        }
        false
    }

    fn child_at(&self, name: &str) -> Option<NodeRef> {
        self.0.borrow().children.get(name).map(|v| v.to_owned())
    }

    pub fn path_str(&self) -> String {
        self.0.borrow().path.to_string()
    }

    pub(super) fn handle_event(&self, value: Value) -> Fallible<()> {
        ensure!(self.is_source(), "received event on non-source node");
        let mut node = self.0.borrow_mut();
        node.cache = Some(value);
        Ok(())
    }

    pub fn location(&self) -> Option<Dimension2> {
        self.0.borrow().location
    }

    pub fn set_location(&self, loc: Dimension2) -> Fallible<()> {
        ensure!(
            self.0.borrow().location.is_none(),
            "location has already been set"
        );
        self.0.borrow_mut().location = Some(loc);
        Ok(())
    }

    pub fn dimensions(&self) -> Option<Dimension2> {
        self.0.borrow().dimensions
    }

    pub fn set_dimensions(&self, dim: Dimension2) -> Fallible<()> {
        ensure!(
            self.0.borrow().dimensions.is_none(),
            "dimensions have already been set"
        );
        self.0.borrow_mut().dimensions = Some(dim);
        Ok(())
    }

    pub fn set_source(&self, from: &str) -> Fallible<()> {
        ensure!(
            self.0.borrow().input.is_none(),
            "parse error: input was set twice @ {}",
            self.0.borrow().path
        );
        self.0.borrow_mut().input = Some(NodeInput::Source(from.to_owned(), Vec::new()));
        Ok(())
    }

    pub fn set_sink(&self, tgt: &str) -> Fallible<()> {
        ensure!(
            self.0.borrow().sink.is_none(),
            "parse error: sink set twice @ {}",
            self.0.borrow().path
        );
        self.0.borrow_mut().sink = Some(tgt.to_owned());
        Ok(())
    }

    pub fn insert_subtree(&self, subtree: &NodeRef) -> Fallible<()> {
        for (name, child) in &subtree.0.borrow().children {
            self.0
                .borrow_mut()
                .children
                .insert(name.to_owned(), child.to_owned());
        }
        Ok(())
    }

    pub fn apply_template(&self, template: &NodeRef) -> Fallible<()> {
        // FIXME: -> copy children... probably needs to be lexical?

        // Simple sigils.
        // location: Option<Dimension2>,
        // dimensions: Option<Dimension2>,

        // Input data binding can either be an external system or a computed value
        // pulling inputs from external systems and other computed values. Or
        // nothing; it's fine for a node to just be structural.
        // input: Option<NodeInput>,
        // _cache: Option<Sample>,

        // Optional output data binding.
        // sink: Option<(String, SinkRef)>,

        if let Some(dim) = template.location() {
            self.set_location(dim)?;
        }
        Ok(())
    }

    pub fn set_script(&self, script: Script) -> Fallible<()> {
        ensure!(
            self.0.borrow().input.is_none(),
            "parse error: input was set twice at {}",
            self.0.borrow().path
        );
        self.0.borrow_mut().input = Some(NodeInput::Script(script));
        Ok(())
    }

    pub fn compute(&self, tree: &Tree) -> Fallible<Value> {
        // FIXME: we need to make this entire path mut, so that we can write back the
        // FIXME: computed entries as we compute them. For now, we'll be re-computing
        // FIXME: intermediate nodes. Note: The cache *is* populated for source nodes
        // FIXME: by handle_event, which is mut.
        if let Some(ref cached_value) = self.0.borrow().cache {
            assert!(self.is_source());
            return Ok(cached_value.to_owned());
        }

        let path = self.path_str();
        trace!("computing @ {}", path);
        match self.0.borrow().input {
            None => bail!("runtime error: computing a non-input path @ {}", path),
            Some(NodeInput::Script(ref script)) => script.compute(tree),
            Some(NodeInput::Source(_, _)) => {
                bail!("computing value of a source node; should have been cached by handle_event");
            }
        }
    }

    pub fn get_sink_nodes_observing(&self) -> Fallible<Vec<NodeRef>> {
        if let Some(NodeInput::Source(_, ref sinks)) = self.0.borrow().input {
            return Ok(sinks.to_owned());
        }
        bail!(
            "runtime: invalid event; occurred on node {} with no source",
            self.path_str()
        )
    }

    pub fn sink_kind(&self) -> Fallible<String> {
        if let Some(ref kind) = self.0.borrow().sink {
            return Ok(kind.to_owned());
        }
        bail!(
            "runtime: tried to get sink kind of the non-sink node at {}",
            self.path_str()
        )
    }

    pub fn maybe_sink_kind(&self) -> Option<String> {
        if let Some(ref kind) = self.0.borrow().sink {
            return Some(kind.to_owned());
        }
        None
    }

    pub fn is_source(&self) -> bool {
        if let Some(NodeInput::Source(_, _)) = self.0.borrow().input {
            return true;
        }
        false
    }

    pub fn maybe_source_kind(&self) -> Option<String> {
        if let Some(NodeInput::Source(ref kind, _)) = self.0.borrow().input {
            return Some(kind.to_owned());
        }
        None
    }
}

enum NodeInput {
    Source(String, Vec<NodeRef>),
    Script(Script),
}

pub struct Node {
    // The tree structure.
    name: String,
    path: ConcretePath,
    children: HashMap<String, NodeRef>,
    linked_and_validated: bool,

    // Simple sigils.
    location: Option<Dimension2>,
    dimensions: Option<Dimension2>,

    // Input data binding can either be an external system or a computed value
    // pulling inputs from external systems and other computed values. Or
    // nothing; it's fine for a node to just be structural.
    input: Option<NodeInput>,
    cache: Option<Value>,

    // Optional output data binding.
    sink: Option<String>,
}

impl Node {
    pub fn new(path: ConcretePath) -> Self {
        Node {
            name: path.basename().to_owned(),
            path,
            children: HashMap::new(),
            linked_and_validated: false,
            location: None,
            dimensions: None,
            input: None,
            cache: None,
            sink: None,
        }
    }

    fn create_new_child(&mut self, name: &str) -> Fallible<NodeRef> {
        let child = NodeRef::new(Node::new(self.path.new_child(name)));
        self.children.insert(name.to_owned(), child.clone());
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tree() -> Fallible<()> {
        let tree = TreeBuilder::empty();
        assert_eq!(None, tree.root().location());

        let d10 = Dimension2::from_str("10x10")?;
        let d20 = Dimension2::from_str("20x20")?;

        let child = tree.lookup("/")?.add_child("foopy")?;
        child.set_location(d10)?;
        assert_eq!(d10, child.location().unwrap());

        let child = tree.lookup("/foopy")?.add_child("barmy")?;
        child.set_location(d20)?;
        child.set_dimensions(d20)?;
        assert_eq!(d20, child.location().unwrap());

        assert_eq!(d10, tree.lookup("/foopy")?.location().unwrap());
        assert_eq!(d20, tree.lookup("/foopy/barmy")?.location().unwrap());
        assert_eq!(d20, tree.lookup("/foopy/barmy")?.location().unwrap());
        assert_eq!(d10, tree.lookup("/foopy")?.location().unwrap());
        Ok(())
    }

    #[test]
    fn test_tree_jail_source_ok() -> Fallible<()> {
        let s = r#"
a ^src1
    a <- ./{/a/b} + "2"
    b <- ./{./c} + "1"
    c <- "c"
    c1 <- "d"
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        let result = tree.lookup("/a/a")?.compute(&tree)?;
        assert_eq!(result, Value::new_str("d2"));
        Ok(())
    }

    #[test]
    fn test_tree_sources() -> Fallible<()> {
        let s = r#"
a ^src1
b <-/{/a}/v
foo
    v<-1
bar
    v<-2
"#;
        let mut tree = TreeBuilder::default().build_from_str(s)?;
        tree.handle_event("/a", Value::new_str("bar"))?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::from_integer(2));

        tree.handle_event("/a", Value::new_str("foo"))?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::from_integer(1));
        Ok(())
    }

    #[test]
    fn test_tree_import_str() -> Fallible<()> {
        let test_ygg = r#"
a
    b
        c <- "hello"
"#;
        let s = r#"
import(test.ygg)
foo <- /a/b/c
"#;
        let tree = TreeBuilder::default()
            .intercept_import("test.ygg", test_ygg)?
            .build_from_str(s)?;
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::new_str("hello")
        );
        Ok(())
    }

    #[test]
    fn test_tree_import_nested() -> Fallible<()> {
        let test_ygg = r#"
a
    b
        c <- "hello"
"#;
        let s = r#"
mnt
    import(test.ygg)
foo <- /mnt/a/b/c
"#;
        let tree = TreeBuilder::default()
            .intercept_import("test.ygg", test_ygg)?
            .build_from_str(s)?;
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::new_str("hello")
        );
        Ok(())
    }
}
