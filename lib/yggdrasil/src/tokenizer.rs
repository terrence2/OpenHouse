// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{float::Float, physical::Dimension2};
use failure::{bail, ensure, Fallible};
use tracing::trace;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    // Layout
    Newline,
    Indent,
    Dedent,
    Template,     // template name [...]
    StartOfBlock, // :

    // Sigil-delimited
    Location(Dimension2), // @
    Size(Dimension2),     // <>
    Source(String),       // ^
    Sink(String),         // $
    ComesFromInline,      // <-
    ComesFromBlock,       // <-\
    UseTemplate(String),  // !

    // Operators
    Add,                 // +
    And,                 // &&
    Subtract,            // -
    Divide,              // '/' shared with path
    Multiply,            // *
    Modulo,              // %
    Equals,              // ==
    NotEquals,           // != shared with use-template
    LessThan,            // <  shared with comes-from
    LessThanOrEquals,    // <= shared with comes-from
    GreaterThan,         // >
    GreaterThanOrEquals, // >=
    Or,                  // ||
    Latch,               // ::
    LeftParen,           // (
    RightParen,          // )

    // Terminals
    NameTerm(String),   // [a-zA-Z][a-zA-Z0-9]*
    StringTerm(String), // ""
    IntegerTerm(i64),   // -?[0-9]+
    FloatTerm(Float),   // -?[0-9.]+
    BooleanTerm(bool),  // true|false
    PathTerm(String),   // (\.\.?)?(/identifier)+
    ImportTerm(String), // import(file.ygg)
}

impl Token {
    fn path_string(&self) -> Fallible<&str> {
        match self {
            Token::PathTerm(s) => Ok(s),
            _ => bail!("tokenize error: expected a path token"),
        }
    }

    pub(crate) fn maybe_name(&self) -> Option<&str> {
        if let Token::NameTerm(s) = self {
            return Some(s.as_str());
        }
        None
    }
}

pub struct TreeTokenizer {}

impl TreeTokenizer {
    #[allow(clippy::comparison_chain)]
    pub fn tokenize(s: &str) -> Fallible<Vec<Token>> {
        let mut tokens = Vec::new();

        let mut indent = vec![0];
        for line_raw in s.lines() {
            let line = LineTokenizer::trim_comment(line_raw);
            if line.is_empty() {
                continue;
            }

            let last_level = *indent.last().unwrap();
            let current_level = LineTokenizer::leading_whitespace(&line);
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

            let mut lt = LineTokenizer {
                chars: line.chars().collect::<Vec<char>>(),
                offset: 0,
            };
            while !lt.is_empty() {
                lt.skip_space();
                let token = lt.tokenize_one()?;
                tokens.push(token);
            }
            tokens.push(Token::Newline);
        }

        Ok(tokens)
    }
}

pub struct LineTokenizer {
    chars: Vec<char>,
    offset: usize,
}

impl LineTokenizer {
    fn skip_space(&mut self) {
        while self.maybe_peek(0) == Some(' ') {
            self.offset += 1;
        }
    }

    fn is_empty(&self) -> bool {
        self.offset >= self.chars.len()
    }

    fn tokenize_one(&mut self) -> Fallible<Token> {
        let c = self.peek(0)?;
        let tok = match c {
            'a'..='z' | 'A'..='Z' => self.tokenize_name_or_keyword(),
            '0'..='9' => self.tokenize_int_or_float(),
            '/' => self.tokenize_absolute_path_or_division(),
            '.' => self.tokenize_path(),
            '^' => self.tokenize_source(),
            '$' => self.tokenize_sink(),
            '!' => self.tokenize_use_template_or_not_eq(),
            '@' => self.tokenize_location(),
            '"' => self.tokenize_string(),
            '<' => self.tokenize_comes_from_or_less_than_or_size(),
            '>' => self.tokenize_greater_than(),
            '|' | '&' => self.tokenize_operator_2(),
            '-' => self.tokenize_subtract_or_number(),
            ':' => self.tokenize_start_of_block_or_latch(),
            '(' => {
                self.offset += 1;
                Ok(Token::LeftParen)
            }
            ')' => {
                self.offset += 1;
                Ok(Token::RightParen)
            }
            '+' => {
                self.offset += 1;
                Ok(Token::Add)
            }
            '*' => {
                self.offset += 1;
                Ok(Token::Multiply)
            }
            '%' => {
                self.offset += 1;
                Ok(Token::Modulo)
            }
            '=' => {
                ensure!(self.peek(1)? == '=', "expected double == sign");
                self.offset += 2;
                Ok(Token::Equals)
            }
            _ => bail!(
                "tokenize error: expected a sigil or name, found: {}",
                self.chars[self.offset]
            ),
        }?;
        trace!("tokenize: {} => {:?}", c, tok);
        Ok(tok)
    }

    fn tokenize_name_or_keyword(&mut self) -> Fallible<Token> {
        let s = self.tokenize_identifier()?;
        if s == "true" {
            Ok(Token::BooleanTerm(true))
        } else if s == "false" {
            Ok(Token::BooleanTerm(false))
        } else if s == "import" {
            self.tokenize_import()
        } else if s == "template" {
            self.tokenize_template()
        } else {
            Ok(Token::NameTerm(s))
        }
    }

    fn tokenize_import(&mut self) -> Fallible<Token> {
        self.skip_space();
        ensure!(
            self.peek(0)? == '(',
            "tokenize error: expected import to take an argument in ()"
        );
        self.offset += 1;
        self.skip_space();
        let filename = self.tokenize_path()?;
        self.skip_space();
        ensure!(
            self.peek(0)? == ')',
            "tokenize error: expected closing ) in import"
        );
        self.offset += 1;
        Ok(Token::ImportTerm(filename.path_string()?.to_owned()))
    }

    fn tokenize_template(&mut self) -> Fallible<Token> {
        self.skip_space();
        let _name = self.tokenize_identifier()?;
        self.skip_space();
        ensure!(
            self.peek(0)? == '[',
            "expected template to be surrounded with []"
        );
        self.offset += 1;
        let start_offset = self.offset;
        while self.peek(0)? != ']' {
            self.offset += 1;
        }
        let template_chars = &self.chars[start_offset..self.offset];
        let template_str = template_chars.iter().collect::<String>();
        self.offset += 1;
        let _template_tokens = TreeTokenizer::tokenize(&template_str)?;
        Ok(Token::Template)
    }

    fn tokenize_subtract_or_number(&mut self) -> Fallible<Token> {
        if let Some(c) = self.maybe_peek(1) {
            if c == ' ' || c == '/' || c == '.' {
                self.offset += 1;
                return Ok(Token::Subtract);
            }
        }
        self.tokenize_int_or_float()
    }

    fn tokenize_start_of_block_or_latch(&mut self) -> Fallible<Token> {
        if let Some(c) = self.maybe_peek(1) {
            if c == ':' {
                self.offset += 2;
                return Ok(Token::Latch);
            }
        }
        self.offset += 1;
        Ok(Token::StartOfBlock)
    }

    fn tokenize_int_or_float(&mut self) -> Fallible<Token> {
        let negative = match self.peek(0)? {
            '-' => {
                self.offset += 1;
                -1
            }
            _ => 1,
        };
        let start = self.offset;
        let mut contains_dot = false;
        while !self.is_empty() {
            match self.peek(0)? {
                '0'..='9' => self.offset += 1,
                '.' => {
                    self.offset += 1;
                    contains_dot = true;
                }
                _ => break,
            }
        }
        let s = self.chars[start..self.offset].iter().collect::<String>();
        if contains_dot {
            return Ok(Token::FloatTerm(Float::new(
                negative as f64 * s.parse::<f64>()?,
            )?));
        }
        Ok(Token::IntegerTerm(negative * s.parse::<i64>()?))
    }

    fn tokenize_source(&mut self) -> Fallible<Token> {
        assert!(self.peek(0)? == '^');
        self.offset += 1;
        Ok(Token::Source(self.tokenize_identifier()?))
    }

    fn tokenize_sink(&mut self) -> Fallible<Token> {
        assert!(self.peek(0)? == '$');
        self.offset += 1;
        Ok(Token::Sink(self.tokenize_identifier()?))
    }

    fn tokenize_absolute_path_or_division(&mut self) -> Fallible<Token> {
        assert!(self.peek(0)? == '/');
        match self.maybe_peek(1) {
            None | Some(' ') => self.tokenize_division(),
            _ => self.tokenize_path(),
        }
    }

    fn tokenize_division(&mut self) -> Fallible<Token> {
        self.offset += 1;
        Ok(Token::Divide)
    }

    fn tokenize_use_template_or_not_eq(&mut self) -> Fallible<Token> {
        if self.peek(1)? == '=' {
            self.offset += 2;
            return Ok(Token::NotEquals);
        }
        self.offset += 1;
        Ok(Token::UseTemplate(self.tokenize_identifier()?))
    }

    fn tokenize_location(&mut self) -> Fallible<Token> {
        ensure!(self.peek(0)? == '@', "expected location start token");
        self.offset += 1;
        let start = self.offset;
        while !self.is_empty() {
            if self.chars[self.offset].is_whitespace() {
                break;
            }
            self.offset += 1;
        }
        let span = self.chars[start..self.offset].iter().collect::<String>();
        Ok(Token::Location(Dimension2::from_str(&span)?))
    }

    fn tokenize_size(&mut self) -> Fallible<Token> {
        ensure!(self.peek(0)? == '<', "expected size start token0");
        ensure!(self.peek(1)? == '>', "expected size start token1");
        self.offset += 2;
        let start = self.offset;
        while !self.is_empty() {
            if self.chars[self.offset].is_whitespace() {
                break;
            }
            self.offset += 1;
        }
        let span = self.chars[start..self.offset].iter().collect::<String>();
        Ok(Token::Size(Dimension2::from_str(&span)?))
    }

    fn tokenize_string(&mut self) -> Fallible<Token> {
        assert_eq!(self.peek(0)?, '"');
        let mut out = Vec::new();
        self.offset += 1;
        while !self.is_empty() {
            match self.chars[self.offset] {
                '\\' => {
                    ensure!(
                        self.peek(1)? == '"',
                        "tokenize error: unsupported \\ escape"
                    );

                    // Skip the following quote.
                    out.push('"');
                    self.offset += 1;
                }
                '"' => {
                    self.offset += 1;
                    return Ok(Token::StringTerm(out.iter().collect::<String>()));
                }
                c => out.push(c),
            }
            self.offset += 1;
        }
        bail!("tokenize error: unmatched \"")
    }

    fn tokenize_comes_from_or_less_than_or_size(&mut self) -> Fallible<Token> {
        match self.maybe_peek(1) {
            Some('-') => {
                if self.maybe_peek(2) == Some('\\') {
                    self.offset += 3;
                    return Ok(Token::ComesFromBlock);
                }
                self.offset += 2;
                Ok(Token::ComesFromInline)
            }
            Some('=') => {
                self.offset += 2;
                Ok(Token::LessThanOrEquals)
            }
            Some('>') => self.tokenize_size(),
            None => {
                self.offset += 1;
                Ok(Token::LessThan)
            }
            _ => {
                self.offset += 1;
                Ok(Token::LessThan)
            }
        }
    }

    fn tokenize_greater_than(&mut self) -> Fallible<Token> {
        assert!(self.peek(0)? == '>');
        if self.peek(1)? == '=' {
            return Ok(Token::GreaterThanOrEquals);
        }
        Ok(Token::GreaterThan)
    }

    fn tokenize_operator_2(&mut self) -> Fallible<Token> {
        let t = match self.peek(1)? {
            '&' => {
                assert!(self.peek(0)? == '&');
                Token::And
            }
            '|' => {
                assert!(self.peek(0)? == '|');
                Token::Or
            }
            _ => bail!("tokenize error: expected && or ||"),
        };
        self.offset += 2;
        Ok(t)
    }

    fn tokenize_path(&mut self) -> Fallible<Token> {
        // Note: this is identifier with [/.{}] included. It is up to the user to
        //       build a real, well-formed path from this.
        let start = self.offset;
        while !self.is_empty() {
            match self.peek(0)? {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '/' | '.' | '{' | '}' => {
                    self.offset += 1
                }
                _ => break,
            }
        }
        let content = self.chars[start..self.offset].iter().collect::<String>();
        Ok(Token::PathTerm(content))
    }

    fn tokenize_identifier(&mut self) -> Fallible<String> {
        let start = self.offset;
        while !self.is_empty() {
            match self.chars[self.offset] {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => self.offset += 1,
                _ => break,
            }
        }
        Ok(self.chars[start..self.offset].iter().collect::<String>())
    }

    fn maybe_peek(&self, n: usize) -> Option<char> {
        if self.offset + n < self.chars.len() {
            return Some(self.chars[self.offset + n]);
        }
        None
    }

    fn peek(&self, n: usize) -> Fallible<char> {
        ensure!(
            self.offset + n < self.chars.len(),
            "tokenize error: out of input too soon"
        );
        Ok(self.chars[self.offset + n])
    }

    fn trim_comment(line_raw: &str) -> String {
        let mut line = line_raw.to_owned();
        if let Some(offset) = line_raw.find('#') {
            line.truncate(offset);
        }
        line.trim_end().to_owned()
    }

    fn leading_whitespace(s: &str) -> usize {
        let mut cnt = 0;
        for c in s.chars() {
            if c != ' ' {
                break;
            }
            cnt += 1;
        }
        cnt
    }
}

#[cfg(test)]
mod test {
    use super::{Dimension2, Fallible, Float, Token, TreeTokenizer as TT};

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
            TT::tokenize(s).unwrap(),
            vec![
                Token::NameTerm("a".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::NameTerm("b".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::NameTerm("c".to_owned()),
                Token::Newline,
                Token::Dedent,
                Token::NameTerm("d".to_owned()),
                Token::Newline,
                Token::NameTerm("e".to_owned()),
                Token::Newline,
                Token::Dedent,
                Token::NameTerm("f".to_owned()),
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
            TT::tokenize(s).unwrap(),
            vec![
                Token::NameTerm("a".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::NameTerm("b".to_owned()),
                Token::Newline,
                Token::Indent,
                Token::NameTerm("c".to_owned()),
                Token::Newline,
                Token::Dedent,
                Token::Dedent,
                Token::NameTerm("d".to_owned()),
                Token::Newline,
            ]
        )
    }

    #[test]
    fn test_tokenize_string_simple() {
        assert_eq!(
            TT::tokenize(r#""a b c d""#).unwrap(),
            vec![Token::StringTerm("a b c d".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_string_empty() {
        assert_eq!(
            TT::tokenize(r#""""#).unwrap(),
            vec![Token::StringTerm("".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_string_quotes() {
        assert_eq!(
            TT::tokenize(r#""\"\"""#).unwrap(),
            vec![Token::StringTerm(r#""""#.to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_location() {
        assert_eq!(
            TT::tokenize("@1'1\"x2'2\"").unwrap(),
            vec![
                Token::Location(Dimension2::from_str("1'1\"x2'2\"").unwrap()),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_size() {
        assert_eq!(
            TT::tokenize("<>1'1\"x2'2\"").unwrap(),
            vec![
                Token::Size(Dimension2::from_str("1'1\"x2'2\"").unwrap()),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_source() {
        assert_eq!(
            TT::tokenize("^a-s-d-f").unwrap(),
            vec![Token::Source("a-s-d-f".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_sink() {
        assert_eq!(
            TT::tokenize("$a-s-d-f").unwrap(),
            vec![Token::Sink("a-s-d-f".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_use_template() {
        assert_eq!(
            TT::tokenize("!a-s-d-f").unwrap(),
            vec![Token::UseTemplate("a-s-d-f".to_owned()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_not_eq() {
        assert_eq!(
            TT::tokenize("!=").unwrap(),
            vec![Token::NotEquals, Token::Newline]
        );
    }

    #[test]
    #[should_panic]
    fn test_tokenize_not() {
        TT::tokenize("!").unwrap();
    }

    #[test]
    fn test_tokenize_comes_from_inline() {
        assert_eq!(
            TT::tokenize("<-").unwrap(),
            vec![Token::ComesFromInline, Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_comes_from_block() {
        assert_eq!(
            TT::tokenize("<-\\").unwrap(),
            vec![Token::ComesFromBlock, Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_less_eq() {
        assert_eq!(
            TT::tokenize("<=").unwrap(),
            vec![Token::LessThanOrEquals, Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_less() {
        assert_eq!(
            TT::tokenize("<").unwrap(),
            vec![Token::LessThan, Token::Newline]
        );
        assert_eq!(
            TT::tokenize("<foo").unwrap(),
            vec![
                Token::LessThan,
                Token::NameTerm("foo".to_owned()),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_div() {
        assert_eq!(
            TT::tokenize("/").unwrap(),
            vec![Token::Divide, Token::Newline]
        );
        assert_eq!(
            TT::tokenize("foo/ bar").unwrap(),
            vec![
                Token::NameTerm("foo".to_owned()),
                Token::Divide,
                Token::NameTerm("bar".to_owned()),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_add() {
        assert_eq!(
            TT::tokenize("0 + 0").unwrap(),
            vec![
                Token::IntegerTerm(0),
                Token::Add,
                Token::IntegerTerm(0),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_sub() {
        assert_eq!(
            TT::tokenize("0 - 0").unwrap(),
            vec![
                Token::IntegerTerm(0),
                Token::Subtract,
                Token::IntegerTerm(0),
                Token::Newline,
            ]
        );

        assert_eq!(
            TT::tokenize("-/a").unwrap(),
            vec![
                Token::Subtract,
                Token::PathTerm("/a".to_owned()),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_latch() {
        assert_eq!(
            TT::tokenize("0 :: 0").unwrap(),
            vec![
                Token::IntegerTerm(0),
                Token::Latch,
                Token::IntegerTerm(0),
                Token::Newline,
            ]
        );
    }

    #[test]
    fn test_tokenize_integer() {
        assert_eq!(
            TT::tokenize("1").unwrap(),
            vec![Token::IntegerTerm(1), Token::Newline]
        );
        assert_eq!(
            TT::tokenize("-1").unwrap(),
            vec![Token::IntegerTerm(-1), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_float() {
        assert_eq!(
            TT::tokenize("1.").unwrap(),
            vec![Token::FloatTerm(Float::new(1f64).unwrap()), Token::Newline]
        );
        assert_eq!(
            TT::tokenize("-1.").unwrap(),
            vec![Token::FloatTerm(Float::new(-1f64).unwrap()), Token::Newline]
        );
    }

    #[test]
    fn test_tokenize_import() -> Fallible<()> {
        assert_eq!(
            TT::tokenize("import(test.ygg)")?,
            vec![Token::ImportTerm("test.ygg".to_owned()), Token::Newline]
        );
        Ok(())
    }
}
