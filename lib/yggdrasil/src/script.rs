// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    bif::NativeFunc,
    graph::Graph,
    parser::TreeParser,
    path::{ConcretePath, ScriptPath},
    tokenizer::Token,
    tree::{NodeRef, Tree},
    value::Value,
};
use failure::{bail, ensure, err_msg, Fallible};
use lazy_static::lazy_static;
use std::collections::HashMap;
use tracing::trace;

#[derive(Clone, Debug)]
pub(super) enum Expr {
    Add(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Call(Box<dyn NativeFunc + Send + Sync>, Box<Expr>),
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

    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<()> {
        trace!("Expr::find_all_possible_inputs({:?})", self);
        map_values!(
            self,
            find_all_possible_inputs,
            |_tok, _a, _b| Ok(()),
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

#[derive(Debug)]
struct IfStatement {
    cases: Vec<(Option<Expr>, Script)>,
}

impl IfStatement {
    fn new(cases: Vec<(Option<Expr>, Script)>) -> Self {
        Self { cases }
    }

    pub fn compute(&self, tree: &Tree) -> Fallible<Value> {
        for (expr, stmt) in &self.cases {
            if let Some(e) = expr {
                let cond = e.compute(tree)?;
                ensure!(cond.is_boolean(), "if statement conditions must be boolean");
                if cond == Value::from_boolean(true) {
                    return Ok(stmt.suite.compute(tree)?);
                }
            } else {
                return Ok(stmt.compute(tree)?);
            }
        }
        bail!("reached end of if conditions without at statement")
    }

    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<()> {
        for (expr, stmt) in &self.cases {
            if let Some(e) = expr {
                e.find_all_possible_inputs(tree, out)?;
            }
            stmt.suite.find_all_possible_inputs(tree, out)?;
        }
        Ok(())
    }

    fn mark_ready(&mut self) {
        for (_, stmt) in self.cases.iter_mut() {
            stmt.mark_ready();
        }
    }
}

#[derive(Debug)]
enum Stmt {
    ExprStmt(Expr),
    IfStmt(IfStatement),
}

impl Stmt {
    pub fn compute(&self, tree: &Tree) -> Fallible<Value> {
        match self {
            Self::ExprStmt(e) => e.compute(tree),
            Self::IfStmt(s) => s.compute(tree),
        }
    }

    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<()> {
        match self {
            Self::ExprStmt(e) => e.find_all_possible_inputs(tree, out),
            Self::IfStmt(s) => s.find_all_possible_inputs(tree, out),
        }
    }

    fn mark_ready(&mut self) {
        match self {
            Self::ExprStmt(_) => {},
            Self::IfStmt(s) => s.mark_ready(),
        }
    }
}

/// The code embedded under a comes-from (<- or <-\) operator in the tree.
#[derive(Debug)]
pub struct Script {
    suite: Stmt,
    phase: CompilationPhase,
    input_map: HashMap<ConcretePath, NodeRef>,
}

impl Script {
    pub fn inline_from_tokens(
        path: String,
        tokens: &[Token],
        nifs: &HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
    ) -> Fallible<Self> {
        let mut parser = ExprParser::from_tokens(path, tokens, nifs);
        let expr = parser.eparser()?;
        let script = Script {
            suite: Stmt::ExprStmt(expr),
            phase: CompilationPhase::NeedInputMap,
            input_map: HashMap::new(),
        };
        Ok(script)
    }

    pub fn block_from_tokens(
        path: String,
        tokens: &[Token],
        nifs: &HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
    ) -> Fallible<Self> {
        match tokens[0].maybe_name() {
            Some("if") => Self::if_from_tokens(path, tokens, nifs),
            _ => {
                let mut parser = ExprParser::from_tokens(path, tokens, nifs);
                let expr = parser.eparser()?;
                let script = Script {
                    suite: Stmt::ExprStmt(expr),
                    phase: CompilationPhase::NeedInputMap,
                    input_map: HashMap::new(),
                };
                Ok(script)
            }
        }
    }

    fn find_token(tokens: &[Token], end_token: &Token) -> Fallible<usize> {
        for (i, token) in tokens.iter().enumerate() {
            if token == end_token {
                return Ok(i);
            }
        }
        bail!("did not find requested token: {:?}", end_token)
    }

    fn find_start_of_block(tokens: &[Token]) -> Fallible<usize> {
        Self::find_token(tokens, &Token::StartOfBlock)
    }

    fn if_from_tokens(
        path: String,
        tokens: &[Token],
        nifs: &HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
    ) -> Fallible<Self> {
        let mut cases: Vec<(Option<Expr>, Script)> = Vec::new();

        // if and block
        let cond_end = Self::find_start_of_block(tokens)?;
        let condition_tokens = &tokens[1..cond_end];
        let if_condition = ExprParser::from_tokens(path.clone(), condition_tokens, nifs).eparser()?;
        ensure!(tokens[cond_end + 0] == Token::StartOfBlock, "expect SOB");
        ensure!(tokens[cond_end + 1] == Token::Newline, "expect newline");
        ensure!(tokens[cond_end + 2] == Token::Indent, "expect indent");
        let cond_end = cond_end + 3;
        let block_end = cond_end + TreeParser::find_matching_dedent(&tokens[cond_end..]);
        let block_tokens = &tokens[cond_end..block_end];
        let block_script = Script::block_from_tokens(path.clone(), block_tokens, nifs)?;
        cases.push((Some(if_condition), block_script));

        // Elifs and blocks
        let mut offset = block_end;
        while offset < tokens.len() && tokens[offset].maybe_name() == Some("elif") {
            let cond_end = offset + 1 + Self::find_start_of_block(&tokens[offset + 1..])?;
            let condition_tokens = &tokens[offset + 1..cond_end];
            let if_condition = ExprParser::from_tokens(path.clone(), condition_tokens, nifs).eparser()?;
            ensure!(tokens[cond_end + 0] == Token::StartOfBlock, "expect SOB");
            ensure!(tokens[cond_end + 1] == Token::Newline, "expect newline");
            ensure!(tokens[cond_end + 2] == Token::Indent, "expect indent");
            let cond_end = cond_end + 3;
            let block_end = cond_end + TreeParser::find_matching_dedent(&tokens[cond_end..]);
            let block_tokens = &tokens[cond_end..block_end];
            let block_script = Script::block_from_tokens(path.clone(), block_tokens, nifs)?;
            cases.push((Some(if_condition), block_script));
            offset = block_end;
        }

        ensure!(tokens[offset].maybe_name() == Some("else"), "if statements must have an else block");
        offset += 1;
        ensure!(tokens[offset + 0] == Token::StartOfBlock, "expect SOB");
        ensure!(tokens[offset + 1] == Token::Newline, "expect newline");
        ensure!(tokens[offset + 2] == Token::Indent, "expect indent");
        offset += 3;
        let block_end = offset + TreeParser::find_matching_dedent(&tokens[offset..]);
        let block_tokens = &tokens[offset..block_end];
        let block_script = Script::block_from_tokens(path.clone(), block_tokens, nifs)?;
        cases.push((None, block_script));

        let script = Script {
            suite: Stmt::IfStmt(IfStatement::new(cases)),
            phase: CompilationPhase::NeedInputMap,
            input_map: HashMap::new(),
        };
        Ok(script)
    }

    // Note that we have to have a separate build and install phase because otherwise we'd be borrowed
    // mutable when searching for inputs and double-borrow if any children are referenced.
    pub fn build_input_map(&self, tree: &Tree) -> Fallible<HashMap<ConcretePath, NodeRef>> {
        assert_eq!(self.phase, CompilationPhase::NeedInputMap);
        let mut inputs = Vec::new();
        self.suite.find_all_possible_inputs(tree, &mut inputs)?;
        let mut input_map = HashMap::new();
        for input in inputs.drain(..) {
            let node = tree.lookup_path(&input)?;
            input_map.insert(input, node);
        }
        Ok(input_map)
    }

    pub fn install_input_map(&mut self, input_map: HashMap<ConcretePath, NodeRef>) -> Fallible<()> {
        assert_eq!(self.phase, CompilationPhase::NeedInputMap);
        self.input_map = input_map;
        self.suite.mark_ready();
        self.mark_ready();
        Ok(())
    }

    fn mark_ready(&mut self) {
        self.phase = CompilationPhase::Ready;
    }

    pub fn populate_flow_graph(&self, tgt_node: &NodeRef, graph: &mut Graph) -> Fallible<()> {
        for src_node in self.input_map.values() {
            graph.add_edge(src_node, tgt_node);
        }
        Ok(())
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
    nifs: &'a HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
}

// Uses textbook precedence climbing.
impl<'a> ExprParser<'a> {
    fn from_tokens(
        path: String,
        tokens: &'a [Token],
        nifs: &'a HashMap<String, Box<dyn NativeFunc + Send + Sync>>,
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
