// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use bif::{tostr::ToStr, NativeFunc};
use failure::Fallible;
use graph::Graph;
use parser::TreeParser;
use path::{ConcretePath, PathComponent, ScriptPath};
use physical::Dimension2;
use script::Script;
use sink::SinkRef;
use source::SourceRef;
use std::{
    cell::RefCell,
    collections::{hash_map::Entry, HashMap},
    fs,
    path::Path,
    sync::Arc,
};
use value::{Value, ValueType};

/// The combination of a Value plus a monotonic ordinal.
pub struct Sample {
    _value: Value,
    _at: usize,
}

pub struct TreeBuilder {
    source_handlers: HashMap<String, SourceRef>,
    sink_handlers: HashMap<String, SinkRef>,
    nifs: HashMap<String, Box<NativeFunc>>,
    add_builtins: bool,
}

impl TreeBuilder {
    pub fn new() -> Self {
        TreeBuilder {
            source_handlers: HashMap::new(),
            sink_handlers: HashMap::new(),
            nifs: HashMap::new(),
            add_builtins: true,
        }
    }

    pub fn add_source_handler(mut self, name: &str, source: &SourceRef) -> Fallible<TreeBuilder> {
        self.source_handlers.insert(name.to_owned(), source.clone());
        return Ok(self);
    }

    pub fn add_sink_handler(mut self, name: &str, sink: &SinkRef) -> Fallible<TreeBuilder> {
        self.sink_handlers.insert(name.to_owned(), sink.clone());
        return Ok(self);
    }

    pub fn add_native_function(
        mut self,
        name: &str,
        nif: Box<NativeFunc>,
    ) -> Fallible<TreeBuilder> {
        self.nifs.insert(name.to_owned(), nif);
        return Ok(self);
    }

    pub fn without_builtins(mut self) -> Fallible<TreeBuilder> {
        self.add_builtins = false;
        return Ok(self);
    }

    pub fn empty() -> Tree {
        Tree {
            root: NodeRef::new(Node::new(ConcretePath::new_root())),
            source_handlers: HashMap::new(),
            sink_handlers: HashMap::new(),
        }
    }

    pub fn build_from_file(self, path: &Path) -> Fallible<Tree> {
        let contents = fs::read_to_string(path)?;
        return self.build_from_str(&contents);
    }

    pub fn build_from_str(mut self, s: &str) -> Fallible<Tree> {
        if self.add_builtins {
            self.nifs.insert("str".to_owned(), Box::new(ToStr));
        }

        let tree = Tree {
            root: NodeRef::new(Node::new(ConcretePath::new_root())),
            source_handlers: self.source_handlers,
            sink_handlers: self.sink_handlers,
        };

        let tree = TreeParser::from_str(tree, s, &self.nifs)?
            .link_and_validate_inputs()?
            .map_inputs_to_outputs()?;

        for sink in tree.sink_handlers.values() {
            sink.on_ready(&tree.root.subtree_here(&tree)?)?;
        }
        return Ok(tree);
    }
}

pub struct Tree {
    root: NodeRef,
    source_handlers: HashMap<String, SourceRef>,
    sink_handlers: HashMap<String, SinkRef>,
}

impl Tree {
    pub fn handle_event(&self, path: &str, value: Value) -> Fallible<()> {
        let source = self.lookup(path)?;
        source.handle_event(value, self)?;
        let sink_nodes = source.get_sink_nodes_observing()?;

        let mut groups = HashMap::new();
        for node in sink_nodes.iter() {
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
        for (kind, values) in groups.iter() {
            self.sink_handlers[kind].values_updated(values)?;
        }
        return Ok(());
    }

    pub fn root(&self) -> NodeRef {
        self.root.clone()
    }

    pub fn lookup(&self, path: &str) -> Fallible<NodeRef> {
        let concrete = ConcretePath::from_str(path)?;
        return self.lookup_path(&concrete);
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
        return Ok(self);
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

// impl Actor for Tree {
//     type Context = Context<Self>;
// }

pub struct SubTree<'a> {
    _tree: &'a Tree,
    _root: NodeRef,
}

impl<'a> SubTree<'a> {
    fn new(tree: &'a Tree, root: &NodeRef) -> Fallible<Self> {
        return Ok(SubTree {
            _tree: tree,
            _root: root.to_owned(),
        });
    }

    pub fn lookup(&self, path: &str) -> Fallible<NodeRef> {
        let concrete = ConcretePath::from_str(path)?;
        return self._root.lookup_path(&concrete.components[0..]);
    }

    pub fn tree(&self) -> &'a Tree {
        return self._tree;
    }
}

#[derive(Clone)]
pub struct NodeRef(Arc<RefCell<Node>>);

impl NodeRef {
    pub fn new(node: Node) -> NodeRef {
        let self_ref = NodeRef(Arc::new(RefCell::new(node)));
        self_ref
            .0
            .borrow_mut()
            .children
            .insert(".".to_owned(), self_ref.clone());
        return self_ref;
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
        );
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
                return Ok(child.to_owned());
            }
            return Ok(child.lookup_dynamic_path(&parts[1..], tree)?);
        }
        bail!(format!(
            "invalid path: did not find path component '{}' @ {}",
            child_name,
            self.path_str()
        ));
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
        return Ok(child);
    }

    pub fn name(&self) -> String {
        self.0.borrow().name.clone()
    }

    pub(super) fn link_and_validate_inputs(&self, tree: &Tree) -> Fallible<()> {
        trace!("+++NodeRef::link_and_validate_input({})", self.path_str());

        // If nodetype is already set, we've already recursed through this node,
        // so can skip recursion as well. If we have no input, there is no easy
        // way to tell if we've already visited this node.
        if self.has_a_nodetype() {
            trace!("---NodeRef::link_and_validate_input({})", self.path_str());
            return Ok(());
        }

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
        for name in children.iter() {
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

        trace!(
            "---NodeRef::link_and_validate_input({})",
            self.0.borrow().path
        );
        return Ok(());
    }

    fn enforce_jail(&self) -> Fallible<()> {
        trace!("enforcing jail @ {}", self.path_str());
        for (name, child) in self.0.borrow().children.iter() {
            if name == "." || name == ".." {
                continue;
            }
            child.enforce_jail_under(&self.path_str())?;
        }
        return Ok(());
    }

    fn enforce_jail_under(&self, jail_path: &str) -> Fallible<()> {
        trace!("enforcing jail path {} @ {}", jail_path, self.path_str());
        if let Some(NodeInput::Script(ref script)) = self.0.borrow().input {
            for input_path in script.all_inputs()?.iter() {
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
        return Ok(());
    }

    fn find_all_sinks(&self, sinks: &mut Vec<NodeRef>) -> Fallible<()> {
        for (name, child) in self.0.borrow().children.iter() {
            if name == "." || name == ".." {
                continue;
            }
            child.find_all_sinks(sinks)?;
        }
        if let Some(_) = self.0.borrow().sink {
            sinks.push(self.to_owned());
        }
        return Ok(());
    }

    fn populate_flow_graph(&self, graph: &mut Graph) -> Fallible<()> {
        graph.add_node(self);
        for (name, child) in self.0.borrow().children.iter() {
            if name == "." || name == ".." {
                continue;
            }
            child.populate_flow_graph(graph)?;
        }

        if let Some(NodeInput::Script(ref script)) = self.0.borrow().input {
            script.populate_flow_graph(self, graph)?;
        }

        return Ok(());
    }

    fn flow_input_to_output(&self, sinks: &Vec<NodeRef>, graph: &Graph) -> Fallible<()> {
        for (name, child) in self.0.borrow().children.iter() {
            if name == "." || name == ".." {
                continue;
            }
            child.flow_input_to_output(sinks, graph)?;
        }

        let mut connected_sinks = None;
        if let Some(NodeInput::Source(_, _)) = self.0.borrow().input {
            let connected = graph.connected_nodes(self, sinks)?;
            if connected.is_empty() {
                warn!(
                    "dataflow warning: source at {} is not connected to any sinks",
                    self.path_str()
                );
            }
            connected_sinks = Some(connected);
        };

        if connected_sinks.is_some() {
            if let Some(NodeInput::Source(_, ref mut sinks)) = self.0.borrow_mut().input {
                assert!(
                    sinks.is_empty(),
                    "dataflow error: found connected sinks at {}, but sinks already set"
                );
                sinks.append(&mut connected_sinks.unwrap());
            } else {
                panic!("expected source to not mutate")
            }
        }

        return Ok(());
    }

    fn has_input(&self) -> bool {
        self.0.borrow().input.is_some()
    }

    fn has_script(&self) -> bool {
        if let Some(NodeInput::Script(_)) = self.0.borrow().input {
            return true;
        }
        return false;
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

    // This will be false if !has_input or we are in the middle of compilation.
    // This should not be used after compilation, as the combination of
    // has_input and nodetype should be sufficient.
    fn has_a_nodetype(&self) -> bool {
        if let Some(ref input) = self.0.borrow().input {
            return input.has_a_nodetype();
        }
        return false;
    }

    // If node type has been set, return it, otherwise do link_and_validate in
    // order to find it. This method is only safe to call during compilation. It
    // is public for use by Script during compilation.
    pub(super) fn get_or_find_node_type(&self, tree: &Tree) -> Fallible<ValueType> {
        ensure!(
            self.has_input(),
            "typeflow error: read from the node @ {} has no inputs",
            self.path_str()
        );

        // If we have already validated this node, return the type.
        if self.has_a_nodetype() {
            return self.nodetype(tree);
        }

        // We need to recurse in order to typecheck the current node.
        self.link_and_validate_inputs(tree)?;
        return self.nodetype(tree);
    }

    pub fn nodetype(&self, tree: &Tree) -> Fallible<ValueType> {
        match self.0.borrow().input {
            None => bail!(
                "runtime error: nodetype request on a non-input node @ {}",
                self.0.borrow().path
            ),
            Some(NodeInput::Script(ref script)) => {
                return script.nodetype();
            }
            Some(NodeInput::Source(ref source, _)) => {
                return source.nodetype(&self.path_str(), &self.subtree_here(tree)?);
            }
        }
    }

    pub(super) fn handle_event(&self, value: Value, tree: &Tree) -> Fallible<()> {
        let path = &self.path_str();
        match self.0.borrow_mut().input {
            Some(NodeInput::Source(ref mut source, _)) => {
                return source.handle_event(path, value, &self.subtree_here(tree)?);
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
        return Ok(());
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
        return Ok(());
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
        return Ok(());
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
        return Ok(());
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
        return Ok(());
    }

    pub fn set_script(&self, script: Script) -> Fallible<()> {
        ensure!(
            self.0.borrow().input.is_none(),
            "parse error: input was set twice at {}",
            self.0.borrow().path
        );
        self.0.borrow_mut().input = Some(NodeInput::Script(script));
        return Ok(());
    }

    pub fn compute(&self, tree: &Tree) -> Fallible<Value> {
        let path = self.path_str();
        trace!("computing @ {}", path);
        match self.0.borrow_mut().input {
            None => {
                bail!("runtime error: computing a non-input path @ {}", path);
            }
            Some(NodeInput::Script(ref script)) => {
                return script.compute(tree);
            }
            Some(NodeInput::Source(ref source, _)) => {
                // FIXME: we need to make this entire path mut
                // if let Some((_, cached)) = self.cache {
                //     return cached;
                // }
                let current = source.get_value(&path, &self.subtree_here(&tree)?);
                return match current {
                    None => bail!("runtime error: no value @ {}", path),
                    Some(v) => Ok(v),
                };
            }
        }
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Fallible<Vec<Value>> {
        trace!("virtually computing @ {}", self.path_str());
        match self.0.borrow().input {
            None => {
                bail!(
                    "typeflow error: reading input from non-input path @ {}",
                    self.path_str()
                );
            }
            Some(NodeInput::Script(ref script)) => {
                return script.virtually_compute_for_path(tree);
            }
            Some(NodeInput::Source(ref source, _)) => {
                return source.get_all_possible_values(&self.path_str(), &self.subtree_here(&tree)?);
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
        );
    }

    pub fn sink_kind(&self) -> Fallible<String> {
        if let Some((ref kind, _)) = self.0.borrow().sink {
            return Ok(kind.to_owned());
        }
        bail!(
            "runtime: tried to get sink kind of the non-sink node at {}",
            self.path_str()
        );
    }
}

enum NodeInput {
    Source(SourceRef, Vec<NodeRef>),
    Script(Script),
}

impl NodeInput {
    fn has_a_nodetype(&self) -> bool {
        match self {
            NodeInput::Source(_, _) => false,
            NodeInput::Script(s) => s.has_a_nodetype(),
        }
    }
}

pub struct Node {
    // The tree structure.
    name: String,
    path: ConcretePath,
    children: HashMap<String, NodeRef>,

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
        return Node {
            name: path.basename().to_owned(),
            path,
            children: HashMap::new(),
            location: None,
            dimensions: None,
            input: None,
            _cache: None,
            sink: None,
        };
    }

    fn create_new_child(&mut self, name: &str) -> Fallible<NodeRef> {
        let child = NodeRef::new(Node::new(self.path.new_child(name)));
        self.children.insert(name.to_owned(), child.clone());
        return Ok(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::test::SimpleSource;

    #[test]
    fn test_build_tree() -> Fallible<()> {
        let tree = TreeBuilder::empty();
        assert_eq!(None, tree.root().location());

        let d10 = Dimension2::from_str("10x10")?;
        let d20 = Dimension2::from_str("20x20")?;

        let child = tree.lookup("/")?.add_child("foopy")?;
        child.set_location(d10.clone())?;
        assert_eq!(d10, child.location().unwrap());

        let child = tree.lookup("/foopy")?.add_child("barmy")?;
        child.set_location(d20.clone())?;
        child.set_dimensions(d20.clone())?;
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
        let src1 = SimpleSource::new(vec![])?;
        let tree = TreeBuilder::new()
            .add_source_handler("src1", &src1)?
            .build_from_str(s)?;
        let result = tree.lookup("/a/a")?.compute(&tree)?;
        assert_eq!(result, Value::String("d2".to_owned()));
        Ok(())
    }

    #[test]
    fn test_tree_jail_source_break_rel() -> Fallible<()> {
        let s = r#"
a ^src1
    a <- ../b
b <- "foo"
"#;
        let src1 = SimpleSource::new(vec![])?;
        let res = TreeBuilder::new()
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
        let src1 = SimpleSource::new(vec![])?;
        let res = TreeBuilder::new()
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
        let srcref: SourceRef = SimpleSource::new(vec!["foo".into(), "bar".into()])?;
        let tree = TreeBuilder::new()
            .add_source_handler("src1", &srcref)?
            .build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::Integer(1));

        tree.handle_event("/a", Value::String("foo".to_owned()))?;

        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::Integer(1));
        Ok(())
    }
}
