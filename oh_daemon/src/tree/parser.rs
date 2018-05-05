// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use tree::{physical::Dimension2, tree::{Node, NodeRef, Tree}};
use failure::Error;
use std::{collections::HashMap, fs::File, path::Path};
use std::io::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Location(Dimension2),
    Literal(String),
    ComesFromInline(String),
    ComesFromBlock(String),
    Source(String),
    Sink(String),
    UseTemplate(String),
    Name(String),
    Newline,
    Indent,
    Dedent,
}

pub struct TreeParser {
    verbosity: u8,
    templates: HashMap<String, NodeRef>,
    tokens: Vec<Token>,
    position: usize,
}

impl TreeParser {
    pub fn from_file(path: &Path, verbosity: u8) -> Result<Tree, Error> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        return Self::from_str(&contents, verbosity);
    }

    pub fn from_str(s: &str, verbosity: u8) -> Result<Tree, Error> {
        let sanitized = s.replace('\t', "    ");

        let tokens = Self::tokenize(&sanitized)?;
        if verbosity >= 3 {
            println!("Tokens:");
            for tok in tokens.iter() {
                println!("\t{:?}", tok);
            }
        }

        let mut tree = Tree::new();
        let mut parser = TreeParser {
            verbosity,
            templates: HashMap::new(),
            tokens,
            position: 0,
        };
        parser.consume_root(&tree.root())?;

        tree.build_flow_graph();
        return Ok(tree);
    }

    fn consume_root(&mut self, root: &NodeRef) -> Result<(), Error> {
        while !self.out_of_input() {
            match self.peek()? {
                Token::Name(n) => {
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
        if self.verbosity >= 3 {
            println!("Consuming template: {}", name);
        }
        let template_root = NodeRef::new(Node::new("template-root"));
        self.consume_tree(&template_root)?;
        self.templates
            .insert(name.clone(), template_root.lookup(&name)?);
        return Ok(());
    }

    fn consume_tree(&mut self, parent: &NodeRef) -> Result<(), Error> {
        let name = self.consume_name()?;
        if self.verbosity >= 2 {
            println!(
                "Consuming tree at: {} under parent: {}",
                name,
                parent.name()
            );
        }

        let child = parent.add_child(&name)?;
        self.consume_inline_suite(&child)?;
        if self.out_of_input() || self.peek()? != Token::Indent {
            if self.verbosity >= 3 {
                println!("finished tree {}", name);
            }
            return Ok(());
        }

        // Next token is indent, so parse any body and any children.
        self.pop()?;
        self.consume_block_suite(&child)?;
        while !self.out_of_input() {
            match self.peek()? {
                Token::Name(ref _s) => self.consume_tree(&child)?,
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

    fn consume_inline_suite(&mut self, node: &NodeRef) -> Result<(), Error> {
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

    fn consume_block_suite(&mut self, node: &NodeRef) -> Result<(), Error> {
        while !self.out_of_input() {
            match self.peek()? {
                Token::Name(ref _s) => return Ok(()),
                Token::Dedent => return Ok(()),
                Token::Indent => bail!("parse error: expected to find a sigil-delimited token"),
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

    fn consume_sigil(&mut self, node: &NodeRef) -> Result<(), Error> {
        if self.verbosity >= 3 {
            println!("Consuming sigil: {:?}", self.peek()?);
        }
        match self.pop()? {
            Token::Location(dim) => node.set_location(dim)?,
            Token::Literal(ref s) => node.set_literal(s)?,
            Token::Source(ref s) => node.set_source(s)?,
            //Token::ComesFrom(ref s) => node.add_comes_from(s)?,
            Token::Sink(ref s) => node.set_sink(s)?,
            Token::UseTemplate(ref s) => {
                let template: &NodeRef = self.templates
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
            Token::Name(s) => Ok(s),
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
        let name = if let Token::Name(n) = self.peek()? {
            n.clone()
        } else {
            bail!("parse error: expected template to have a name");
        };
        return Ok(name);
    }

    fn tokenize(s: &str) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        let all_chars = s.chars().collect::<Vec<char>>();
        let mut indent = vec![0];
        for line_raw in s.lines() {
            let line = Self::trim_comment(line_raw);
            if line.is_empty() {
                continue;
            }

            let last_level = *indent.last().unwrap();
            let current_level = Self::leading_whitespace(&line);
            if current_level > last_level {
                indent.push(current_level);
                tokens.push(Token::Indent);
            } else if current_level < last_level {
                if let Ok(offset) = indent.binary_search(&current_level) {
                    let cnt = indent.len() - offset - 1;
                    for _ in 0..cnt {
                        indent.pop();
                        tokens.push(Token::Dedent);
                    }
                } else {
                    bail!("dedent not aligned with a prior indent level");
                }
            }

            let chars = line.chars().collect::<Vec<char>>();
            let mut offset = 0;
            while offset < chars.len() {
                match chars[offset] {
                    ' ' => offset += 1,
                    'a'...'z' | 'A'...'Z' => tokens.push(Self::tokenize_name(&chars, &mut offset)?),
                    '^' => tokens.push(Self::tokenize_source(&chars, &mut offset)?),
                    '$' => tokens.push(Self::tokenize_sink(&chars, &mut offset)?),
                    '!' => tokens.push(Self::tokenize_use_template(&chars, &mut offset)?),
                    '@' => tokens.push(Self::tokenize_location(&chars, &mut offset)?),
                    '"' => tokens.push(Self::tokenize_literal(&chars, &mut offset)?),
                    '<' => tokens.push(Self::tokenize_comes_from(&chars, &mut offset)?),
                    _ => bail!("tokenize error: expected a sigil or name"),
                }
            }
            tokens.push(Token::Newline);
        }

        return Ok(tokens);
    }

    fn tokenize_name(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        return Ok(Token::Name(Self::tokenize_identifier(chars, offset)?));
    }

    fn tokenize_source(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        return Ok(Token::Source(Self::tokenize_identifier(chars, offset)?));
    }

    fn tokenize_sink(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        return Ok(Token::Sink(Self::tokenize_identifier(chars, offset)?));
    }

    fn tokenize_use_template(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        return Ok(Token::UseTemplate(Self::tokenize_identifier(
            chars,
            offset,
        )?));
    }

    fn tokenize_location(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        let start = *offset;
        while *offset < chars.len() {
            if chars[*offset].is_whitespace() {
                break;
            }
            *offset += 1;
        }
        let span = chars[start..*offset].iter().collect::<String>();
        return Ok(Token::Location(Dimension2::from_str(&span)?));
    }

    fn tokenize_literal(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        ensure!(
            chars[*offset] == '"',
            "tokenize_error: expected literal to start with \""
        );
        let mut out = Vec::new();
        *offset += 1;
        let start = *offset;
        while *offset < chars.len() {
            match chars[*offset] {
                '\\' => {
                    ensure!(
                        *offset + 1 < chars.len(),
                        "tokenize error: quoted literals must be on one line"
                    );
                    ensure!(
                        chars[*offset + 1] == '"',
                        "tokenize error: unsupported \\ escape"
                    );

                    // Skip the following quote.
                    out.push('"');
                    *offset += 1;
                }
                '"' => {
                    break;
                }
                c @ _ => out.push(c),
            }
            *offset += 1;
        }
        ensure!(
            *offset < chars.len() && chars[*offset] == '"',
            "tokenize error: unclosed \""
        );
        *offset += 1;
        return Ok(Token::Literal(out.iter().collect::<String>()));
    }

    fn tokenize_comes_from(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        ensure!(
            *offset + 2 < chars.len(),
            "tokenize error: <- sigil must not cross lines"
        );
        return Ok(match chars[*offset + 2] {
            '.' | '/' => Self::tokenize_comes_from_path(chars, offset)?,
            '{' => Self::tokenize_comes_from_block(chars, offset)?,
            _ => bail!("tokenize error: <- sigil must be followed immediately by one of . / or {"),
        });
    }

    fn tokenize_comes_from_path(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        let start = *offset;
        while *offset < chars.len() {
            // Note: this is tokenize_literal + /
            match chars[*offset] {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '_' | '/' => *offset += 1,
                _ => break,
            }
        }
        return Ok(Token::ComesFromInline(
            chars[start..*offset].iter().collect::<String>(),
        ));
    }

    fn tokenize_comes_from_block(chars: &Vec<char>, offset: &mut usize) -> Result<Token, Error> {
        bail!("unimplemented");
    }

    fn tokenize_identifier(chars: &Vec<char>, offset: &mut usize) -> Result<String, Error> {
        let start = *offset;
        while *offset < chars.len() {
            match chars[*offset] {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '_' => *offset += 1,
                _ => break,
            }
        }
        return Ok(chars[start..*offset].iter().collect::<String>());
    }

    fn trim_comment(line_raw: &str) -> String {
        let mut line = line_raw.to_owned();
        if let Some(offset) = line_raw.find('#') {
            line.truncate(offset);
        }
        return line.trim_right().to_owned();
    }

    fn leading_whitespace(s: &str) -> usize {
        let mut cnt = 0;
        for c in s.chars() {
            if c != ' ' {
                break;
            }
            cnt += 1;
        }
        return cnt;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_tokenize_dedent1() {
        let s = "
a
    b
        c
    d
    e
f";
        assert_eq!(
            TreeParser::tokenize(s).unwrap(),
            vec![
                Token::Name("a".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::Name("b".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::Name("c".to_owned()),
                Token::Newline,
                Token::Dedent,
                Token::Name("d".to_owned()),
                Token::Newline,
                Token::Name("e".to_owned()),
                Token::Newline,
                Token::Dedent,
                Token::Name("f".to_owned()),
                Token::Newline,
            ]
        )
    }

    #[test]
    fn test_tokenize_dedent2() {
        let s = "
a
    b
        c
d";
        assert_eq!(
            TreeParser::tokenize(s).unwrap(),
            vec![
                Token::Name("a".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::Name("b".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::Name("c".to_owned()),
                Token::Newline,
                Token::Dedent,
                Token::Dedent,
                Token::Name("d".to_owned()),
                Token::Newline,
            ]
        )
    }

    #[test]
    fn test_tokenize_literal_simple() {
        assert_eq!(
            TreeParser::tokenize(r#""a b c d""#).unwrap(),
            vec![Token::Literal("a b c d".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_literal_empty() {
        assert_eq!(
            TreeParser::tokenize(r#""""#).unwrap(),
            vec![Token::Literal("".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_literal_quotes() {
        assert_eq!(
            TreeParser::tokenize(r#""\"\"""#).unwrap(),
            vec![Token::Literal(r#""""#.to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_parse_siblings() {
        let tree = TreeParser::from_str("a\nb", 5).unwrap();
        assert!(tree.lookup("/a").unwrap().location().is_none());
        assert!(tree.lookup("/b").unwrap().location().is_none());
    }

    #[test]
    fn test_parse_tree_dedent() {
        let s = "
a @1x1
    b @2x2
c @3x3";
        let tree = TreeParser::from_str(s, 5).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("@1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("@2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/c").unwrap().location().unwrap(),
            Dimension2::from_str("@3x3").unwrap()
        );
    }

    #[test]
    fn test_parse_tree_sibling() {
        let s = "
a @1x1
    b @2x2
    c @3x3";
        let tree = TreeParser::from_str(s, 5).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("@1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("@2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/c").unwrap().location().unwrap(),
            Dimension2::from_str("@3x3").unwrap()
        );
    }

    #[test]
    fn test_parse_tree_prop_child_prop() {
        let s = "
a @1x1
    ^foo
    b @2x2 $redstone
c @3x3
    ^bar";
        let tree = TreeParser::from_str(s, 0).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("@1x1").unwrap()
        );
        assert_eq!(tree.lookup("/a").unwrap().source().unwrap(), "foo");
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("@2x2").unwrap()
        );
        assert_eq!(tree.lookup("/a/b").unwrap().sink().unwrap(), "redstone");
        assert_eq!(
            tree.lookup("/c").unwrap().location().unwrap(),
            Dimension2::from_str("@3x3").unwrap()
        );
        assert_eq!(tree.lookup("/c").unwrap().source().unwrap(), "bar");
    }

    #[test]
    fn test_parse_tree_double_dedent() {
        let s = "
a @1x1
    b @2x2
        c @3x3
d @4x4";
        let tree = TreeParser::from_str(s, 0).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("@1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("@2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b/c").unwrap().location().unwrap(),
            Dimension2::from_str("@3x3").unwrap()
        );
        assert_eq!(
            tree.lookup("/d").unwrap().location().unwrap(),
            Dimension2::from_str("@4x4").unwrap()
        );
    }

    #[test]
    fn test_parse_tree_templates() {
        let s = "
template foo @1x1
template bar @2x2
a !foo
b !bar
";
        let tree = TreeParser::from_str(s, 5).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("@1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/b").unwrap().location().unwrap(),
            Dimension2::from_str("@2x2").unwrap()
        );
    }

    #[test]
    #[should_panic]
    fn test_parse_node_before_newline() {
        TreeParser::from_str("a b", 5).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_parse_indirect() {
        let s = "
a ^foo
b <-/a
";
        let tree = TreeParser::from_str(s, 5).unwrap();
        assert_eq!(tree.lookup("/a").unwrap().source().unwrap(), "foo");
        //tree.send_event("foo", HashMap::new()).unwrap();
    }
}
