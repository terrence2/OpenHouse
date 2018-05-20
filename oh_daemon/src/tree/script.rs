// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Error;
use tree::{float::Float, tokenizer::{RawPath, Token}, tree::{NodeRef, Tree}};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathComponent {
    Lookup(ScriptPath),
    Part(String),
}

/// An absolute path, which may contain children which are themselves path lookups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptPath {
    parts: Vec<PathComponent>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Assoc {
    Left,
}

struct Operator {
    token: Token,
    precedence: usize,
    arity: usize,
    assoc: Option<Assoc>,
}

impl Operator {
    fn new(token: Token, precedence: usize, arity: usize, assoc: Option<Assoc>) -> Self {
        Operator {
            token,
            precedence,
            arity,
            assoc,
        }
    }

    fn maybe_op(t: &Token, arity: usize) -> Option<&Operator> {
        for i in OPERATORS.iter() {
            if t == &i.token && arity == i.arity {
                return Some(i);
            }
        }
        return None;
    }

    fn op(t: &Token, arity: usize) -> &Operator {
        return Self::maybe_op(t, arity).expect("requested a non-existent operator.");
    }

    fn precedence_of(t: &Token, arity: usize) -> usize {
        Self::op(t, arity).precedence
    }

    fn assoc_of(t: &Token) -> Assoc {
        Self::op(t, 2).assoc.unwrap()
    }

    fn is_bin_op(t: &Token) -> bool {
        Self::maybe_op(t, 2).is_some()
    }
}

lazy_static! {
    static ref OPERATORS: Vec<Operator> = {
        let mut v = Vec::new();
        v.push(Operator::new(Token::Divide, 15, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::Multiply, 15, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::Subtract, 14, 1, None));
        v.push(Operator::new(Token::Subtract, 13, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::Add, 13, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::GreaterThan, 12, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::LessThan, 12, 2, Some(Assoc::Left)));
        v.push(Operator::new(
            Token::GreaterThanOrEquals,
            12,
            2,
            Some(Assoc::Left),
        ));
        v.push(Operator::new(
            Token::LessThanOrEquals,
            12,
            2,
            Some(Assoc::Left),
        ));
        v.push(Operator::new(Token::Equals, 11, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::NotEquals, 11, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::And, 10, 2, Some(Assoc::Left)));
        v.push(Operator::new(Token::Or, 9, 2, Some(Assoc::Left)));
        v
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Boolean,
    Float,
    Integer,
    String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Boolean(bool),
    Float(Float),
    Integer(i64),
    Path(ScriptPath),
    String(String),
}

impl Value {
    pub fn add(&self, other: &Value, t: &Tree) -> Result<Value, Error> {
        ensure!(
            self.value_type(t)? == other.value_type(t)?,
            "mismatched types"
        );
        bail!("not implemented")
    }

    pub fn value_type(&self, t: &Tree) -> Result<ValueType, Error> {
        Ok(match self {
            Value::Boolean(_) => ValueType::Boolean,
            Value::Float(_) => ValueType::Float,
            Value::Integer(_) => ValueType::Integer,
            Value::String(_) => ValueType::String,
            //Value::Path(p) => t.lookup(p)?.value_type(t),
            _ => bail!("not implemented"),
        })
    }

    pub fn as_boolean(&self) -> Result<bool, Error> {
        if let Value::Boolean(b) = self {
            return Ok(*b);
        }
        bail!("compute error: value is not boolean")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Add(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Divide(Box<Expr>, Box<Expr>),
    Equal(Box<Expr>, Box<Expr>),
    GreaterThan(Box<Expr>, Box<Expr>),
    GreaterThanOrEqual(Box<Expr>, Box<Expr>),
    LessThan(Box<Expr>, Box<Expr>),
    LessThanOrEqual(Box<Expr>, Box<Expr>),
    Modulo(Box<Expr>, Box<Expr>),
    Multiply(Box<Expr>, Box<Expr>),
    Negate(Box<Expr>),
    NotEqual(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Subtract(Box<Expr>, Box<Expr>),
    Value(Value),
}

impl Expr {
    pub fn compute(&self, t: &Tree) -> Result<Value, Error> {
        Ok(match self {
            //Expr::Add(a, b) => a.compute(t)?.add(b.compute(t)?, t),
            //Expr::And(a, b) =>
            _ => Value::Boolean(false),
        })
    }

    pub fn find_all_inputs(&self, out: &mut Vec<ScriptPath>) -> Result<(), Error> {
        Ok(match self {
            Expr::Add(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::And(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Divide(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Equal(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::GreaterThan(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::GreaterThanOrEqual(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::LessThan(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::LessThanOrEqual(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Modulo(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Multiply(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Negate(a) => {
                a.find_all_inputs(out)?;
            }
            Expr::NotEqual(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Or(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Subtract(a, b) => {
                a.find_all_inputs(out)?;
                b.find_all_inputs(out)?;
            }
            Expr::Value(v) => match v {
                Value::Path(p) => out.push(p.to_owned()),
                _ => {}
            },
        })
    }
}

struct ExprParser<'a> {
    tokens: &'a [Token],
    offset: usize,
    node: NodeRef,
}

// Uses textbook precedence climbing.
impl<'a> ExprParser<'a> {
    fn from_tokens(tokens: &'a [Token], node: &NodeRef) -> Self {
        return Self {
            tokens: tokens,
            offset: 0,
            node: node.to_owned(),
        };
    }

    fn eparser(&mut self) -> Result<Expr, Error> {
        let e = self.exp_p(0)?;
        ensure!(
            self.offset + 1 == self.tokens.len(),
            "parse error: extra tokens after script"
        );
        return Ok(e);
    }

    fn peek(&self) -> &Token {
        return &self.tokens[self.offset];
    }

    fn pop(&mut self) -> Token {
        let op = self.tokens[self.offset].clone();
        self.offset += 1;
        return op;
    }

    fn exp_p(&mut self, p: usize) -> Result<Expr, Error> {
        let mut t = self.p()?;
        while self.offset < self.tokens.len() && Operator::is_bin_op(&self.tokens[self.offset])
            && Operator::precedence_of(self.peek(), 2) >= p
        {
            let op = self.pop();
            let q = match Operator::assoc_of(&op) {
                Assoc::Left => Operator::precedence_of(&op, 2) + 1,
                //Assoc::Right => Operator::precedence_of(&op, 2),
            };
            let t1 = self.exp_p(q)?;
            t = match op {
                Token::Add => Expr::Add(Box::new(t), Box::new(t1)),
                Token::And => Expr::And(Box::new(t), Box::new(t1)),
                Token::Divide => Expr::Divide(Box::new(t), Box::new(t1)),
                Token::Equals => Expr::Equal(Box::new(t), Box::new(t1)),
                Token::GreaterThan => Expr::GreaterThan(Box::new(t), Box::new(t1)),
                Token::GreaterThanOrEquals => Expr::GreaterThanOrEqual(Box::new(t), Box::new(t1)),
                Token::LessThan => Expr::LessThan(Box::new(t), Box::new(t1)),
                Token::LessThanOrEquals => Expr::LessThanOrEqual(Box::new(t), Box::new(t1)),
                Token::Modulo => Expr::Modulo(Box::new(t), Box::new(t1)),
                Token::Multiply => Expr::Multiply(Box::new(t), Box::new(t1)),
                Token::NotEquals => Expr::NotEqual(Box::new(t), Box::new(t1)),
                Token::Or => Expr::Or(Box::new(t), Box::new(t1)),
                Token::Subtract => Expr::Subtract(Box::new(t), Box::new(t1)),
                _ => panic!("unexpected token {:?} in binop position", op),
            };
        }

        return Ok(t);
    }

    fn p(&mut self) -> Result<Expr, Error> {
        return Ok(match self.pop() {
            Token::BooleanTerm(b) => Expr::Value(Value::Boolean(b)),
            Token::FloatTerm(f) => Expr::Value(Value::Float(f)),
            Token::IntegerTerm(i) => Expr::Value(Value::Integer(i)),
            Token::PathTerm(rel_path) => Expr::Value(Value::Path(self.node.realpath(&rel_path)?)),
            Token::StringTerm(s) => Expr::Value(Value::String(s)),
            Token::LeftParen => {
                let t = self.exp_p(0)?;
                ensure!(
                    *self.peek() == Token::RightParen,
                    "parse error: expected right paren after sub-expression"
                );
                t
            }
            t => panic!("unexpected token {:?} in parser", t),
        });
    }
}

/// The code embedded under a comes-from (<- or <-\) operator in the tree.
#[derive(Debug)]
pub struct Script {
    suite: Expr,
    pub inputs: Vec<ScriptPath>,
}

impl Script {
    pub fn inline_from_tokens(tokens: &[Token], node: &NodeRef) -> Result<Self, Error> {
        let mut parser = ExprParser::from_tokens(tokens, node);
        let expr = parser.eparser()?;
        let mut inputs = Vec::new();
        expr.find_all_inputs(&mut inputs)?;
        return Ok(Script {
            suite: expr,
            inputs,
        });
    }

    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        self.suite.compute(tree)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tree::tokenizer::TreeTokenizer;

    #[test]
    fn test_script_add() {
        let tok = TreeTokenizer::tokenize("a <- 2 + 2").unwrap();
        let tree = Tree::new();
        let script = Script::inline_from_tokens(&tok[2..], &tree.root()).unwrap();
        //assert_eq!(script.compute(&Tree::new()).unwrap(), Value::Integer(4));
    }

    #[test]
    fn test_script_or() {
        let tok = TreeTokenizer::tokenize("a <- true || true").unwrap();
        let tree = Tree::new();
        ExprParser::from_tokens(&tok[2..], &tree.root())
            .eparser()
            .unwrap();
    }
}
