// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use float::Float;
use path::{ConcretePath, ScriptPath};
use std::{convert::From, fmt};
use tokenizer::Token;
use tree::Tree;

fn ensure_same_types(types: &[ValueType]) -> Fallible<ValueType> {
    ensure!(
        !types.is_empty(),
        "typecheck error: trying to reify empty type list"
    );
    let expect_type = types[0];
    for ty in &types[1..] {
        ensure!(
            *ty == expect_type,
            "typecheck error: mismatched types in ensure_same_types"
        );
    }
    return Ok(expect_type);
}

bitflags! {
    pub struct ValueType : usize {
        const BOOLEAN = 0b0001;
        const FLOAT   = 0b0010;
        const INTEGER = 0b0100;
        const STRING  = 0b1000;
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
    pub(super) fn virtually_compute_for_path(&self, tree: &Tree) -> Fallible<Vec<Value>> {
        trace!("Value::virtually_compute_for_path({})", self);
        if let Value::Path(p) = self {
            let noderef = tree.lookup_dynamic_path(p)?;
            return noderef.virtually_compute_for_path(tree);
        }
        return Ok(vec![self.to_owned()]);
    }

    pub(super) fn compute(&self, tree: &Tree) -> Fallible<Value> {
        if let Value::Path(p) = self {
            let noderef = tree.lookup_dynamic_path(p)?;
            return noderef.compute(tree);
        }
        return Ok(self.to_owned());
    }

    pub(super) fn apply(&self, tok: &Token, other: &Value) -> Fallible<Value> {
        ensure!(
            !self.is_path(),
            "runtime error: attempting to apply a non-path"
        );
        ensure!(
            !other.is_path(),
            "runtime error: attempting to apply a non-path"
        );
        return Ok(match self {
            Value::Boolean(b0) => {
                Value::Boolean(Self::apply_boolean(tok, *b0, other.as_boolean()?)?)
            }
            Value::Integer(i0) => Self::apply_integer(tok, *i0, other.as_integer()?)?,
            Value::Float(f0) => Self::apply_float(tok, *f0, other.as_float()?)?,
            Value::String(s0) => Value::String(Self::apply_string(tok, s0, &other.as_string()?)?),
            _ => bail!("runtime error: apply reached a path node"),
        });
    }

    pub(super) fn apply_boolean(tok: &Token, a: bool, b: bool) -> Fallible<bool> {
        return Ok(match tok {
            Token::Or => a || b,
            Token::And => a && b,
            Token::Equals => a == b,
            Token::NotEquals => a != b,
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a bool",
                tok
            ),
        });
    }

    pub(super) fn apply_integer(tok: &Token, a: i64, b: i64) -> Fallible<Value> {
        return Ok(match tok {
            Token::Add => Value::Integer(a + b),
            Token::Subtract => Value::Integer(a - b),
            Token::Multiply => Value::Integer(a * b),
            Token::Divide => Value::Float(Float::new(a as f64)? / Float::new(b as f64)?),
            Token::Modulo => Value::Integer(a % b),
            Token::Equals => Value::Boolean(a == b),
            Token::NotEquals => Value::Boolean(a != b),
            Token::GreaterThan => Value::Boolean(a > b),
            Token::LessThan => Value::Boolean(a < b),
            Token::GreaterThanOrEquals => Value::Boolean(a >= b),
            Token::LessThanOrEquals => Value::Boolean(a <= b),
            _ => bail!(
                "runtime error: {:?} is not a valid operation on an integer",
                tok
            ),
        });
    }

    pub(super) fn apply_float(tok: &Token, a: Float, b: Float) -> Fallible<Value> {
        return Ok(match tok {
            Token::Add => Value::Float(a + b),
            Token::Subtract => Value::Float(a - b),
            Token::Multiply => Value::Float(a * b),
            Token::Divide => Value::Float(a / b),
            Token::Equals => Value::Boolean(a == b),
            Token::NotEquals => Value::Boolean(a != b),
            Token::GreaterThan => Value::Boolean(a > b),
            Token::LessThan => Value::Boolean(a < b),
            Token::GreaterThanOrEquals => Value::Boolean(a >= b),
            Token::LessThanOrEquals => Value::Boolean(a <= b),
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a float",
                tok
            ),
        });
    }

    pub(super) fn apply_string(tok: &Token, a: &str, b: &str) -> Fallible<String> {
        return Ok(match tok {
            Token::Add => a.to_owned() + b,
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a string",
                tok
            ),
        });
    }

    pub fn is_path(&self) -> bool {
        if let Value::Path(_) = self {
            return true;
        }
        return false;
    }

    pub fn as_boolean(&self) -> Fallible<bool> {
        if let Value::Boolean(b) = self {
            return Ok(*b);
        }
        bail!("runtime error: attempted to use a non-boolean value in boolean context")
    }

    pub fn as_integer(&self) -> Fallible<i64> {
        if let Value::Integer(i) = self {
            return Ok(*i);
        }
        bail!("runtime error: attempted to use a non-integer value in integer context")
    }

    pub fn as_float(&self) -> Fallible<Float> {
        if let Value::Float(f) = self {
            return Ok(*f);
        }
        bail!("runtime error: attempted to use a non-float value in float context")
    }

    pub fn as_string(&self) -> Fallible<String> {
        if let Value::String(s) = self {
            return Ok(s.to_owned());
        }
        bail!("runtime error: attempted to use a non-stringvalue in string context")
    }

    pub fn as_path_component(&self) -> Fallible<String> {
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

    // Devirtualize and return all concrete paths, if this is a path.
    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<ValueType> {
        trace!("Value::find_all_possible_inputs: {}", self);
        if let Value::Path(path) = self {
            // Our virtual path will depend on concrete inputs that may or may
            // not have been visited yet. Find them, and make sure they have been
            // visited so that we can virtually_compute on them when devirtualizing.
            let mut concrete_inputs = Vec::new();
            path.find_concrete_inputs(&mut concrete_inputs)?;
            for concrete in &concrete_inputs {
                tree.lookup_path(concrete)?.link_and_validate_inputs(tree)?;
            }

            let mut direct_inputs = path.devirtualize(tree)?;

            // Do type checking as we collect paths, since we won't have another opportunity.
            let mut value_types = Vec::new();
            for inp in &direct_inputs {
                let noderef = tree.lookup_path(inp)?;
                let nodetype = noderef.get_or_find_node_type(tree)?;
                value_types.push(nodetype);
            }
            ensure_same_types(&value_types)?;

            // Collect both direct and indirect inputs at this value.
            out.append(&mut direct_inputs);
            out.append(&mut concrete_inputs);

            return Ok(value_types[0]);
        }
        return Ok(match self {
            Value::Boolean(_) => ValueType::BOOLEAN,
            Value::Float(_) => ValueType::FLOAT,
            Value::Integer(_) => ValueType::INTEGER,
            Value::String(_) => ValueType::STRING,
            Value::Path(_) => panic!("typeflow error: we already filtered out path"),
        });
    }
}

impl<'a> From<&'a str> for Value {
    fn from(t: &str) -> Value {
        Value::String(t.to_owned())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Integer(i) => write!(f, "{}i64", i),
            Value::Float(v) => write!(f, "{}f64", v),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Path(p) => write!(f, "{}", p),
        }
    }
}
