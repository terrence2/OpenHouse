// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use actix::prelude::*;
use failure::Error;
use tree::physical::Dimension2;
use std::{fmt, cell::RefCell, collections::HashMap, rc::Rc};

pub struct Tree {
    root: NodeRef,
    sink_handlers: HashMap<String, Recipient<Syn, SinkEvent>>,
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            root: NodeRef::new(Node::new("(root)")),
            sink_handlers: HashMap::new(),
        }
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

    pub fn build_flow_graph(&mut self) -> Result<(), Error> {
        // For every, sink, for every comes-from: convert the comes-from into a goes-to with the real node(s) mapped by name.
        // This will let us walk quickly outward from a source to all possible sinks when we get an event on a source.
        let mut sinks = HashMap::new();
        self.root
            .find_matching("", &mut sinks, &|node: &NodeRef| node.sink().is_some())?;

        //let mut visited_sources = Vec::new();
        for (_, sink) in sinks.iter() {
            for comes_from in sink.comes_froms().iter() {
                for reference in comes_from.references.iter() {
                    let referencing_node = self.lookup(&reference)?;
                    referencing_node.add_goes_to(sink)?;
                }
            }
        }

        // Invert the map above so that we can go from any source to all sinks that might be affected.
        // When changes occur on a source, this will let us re-pull all sinks value

        // Get the set of all sources and compare that to the keys in our effects map; warn about any
        // sources that do not map to any sinks.
        let mut all_sources = HashMap::new();
        self.root
            .find_matching("", &mut all_sources, &|node: &NodeRef| {
                node.source().is_some()
            })?;

        return Ok(());
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

    pub fn lookup(&self, path: &str) -> Result<NodeRef, Error> {
        self.0.borrow().lookup(path)
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

    pub fn find_matching<MF>(
        &self,
        path: &str,
        matches: &mut HashMap<String, NodeRef>,
        match_func: &MF,
    ) -> Result<(), Error>
    where
        MF: Fn(&NodeRef) -> bool,
    {
        if match_func(&self) {
            matches.insert(path.to_owned(), self.clone());
        }
        for (name, child) in self.0.borrow().children.iter() {
            if name.starts_with(".") {
                continue;
            }
            let child_path = format!("{}/{}", path, name);
            child.find_matching(&child_path, matches, match_func)?;
        }
        return Ok(());
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

    pub fn source(&self) -> Option<String> {
        self.0.borrow().source.clone()
    }

    pub fn set_source(&self, from: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().source = Some(from.to_owned());
        return Ok(());
    }

    pub fn comes_froms(&self) -> Vec<ComesFrom> {
        self.0.borrow().comes_froms.clone()
    }

    pub fn add_comes_from(&self, from: &ComesFrom) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().comes_froms.push(from.to_owned());
        return Ok(());
    }

    pub fn add_goes_to(&self, to: &NodeRef) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            "source has already been set"
        );
        self.0.borrow_mut().goes_to.push(to.to_owned());
        return Ok(());
    }

    pub fn literal(&self) -> Option<String> {
        self.0.borrow().literal.clone()
    }

    pub fn set_literal(&self, content: &str) -> Result<(), Error> {
        ensure!(
            self.0.borrow().source.is_none(),
            format!("source has already been set: {:?}", self.source().clone())
        );
        self.0.borrow_mut().literal = Some(content.to_owned());
        return Ok(());
    }

    pub fn sink(&self) -> Option<String> {
        self.0.borrow().sink.clone()
    }

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
}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0.borrow())
    }
}

#[derive(Clone, Debug)]
pub struct ComesFrom {
    references: Vec<String>,
}

impl ComesFrom {
    pub fn new() -> Self {
        ComesFrom {
            references: Vec::new(),
        }
    }
}

pub struct Node {
    name: String,
    location: Option<Dimension2>,
    dimensions: Option<Dimension2>,

    goes_to: Vec<NodeRef>,
    comes_froms: Vec<ComesFrom>,
    literal: Option<String>,
    source: Option<String>,
    sink: Option<String>,

    children: HashMap<String, NodeRef>,
}

impl Node {
    pub fn new(name: &str) -> Self {
        Node {
            name: name.to_owned(),
            location: None,
            dimensions: None,
            goes_to: Vec::new(),
            comes_froms: Vec::new(),
            literal: None,
            source: None,
            sink: None,
            children: HashMap::new(),
        }
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

    fn add_child(&mut self, name: &str) -> Result<NodeRef, Error> {
        let child = NodeRef::new(Node::new(name));
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
    use simplelog::*;

    #[test]
    fn test_build_tree() {
        let tree = Tree::new();
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
