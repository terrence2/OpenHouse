// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::prelude::*;
use failure::Error;
use std::{fmt, cell::RefCell, collections::HashMap, path::Path, rc::Rc};
use tree::{parser::TreeParser, path::{ConcretePath, PathComponent, ScriptPath},
           physical::Dimension2, script::{Script, Value, ValueType}};

pub struct Tree {
    root: NodeRef,
    sink_handlers: HashMap<String, Recipient<Syn, SinkEvent>>,
}

impl Tree {
    pub fn new_empty() -> Self {
        Tree {
            root: NodeRef::new(Node::new("", "")),
            sink_handlers: HashMap::new(),
        }
    }

    pub fn from_file(path: &Path) -> Result<Tree, Error> {
        TreeParser::from_file(Self::new_empty(), path)
    }

    pub fn from_str(self, s: &str) -> Result<Tree, Error> {
        TreeParser::from_str(Self::new_empty(), s)
    }

    pub fn root(&self) -> NodeRef {
        self.root.clone()
    }

    pub fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        ensure!(
            path.starts_with('/'),
            "invalid path: tree lookups must start at /"
        );
        let relative: &str = &path[1..];
        if relative.is_empty() {
            return Ok(self.root.clone());
        }
        return self.root.lookup(relative);
    }

    pub fn lookup_path(&self, path: &ScriptPath) -> Result<NodeRef, Error> {
        self.root.lookup_path(&path.components[0..], self)
    }

    pub fn lookup_c_path(&self, path: &ConcretePath) -> Result<NodeRef, Error> {
        self.root.lookup_c_path(&path.components[0..])
    }

    // After the tree has been built, visit all nodes looking up references and
    // storing those references directly in the inputs list per script.
    pub fn link_and_validate_inputs(self) -> Result<Tree, Error> {
        self.root.link_and_validate_inputs(&self)?;
        return Ok(self);
    }

    pub fn invert_flow_graph(self) -> Result<Tree, Error> {
        Ok(self)
    }
}

impl fmt::Debug for Tree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.root)
    }
}

impl Actor for Tree {
    type Context = Context<Self>;
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

    pub fn path(&self) -> String {
        return self.0.borrow().path.clone();
    }

    pub fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        self.0.borrow().lookup(path)
    }

    pub fn lookup_path(&self, parts: &[PathComponent], tree: &Tree) -> Result<NodeRef, Error> {
        self.0.borrow().lookup_path(parts, tree)
    }

    pub fn lookup_c_path(&self, parts: &[String]) -> Result<NodeRef, Error> {
        if parts.is_empty() {
            return Ok(self.to_owned());
        }
        return self.0.borrow().lookup_c_path(parts);
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

    fn link_and_validate_inputs(&self, tree: &Tree) -> Result<(), Error> {
        trace!(
            "+++NodeRef::link_and_validate_input({})",
            self.0.borrow().path
        );

        // If nodetype is already set, we've either already recursed through
        // this node, or it has no scripts, so it doesn't matter.
        if self.0.borrow().nodetype.is_some() {
            return Ok(());
        }

        if self.0.borrow().script.is_some() {
            // Collect input map while borrowed read-only, so that we can find children.
            let data = if let Some(ref script) = self.0.borrow().script {
                script.build_input_map(tree)?
            } else {
                bail!("do not pass go")
            };
            //let data = self.0.borrow().script.unwrap().build_input_map(tree)?;

            // Re-borrow read-write to install the input map we built above.
            let nodetype = data.1;
            if let Some(ref mut script) = self.0.borrow_mut().script {
                script.install_input_map(data)?;
            }

            // And store it on the node
            // FIXME: we can just store this in the script.
            // let nodetype = self.0.borrow().script.unwrap().node_type()?;
            self.0.borrow_mut().nodetype = Some(nodetype);
        } else if self.0.borrow().source.is_some() {
            bail!("dont know how to treat sources yet")
        }

        // Recurse into our children.
        for (name, child) in self.0.borrow().children.iter() {
            if name == "." || name == ".." {
                continue;
            }
            child.link_and_validate_inputs(tree)?;
        }

        trace!(
            "---NodeRef::link_and_validate_input({})",
            self.0.borrow().path
        );
        return Ok(());
    }

    pub fn has_value(&self) -> bool {
        let node = self.0.borrow();
        return node.source.is_some() || node.script.is_some();
    }

    pub fn get_node_type(&self, tree: &Tree) -> Result<Option<ValueType>, Error> {
        // If we have already validated this node, return the type.
        if let Some(nodetype) = self.0.borrow().nodetype {
            return Ok(Some(nodetype));
        }

        // Not all nodes have a value.
        if !self.has_value() {
            return Ok(None);
        }

        // We need to recurse in order to typecheck the current node.
        self.link_and_validate_inputs(tree)?;

        // Note: unwrap and then rewrap so that we will panic if we don't have a type after link_and_validate.
        return Ok(Some(self.0.borrow().nodetype.unwrap()));
    }

    pub fn nodetype(&self) -> Result<ValueType, Error> {
        ensure!(
            self.has_value(),
            "typecheck error: nodetype request on a node with no value"
        );
        ensure!(
            self.0.borrow().nodetype != None,
            "typecheck error: nodetype request on a node that has not been validated"
        );
        return Ok(self.0.borrow().nodetype.unwrap());
    }

    pub fn location(&self) -> Option<Dimension2> {
        self.0.borrow().location
    }

    pub fn set_location(&self, loc: Dimension2) -> Result<(), Error> {
        ensure!(
            self.0.borrow().location.is_none(),
            "location has already been set"
        );
        self.0.borrow_mut().location = Some(loc);
        return Ok(());
    }

    // pub fn source(&self) -> Option<String> {
    //     self.0.borrow().source.clone()
    // }

    pub fn set_source(&self, from: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().source = Some(from.to_owned());
        return Ok(());
    }

    // pub fn sink(&self) -> Option<String> {
    //     self.0.borrow().sink.clone()
    // }

    pub fn set_sink(&self, tgt: &str) -> Result<(), Error> {
        ensure!(self.0.borrow().sink.is_none(), "sink has already been set");
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
        self.0.borrow_mut().script = Some(script);
        return Ok(());
    }

    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        self.0.borrow().compute(tree)
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Result<Vec<Value>, Error> {
        self.0.borrow().virtually_compute_for_path(tree)
    }
}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0.borrow())
    }
}

pub struct Node {
    // The tree structure.
    name: String,
    path: String,
    children: HashMap<String, NodeRef>,

    // Simple sigils.
    location: Option<Dimension2>,
    dimensions: Option<Dimension2>,

    // Data binding to external systems.
    source: Option<String>,
    sink: Option<String>,

    // The i/o transform function.
    script: Option<Script>,
    nodetype: Option<ValueType>,
    cache: (usize, Option<Value>),
}

impl Node {
    pub fn new(parent: &str, name: &str) -> Self {
        assert!(name.find('/').is_none());
        let path = if parent == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent, name)
        };
        return Node {
            name: name.to_owned(),
            path,
            children: HashMap::new(),
            location: None,
            dimensions: None,
            source: None,
            sink: None,
            script: None,
            nodetype: None,
            cache: (0, None),
        };
    }

    fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        ensure!(
            !path.starts_with('/'),
            "invalid path: node lookups must not start at /"
        );
        ensure!(!path.is_empty(), "invalid path: empty path component");
        let parts = path.splitn(2, '/').collect::<Vec<_>>();
        ensure!(parts.len() >= 1, "invalid path: did not find a component");
        ensure!(
            self.children.contains_key(parts[0]),
            format!("invalid path: did not find path component: {}", parts[0])
        );
        let child = self.children[parts[0]].clone();
        return match parts.len() {
            1 => Ok(child),
            2 => child.lookup(parts[1]),
            _ => unreachable!(),
        };
    }

    pub fn lookup_path(&self, parts: &[PathComponent], tree: &Tree) -> Result<NodeRef, Error> {
        trace!("Node::lookup_path @ {}, looking up {:?}", self.path, parts);
        let child_name = match &parts[0] {
            PathComponent::Name(n) => n.to_owned(),
            PathComponent::Lookup(p) => tree.lookup_path(p)?.compute(tree)?.as_path_component()?,
        };
        ensure!(
            self.children.contains_key(&child_name),
            format!("invalid path: did not find path component: {}", child_name)
        );
        let child = self.children.get(&child_name).unwrap();
        if parts.len() == 1 {
            return Ok(child.to_owned());
        }
        return Ok(child.lookup_path(&parts[1..], tree)?);
    }

    pub fn lookup_c_path(&self, parts: &[String]) -> Result<NodeRef, Error> {
        if let Some(child) = self.children.get(&parts[0]) {
            return child.lookup_c_path(&parts[1..]);
        }
        bail!(
            "runtime error: lookup on path that does not exist; at {} -> {:?}",
            self.path,
            parts
        );
    }

    fn add_child(&mut self, name: &str) -> Result<NodeRef, Error> {
        let child = NodeRef::new(Node::new(&self.path, name));
        self.children.insert(name.to_owned(), child.clone());
        return Ok(child);
    }

    fn level(&self) -> Result<usize, Error> {
        let mut cnt = 0;
        let mut cursor = self.children["."].clone();
        while cursor.0.borrow().children.contains_key("..") {
            let next_cursor = cursor.0.borrow().children[".."].clone();
            cursor = next_cursor;
            cnt += 1;
        }
        return Ok(cnt);
    }

    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        ensure!(
            self.script.is_some(),
            "runtime error: computing a non-script path @ {}",
            self.path
        );
        if let Some(ref script) = self.script {
            return Ok(script.compute(tree)?);
        }
        unreachable!()
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Result<Vec<Value>, Error> {
        ensure!(
            self.script.is_some(),
            "typeflow error: reading input from non-script path {}",
            self.path
        );
        if let Some(ref script) = self.script {
            return Ok(script.virtually_compute_for_path(tree)?);
        }
        unreachable!()
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut lvl = String::new();
        for _ in self.level() {
            lvl += "  ";
        }
        let mut out = String::new();
        if let Some(dim) = self.location {
            out += &format!("@{:?}\n", dim);
        }
        if let Some(dim) = self.dimensions {
            out += &format!("*{:?}\n", dim);
        }
        if let &Some(ref src) = &self.source {
            out += &format!("${:?}\n", src);
        }
        if let &Some(ref sink) = &self.sink {
            out += &format!("^{:?}\n", sink);
        }
        for name in self.children.keys() {
            if name.starts_with('.') {
                continue;
            }
            out += &format!("{}\n", name);
            let child_bits = format!("{:?}", self.children[name]);
            let child_bits = child_bits
                .split('\n')
                .map(|p| format!("{}{}", lvl, p))
                .collect::<Vec<String>>()
                .join("\n");
            out += &child_bits;
        }
        write!(f, "{}", out)
    }
}

/// Sent on state changes that contain Sinks with the subscribed name.
pub struct SinkEvent {
    affected_paths: Vec<String>,
}

impl Message for SinkEvent {
    type Result = Result<(), Error>;
}

pub struct AddSinkHandler {
    name: String,
    recipient: Recipient<Syn, SinkEvent>,
}

impl AddSinkHandler {
    pub fn new(name: &str, recipient: Recipient<Syn, SinkEvent>) -> Self {
        AddSinkHandler {
            name: name.to_owned(),
            recipient,
        }
    }
}

impl Message for AddSinkHandler {
    type Result = Result<(), Error>;
}

impl Handler<AddSinkHandler> for Tree {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: AddSinkHandler, _ctx: &mut Context<Self>) -> Self::Result {
        info!("adding handler for {}", msg.name);
        self.sink_handlers.insert(msg.name, msg.recipient);
        return Ok(());
    }
}

pub struct SourceEvent {
    affecting_path: String,
}

impl SourceEvent {
    pub fn new(path: &str) -> Self {
        SourceEvent {
            affecting_path: path.to_owned(),
        }
    }
}

impl Message for SourceEvent {
    type Result = Result<(), Error>;
}

impl Handler<SourceEvent> for Tree {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: SourceEvent, _ctx: &mut Context<Self>) -> Self::Result {
        return Ok(());
    }
}

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
            tree.lookup("/foopy/barmy")
                .unwrap()
                .lookup(".")
                .unwrap()
                .location()
                .unwrap()
        );
        assert_eq!(
            d10,
            tree.lookup("/foopy/barmy")
                .unwrap()
                .lookup("..")
                .unwrap()
                .location()
                .unwrap()
        );
    }

    struct TestSink {
        count: usize,
    }
    impl Actor for TestSink {
        type Context = Context<Self>;
    }
    impl Handler<SinkEvent> for TestSink {
        type Result = Result<(), Error>;

        fn handle(&mut self, msg: SinkEvent, _ctx: &mut Context<Self>) -> Self::Result {
            println!("TestSink: received change event {:?}", msg.affected_paths);
            // TODO: shutdown the system here
            return Ok(());
        }
    }

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
