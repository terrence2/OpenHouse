// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use downcast_rs::Downcast;
use failure::Error;
use parser::TreeParser;
use path::{ConcretePath, PathComponent, ScriptPath};
use physical::Dimension2;
use script::{Script, Value, ValueType};
use std::{fs, cell::{RefCell, RefMut}, collections::HashMap, path::Path, rc::Rc};

/// The combination of a Value plus a monotonic ordinal.
pub struct Sample {
    value: Value,
    at: usize,
}

pub trait TreeSource: Downcast {
    /// Note the following path listed as a source using this handler.
    fn add_path(&mut self, path: &str, tree: &SubTree) -> Result<(), Error>;

    /// Return the type of the given path.
    fn nodetype(&self, path: &str, tree: &SubTree) -> Result<ValueType, Error>;

    /// Return all possible values that the given source can take. This is only
    /// called for sources that are used as a path component elsewhere. In the
    /// event this is called for a source that does not have a constrained set of
    /// possible values -- floats, arbitrary strings, etc -- return an error.
    fn get_all_possible_values(&self, path: &str, tree: &SubTree) -> Result<Vec<Value>, Error>;

    /// Return the current value of the given source. Sources are generally
    /// expected to be delivered asyncronously and the latest value will be
    /// cached indefinitely, This is only called when the value is used as a path
    /// component before a change event has occurred.
    fn get_value(&self, path: &str, tree: &SubTree) -> Option<Value>;
}
impl_downcast!(TreeSource);

#[derive(Clone)]
pub struct SourceRef(Rc<RefCell<Box<TreeSource>>>);

impl SourceRef {
    pub fn new(source: Box<TreeSource>) -> Self {
        SourceRef(Rc::new(RefCell::new(source)))
    }

    pub fn mutate_as<T>(&self, f: &mut FnMut(&mut T)) -> Result<(), Error>
    where
        T: TreeSource,
    {
        RefMut::map(self.0.borrow_mut(), |ts| {
            if let Some(real) = ts.downcast_mut::<T>() {
                f(real);
            }
            ts
        });
        return Ok(());
    }
}

pub struct Tree {
    root: NodeRef,
    source_handlers: HashMap<String, SourceRef>,
    //source_handlers_base: HashMap<String, Box<FnOnce() -> ()>>,
    //sink_handlers: HashMap<String, Recipient<Syn, SinkEvent>>,
}

// pub trait EventHandler {
//     fn handle_event(&self, path: &str, value: &Value) -> Result<(), Error>;
// }

// impl EventHandler for Tree {
//     fn handle_event(&self, path: &str, value: &Value) -> Result<(), Error> {
//         debug!("handle_event called on path {} with value {}", path, value);
//         return Ok(());
//     }
// }

impl Tree {
    pub fn new_empty() -> Self {
        Tree {
            root: NodeRef::new(Node::new(ConcretePath::new_root())),
            source_handlers: HashMap::new(),
            //sink_handlers: HashMap::new(),
        }
    }

    pub fn add_source_handler(mut self, name: &str, source: &SourceRef) -> Result<Tree, Error> {
        self.source_handlers.insert(name.to_owned(), source.clone());
        return Ok(self);
    }

    pub fn build_from_file(self, path: &Path) -> Result<Tree, Error> {
        let contents = fs::read_to_string(path)?;
        return self.build_from_str(&contents);
    }

    pub fn build_from_str(self, s: &str) -> Result<Tree, Error> {
        return TreeParser::from_str(self, s)?
            .link_and_validate_inputs()?
            .invert_flow_graph();
    }

    pub fn root(&self) -> NodeRef {
        self.root.clone()
    }

    pub fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        let concrete = ConcretePath::from_str(path)?;
        return self.lookup_path(&concrete);
    }

    pub fn lookup_path(&self, path: &ConcretePath) -> Result<NodeRef, Error> {
        self.root.lookup_path(&path.components[0..])
    }

    pub fn lookup_dynamic_path(&self, path: &ScriptPath) -> Result<NodeRef, Error> {
        self.root.lookup_dynamic_path(&path.components[0..], self)
    }

    // After the tree has been built, visit all nodes looking up references and
    // storing those references directly in the inputs list per script.
    fn link_and_validate_inputs(self) -> Result<Tree, Error> {
        self.root.link_and_validate_inputs(&self)?;
        return Ok(self);
    }

    fn invert_flow_graph(self) -> Result<Tree, Error> {
        Ok(self)
    }

    fn subtree_at(&self, root: NodeRef) -> Result<SubTree, Error> {
        SubTree::new(self, root)
    }
}

// impl Actor for Tree {
//     type Context = Context<Self>;
// }

pub struct SubTree<'a> {
    tree: &'a Tree,
    root: NodeRef,
}

impl<'a> SubTree<'a> {
    fn new(tree: &'a Tree, root: NodeRef) -> Result<Self, Error> {
        //root.enforce_jail()?;
        return Ok(SubTree { tree, root });
    }
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

    pub fn path(&self) -> ConcretePath {
        return self.0.borrow().path.clone();
    }

    pub fn lookup_path(&self, parts: &[String]) -> Result<NodeRef, Error> {
        if parts.is_empty() {
            return Ok(self.to_owned());
        }
        return self.0.borrow().lookup_path(parts);
    }

    pub fn lookup_dynamic_path(
        &self,
        parts: &[PathComponent],
        tree: &Tree,
    ) -> Result<NodeRef, Error> {
        self.0.borrow().lookup_dynamic_path(parts, tree)
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

    pub fn name(&self) -> String {
        self.0.borrow().name.clone()
    }

    pub(super) fn link_and_validate_inputs(&self, tree: &Tree) -> Result<(), Error> {
        trace!(
            "+++NodeRef::link_and_validate_input({})",
            self.0.borrow().path
        );

        // If nodetype is already set, we've already recursed through this node,
        // so can skip recursion as well. If we have no input, there is no easy
        // way to tell if we've already visited this node.
        if self.0.borrow().has_a_nodetype() {
            trace!(
                "---NodeRef::link_and_validate_input({})",
                self.0.borrow().path
            );
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
        let mut children: Vec<String> = self.0
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
        if let Some(NodeInput::Source(ref source)) = self.0.borrow().input {
            self.enforce_jail()?;
            source
                .0
                .borrow_mut()
                .add_path(&self.path().to_string(), &tree.subtree_at(self.to_owned())?)?;
        }

        trace!(
            "---NodeRef::link_and_validate_input({})",
            self.0.borrow().path
        );
        return Ok(());
    }

    fn enforce_jail(&self) -> Result<(), Error> {
        trace!("enforcing jail under {}", self.path());
        for (name, child) in self.0.borrow().children.iter() {
            if name == "." || name == ".." {
                continue;
            }
            child.enforce_jail_under(&self.0.borrow().path.to_string())?;
        }
        return Ok(());
    }

    fn enforce_jail_under(&self, jail_path: &str) -> Result<(), Error> {
        trace!("enforcing jail path {} at path {}", jail_path, self.path());
        if let Some(NodeInput::Script(ref script)) = self.0.borrow().input {
            for input_path in script.all_inputs()?.iter() {
                trace!("checking input: {}", input_path);
                if !input_path.starts_with(jail_path) {
                    bail!(
                        "jailbreak error @ {}: referenced path {} outside of jail in {}",
                        self.path(),
                        input_path,
                        jail_path
                    );
                }
            }
        }
        return Ok(());
    }

    pub fn has_input(&self) -> bool {
        self.0.borrow().has_input()
    }

    pub fn has_script(&self) -> bool {
        self.0.borrow().has_script()
    }

    // If node type has been set, return it, otherwise do link_and_validate in
    // order to find it. This method is only safe to call during compilation. It
    // is public for use by Script during compilation.
    pub(super) fn get_or_find_node_type(&self, tree: &Tree) -> Result<ValueType, Error> {
        ensure!(
            self.has_input(),
            "typeflow error: read from the node @ {} has no inputs",
            self.path()
        );

        // If we have already validated this node, return the type.
        if self.0.borrow().has_a_nodetype() {
            return self.nodetype(tree);
        }

        // We need to recurse in order to typecheck the current node.
        self.link_and_validate_inputs(tree)?;
        return self.nodetype(tree);
    }

    pub fn nodetype(&self, tree: &Tree) -> Result<ValueType, Error> {
        return self.0.borrow().nodetype(tree);
    }

    pub fn location(&self) -> Option<Dimension2> {
        self.0.borrow().location
    }

    pub fn set_location(&self, loc: Dimension2) -> Result<(), Error> {
        ensure!(
            self.0.borrow().location.is_none(),
            "location has already been set"
        );
        debug!("set_location about to borrow_mut: {}", self.0.borrow().path);
        self.0.borrow_mut().location = Some(loc);
        return Ok(());
    }

    pub fn set_source(&self, from: &str, tree: &Tree) -> Result<(), Error> {
        ensure!(
            self.0.borrow().input.is_none(),
            "parse error: input was set twice @ {}",
            self.0.borrow().path
        );
        ensure!(
            tree.source_handlers.contains_key(from),
            "parse error: unknown source kind '{}'",
            from
        );
        self.0.borrow_mut().input = Some(NodeInput::Source(tree.source_handlers[from].to_owned()));
        return Ok(());
    }

    pub fn set_sink(&self, tgt: &str, tree: &Tree) -> Result<(), Error> {
        ensure!(
            self.0.borrow().sink.is_none(),
            "parse error: sink set twice @ {}",
            self.0.borrow().path
        );
        self.0.borrow_mut().sink = Some(tgt.to_owned());
        return Ok(());
    }

    pub fn apply_template(&self, template: &NodeRef) -> Result<(), Error> {
        if let Some(dim) = template.location() {
            self.set_location(dim)?;
        }
        return Ok(());
    }

    pub fn set_script(&self, script: Script) -> Result<(), Error> {
        ensure!(
            self.0.borrow().input.is_none(),
            "parse error: input was set twice at {}",
            self.0.borrow().path
        );
        self.0.borrow_mut().input = Some(NodeInput::Script(script));
        return Ok(());
    }

    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        trace!("computing @ {}", self.path());
        self.0.borrow().compute(tree)
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Result<Vec<Value>, Error> {
        trace!("virtually computing @ {}", self.path());
        self.0.borrow().virtually_compute_for_path(tree)
    }
}

enum NodeInput {
    Source(SourceRef),
    Script(Script),
}

impl NodeInput {
    fn has_a_nodetype(&self) -> bool {
        match self {
            NodeInput::Source(_) => false,
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
    cache: Option<Sample>,

    // Optional output data binding.
    sink: Option<String>,
    // The i/o transform function.
    // source: Option<String>,
    // script: Option<Script>,
    // nodetype: Option<ValueType>,
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
            cache: None,
            sink: None,
            //source: None,
            //script: None,
            //nodetype: None,
            //cache: (0, None),
        };
    }

    pub fn lookup_path(&self, parts: &[String]) -> Result<NodeRef, Error> {
        if let Some(child) = self.children.get(&parts[0]) {
            return child.lookup_path(&parts[1..]);
        }
        bail!(
            "runtime error: lookup on path that does not exist; at {} -> {:?}",
            self.path,
            parts
        );
    }

    pub fn lookup_dynamic_path(
        &self,
        parts: &[PathComponent],
        tree: &Tree,
    ) -> Result<NodeRef, Error> {
        trace!(
            "Node::lookup_dynamic_path @ {}, looking up {:?}",
            self.path,
            parts
        );
        let child_name = match &parts[0] {
            PathComponent::Name(n) => n.to_owned(),
            PathComponent::Lookup(p) => tree.lookup_dynamic_path(p)?
                .compute(tree)?
                .as_path_component()?,
        };
        ensure!(
            self.children.contains_key(&child_name),
            format!("invalid path: did not find path component: {}", child_name)
        );
        let child = self.children.get(&child_name).unwrap();
        if parts.len() == 1 {
            return Ok(child.to_owned());
        }
        return Ok(child.lookup_dynamic_path(&parts[1..], tree)?);
    }

    fn add_child(&mut self, name: &str) -> Result<NodeRef, Error> {
        let child = NodeRef::new(Node::new(self.path.new_child(name)));
        self.children.insert(name.to_owned(), child.clone());
        return Ok(child);
    }

    pub fn has_input(&self) -> bool {
        self.input.is_some()
    }

    pub fn has_script(&self) -> bool {
        if let Some(NodeInput::Script(_)) = self.input {
            return true;
        }
        return false;
    }

    // This will be false if !has_input or we are in the middle of compilation.
    // This should not be used after compilation, as the combination of
    // has_input and nodetype should be sufficient.
    fn has_a_nodetype(&self) -> bool {
        if let Some(ref input) = self.input {
            return input.has_a_nodetype();
        }
        return false;
    }

    fn nodetype(&self, tree: &Tree) -> Result<ValueType, Error> {
        match self.input {
            None => bail!(
                "runtime error: nodetype request on a non-input node @ {}",
                self.path
            ),
            Some(NodeInput::Script(ref script)) => {
                return script.nodetype();
            }
            Some(NodeInput::Source(ref source)) => {
                return source.0.borrow().nodetype(
                    &self.path.to_string(),
                    &tree.subtree_at(self.children["."].to_owned())?,
                );
            }
        }
    }

    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        match self.input {
            None => {
                bail!("runtime error: computing a non-input path @ {}", self.path);
            }
            Some(NodeInput::Script(ref script)) => {
                return script.compute(tree);
            }
            Some(NodeInput::Source(ref source)) => {
                // FIXME: we need to make this entire path mut
                // if let Some((_, cached)) = self.cache {
                //     return cached;
                // }
                let current = source.0.borrow().get_value(
                    &self.path.to_string(),
                    &tree.subtree_at(self.children["."].to_owned())?,
                );
                return match current {
                    None => bail!("runtime error: no value @ {}", self.path),
                    Some(v) => Ok(v),
                };
            }
        }
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Result<Vec<Value>, Error> {
        match self.input {
            None => {
                bail!(
                    "typeflow error: reading input from non-script path {}",
                    self.path
                );
            }
            Some(NodeInput::Script(ref script)) => {
                return script.virtually_compute_for_path(tree);
            }
            Some(NodeInput::Source(ref source)) => {
                return source.0.borrow().get_all_possible_values(
                    &self.path.to_string(),
                    &tree.subtree_at(self.children["."].to_owned())?,
                );
            }
        }
    }
}

/// Sent on state changes that contain Sinks with the subscribed name.
// pub struct SinkEvent {
//     affected_paths: Vec<String>,
// }

// impl Message for SinkEvent {
//     type Result = Result<(), Error>;
// }

// pub struct AddSinkHandler {
//     name: String,
//     recipient: Recipient<Syn, SinkEvent>,
// }

// impl AddSinkHandler {
//     pub fn new(name: &str, recipient: Recipient<Syn, SinkEvent>) -> Self {
//         AddSinkHandler {
//             name: name.to_owned(),
//             recipient,
//         }
//     }
// }

// impl Message for AddSinkHandler {
//     type Result = Result<(), Error>;
// }

// impl Handler<AddSinkHandler> for Tree {
//     type Result = Result<(), Error>;

//     fn handle(&mut self, msg: AddSinkHandler, _ctx: &mut Context<Self>) -> Self::Result {
//         info!("adding handler for {}", msg.name);
//         self.sink_handlers.insert(msg.name, msg.recipient);
//         return Ok(());
//     }
// }

// pub struct SourceEvent {
//     affecting_path: String,
// }

// impl SourceEvent {
//     pub fn new(path: &str) -> Self {
//         SourceEvent {
//             affecting_path: path.to_owned(),
//         }
//     }
// }

// impl Message for SourceEvent {
//     type Result = Result<(), Error>;
// }

// impl Handler<SourceEvent> for Tree {
//     type Result = Result<(), Error>;

//     fn handle(&mut self, msg: SourceEvent, _ctx: &mut Context<Self>) -> Self::Result {
//         return Ok(());
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tree() {
        let tree = Tree::new_empty();
        assert_eq!(None, tree.root().location());

        let d10 = Dimension2::from_str("@10x10").unwrap();
        let d20 = Dimension2::from_str("@20x20").unwrap();

        let child = tree.lookup("/").unwrap().add_child("foopy").unwrap();
        child.set_location(d10.clone()).unwrap();
        assert_eq!(d10, child.location().unwrap());

        let child = tree.lookup("/foopy").unwrap().add_child("barmy").unwrap();
        child.set_location(d20.clone()).unwrap();
        assert_eq!(d20, child.location().unwrap());

        assert_eq!(d10, tree.lookup("/foopy").unwrap().location().unwrap());
        assert_eq!(
            d20,
            tree.lookup("/foopy/barmy").unwrap().location().unwrap()
        );
        assert_eq!(
            d20,
            tree.lookup("/foopy/barmy").unwrap().location().unwrap()
        );
        assert_eq!(d10, tree.lookup("/foopy").unwrap().location().unwrap());
    }

    struct TestSource1 {
        values: Vec<String>,
        input: usize,
    }

    impl TestSource1 {
        fn new_box(values: Vec<String>) -> Result<Box<TreeSource>, Error> {
            let src = Box::new(Self { values, input: 0 });
            return Ok(src);
        }

        fn new(values: Vec<String>) -> Result<SourceRef, Error> {
            let src = Box::new(Self { values, input: 0 });
            return Ok(SourceRef::new(src));
        }

        fn set_input(&mut self, input: usize) {
            self.input = input;
        }
    }

    impl TreeSource for TestSource1 {
        fn get_all_possible_values(
            &self,
            _path: &str,
            _tree: &SubTree,
        ) -> Result<Vec<Value>, Error> {
            Ok(self.values
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect::<Vec<Value>>())
        }

        fn add_path(&mut self, _path: &str, _tree: &SubTree) -> Result<(), Error> {
            return Ok(());
        }

        fn get_value(&self, _path: &str, _tree: &SubTree) -> Option<Value> {
            return Some(Value::String(self.values[self.input].clone()));
        }

        fn nodetype(&self, _path: &str, _tree: &SubTree) -> Result<ValueType, Error> {
            Ok(ValueType::STRING)
        }
    }

    #[test]
    fn test_tree_jail_source_ok() {
        let s = r#"
a ^src1
    a <- ./{/a/b} + "2"
    b <- ./{./c} + "1"
    c <- "c"
    c1 <- "d"
"#;
        let src1 = TestSource1::new(vec![]).unwrap();
        let tree = Tree::new_empty()
            .add_source_handler("src1", &src1)
            .unwrap()
            .build_from_str(s)
            .unwrap();
        let result = tree.lookup("/a/a").unwrap().compute(&tree).unwrap();
        assert_eq!(result, Value::String("d2".to_owned()));
    }

    #[test]
    #[should_panic]
    fn test_tree_jail_source_break_rel() {
        let s = r#"
a ^src1
    a <- ../b
b <- "foo"
"#;
        let src1 = TestSource1::new(vec![]).unwrap();
        let tree = Tree::new_empty()
            .add_source_handler("src1", &src1)
            .unwrap()
            .build_from_str(s)
            .unwrap();
        tree.lookup("/a/a").unwrap().compute(&tree).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_tree_jail_source_break_abs() {
        let s = r#"
a ^src1
    a <- /b
b <- "foo"
"#;
        let src1 = TestSource1::new(vec![]).unwrap();
        let tree = Tree::new_empty()
            .add_source_handler("src1", &src1)
            .unwrap()
            .build_from_str(s)
            .unwrap();
        let result = tree.lookup("/a/a").unwrap().compute(&tree).unwrap();
        assert_eq!(result, Value::String("foo".to_owned()));
    }

    #[test]
    fn test_tree_sources() {
        let s = r#"
a ^src1
b <-/{/a}/v
foo
    v<-1
bar
    v<-2
"#;
        let srcref: SourceRef = TestSource1::new(vec!["foo".to_owned(), "bar".to_owned()]).unwrap();
        let tree = Tree::new_empty()
            .add_source_handler("src1", &srcref)
            .unwrap()
            .build_from_str(s)
            .unwrap();
        assert_eq!(
            tree.lookup("/b").unwrap().compute(&tree).unwrap(),
            Value::Integer(1)
        );

        {
            srcref.mutate_as::<TestSource1>(&mut |src1: &mut TestSource1| src1.set_input(1));
        }

        assert_eq!(
            tree.lookup("/b").unwrap().compute(&tree).unwrap(),
            Value::Integer(2)
        );
    }

    // struct TestSink {
    //     count: usize,
    // }
    // impl Actor for TestSink {
    //     type Context = Context<Self>;
    // }
    // impl Handler<SinkEvent> for TestSink {
    //     type Result = Result<(), Error>;

    //     fn handle(&mut self, msg: SinkEvent, _ctx: &mut Context<Self>) -> Self::Result {
    //         println!("TestSink: received change event {:?}", msg.affected_paths);
    //         // TODO: shutdown the system here
    //         return Ok(());
    //     }
    // }

    // #[test]
    // fn test_run_tree() {
    //     TermLogger::init(LevelFilter::Trace, Config::default()).unwrap();
    //     let sys = System::new("test");

    //     let mut tree = Tree::new();
    //     let a = tree.root().add_child("a").unwrap();
    //     a.set_sink("foo").unwrap();
    //     //a.add_comes_from("/b").unwrap();
    //     let b = tree.root().add_child("b").unwrap();
    //     b.set_source("bar").unwrap();
    //     tree.build_flow_graph();
    //     let tree_addr: Addr<Syn, _> = tree.start();

    //     let sink = TestSink { count: 0 };
    //     let sink_addr: Addr<Syn, _> = sink.start();
    //     let result = tree_addr.send(AddSinkHandler::new("foo", sink_addr.recipient()));

    //     tree_addr.send(SourceEvent::new("/b"));

    //     // Arbiter::handle().spawn(
    //     //     result
    //     //         .map(|res| match res {
    //     //             Ok(result) => println!("Got result: {}", result),
    //     //             Err(err) => println!("Got error: {}", err),
    //     //         })
    //     //         .map_err(|e| {
    //     //             println!("Actor is probably died: {}", e);
    //     //         }),
    //     // );

    //     sys.run();
    // }
}
