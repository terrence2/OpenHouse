// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{Error, Fallible};
use path::ConcretePath;
use script::Script;
use std::collections::HashMap;
use tokenizer::{Token, TreeTokenizer};
use tree::{Node, NodeRef, Tree};

pub struct TreeParser<'a> {
    tree: &'a Tree,
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
    pub fn from_str(tree: Tree, s: &str) -> Result<Tree, Error> {
        let sanitized = s.replace('\t', "    ");

        {
            let tokens = TreeTokenizer::tokenize(&sanitized)?;
            let mut parser = TreeParser {
                tree: &tree,
                templates: HashMap::new(),
                tokens,
                position: 0,
            };
            parser.consume_root(&tree.root())?;
        }

        return Ok(tree);
    }

    fn consume_root(&mut self, root: &NodeRef) -> Result<(), Error> {
        while !self.out_of_input() {
            match self.peek()? {
                Token::NameTerm(n) => {
                    if n == "template" {
                        self.consume_template()?;
                    } else {
                        self.consume_tree(root)?;
                    }
                }
                _ => bail!("parse error: expected name at top level"),
            }
        }
        return Ok(());
    }

    fn consume_template(&mut self) -> Result<(), Error> {
        let magic = self.consume_name()?;
        ensure!(
            magic == "template",
            "parse error: expected template to start with 'template'"
        );
        let name = self.peek_name()?;
        trace!("Consuming template: {}", name);
        let template_root = NodeRef::new(Node::new(
            ConcretePath::new_root().new_child("template-root"),
        ));
        self.consume_tree(&template_root)?;
        self.templates
            .insert(name.clone(), template_root.lookup_path(&vec![name])?);
        return Ok(());
    }

    fn consume_tree(&mut self, parent: &NodeRef) -> Result<(), Error> {
        let name = self.consume_name()?;
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
        return Ok(());
    }

    // After name up to the newline.
    fn consume_inline_suite(&mut self, node: &NodeRef) -> Result<(), Error> {
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
        return Ok(());
    }

    // after name + inline_suite + indent up to dedent.
    fn consume_block_suite(&mut self, node: &NodeRef) -> Result<(), Error> {
        while !self.out_of_input() {
            match self.peek()? {
                Token::NameTerm(ref _s) => return Ok(()),
                Token::Dedent => return Ok(()),
                Token::Indent => bail!("parse error: expected a sigil before another indent"),
                _ => {
                    self.consume_sigil(node)?;
                    ensure!(
                        self.peek()? == Token::Newline,
                        "parse error: expected a newline after every block sigil"
                    );
                    self.pop()?;
                }
            }
        }
        // Or end of file.
        return Ok(());
    }

    fn find_next_token(&self, tok: Token) -> Result<usize, Error> {
        let mut i = self.position;
        while i < self.tokens.len() {
            if self.tokens[i] == tok {
                return Ok(i);
            }
            i += 1;
        }
        bail!("Did not find a matching token for: {:?}", tok);
    }

    fn consume_sigil(&mut self, node: &NodeRef) -> Result<(), Error> {
        trace!("Consuming sigil: {:?}", self.peek()?);
        match self.pop()? {
            Token::Location(dim) => node.set_location(dim)?,
            Token::Size(dim) => node.set_dimensions(dim)?,
            Token::Source(ref s) => node.set_source(s, &self.tree)?,
            Token::Sink(ref s) => node.set_sink(s, &self.tree)?,
            Token::ComesFromInline => {
                let end = self.find_next_token(Token::Newline)?;
                let s =
                    Script::inline_from_tokens(node.path_str(), &self.tokens[self.position..end])?;
                self.position = end;
                node.set_script(s)?
            }
            Token::UseTemplate(ref s) => {
                let template: &NodeRef = self
                    .templates
                    .get(s)
                    .ok_or(format_err!("parse error: unknown template: {}", s))?;
                node.apply_template(template)?
            }
            _ => bail!("parse error: expected to find a sigil-delimited token"),
        }
        return Ok(());
    }

    fn consume_name(&mut self) -> Result<String, Error> {
        ensure!(
            !self.out_of_input(),
            "parse error: no tokens to consume when looking for name"
        );
        match self.pop()? {
            Token::NameTerm(s) => Ok(s),
            _ => bail!("parse error: did not find a name in expected position"),
        }
    }

    fn out_of_input(&self) -> bool {
        return self.position >= self.tokens.len();
    }

    fn pop(&mut self) -> Result<Token, Error> {
        ensure!(!self.out_of_input(), "parse error: no tokens to pop");
        let out = self.tokens[self.position].clone();
        self.position += 1;
        return Ok(out);
    }

    fn peek(&self) -> Result<Token, Error> {
        ensure!(
            self.position < self.tokens.len(),
            "parse error: enexpected end of input"
        );
        return Ok(self.tokens[self.position].clone());
    }

    fn peek_name(&self) -> Result<String, Error> {
        let name = if let Token::NameTerm(n) = self.peek()? {
            n.clone()
        } else {
            bail!("parse error: expected template to have a name");
        };
        return Ok(name);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use physical::Dimension2;
    use simplelog::*;
    use value::{Value, ValueType};

    #[test]
    fn test_parse_minimal() {
        let tree = Tree::new_empty().build_from_str("a").unwrap();
        assert!(tree.lookup("/a").unwrap().location().is_none());
    }

    #[test]
    fn test_parse_siblings() {
        let tree = Tree::new_empty().build_from_str("a\nb").unwrap();
        assert!(tree.lookup("/a").unwrap().location().is_none());
        assert!(tree.lookup("/b").unwrap().location().is_none());
    }

    #[test]
    fn test_parse_tree_dedent() {
        let s = "
a @1x1
    b @2x2
c @3x3";
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/c").unwrap().location().unwrap(),
            Dimension2::from_str("3x3").unwrap()
        );
    }

    #[test]
    fn test_parse_tree_sibling() {
        let s = "
a @1x1
    b @2x2
    c @3x3";
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/c").unwrap().location().unwrap(),
            Dimension2::from_str("3x3").unwrap()
        );
    }

    #[test]
    fn test_parse_tree_prop_child_prop() {
        let s = r#"
a @1x1 <>1x1
    <- "foo"
    b @2x2
c @3x3
    <-"bar"
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a").unwrap().dimensions().unwrap(),
            Dimension2::from_str("1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/c").unwrap().location().unwrap(),
            Dimension2::from_str("3x3").unwrap()
        );
        assert_eq!(
            tree.lookup("/c").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
    }

    #[test]
    fn test_parse_tree_double_dedent() {
        let s = "
a @1x1
    b @2x2
        c @3x3
d @4x4";
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b/c").unwrap().location().unwrap(),
            Dimension2::from_str("3x3").unwrap()
        );
        assert_eq!(
            tree.lookup("/d").unwrap().location().unwrap(),
            Dimension2::from_str("4x4").unwrap()
        );
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
    //         let tree = Tree::new_empty().build_from_str(s).unwrap();
    //         assert_eq!(
    //             tree.lookup("/a").unwrap().location().unwrap(),
    //             Dimension2::from_str("1x1").unwrap()
    //         );
    //         assert_eq!(
    //             tree.lookup("/b").unwrap().location().unwrap(),
    //             Dimension2::from_str("2x2").unwrap()
    //         );
    //     }

    #[test]
    #[should_panic]
    fn test_parse_node_before_newline() {
        TreeParser::from_str(Tree::new_empty(), "a b").unwrap();
    }

    #[test]
    fn test_parse_script() {
        // use simplelog::*;
        // let _ = TermLogger::init(LevelFilter::Trace, Config::default());
        let s = "a <- 2 + 2";
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().compute(&tree).unwrap(),
            Value::Integer(4)
        );
    }

    #[test]
    fn test_parse_reify_absolute() {
        let s = r#"
a <-"foo"
b <-/a
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
    }

    #[test]
    fn test_parse_reify_relative() {
        let s = r#"
a <-"foo"
b <-./a
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
    }

    #[test]
    fn test_parse_child_to_parent() {
        let s = r#"
a <-"foo"
b <-./a
    c <-../b
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b/c").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
    }

    #[test]
    fn test_parse_parent_to_child() {
        let s = r#"
a <-"foo"
b <-./b/c
    c <-../a
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b/c").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
    }

    #[test]
    fn test_parse_indirect() {
        let s = r#"
a <- "y"
b <-/{./a}/v
y
    v <- 2
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b").unwrap().nodetype(&tree).unwrap(),
            ValueType::INTEGER
        );
        assert_eq!(
            tree.lookup("/y/v").unwrap().nodetype(&tree).unwrap(),
            ValueType::INTEGER
        );
    }

    #[test]
    fn test_parse_indirect_computed() {
        let s = r#"
a <- "y"
b <- /a + "z"
c <-/{./b}/v
y
    v <- 2
yz
    v <- 3
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/b").unwrap().nodetype(&tree).unwrap(),
            ValueType::STRING
        );
        assert_eq!(
            tree.lookup("/y/v").unwrap().nodetype(&tree).unwrap(),
            ValueType::INTEGER
        );
        assert_eq!(
            tree.lookup("/yz/v").unwrap().nodetype(&tree).unwrap(),
            ValueType::INTEGER
        );
        assert_eq!(
            tree.lookup("/c").unwrap().nodetype(&tree).unwrap(),
            ValueType::INTEGER
        );
        assert_eq!(
            tree.lookup("/c").unwrap().compute(&tree).unwrap(),
            Value::Integer(3)
        );
    }

    #[test]
    fn test_parse_formula_str() -> Fallible<()> {
        let s = r#"
foo <- "a" + /bar + "c"
bar <- "b"
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::String("abc".to_owned())
        );
        Ok(())
    }

    #[test]
    fn test_parse_formula_parens() -> Fallible<()> {
        let s = r#"
foo <- "a" + (/bar + /baz) + "c"
bar <- "b"
baz <- "b"
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::String("abbc".to_owned())
        );
        Ok(())
    }

    #[test]
    fn test_parse_formula_number() -> Fallible<()> {
        let s = r#"
foo <- 2 + (/bar * /baz) + 2
bar <- 3
baz <- 3
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::Integer(13));
        Ok(())
    }

    #[test]
    fn test_parse_str() -> Fallible<()> {
        let s = r#"
foo <- "a" + str(/bar * /baz) + "b"
bar <- 3
baz <- 3
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::String("a9b".to_owned())
        );
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
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/foo")?.compute(&tree)?,
            Value::String("abc".to_owned())
        );
        Ok(())
    }

    #[test]
    fn test_parse_modulo() -> Fallible<()> {
        let s = r#"
foo <- /bar % 3
bar <- 2
"#;
        let tree = Tree::new_empty().build_from_str(s).unwrap();
        assert_eq!(tree.lookup("/foo")?.compute(&tree)?, Value::Integer(2));
        Ok(())
    }

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
    //         let tree = TreeParser::from_str(Tree::new_empty(), s).unwrap();
    //         assert_eq!(tree.lookup("/a").unwrap().source().unwrap(), "foo");
    //         assert_eq!(
    //             tree.lookup("/a").unwrap().nodetype().unwrap(),
    //             ValueType::STRING
    //         );
    //         assert_eq!(
    //             tree.lookup("/b").unwrap().nodetype().unwrap(),
    //             ValueType::INTEGER
    //         );
    //         assert_eq!(
    //             tree.lookup("/b/c").unwrap().nodetype().unwrap(),
    //             ValueType::INTEGER
    //         );
    //     }
}
