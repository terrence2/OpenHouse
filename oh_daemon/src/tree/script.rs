// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Error;
use std::collections::HashMap;
use tree::{float::Float, path::ScriptPath, tokenizer::Token, tree::{NodeRef, Tree}};

bitflags! {
    pub struct ValueType : usize {
        const BOOLEAN = 0b0001;
        const FLOAT   = 0b0010;
        const INTEGER = 0b0011;
        const STRING  = 0b0100;
    }
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
        let result = match self {
            Value::Integer(i0) => Value::Integer(i0 + other.as_integer()?),
            Value::Float(f0) => Value::Float(f0.checked_add(&other.as_float()?)?),
            Value::String(s0) => Value::String(format!("{}{}", s0, other.as_string()?)),
            Value::Boolean(_) => bail!("runtime error: attempt to add two boolean values"),
            Value::Path(p) => t.lookup_path(p)?.compute(t)?,
        };
        return Ok(result);
    }

    pub fn subtract(&self, other: &Value, t: &Tree) -> Result<Value, Error> {
        let result = match self {
            Value::Integer(i0) => Value::Integer(i0 - other.as_integer()?),
            Value::Float(f0) => Value::Float(f0.checked_sub(&other.as_float()?)?),
            Value::String(_) => bail!("runtime error: attempt to subtract two string values"),
            Value::Boolean(_) => bail!("runtime error: attempt to subtract two boolean values"),
            Value::Path(p) => t.lookup_path(p)?.compute(t)?,
        };
        return Ok(result);
    }

    fn typecheck(&self, input_map: &HashMap<ScriptPath, NodeRef>) -> Result<ValueType, Error> {
        Ok(match self {
            Value::Boolean(_) => ValueType::BOOLEAN,
            Value::Float(_) => ValueType::FLOAT,
            Value::Integer(_) => ValueType::INTEGER,
            Value::String(_) => ValueType::STRING,
            Value::Path(p) => {
                let noderef = &input_map[p];
                noderef.nodetype()?
            }
        })
    }

    pub fn as_boolean(&self) -> Result<bool, Error> {
        if let Value::Boolean(b) = self {
            return Ok(*b);
        }
        bail!("runtime error: attempted to use a non-boolean value in boolean context")
    }

    pub fn as_integer(&self) -> Result<i64, Error> {
        if let Value::Integer(i) = self {
            return Ok(*i);
        }
        bail!("runtime error: attempted to use a non-integer value in integer context")
    }

    pub fn as_float(&self) -> Result<Float, Error> {
        if let Value::Float(f) = self {
            return Ok(*f);
        }
        bail!("runtime error: attempted to use a non-float value in float context")
    }

    pub fn as_string(&self) -> Result<String, Error> {
        if let Value::String(s) = self {
            return Ok(s.to_owned());
        }
        bail!("runtime error: attempted to use a non-stringvalue in string context")
    }

    pub fn as_path_component(&self) -> Result<String, Error> {
        match self {
            Value::Integer(i) => Ok(i.to_string()),
            Value::Boolean(b) => Ok(b.to_string()),
            Value::String(s) => Ok(s.to_owned()),
            Value::Float(_) => {
                bail!("runtime error: a float value cannot be used as a path component")
            }
            Value::Path(_) => bail!("runtime error: did not expect a path as path component"),
        }
    }

    pub fn find_all_inputs(&self, out: &mut Vec<ScriptPath>) -> Result<(), Error> {
        match self {
            Value::Path(sp) => out.push(sp.clone()),
            _ => {}
        }
        return Ok(());
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

macro_rules! map_values {
    ($self:ident, $f:ident, $reduce:expr, $($args:ident),*) => {
        match $self {
            Expr::Add(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::And(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Divide(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Equal(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::GreaterThan(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::GreaterThanOrEqual(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::LessThan(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::LessThanOrEqual(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Modulo(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Multiply(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Negate(a) => {
                a.$f($($args),*)
            }
            Expr::NotEqual(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Or(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Subtract(a, b) => {
                $reduce(a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Value(v) => {
                v.$f($($args),*)
            }
        }
    };
}

impl Expr {
    pub fn compute(&self, t: &Tree) -> Result<Value, Error> {
        println!("in expr: {:?}", self);
        Ok(match self {
            Expr::Value(v) => v.to_owned(),
            Expr::Add(a, b) => a.compute(t)?.add(&b.compute(t)?, t)?,
            Expr::Subtract(a, b) => a.compute(t)?.subtract(&b.compute(t)?, t)?,
            // Expr::Multiply(a, b) => a.compute(t)?.multiply(&b.compute(t)?, t)?,
            // Expr::Divide(a, b) => a.compute(t)?.divide(&b.compute(t)?, t)?,
            //Expr::And(a, b) =>
            _ => bail!("not impl"),
        })
    }

    pub fn find_all_inputs(&self, out: &mut Vec<ScriptPath>) -> Result<(), Error> {
        map_values!(self, find_all_inputs, |_a, _b| Ok(()), out)
    }

    pub fn typecheck(&self, input_map: &HashMap<ScriptPath, NodeRef>) -> Result<ValueType, Error> {
        map_values!(
            self,
            typecheck,
            |a, b| {
                ensure!(a == b, "type check failure: mismatched types in {:?}", self);
                Ok(a)
            },
            input_map
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CompilationPhase {
    NeedInputMap,
    NeedTypeCheck,
    Ready,
}

/// The code embedded under a comes-from (<- or <-\) operator in the tree.
#[derive(Debug)]
pub struct Script {
    suite: Expr,
    phase: CompilationPhase,
    input_map: HashMap<ScriptPath, NodeRef>,
    produces_type: Option<ValueType>,
}

impl Script {
    pub fn inline_from_tokens(path: String, tokens: &[Token]) -> Result<Self, Error> {
        let mut parser = ExprParser::from_tokens(path, tokens);
        let expr = parser.eparser()?;
        let script = Script {
            suite: expr,
            phase: CompilationPhase::NeedInputMap,
            input_map: HashMap::new(),
            produces_type: None,
        };
        return Ok(script);
    }

    // Note that we have to have a separate build and install phase because otherwise we'd be borrowed
    // mutable when searching for inputs and double-borrow if any children are referenced.
    pub fn build_input_map(&self, tree: &Tree) -> Result<HashMap<ScriptPath, NodeRef>, Error> {
        assert!(self.phase == CompilationPhase::NeedInputMap);
        let mut inputs = Vec::new();
        self.suite.find_all_inputs(&mut inputs)?;
        let mut input_map = HashMap::new();
        for input in inputs.drain(..) {
            let node = tree.lookup_path(&input)?;
            input_map.insert(input, node);
        }
        return Ok(input_map);
    }

    pub fn install_input_map(
        &mut self,
        input_map: HashMap<ScriptPath, NodeRef>,
    ) -> Result<(), Error> {
        assert!(self.phase == CompilationPhase::NeedInputMap);
        self.input_map = input_map;
        self.phase = CompilationPhase::NeedTypeCheck;
        return Ok(());
    }

    pub fn typecheck(&mut self) -> Result<ValueType, Error> {
        assert!(
            self.phase == CompilationPhase::NeedTypeCheck || self.phase == CompilationPhase::Ready
        );
        if self.produces_type.is_none() {
            self.produces_type = Some(self.suite.typecheck(&self.input_map)?);
            self.phase = CompilationPhase::Ready;
        }
        return Ok(self.produces_type.unwrap());
    }

    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        ensure!(
            self.phase == CompilationPhase::Ready,
            "runtime error: attempting script usage before ready: {:?}",
            self.phase
        );
        self.suite.compute(tree)
    }
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

struct ExprParser<'a> {
    path: String,
    tokens: &'a [Token],
    offset: usize,
}

// Uses textbook precedence climbing.
impl<'a> ExprParser<'a> {
    fn from_tokens(path: String, tokens: &'a [Token]) -> Self {
        return Self {
            path,
            tokens: tokens,
            offset: 0,
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
            Token::PathTerm(p) => {
                Expr::Value(Value::Path(ScriptPath::from_str_at_path(&self.path, &p)?))
            }
            Token::StringTerm(s) => Expr::Value(Value::String(s)),
            Token::LeftParen => {
                let t = self.exp_p(0)?;
                ensure!(
                    *self.peek() == Token::RightParen,
                    "parse error: expected right paren after sub-expression"
                );
                t
            }
            t => panic!("parse error: unexpected token {:?}", t),
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tree::tokenizer::TreeTokenizer;

    fn do_compute(expr: &str) -> Result<Value, Error> {
        let tok = TreeTokenizer::tokenize(&format!("a <- {}", expr)).unwrap();
        let mut script = Script::inline_from_tokens("/a".to_owned(), &tok[2..]).unwrap();
        let tree = Tree::new();
        let input_map = script.build_input_map(&tree).unwrap();
        ensure!(
            script.install_input_map(input_map).is_ok(),
            "typecheck failure"
        );
        ensure!(script.typecheck().is_ok(), "typecheck failure");
        return script.compute(&Tree::new());
    }

    #[test]
    fn test_script_basic() {
        let expect = vec![
            ("2 + 3", Value::Integer(5)),
            ("2. + 3.", Value::Float(Float::new(5.0).unwrap())),
            (r#" "2" + "3" "#, Value::String("23".to_owned())),
            ("2 - 3", Value::Integer(-1)),
            ("2. - 3.5", Value::Float(Float::new(-1.5).unwrap())),
            // ("2 * 3", Value::Integer(6)),
            // ("2 - 3", Value::Integer(-1)),
            // ("2 / 3", Value::Integer(0)),
        ];
        for (expr, value) in expect.iter() {
            assert_eq!(do_compute(expr).unwrap(), *value);
        }
    }

    #[test]
    fn test_script_failures() {
        let expect = vec!["1 + 2.", "true + false", r#" "2" - "3" "#];
        for expr in expect.iter() {
            assert!(do_compute(expr).is_err());
        }
    }

    #[test]
    fn test_script_or() {
        let tok = TreeTokenizer::tokenize("a <- true || true").unwrap();
        ExprParser::from_tokens("/a".to_owned(), &tok[2..])
            .eparser()
            .unwrap();
    }

    // #[test]
    // fn test_script_inputs() {
    //     let tok = TreeTokenizer::tokenize("a <- /foo/bar/baz").unwrap();
    //     let tree = Tree::new();
    //     ExprParser::from_tokens(&tok[2..], &tree.root())
    //         .eparser()
    //         .unwrap();
    // }
}
