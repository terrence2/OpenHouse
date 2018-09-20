// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{Error, Fallible};
use graph::Graph;
use path::{ConcretePath, ScriptPath};
use std::{collections::HashMap, fmt};
use tokenizer::Token;
use tree::{NodeRef, Tree};
use value::{Value, ValueType};

#[derive(Clone, Debug)]
pub(super) enum Expr {
    Add(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Call(Box<BuiltinFunc>, Box<Expr>),
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

pub(super) trait BuiltinFunc {
    fn compute(&self, value: Value, tree: &Tree) -> Fallible<Value>;
    fn virtually_compute_for_path(&self, values: Vec<Value>, tree: &Tree) -> Fallible<Vec<Value>>;
    fn find_all_possible_inputs(
        &self,
        value_type: ValueType,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<ValueType>;
    fn box_clone(&self) -> Box<BuiltinFunc>;
}

impl Clone for Box<BuiltinFunc> {
    fn clone(&self) -> Box<BuiltinFunc> {
        self.box_clone()
    }
}

impl fmt::Debug for Box<BuiltinFunc> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TEST")
    }
}

#[derive(Clone, Debug)]
pub(super) struct ToStr;

impl BuiltinFunc for ToStr {
    fn compute(&self, value: Value, tree: &Tree) -> Fallible<Value> {
        Ok(Value::String(match value {
            Value::String(s) => s,
            Value::Integer(i) => format!("{}", i),
            Value::Float(f) => format!("{}", f),
            Value::Boolean(b) => format!("{}", b),
            Value::Path(p) => {
                let noderef = tree.lookup_dynamic_path(&p)?;
                self.compute(noderef.compute(tree)?, tree)?.as_string()?
            }
        }))
    }

    fn virtually_compute_for_path(&self, values: Vec<Value>, tree: &Tree) -> Fallible<Vec<Value>> {
        let mut results = Vec::new();
        for v in values {
            results.push(self.compute(v, tree)?);
        }
        return Ok(results);
    }

    fn find_all_possible_inputs(
        &self,
        value_type: ValueType,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<ValueType> {
        Ok(ValueType::STRING)
    }

    fn box_clone(&self) -> Box<BuiltinFunc> {
        Box::new((*self).clone())
    }
}

enum Builtins {
    ToStr,
}

impl Builtins {
    fn from_name(name: &str) -> Fallible<Box<BuiltinFunc>> {
        Ok(match name {
            "str" => Box::new(ToStr {}),
            _ => bail!("parse error: unknown builtin function {}", name),
        })
    }
}

macro_rules! map_values {
    ($self:ident, $f:ident, $reduce:expr, $($args:ident),*) => {
        match $self {
            Expr::Add(a, b) => {
                $reduce(Token::Add, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::And(a, b) => {
                $reduce(Token::And, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Call(fun, a) => {
                fun.$f(a.$f($($args),*)?, $($args),*)
            }
            Expr::Divide(a, b) => {
                $reduce(Token::Divide, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Equal(a, b) => {
                $reduce(Token::Equals, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::GreaterThan(a, b) => {
                $reduce(Token::GreaterThan, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::GreaterThanOrEqual(a, b) => {
                $reduce(Token::GreaterThanOrEquals, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::LessThan(a, b) => {
                $reduce(Token::LessThan, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::LessThanOrEqual(a, b) => {
                $reduce(Token::LessThanOrEquals, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Modulo(a, b) => {
                $reduce(Token::Modulo, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Multiply(a, b) => {
                $reduce(Token::Multiply, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Negate(a) => {
                a.$f($($args),*)
            }
            Expr::NotEqual(a, b) => {
                $reduce(Token::NotEquals, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Or(a, b) => {
                $reduce(Token::Or, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Subtract(a, b) => {
                $reduce(Token::Subtract, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Value(v) => {
                v.$f($($args),*)
            }
        }
    };
}

impl Expr {
    pub fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        map_values!(
            self,
            compute,
            |tok, lhs: Value, rhs: Value| {
                trace!("compute: reduce {:?} {:?} {:?}", lhs, tok, rhs);
                lhs.apply(&tok, &rhs)
            },
            tree
        )
    }

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Result<Vec<Value>, Error> {
        map_values!(
            self,
            virtually_compute_for_path,
            |tok, lhs: Vec<Value>, rhs: Vec<Value>| {
                trace!("vcomp: reduce {:?} {:?} {:?}", lhs, tok, rhs);
                let mut out = Vec::new();
                for a in lhs.iter() {
                    for b in rhs.iter() {
                        trace!("vcomp: reduce1 {:?} {:?} {:?}", a, tok, b);
                        out.push(a.apply(&tok, b)?);
                    }
                }
                Ok(out)
            },
            tree
        )
    }

    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Result<ValueType, Error> {
        trace!("Expr::find_all_possible_inputs({:?})", self);
        map_values!(
            self,
            find_all_possible_inputs,
            |_tok, a, b| {
                ensure!(a == b, "type check failure: mismatched types in {:?}", self);
                Ok(a)
            },
            tree,
            out
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CompilationPhase {
    NeedInputMap,
    Ready,
}

/// The code embedded under a comes-from (<- or <-\) operator in the tree.
pub struct Script {
    suite: Expr,
    phase: CompilationPhase,
    input_map: HashMap<ConcretePath, NodeRef>,
    nodetype: Option<ValueType>,
}

impl Script {
    pub fn inline_from_tokens(path: String, tokens: &[Token]) -> Result<Self, Error> {
        let mut parser = ExprParser::from_tokens(path, tokens);
        let expr = parser.eparser()?;
        let script = Script {
            suite: expr,
            phase: CompilationPhase::NeedInputMap,
            input_map: HashMap::new(),
            nodetype: None,
        };
        return Ok(script);
    }

    // Note that we have to have a separate build and install phase because otherwise we'd be borrowed
    // mutable when searching for inputs and double-borrow if any children are referenced.
    pub fn build_input_map(
        &self,
        tree: &Tree,
    ) -> Result<(HashMap<ConcretePath, NodeRef>, ValueType), Error> {
        assert!(self.phase == CompilationPhase::NeedInputMap);
        let mut inputs = Vec::new();
        let ty = self.suite.find_all_possible_inputs(tree, &mut inputs)?;
        let mut input_map = HashMap::new();
        for input in inputs.drain(..) {
            let node = tree.lookup_path(&input)?;
            input_map.insert(input, node);
        }
        return Ok((input_map, ty));
    }

    pub fn install_input_map(
        &mut self,
        input_map: (HashMap<ConcretePath, NodeRef>, ValueType),
    ) -> Result<(), Error> {
        assert!(self.phase == CompilationPhase::NeedInputMap);
        self.input_map = input_map.0;
        self.nodetype = Some(input_map.1);
        self.phase = CompilationPhase::Ready;
        return Ok(());
    }

    pub fn populate_flow_graph(&self, tgt_node: &NodeRef, graph: &mut Graph) -> Result<(), Error> {
        for src_node in self.input_map.values() {
            graph.add_edge(src_node, tgt_node);
        }
        return Ok(());
    }

    pub fn nodetype(&self) -> Result<ValueType, Error> {
        ensure!(
            self.nodetype.is_some(),
            "typecheck error: querying node type before ready"
        );
        return Ok(self.nodetype.unwrap());
    }

    pub(super) fn has_a_nodetype(&self) -> bool {
        return self.nodetype.is_some();
    }

    pub(super) fn all_inputs(&self) -> Result<Vec<String>, Error> {
        return Ok(self
            .input_map
            .keys()
            .map(|concrete| concrete.to_string())
            .collect::<Vec<_>>());
    }

    pub(super) fn compute(&self, tree: &Tree) -> Result<Value, Error> {
        ensure!(
            self.phase == CompilationPhase::Ready,
            "runtime error: attempting script usage before ready: {:?} => {:?}",
            self.phase,
            self.suite
        );
        self.suite.compute(tree)
    }

    pub(super) fn virtually_compute_for_path(&self, tree: &Tree) -> Result<Vec<Value>, Error> {
        self.suite.virtually_compute_for_path(tree)
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
        v.push(Operator::new(Token::Modulo, 15, 2, Some(Assoc::Left)));
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
            self.offset == self.tokens.len(),
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
        while self.offset < self.tokens.len()
            && Operator::is_bin_op(&self.tokens[self.offset])
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
                    self.pop() == Token::RightParen,
                    "parse error: expected right paren after sub-expression"
                );
                t
            }
            Token::Subtract => {
                let op = Operator::op(&Token::Subtract, 1);
                let q = op.precedence;
                let t = self.exp_p(q)?;
                Expr::Negate(Box::new(t))
            }
            Token::NameTerm(name) => {
                ensure!(
                    self.pop() == Token::LeftParen,
                    "parse error: expected () in call to {}",
                    name
                );
                let t = self.exp_p(0)?;
                ensure!(
                    self.pop() == Token::RightParen,
                    "parse error: expected right paren after call to {}",
                    name
                );
                Expr::Call(Builtins::from_name(&name)?, Box::new(t))
            }
            t => panic!("parse error: unexpected token {:?}", t),
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use float::Float;
    use tokenizer::TreeTokenizer;

    fn do_compute(expr: &str) -> Result<Value, Error> {
        let tok = TreeTokenizer::tokenize(&format!("a <- {}", expr))?;
        let mut script = Script::inline_from_tokens("/a".to_owned(), &tok[2..tok.len() - 1])?;
        let tree = Tree::new_empty();
        let input_map = script.build_input_map(&tree)?;
        ensure!(
            script.install_input_map(input_map).is_ok(),
            "typecheck failure"
        );
        return script.compute(&tree);
    }

    #[test]
    fn test_script_basic() {
        let expect = vec![
            ("2 + 3", Value::Integer(5)),
            ("2. + 3.", Value::Float(Float::new(5.0).unwrap())),
            (r#" "2" + "3" "#, Value::String("23".to_owned())),
            ("2 - 3", Value::Integer(-1)),
            ("2. - 3.5", Value::Float(Float::new(-1.5).unwrap())),
            ("-2", Value::Integer(-2)),
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
        ExprParser::from_tokens("/a".to_owned(), &tok[2..tok.len() - 1])
            .eparser()
            .unwrap();
    }

    #[test]
    fn test_script_inputs() {
        let tok = TreeTokenizer::tokenize("a <- /foo/bar/baz").unwrap();
        ExprParser::from_tokens("/a".to_owned(), &tok[2..tok.len() - 1])
            .eparser()
            .unwrap();
    }

    #[test]
    fn test_script_negate() {
        let tok = TreeTokenizer::tokenize("a <- -/foo/bar/baz").unwrap();
        ExprParser::from_tokens("/a".to_owned(), &tok[2..tok.len() - 1])
            .eparser()
            .unwrap();
    }
}
