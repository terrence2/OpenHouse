// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    bif::NativeFunc,
    graph::Graph,
    path::{ConcretePath, ScriptPath},
    tokenizer::Token,
    tree::{NodeRef, Tree},
    value::{Value, ValueType},
};
use failure::{ensure, err_msg, Fallible};
use lazy_static::lazy_static;
use std::collections::HashMap;
use tracing::trace;

#[derive(Clone, Debug)]
pub(super) enum Expr {
    Add(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Call(Box<dyn NativeFunc>, Box<Expr>),
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
    Latch(Box<Expr>, Box<Expr>),
    Value(Value),
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
            Expr::Latch(a, b) => {
                $reduce(Token::Latch, a.$f($($args),*)?, b.$f($($args),*)?)
            }
            Expr::Value(v) => {
                v.$f($($args),*)
            }
        }
    };
}

impl Expr {
    pub fn compute(&self, tree: &Tree) -> Fallible<Value> {
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

    pub fn virtually_compute_for_path(&self, tree: &Tree) -> Fallible<Vec<Value>> {
        map_values!(
            self,
            virtually_compute_for_path,
            |tok, lhs: Vec<Value>, rhs: Vec<Value>| {
                trace!("vcomp: reduce {:?} {:?} {:?}", lhs, tok, rhs);
                let mut out = Vec::new();
                for a in &lhs {
                    for b in &rhs {
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
    ) -> Fallible<ValueType> {
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
    pub fn inline_from_tokens(
        path: String,
        tokens: &[Token],
        nifs: &HashMap<String, Box<dyn NativeFunc>>,
    ) -> Fallible<Self> {
        let mut parser = ExprParser::from_tokens(path, tokens, nifs);
        let expr = parser.eparser()?;
        let script = Script {
            suite: expr,
            phase: CompilationPhase::NeedInputMap,
            input_map: HashMap::new(),
            nodetype: None,
        };
        Ok(script)
    }

    // Note that we have to have a separate build and install phase because otherwise we'd be borrowed
    // mutable when searching for inputs and double-borrow if any children are referenced.
    pub fn build_input_map(
        &self,
        tree: &Tree,
    ) -> Fallible<(HashMap<ConcretePath, NodeRef>, ValueType)> {
        assert!(self.phase == CompilationPhase::NeedInputMap);
        let mut inputs = Vec::new();
        let ty = self.suite.find_all_possible_inputs(tree, &mut inputs)?;
        let mut input_map = HashMap::new();
        for input in inputs.drain(..) {
            let node = tree.lookup_path(&input)?;
            input_map.insert(input, node);
        }
        Ok((input_map, ty))
    }

    pub fn install_input_map(
        &mut self,
        input_map: (HashMap<ConcretePath, NodeRef>, ValueType),
    ) -> Fallible<()> {
        assert!(self.phase == CompilationPhase::NeedInputMap);
        self.input_map = input_map.0;
        self.nodetype = Some(input_map.1);
        self.phase = CompilationPhase::Ready;
        Ok(())
    }

    pub fn populate_flow_graph(&self, tgt_node: &NodeRef, graph: &mut Graph) -> Fallible<()> {
        for src_node in self.input_map.values() {
            graph.add_edge(src_node, tgt_node);
        }
        Ok(())
    }

    pub fn nodetype(&self) -> Fallible<ValueType> {
        ensure!(
            self.nodetype.is_some(),
            "typecheck error: querying node type before ready"
        );
        Ok(self.nodetype.unwrap())
    }

    pub(super) fn has_a_nodetype(&self) -> bool {
        self.nodetype.is_some()
    }

    pub(super) fn all_inputs(&self) -> Fallible<Vec<String>> {
        Ok(self
            .input_map
            .keys()
            .map(|concrete| concrete.to_string())
            .collect::<Vec<_>>())
    }

    pub(super) fn compute(&self, tree: &Tree) -> Fallible<Value> {
        ensure!(
            self.phase == CompilationPhase::Ready,
            "runtime error: attempting script usage before ready: {:?} => {:?}",
            self.phase,
            self.suite
        );
        self.suite.compute(tree)
    }

    pub(super) fn virtually_compute_for_path(&self, tree: &Tree) -> Fallible<Vec<Value>> {
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
        None
    }

    fn op(t: &Token, arity: usize) -> &Operator {
        Self::maybe_op(t, arity).expect("requested a non-existent operator.")
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
        v.push(Operator::new(Token::Latch, 8, 2, Some(Assoc::Left)));
        v
    };
}

struct ExprParser<'a> {
    path: String,
    tokens: &'a [Token],
    offset: usize,
    nifs: &'a HashMap<String, Box<dyn NativeFunc>>,
}

// Uses textbook precedence climbing.
impl<'a> ExprParser<'a> {
    fn from_tokens(
        path: String,
        tokens: &'a [Token],
        nifs: &'a HashMap<String, Box<dyn NativeFunc>>,
    ) -> Self {
        Self {
            path,
            tokens,
            offset: 0,
            nifs,
        }
    }

    fn eparser(&mut self) -> Fallible<Expr> {
        let e = self.exp_p(0)?;
        ensure!(
            self.tokens[self.offset..].iter().all(|t| [
                Token::Newline,
                Token::Indent,
                Token::Dedent
            ]
            .contains(t)),
            "parse error: extra non-whitespace tokens after script"
        );
        Ok(e)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.offset]
    }

    fn pop(&mut self) -> Token {
        let op = self.tokens[self.offset].clone();
        self.offset += 1;
        op
    }

    fn exp_p(&mut self, p: usize) -> Fallible<Expr> {
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
                Token::Latch => Expr::Latch(Box::new(t), Box::new(t1)),
                _ => panic!("unexpected token {:?} in binop position", op),
            };
        }

        Ok(t)
    }

    fn p(&mut self) -> Fallible<Expr> {
        Ok(match self.pop() {
            Token::BooleanTerm(b) => Expr::Value(Value::from_boolean(b)),
            Token::FloatTerm(f) => Expr::Value(Value::from_float(f)),
            Token::IntegerTerm(i) => Expr::Value(Value::from_integer(i)),
            Token::PathTerm(p) => Expr::Value(Value::from_path(ScriptPath::from_str_at_path(
                &self.path, &p,
            )?)),
            Token::StringTerm(s) => Expr::Value(Value::from_string(s)),
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
                let nif = self
                    .nifs
                    .get(&name)
                    .ok_or_else(|| err_msg(format!("parse error: no such function {}", name)))?
                    .clone();
                Expr::Call(nif, Box::new(t))
            }
            t => panic!("parse error: unexpected token {:?}", t),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{float::Float, tokenizer::TreeTokenizer, tree::TreeBuilder};

    fn do_compute(expr: &str) -> Fallible<Value> {
        let tok = TreeTokenizer::tokenize(&format!("a <- {}", expr))?;
        let mut script =
            Script::inline_from_tokens("/a".to_owned(), &tok[2..tok.len() - 1], &HashMap::new())?;
        let tree = TreeBuilder::empty();
        let input_map = script.build_input_map(&tree)?;
        ensure!(
            script.install_input_map(input_map).is_ok(),
            "typecheck failure"
        );
        script.compute(&tree)
    }

    #[test]
    fn test_script_basic() -> Fallible<()> {
        let expect = vec![
            ("2 + 3", Value::from_integer(5)),
            ("2 :: 3", Value::from_integer(2)),
            ("2. + 3.", Value::from_float(Float::new(5.0)?)),
            (r#" "2" + "3" "#, Value::new_str("23")),
            ("2 - 3", Value::from_integer(-1)),
            ("2. - 3.5", Value::from_float(Float::new(-1.5)?)),
            ("-2", Value::from_integer(-2)),
            ("2 - 3", Value::from_integer(-1)),
            ("2 / 3", Value::from_float(Float::new(2f64 / 3f64)?)),
        ];
        for (expr, value) in expect.iter() {
            assert_eq!(do_compute(expr)?, *value);
        }
        Ok(())
    }

    #[test]
    fn test_script_failures() -> Fallible<()> {
        let expect = vec!["1 + 2.", "true + false", r#" "2" - "3" "#];
        for expr in expect.iter() {
            assert!(do_compute(expr).is_err());
        }
        Ok(())
    }

    #[test]
    fn test_script_or() -> Fallible<()> {
        let tok = TreeTokenizer::tokenize("a <- true || true")?;
        ExprParser::from_tokens("/a".to_owned(), &tok[2..tok.len() - 1], &HashMap::new())
            .eparser()?;
        Ok(())
    }

    #[test]
    fn test_script_inputs() -> Fallible<()> {
        let tok = TreeTokenizer::tokenize("a <- /foo/bar/baz")?;
        ExprParser::from_tokens("/a".to_owned(), &tok[2..tok.len() - 1], &HashMap::new())
            .eparser()?;
        Ok(())
    }

    #[test]
    fn test_script_negate() -> Fallible<()> {
        let tok = TreeTokenizer::tokenize("a <- -/foo/bar/baz")?;
        ExprParser::from_tokens("/a".to_owned(), &tok[2..tok.len() - 1], &HashMap::new())
            .eparser()?;
        Ok(())
    }
}
