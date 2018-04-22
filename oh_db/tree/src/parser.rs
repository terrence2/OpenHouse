// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.

use Tree;
use NodeRef;
use Dimension2;
use failure::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    At(Dimension2),
    LiteralSource(String),
    IndirectSource(String),
    SensorSource(String),
    Target(String),
    Name(String),
    Newline,
    Indent,
    Dedent,
}

pub struct TreeParser;

impl TreeParser {
    pub fn from_str(s: &str) -> Result<Tree, Error> {
        let sanitized = s.replace('\t', "    ");
        let mut tokens = Self::tokenize(&sanitized)?;
        let tree = Tree::new();
        Self::consume_tree(tree.root(), &mut tokens)?;

        // TODO: analyze the tree to hook up sources to destinations.

        return Ok(tree);
    }

    fn consume_tree(parent: NodeRef, tokens: &mut Vec<Token>) -> Result<(), Error> {
        let name = Self::consume_name(tokens)?;
        let child = parent.add_child(&name)?;
        let mut indented = false;
        while tokens.len() != 0 {
            let token = tokens.remove(0);
            match &token {
                &Token::At(ref dim) => child.set_location(*dim),
                &Token::LiteralSource(ref content) => child.set_literal_source(&content)?,
                &Token::IndirectSource(ref from) => child.set_indirect_source(&from)?,
                &Token::SensorSource(ref from) => child.set_sensor_source(&from)?,
                &Token::Target(ref to) => child.set_target(&to)?,
                &Token::Name(ref _s) => {
                    if indented {
                        // If we indented, it is a child of child.
                        tokens.insert(0, token.clone());
                        return Self::consume_tree(child.clone(), tokens);
                    } else {
                        // If we have not indented, then it is a sibling of child.
                        tokens.insert(0, token.clone());
                        return Self::consume_tree(parent, tokens);
                    }
                }
                &Token::Dedent => {
                    if indented {
                        // The new name is a sibling of child.
                        return Self::consume_tree(parent, tokens);
                    } else {
                        // The new name is a sibling of parent.
                        return Self::consume_tree(parent.lookup("..")?, tokens);
                    }
                }
                &Token::Indent => {
                    ensure!(!indented, "indented without being in a child");
                    indented = true;
                }
                &Token::Newline => {}
            }
        }
        return Ok(());
    }

    fn consume_name(tokens: &mut Vec<Token>) -> Result<String, Error> {
        ensure!(
            !tokens.is_empty(),
            "no tokens to consume when looking for name"
        );
        match tokens.remove(0) {
            Token::Name(s) => Ok(s),
            _ => bail!("Did not find a name in first position."),
        }
    }

    fn tokenize(s: &str) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        let mut indent = vec![0];
        for line_raw in s.lines() {
            let line = line_raw.trim_right();
            if line.is_empty() {
                continue;
            }

            let last_level = *indent.last().unwrap();
            let current_level = Self::leading_whitespace(line);
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

            for tok in line.trim().split(' ') {
                match tok.chars().next() {
                    None => continue,
                    Some('@') => tokens.push(Token::At(Dimension2::from_str(tok)?)),
                    Some('=') => tokens.push(Token::LiteralSource(
                        tok.chars().skip(1).collect::<String>(),
                    )),
                    Some('$') => {
                        tokens.push(Token::SensorSource(tok.chars().skip(1).collect::<String>()))
                    }
                    Some('<') => {
                        ensure!(tok.starts_with("<-"), "expected < to have a - after it");
                        tokens.push(Token::IndirectSource(
                            tok.chars().skip(2).collect::<String>(),
                        ))
                    }
                    Some('-') => {
                        ensure!(tok.starts_with("->"), "expected - to have a > after it");
                        tokens.push(Token::Target(tok.chars().skip(2).collect::<String>()))
                    }
                    Some(_) => tokens.push(Token::Name(tok.to_owned())),
                }
            }
            tokens.push(Token::Newline);
        }

        return Ok(tokens);
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
    use DataSource;

    #[test]
    fn test_parse_dedent1() {
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
    fn test_parse_dedent2() {
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
    fn test_parse_tree_dedent() {
        let s = "
a @1x1
    b @2x2
c @3x3";
        let tree = TreeParser::from_str(s).unwrap();
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
        let tree = TreeParser::from_str(s).unwrap();
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
    =foobar
    b @2x2 $redstone
c @3x3
    <-/a/b";
        let tree = TreeParser::from_str(s).unwrap();
        assert_eq!(
            tree.lookup("/a").unwrap().location().unwrap(),
            Dimension2::from_str("@1x1").unwrap()
        );
        assert_eq!(
            tree.lookup("/a").unwrap().source().unwrap(),
            DataSource::Literal("foobar".to_owned())
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().location().unwrap(),
            Dimension2::from_str("@2x2").unwrap()
        );
        assert_eq!(
            tree.lookup("/a/b").unwrap().source().unwrap(),
            DataSource::Sensor("redstone".to_owned())
        );
        assert_eq!(
            tree.lookup("/c").unwrap().location().unwrap(),
            Dimension2::from_str("@3x3").unwrap()
        );
        assert_eq!(
            tree.lookup("/c").unwrap().source().unwrap(),
            DataSource::Indirect("/a/b".to_owned())
        );
    }
}
