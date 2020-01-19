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
    sink::SinkRef,
    source::SourceRef,
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

/// The combination of a Value plus a monotonic ordinal.
pub struct Sample {
    _value: Value,
    _at: usize,
}

pub struct TreeBuilder {
    // Input and output definitions.
    source_handlers: HashMap<String, SourceRef>,
    sink_handlers: HashMap<String, SinkRef>,

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
            source_handlers: HashMap::new(),
            sink_handlers: HashMap::new(),
            nifs: HashMap::new(),
            add_builtin_nifs: true,
            import_interceptors: HashMap::new(),
        }
    }
}

impl TreeBuilder {
    pub fn add_source_handler(mut self, name: &str, source: &SourceRef) -> Fallible<TreeBuilder> {
        self.source_handlers.insert(name.to_owned(), source.clone());
        Ok(self)
    }

    pub fn add_sink_handler(mut self, name: &str, sink: &SinkRef) -> Fallible<TreeBuilder> {
        self.sink_handlers.insert(name.to_owned(), sink.clone());
        Ok(self)
    }

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
            source_handlers: self.source_handlers.clone(),
            sink_handlers: self.sink_handlers.clone(),
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
            source_handlers: HashMap::new(),
            sink_handlers: HashMap::new(),
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
            source_handlers: self.source_handlers,
            sink_handlers: self.sink_handlers,
        };

        let tree = TreeParser::from_str(tree, s, &self.nifs, &self.import_interceptors)?
            .link_and_validate_inputs()?
            .map_inputs_to_outputs()?;

        for sink in tree.sink_handlers.values() {
            sink.on_ready(&tree.root.subtree_here(&tree)?)?;
        }
        Ok(tree)
    }
}

pub struct Tree {
    root: NodeRef,
    generation: usize,
    source_handlers: HashMap<String, SourceRef>,
    sink_handlers: HashMap<String, SinkRef>,
}

impl Tree {
    pub fn handle_event(&self, path: &str, mut value: Value) -> Fallible<()> {
        // FIXME: make this mutable once we back off systems
        //self.generation += 1;
        value.set_generation(self.generation);

        let source = self.lookup(path)?;
        source.handle_event(value, self)?;
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
        for (kind, values) in &groups {
            self.sink_handlers[kind].values_updated(values)?;
        }
        Ok(())
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

    pub(super) fn subtree_at(&self, root: &NodeRef) -> Fallible<SubTree> {
        SubTree::new(self, root)
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

        // If this is a source node, tell the associated source handler about it.
        if let Some(NodeInput::Source(ref source, _)) = self.0.borrow().input {
            self.enforce_jail()?;
            source.add_path(&self.path_str(), &tree.subtree_at(self)?)?;
        }
        if let Some((ref _kind, ref sink)) = self.0.borrow().sink {
            self.enforce_jail()?;
            sink.add_path(&self.path_str(), &tree.subtree_at(self)?)?;
        }

        Ok(())
    }

    fn enforce_jail(&self) -> Fallible<()> {
        trace!("enforcing jail @ {}", self.path_str());
        for (name, child) in &self.0.borrow().children {
            if name == "." || name == ".." {
                continue;
            }
            child.enforce_jail_under(&self.path_str())?;
        }
        Ok(())
    }

    fn enforce_jail_under(&self, jail_path: &str) -> Fallible<()> {
        trace!("enforcing jail path {} @ {}", jail_path, self.path_str());
        if let Some(NodeInput::Script(ref script)) = self.0.borrow().input {
            for input_path in &script.all_inputs()? {
                trace!("checking input: {}", input_path);
                if !input_path.starts_with(jail_path) {
                    bail!(
                        "jailbreak error @ {}: referenced path {} outside of jail in {}",
                        self.path_str(),
                        input_path,
                        jail_path
                    );
                }
            }
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

    fn subtree_here<'a>(&'a self, tree: &'a Tree) -> Fallible<SubTree> {
        tree.subtree_at(self)
    }

    pub fn path_str(&self) -> String {
        self.0.borrow().path.to_string()
    }

    pub(super) fn handle_event(&self, value: Value, tree: &Tree) -> Fallible<()> {
        let path = &self.path_str();
        match self.0.borrow_mut().input {
            Some(NodeInput::Source(ref mut source, _)) => {
                source.handle_event(path, value, &self.subtree_here(tree)?)
            }
            _ => bail!(
                "runtime error: handle_event request on a non-source node @ {}",
                self.0.borrow().path
            ),
        }
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

    pub fn set_source(&self, from: &str, tree: &Tree) -> Fallible<()> {
        ensure!(
            self.0.borrow().input.is_none(),
            "parse error: input was set twice @ {}",
            self.0.borrow().path
        );
        ensure!(
            tree.source_handlers.contains_key(from),
            "parse error: unknown source kind '{}' referenced @ {}",
            from,
            self.0.borrow().path
        );
        self.0.borrow_mut().input = Some(NodeInput::Source(
            tree.source_handlers[from].to_owned(),
            Vec::new(),
        ));
        Ok(())
    }

    pub fn set_sink(&self, tgt: &str, tree: &Tree) -> Fallible<()> {
        ensure!(
            self.0.borrow().sink.is_none(),
            "parse error: sink set twice @ {}",
            self.0.borrow().path
        );
        ensure!(
            tree.sink_handlers.contains_key(tgt),
            "parse error: unknown sink kind '{}' referenced @ {}",
            tgt,
            self.0.borrow().path
        );
        self.0.borrow_mut().sink = Some((tgt.to_owned(), tree.sink_handlers[tgt].to_owned()));
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
        let path = self.path_str();
        trace!("computing @ {}", path);
        match self.0.borrow().input {
            None => bail!("runtime error: computing a non-input path @ {}", path),
            Some(NodeInput::Script(ref script)) => script.compute(tree),
            Some(NodeInput::Source(ref source, _)) => {
                // FIXME: we need to make this entire path mut
                // if let Some((_, cached)) = self.cache {
                //     return cached;
                // }
                let current = source.get_value(&path, &self.subtree_here(&tree)?);
                match current {
                    None => bail!("runtime error: no value @ {}", path),
                    Some(v) => Ok(v),
                }
            }
        }
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Fallible<Vec<Value>> {
        trace!("virtually computing @ {}", self.path_str());
        match self.0.borrow().input {
            None => bail!(
                "typeflow error: reading input from non-input path @ {}",
                self.path_str()
            ),
            Some(NodeInput::Script(ref script)) => script.virtually_compute_for_path(tree),
            Some(NodeInput::Source(_, _)) => Ok(vec![Value::input_flag()]),
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
        if let Some((ref kind, _)) = self.0.borrow().sink {
            return Ok(kind.to_owned());
        }
        bail!(
            "runtime: tried to get sink kind of the non-sink node at {}",
            self.path_str()
        )
    }
}

enum NodeInput {
    Source(SourceRef, Vec<NodeRef>),
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
    _cache: Option<Sample>,

    // Optional output data binding.
    sink: Option<(String, SinkRef)>,
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
            _cache: None,
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
    use crate::source::test::SimpleSource;

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
        let src1 = SimpleSource::new_ref()?;
        let tree = TreeBuilder::default()
            .add_source_handler("src1", &src1)?
            .build_from_str(s)?;
        let result = tree.lookup("/a/a")?.compute(&tree)?;
        assert_eq!(result, Value::new_str("d2"));
        Ok(())
    }

    #[test]
    fn test_tree_jail_source_break_rel() -> Fallible<()> {
        let s = r#"
a ^src1
    a <- ../b
b <- "foo"
"#;
        let src1 = SimpleSource::new_ref()?;
        let res = TreeBuilder::default()
            .add_source_handler("src1", &src1)?
            .build_from_str(s);
        assert!(res.is_err());
        Ok(())
    }

    #[test]
    fn test_tree_jail_source_break_abs() -> Fallible<()> {
        let s = r#"
a ^src1
    a <- /b
b <- "foo"
"#;
        let src1 = SimpleSource::new_ref()?;
        let res = TreeBuilder::default()
            .add_source_handler("src1", &src1)?
            .build_from_str(s);
        assert!(res.is_err());
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
        let srcref: SourceRef = SimpleSource::new_ref()?;
        let tree = TreeBuilder::default()
            .add_source_handler("src1", &srcref)?
            .build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::from_integer(1));

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
