// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    bif::NativeFunc,
    script::Script,
    tokenizer::{Token, TreeTokenizer},
    tree::{NodeRef, Tree},
};
use failure::{bail, ensure, format_err, Fallible};
use std::collections::HashMap;
use tracing::trace;

pub struct TreeParser<'a> {
    nifs: &'a HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
    import_interceptors: &'a HashMap<String, Tree>,
    templates: HashMap<String, NodeRef>,
    tokens: Vec<Token>,
    position: usize,
}

impl<'a> TreeParser<'a> {
    // Parsing strategy:
    //   (1) Tokenize:
    //          - keeps indent state and emits indent and dedent events with tokens
    //          - LL(1) or thereabouts
    //   (2) Parse:
    //          - manual recursive descent
    //          - LL(1) exactly
    //          - Separate sub-parsers for scripts, paths, and expressions
    //   (3) Link inputs
    //          - Walk tree and look up all paths attaching the referenced Node as inputs to scripts.
    //   (4) Type checking
    //          - Find all scripts and walk them from values up, asserting that all types match up.
    //   (5) Data flow
    //          - Invert the comes-from in order to build a goes-to set for each node.
    //
    pub fn from_str(
        tree: Tree,
        s: &str,
        nifs: &HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
        import_interceptors: &HashMap<String, Tree>,
    ) -> Fallible<Tree> {
        let sanitized = s.replace('\t', "    ");

        {
            let tokens = TreeTokenizer::tokenize(&sanitized)?;
            let mut parser = TreeParser {
                nifs,
                import_interceptors,
                templates: HashMap::new(),
                tokens,
                position: 0,
            };
            parser.consume_root(&tree.root())?;
        }

        Ok(tree)
    }

    fn consume_root(&mut self, root: &NodeRef) -> Fallible<()> {
        while !self.out_of_input() {
            match self.peek()? {
                Token::NameTerm(_n) => {
                    self.consume_tree(root)?;
                }
                Token::ImportTerm(filename) => {
                    self.do_import(&filename, root)?;
                    self.pop()?;
                    ensure!(
                        self.pop()? == Token::Newline,
                        "parse error: import must be the last thing in the line"
                    );
                }
                _ => bail!(
                    "parse error: expected name at top level, not: {:?}",
                    self.peek()?
                ),
            }
        }
        Ok(())
    }

    fn consume_tree(&mut self, parent: &NodeRef) -> Fallible<()> {
        let name = self.consume_node_name()?;
        trace!(
            "Consuming tree at: {} under parent: {}",
            name,
            parent.name()
        );
        let child = parent.add_child(&name)?;
        self.consume_inline_suite(&child)?;
        if self.out_of_input() || self.peek()? != Token::Indent {
            trace!("finished tree {}", name);
            return Ok(());
        }

        // Next token is indent, so parse any body and any children.
        self.pop()?;
        self.consume_block_suite(&child)?;
        while !self.out_of_input() {
            match self.peek()? {
                Token::NameTerm(ref _s) => self.consume_tree(&child)?,
                Token::BooleanTerm(ref _b) => self.consume_tree(&child)?,
                Token::Dedent => {
                    self.pop()?;
                    return Ok(());
                }
                _ => bail!(
                    "parse error: unexpected token after child block: {:?}",
                    self.peek()?
                ),
            };
        }
        Ok(())
    }

    // After name up to the newline.
    fn consume_inline_suite(&mut self, node: &NodeRef) -> Fallible<()> {
        trace!(
            "consuming inline suite at: {:?}",
            &self.tokens[self.position..]
        );
        while !self.out_of_input() {
            match self.peek()? {
                Token::Newline => {
                    self.pop()?;
                    return Ok(());
                }
                _ => self.consume_sigil(node)?,
            }
        }
        // End of file is fine too.
        Ok(())
    }

    // after name + inline_suite + indent up to dedent.
    fn consume_block_suite(&mut self, node: &NodeRef) -> Fallible<()> {
        while !self.out_of_input() {
            match self.peek()? {
                // A name (or bool in this context) is a child and will be parsed elsewhere
                Token::NameTerm(ref _s) => return Ok(()),
                Token::BooleanTerm(_v) => return Ok(()),
                Token::Dedent => return Ok(()),
                Token::Indent => bail!("parse error: expected a sigil before another indent"),
                _ => {
                    self.consume_sigil(node)?;
                    ensure!(
                        self.pop()? == Token::Newline,
                        "parse error: expected a newline after every block sigil"
                    );
                }
            }
        }
        // Or end of file.
        Ok(())
    }

    fn find_next_token(&self, tok: &Token) -> Fallible<usize> {
        let mut i = self.position;
        while i < self.tokens.len() {
            if &self.tokens[i] == tok {
                return Ok(i);
            }
            i += 1;
        }
        bail!("Did not find a matching token for: {:?}", tok)
    }

    fn find_next_matching_dedent(&self) -> Fallible<usize> {
        let mut level = 0;
        let mut i = self.position;
        while i < self.tokens.len() {
            match &self.tokens[i] {
                Token::Indent => level += 1,
                Token::Dedent => {
                    if level == 0 {
                        return Ok(i + 1);
                    }
                    level -= 1;
                }
                _ => {}
            }
            i += 1;
        }
        Ok(i)
    }

    fn consume_sigil(&mut self, node: &NodeRef) -> Fallible<()> {
        trace!("Consuming sigil: {:?}", self.peek()?);
        match self.pop()? {
            Token::Location(dim) => node.set_location(dim)?,
            Token::Size(dim) => node.set_dimensions(dim)?,
            Token::Source(ref s) => node.set_source(s)?,
            Token::Sink(ref s) => node.set_sink(s)?,
            Token::ComesFromInline => {
                let end = self.find_next_token(&Token::Newline)?;
                let s = Script::inline_from_tokens(
                    node.path_str(),
                    &self.tokens[self.position..end],
                    self.nifs,
                )?;
                self.position = end;
                node.set_script(s)?
            }
            Token::ComesFromBlock => {
                ensure!(self.pop()? == Token::Newline, "expected newline after <-\\");
                ensure!(self.pop()? == Token::Indent, "expected indent after <-\\");
                let end = self.find_next_matching_dedent()?;
                let s = Script::inline_from_tokens(
                    node.path_str(),
                    &self.tokens[self.position..end],
                    self.nifs,
                )?;
                self.position = end;
                // Since this is parsed as a sigil, we expect to end with a newline, but since
                // we were indented the Dedent happened after the closing Newline, so inject
                // an extra one here.
                self.tokens.insert(self.position, Token::Newline);
                node.set_script(s)?
            }
            Token::ImportTerm(filename) => self.do_import(&filename, node)?,
            Token::UseTemplate(ref s) => {
                let template: &NodeRef = self
                    .templates
                    .get(s)
                    .ok_or_else(|| format_err!("parse error: unknown template: {}", s))?;
                node.apply_template(template)?
            }
            _ => bail!("parse error: expected to find a sigil-delimited token"),
        }
        Ok(())
    }

    fn do_import(&mut self, filename: &str, parent: &NodeRef) -> Fallible<()> {
        if let Some(subtree) = self.import_interceptors.get(filename) {
            return parent.insert_subtree(&subtree.root());
        }
        bail!("would import {} from file", filename)
    }

    fn consume_node_name(&mut self) -> Fallible<String> {
        ensure!(
            !self.out_of_input(),
            "parse error: no tokens to consume when looking for name"
        );
        Ok(match self.pop()? {
            Token::NameTerm(s) => s,
            Token::BooleanTerm(b) => {
                let v = if b { "true" } else { "false" };
                v.to_owned()
            }
            _ => bail!("parse error: did not find a name in expected position"),
        })
    }

    fn out_of_input(&self) -> bool {
        self.position >= self.tokens.len()
    }

    fn pop(&mut self) -> Fallible<Token> {
        ensure!(!self.out_of_input(), "parse error: no tokens to pop");
        let out = self.tokens[self.position].clone();
        self.position += 1;
        Ok(out)
    }

    fn peek(&self) -> Fallible<Token> {
        ensure!(
            self.position < self.tokens.len(),
            "parse error: enexpected end of input"
        );
        Ok(self.tokens[self.position].clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{physical::Dimension2, tree::TreeBuilder, value::Value};

    /* Note: tracing setup code if we need to debug
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting defualt subscriber failed");
    */

    #[test]
    fn test_parse_minimal() -> Fallible<()> {
        let tree = TreeBuilder::default().build_from_str("a")?;
        assert!(tree.lookup("/a")?.location().is_none());
        Ok(())
    }

    #[test]
    fn test_parse_siblings() -> Fallible<()> {
        let tree = TreeBuilder::default().build_from_str("a\nb")?;
        assert!(tree.lookup("/a")?.location().is_none());
        assert!(tree.lookup("/b")?.location().is_none());
        Ok(())
    }

    #[test]
    fn test_parse_tree_dedent() -> Fallible<()> {
        let s = "
a @1x1
    b @2x2
c @3x3";
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/a")?.location().unwrap(),
            Dimension2::from_str("1x1")?
        );
        assert_eq!(
            tree.lookup("/a/b")?.location().unwrap(),
            Dimension2::from_str("2x2")?
        );
        assert_eq!(
            tree.lookup("/c")?.location().unwrap(),
            Dimension2::from_str("3x3")?
        );
        Ok(())
    }

    #[test]
    fn test_parse_tree_sibling() -> Fallible<()> {
        let s = "
a @1x1
    b @2x2
    c @3x3";
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/a")?.location().unwrap(),
            Dimension2::from_str("1x1")?
        );
        assert_eq!(
            tree.lookup("/a/b")?.location().unwrap(),
            Dimension2::from_str("2x2")?
        );
        assert_eq!(
            tree.lookup("/a/c")?.location().unwrap(),
            Dimension2::from_str("3x3")?
        );
        Ok(())
    }

    #[test]
    fn test_parse_tree_prop_child_prop() -> Fallible<()> {
        let s = r#"
a @1x1 <>1x1
    <- "foo"
    b @2x2
c @3x3
    <-"bar"
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/a")?.location().unwrap(),
            Dimension2::from_str("1x1")?
        );
        assert_eq!(
            tree.lookup("/a")?.dimensions().unwrap(),
            Dimension2::from_str("1x1")?
        );
        assert_eq!(tree.lookup("/a")?.compute(&tree)?, Value::new_str("foo"));
        assert_eq!(
            tree.lookup("/a/b")?.location().unwrap(),
            Dimension2::from_str("2x2")?
        );
        assert_eq!(
            tree.lookup("/c")?.location().unwrap(),
            Dimension2::from_str("3x3")?
        );
        assert_eq!(tree.lookup("/c")?.compute(&tree)?, Value::new_str("bar"));
        Ok(())
    }

    #[test]
    fn test_parse_tree_double_dedent() -> Fallible<()> {
        let s = "
a @1x1
    b @2x2
        c @3x3
d @4x4";
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/a")?.location().unwrap(),
            Dimension2::from_str("1x1")?
        );
        assert_eq!(
            tree.lookup("/a/b")?.location().unwrap(),
            Dimension2::from_str("2x2")?
        );
        assert_eq!(
            tree.lookup("/a/b/c")?.location().unwrap(),
            Dimension2::from_str("3x3")?
        );
        assert_eq!(
            tree.lookup("/d")?.location().unwrap(),
            Dimension2::from_str("4x4")?
        );
        Ok(())
    }

    //     #[test]
    //     fn test_parse_tree_templates() {
    //         let _ = TermLogger::init(LevelFilter::Trace, Config::default());
    //         let s = "
    // template foo [@1x1 <-/b]
    // template bar [
    //     @2x2
    //     #comment
    // ]
    // a !foo
    // b !bar
    // ";
    //         let tree = TreeBuilder::default().build_from_str(s)?;
    //         assert_eq!(
    //             tree.lookup("/a")?.location().unwrap(),
    //             Dimension2::from_str("1x1")?
    //         );
    //         assert_eq!(
    //             tree.lookup("/b")?.location().unwrap(),
    //             Dimension2::from_str("2x2")?
    //         );
    //     }

    #[test]
    #[should_panic]
    fn test_parse_node_before_newline() {
        TreeParser::from_str(
            TreeBuilder::empty(),
            "a b",
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    }

    #[test]
    fn test_parse_script() -> Fallible<()> {
        let s = "a <- 2 + 2";
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/a")?.compute(&tree)?, Value::from_integer(4));
        Ok(())
    }

    #[test]
    fn test_parse_reify_absolute() -> Fallible<()> {
        let s = r#"
a <-"foo"
b <-/a
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::new_str("foo"));
        Ok(())
    }

    #[test]
    fn test_parse_reify_relative() -> Fallible<()> {
        let s = r#"
a <-"foo"
b <-./a
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::new_str("foo"));
        Ok(())
    }

    #[test]
    fn test_parse_child_to_parent() -> Fallible<()> {
        let s = r#"
a <-"foo"
b <-./a
    c <-../b
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        let c = tree.lookup("/b/c")?.compute(&tree)?;
        assert_eq!(c, Value::new_str("foo"));
        Ok(())
    }

    #[test]
    fn test_parse_parent_to_child() -> Fallible<()> {
        let s = r#"
a <-"foo"
b <-./b/c
    c <-../a
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::new_str("foo"));
        Ok(())
    }

    #[test]
    fn test_parse_indirect_simple() -> Fallible<()> {
        let s = r#"
a <- "y"
b <-/{./a}/v
y
    v <- 2
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::from_integer(2));
        Ok(())
    }

    #[test]
    fn test_parse_indirect_computed() -> Fallible<()> {
        let s = r#"
a <- "y"
b <- /a + "z"
c <-/{./b}/v
y
    v <- 2
yz
    v <- 3
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/c")?.compute(&tree)?, Value::from_integer(3));
        Ok(())
    }

    #[test]
    fn test_parse_formula_str() -> Fallible<()> {
        let s = r#"
foo <- "a" + /bar + "c"
bar <- "b"
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::new_str("abc"));
        Ok(())
    }

    #[test]
    fn test_parse_formula_parens() -> Fallible<()> {
        let s = r#"
foo <- "a" + (/bar + /baz) + "c"
bar <- "b"
baz <- "b"
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::new_str("abbc"));
        Ok(())
    }

    #[test]
    fn test_parse_formula_number() -> Fallible<()> {
        let s = r#"
foo <- 2 + (/bar * /baz) + 2
bar <- 3
baz <- 3
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::from_integer(13)
        );
        Ok(())
    }

    #[test]
    fn test_parse_str() -> Fallible<()> {
        let s = r#"
foo <- "a" + str(/bar * /baz) + "b"
bar <- 3
baz <- 3
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::new_str("a9b"));
        Ok(())
    }

    #[test]
    fn test_parse_str_in_path() -> Fallible<()> {
        let s = r#"
foo <- "a" + /{/quux} + "c"
z6 <- "b"
bar <- 2
baz <- 3
quux <- "z" + str(/bar * /baz)
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::new_str("abc"));
        Ok(())
    }

    #[test]
    fn test_parse_modulo() -> Fallible<()> {
        let s = r#"
foo <- /bar % 3
bar <- 2
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::from_integer(2));
        Ok(())
    }

    #[test]
    fn test_parse_bools_in_name_position() -> Fallible<()> {
        let s = r#"
foo
  true <- "hello"
  false <- "world"
bar <-/foo/true + " " + /foo/false
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/bar")?.compute(&tree)?,
            Value::new_str("hello world")
        );
        Ok(())
    }

    #[test]
    fn test_multiline_comesfrom() -> Fallible<()> {
        let s = r#"
a <-"foo"
b <-\
    ./a
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/b")?.compute(&tree)?, Value::new_str("foo"));
        Ok(())
    }

    #[test]
    fn test_multiline_comesfrom_middle() -> Fallible<()> {
        let s = r#"
a <-"foo"
b <-\
    ./a
c <- ./b
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(tree.lookup("/c")?.compute(&tree)?, Value::new_str("foo"));
        Ok(())
    }

    #[test]
    fn test_multiline_comesfrom_nested() -> Fallible<()> {
        let s = r#"
z
    y
        x
            a <-"foo"
            b <-\
                ./a
            c <- ./b
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/z/y/x/c")?.compute(&tree)?,
            Value::new_str("foo")
        );
        Ok(())
    }

    #[test]
    fn test_multiline_comesfrom_nested_end() -> Fallible<()> {
        let s = r#"
z
    y
        x
            a <-"foo"
            b <-\
                ./a
"#;
        let tree = TreeBuilder::default().build_from_str(s)?;
        assert_eq!(
            tree.lookup("/z/y/x/b")?.compute(&tree)?,
            Value::new_str("foo")
        );
        Ok(())
    }

    //     #[test]
    //     fn test_parse_if_statement() -> Fallible<()> {
    //         let s = r#"
    // foo <- true
    // bar <- false
    // quux <- if /foo

    // "#;
    //         let tree = TreeBuilder::default().build_from_str(s)?;
    //         Ok(())
    //     }

    //default   <- "bhs(255, " + (/time/seconds/unix % 65535) + ", 255)"

    //     #[test]
    //     fn test_parse_indirect_latching() {
    //         let s = r#"
    // a <- "y"
    // b <- "z"
    // c
    //     <- latest(/{./a}/v, /{./b}/v)
    // y
    //     v <- 2
    // z
    //     v <- 3
    // "#;
    //         let tree = TreeParser::from_str(TreeBuilder::default(), s)?;
    //         assert_eq!(tree.lookup("/a")?.source().unwrap(), "foo");
    //         assert_eq!(
    //             tree.lookup("/a")?.nodetype().unwrap(),
    //             ValueType::STRING
    //         );
    //         assert_eq!(
    //             tree.lookup("/b")?.nodetype().unwrap(),
    //             ValueType::INTEGER
    //         );
    //         assert_eq!(
    //             tree.lookup("/b/c")?.nodetype().unwrap(),
    //             ValueType::INTEGER
    //         );
    //     }
}
