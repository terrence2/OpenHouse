// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Error;
use float::Float;
use physical::Dimension2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    // Layout
    Newline,
    Indent,
    Dedent,

    // Sigil-delimited
    Location(Dimension2), // @
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
    LeftParen,           // (
    RightParen,          // )

    // Terminals
    NameTerm(String),   // [a-zA-Z][a-zA-Z0-9]*
    StringTerm(String), // ""
    IntegerTerm(i64),   // [0-9]+
    FloatTerm(Float),   // [0-9.]+
    BooleanTerm(bool),  // true|false
    PathTerm(String),   // (\.\.?)?(/identifier)+
}

pub struct TreeTokenizer {
    chars: Vec<char>,
    offset: usize,
}

impl TreeTokenizer {
    pub fn tokenize(s: &str) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

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

            let mut tt = TreeTokenizer {
                chars: line.chars().collect::<Vec<char>>(),
                offset: 0,
            };
            while !tt.is_empty() {
                while tt.peek(0)? == ' ' {
                    tt.offset += 1;
                }
                tokens.push(tt.tokenize_one()?);
            }
            tokens.push(Token::Newline);
        }

        return Ok(tokens);
    }

    fn is_empty(&self) -> bool {
        return self.offset >= self.chars.len();
    }

    fn tokenize_one(&mut self) -> Result<Token, Error> {
        let c = self.peek(0)?;
        let tok = match c {
            'a'...'z' | 'A'...'Z' => self.tokenize_name_or_keyword(),
            '0'...'9' => self.tokenize_int_or_float(),
            '/' => self.tokenize_absolute_path_or_division(),
            '.' => self.tokenize_path(),
            '^' => self.tokenize_source(),
            '$' => self.tokenize_sink(),
            '!' => self.tokenize_use_template_or_not_eq(),
            '@' => self.tokenize_location(),
            '"' => self.tokenize_string(),
            '<' => self.tokenize_comes_from_or_less_than(),
            '>' => self.tokenize_greater_than(),
            '|' | '&' => self.tokenize_operator_2(),
            //'=' => tokens.push(self.tokenize_equals()?),
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
            '-' => {
                self.offset += 1;
                Ok(Token::Subtract)
            }
            '*' => {
                self.offset += 1;
                Ok(Token::Multiply)
            }
            '%' => {
                self.offset += 1;
                Ok(Token::Modulo)
            }
            _ => bail!(
                "tokenize error: expected a sigil or name, found: {}",
                self.chars[self.offset]
            ),
        }?;
        trace!("tokenize: {} => {:?}", c, tok);
        return Ok(tok);
    }

    fn tokenize_name_or_keyword(&mut self) -> Result<Token, Error> {
        let s = self.tokenize_identifier()?;
        if s == "true" {
            return Ok(Token::BooleanTerm(true));
        } else if s == "false" {
            return Ok(Token::BooleanTerm(false));
        } else {
            return Ok(Token::NameTerm(s));
        }
    }

    fn tokenize_int_or_float(&mut self) -> Result<Token, Error> {
        let start = self.offset;
        let mut contains_dot = false;
        while !self.is_empty() {
            match self.peek(0)? {
                '0'...'9' => self.offset += 1,
                '.' => {
                    self.offset += 1;
                    contains_dot = true;
                }
                _ => break,
            }
        }
        let s = self.chars[start..self.offset].iter().collect::<String>();
        if contains_dot {
            return Ok(Token::FloatTerm(Float::new(s.parse::<f64>()?)?));
        }
        return Ok(Token::IntegerTerm(s.parse::<i64>()?));
    }

    fn tokenize_source(&mut self) -> Result<Token, Error> {
        assert!(self.peek(0)? == '^');
        self.offset += 1;
        return Ok(Token::Source(self.tokenize_identifier()?));
    }

    fn tokenize_sink(&mut self) -> Result<Token, Error> {
        assert!(self.peek(0)? == '$');
        self.offset += 1;
        return Ok(Token::Sink(self.tokenize_identifier()?));
    }

    fn tokenize_absolute_path_or_division(&mut self) -> Result<Token, Error> {
        assert!(self.peek(0)? == '/');
        return match self.maybe_peek(1) {
            None | Some(' ') => self.tokenize_division(),
            _ => self.tokenize_path(),
        };
    }

    fn tokenize_division(&mut self) -> Result<Token, Error> {
        self.offset += 1;
        return Ok(Token::Divide);
    }

    fn tokenize_use_template_or_not_eq(&mut self) -> Result<Token, Error> {
        if self.peek(1)? == '=' {
            self.offset += 2;
            return Ok(Token::NotEquals);
        }
        self.offset += 1;
        return Ok(Token::UseTemplate(self.tokenize_identifier()?));
    }

    fn tokenize_location(&mut self) -> Result<Token, Error> {
        let start = self.offset;
        while !self.is_empty() {
            if self.chars[self.offset].is_whitespace() {
                break;
            }
            self.offset += 1;
        }
        let span = self.chars[start..self.offset].iter().collect::<String>();
        return Ok(Token::Location(Dimension2::from_str(&span)?));
    }

    fn tokenize_string(&mut self) -> Result<Token, Error> {
        assert!(self.peek(0)? == '"');
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
                c @ _ => out.push(c),
            }
            self.offset += 1;
        }
        bail!("tokenize error: unmatched \"");
    }

    fn tokenize_comes_from_or_less_than(&mut self) -> Result<Token, Error> {
        match self.maybe_peek(1) {
            None => {
                self.offset += 1;
                return Ok(Token::LessThan);
            }
            Some('-') => {
                if self.maybe_peek(2) == Some('\\') {
                    self.offset += 3;
                    return Ok(Token::ComesFromBlock);
                }
                self.offset += 2;
                return Ok(Token::ComesFromInline);
            }
            Some('=') => {
                self.offset += 2;
                return Ok(Token::LessThanOrEquals);
            }
            _ => {
                self.offset += 1;
                return Ok(Token::LessThan);
            }
        }
    }

    fn tokenize_greater_than(&mut self) -> Result<Token, Error> {
        assert!(self.peek(0)? == '>');
        if self.peek(1)? == '=' {
            return Ok(Token::GreaterThanOrEquals);
        }
        return Ok(Token::GreaterThan);
    }

    fn tokenize_operator_2(&mut self) -> Result<Token, Error> {
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
        return Ok(t);
    }

    fn tokenize_path(&mut self) -> Result<Token, Error> {
        // Note: this is identifier with [/.{}] included. It is up to the user to
        //       build a real, well-formed path from this.
        let start = self.offset;
        while !self.is_empty() {
            match self.peek(0)? {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '_' | '/' | '.' | '{' | '}' => {
                    self.offset += 1
                }
                _ => break,
            }
        }
        let content = self.chars[start..self.offset].iter().collect::<String>();
        return Ok(Token::PathTerm(content));
    }

    fn tokenize_identifier(&mut self) -> Result<String, Error> {
        let start = self.offset;
        while !self.is_empty() {
            match self.chars[self.offset] {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '_' => self.offset += 1,
                _ => break,
            }
        }
        return Ok(self.chars[start..self.offset].iter().collect::<String>());
    }

    fn maybe_peek(&self, n: usize) -> Option<char> {
        if self.offset + n < self.chars.len() {
            return Some(self.chars[self.offset + n]);
        }
        return None;
    }

    fn peek(&self, n: usize) -> Result<char, Error> {
        ensure!(
            self.offset + n < self.chars.len(),
            "tokenize error: out of input too soon"
        );
        return Ok(self.chars[self.offset + n]);
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
    use super::{Dimension2, Token, TreeTokenizer as TT};

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
                Token::Location(Dimension2::from_str("@1'1\"x2'2\"").unwrap()),
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
    }
}
