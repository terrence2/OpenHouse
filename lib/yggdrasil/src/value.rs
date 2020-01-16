// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    float::Float,
    path::{ConcretePath, ScriptPath},
    tokenizer::Token,
    tree::Tree,
};
use bitflags::bitflags;
use failure::{bail, ensure, Fallible};
use log::trace;
use std::{convert::From, fmt};

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
    Ok(expect_type)
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
pub enum ValueData {
    Boolean(bool),
    Float(Float),
    Integer(i64),
    Path(ScriptPath),
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub data: ValueData,
}

impl Value {
    pub fn new_boolean(b: bool) -> Self {
        Self {
            data: ValueData::Boolean(b),
        }
    }

    pub fn new_integer(i: i64) -> Self {
        Self {
            data: ValueData::Integer(i),
        }
    }

    pub fn new_float(f: Float) -> Self {
        Self {
            data: ValueData::Float(f),
        }
    }

    pub fn new_string(s: String) -> Self {
        Self {
            data: ValueData::String(s),
        }
    }

    pub fn new_str(s: &str) -> Self {
        Self {
            data: ValueData::String(s.to_owned()),
        }
    }

    pub fn new_path(p: ScriptPath) -> Self {
        Self {
            data: ValueData::Path(p),
        }
    }

    pub(super) fn virtually_compute_for_path(&self, tree: &Tree) -> Fallible<Vec<Value>> {
        trace!("Value::virtually_compute_for_path({})", self);
        if let ValueData::Path(ref p) = self.data {
            let noderef = tree.lookup_dynamic_path(p)?;
            return noderef.virtually_compute_for_path(tree);
        }
        Ok(vec![self.to_owned()])
    }

    pub(super) fn compute(&self, tree: &Tree) -> Fallible<Value> {
        if let ValueData::Path(ref p) = self.data {
            let noderef = tree.lookup_dynamic_path(p)?;
            return noderef.compute(tree);
        }
        Ok(self.to_owned())
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
        Ok(match self.data {
            ValueData::Boolean(b0) => Self::apply_boolean(tok, b0, other.as_boolean()?)?,
            ValueData::Integer(i0) => Self::apply_integer(tok, i0, other.as_integer()?)?,
            ValueData::Float(f0) => Self::apply_float(tok, f0, other.as_float()?)?,
            ValueData::String(ref s0) => Self::apply_string(tok, s0, &other.as_string()?)?,
            _ => bail!("runtime error: apply reached a path node"),
        })
    }

    pub(super) fn apply_boolean(tok: &Token, a: bool, b: bool) -> Fallible<Value> {
        let b = match tok {
            Token::Or => a || b,
            Token::And => a && b,
            Token::Equals => a == b,
            Token::NotEquals => a != b,
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a bool",
                tok
            ),
        };
        Ok(Value::new_boolean(b))
    }

    pub(super) fn apply_integer(tok: &Token, a: i64, b: i64) -> Fallible<Value> {
        let data = match tok {
            Token::Add => ValueData::Integer(a + b),
            Token::Subtract => ValueData::Integer(a - b),
            Token::Multiply => ValueData::Integer(a * b),
            Token::Divide => ValueData::Float(Float::new(a as f64)? / Float::new(b as f64)?),
            Token::Modulo => ValueData::Integer(a % b),
            Token::Equals => ValueData::Boolean(a == b),
            Token::NotEquals => ValueData::Boolean(a != b),
            Token::GreaterThan => ValueData::Boolean(a > b),
            Token::LessThan => ValueData::Boolean(a < b),
            Token::GreaterThanOrEquals => ValueData::Boolean(a >= b),
            Token::LessThanOrEquals => ValueData::Boolean(a <= b),
            _ => bail!(
                "runtime error: {:?} is not a valid operation on an integer",
                tok
            ),
        };
        Ok(Value { data })
    }

    pub(super) fn apply_float(tok: &Token, a: Float, b: Float) -> Fallible<Value> {
        let data = match tok {
            Token::Add => ValueData::Float(a + b),
            Token::Subtract => ValueData::Float(a - b),
            Token::Multiply => ValueData::Float(a * b),
            Token::Divide => ValueData::Float(a / b),
            Token::Equals => ValueData::Boolean(a == b),
            Token::NotEquals => ValueData::Boolean(a != b),
            Token::GreaterThan => ValueData::Boolean(a > b),
            Token::LessThan => ValueData::Boolean(a < b),
            Token::GreaterThanOrEquals => ValueData::Boolean(a >= b),
            Token::LessThanOrEquals => ValueData::Boolean(a <= b),
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a float",
                tok
            ),
        };
        Ok(Value { data })
    }

    pub(super) fn apply_string(tok: &Token, a: &str, b: &str) -> Fallible<Value> {
        let s = match tok {
            Token::Add => a.to_owned() + b,
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a string",
                tok
            ),
        };
        Ok(Value::new_string(s))
    }

    pub fn is_path(&self) -> bool {
        if let ValueData::Path(_) = self.data {
            return true;
        }
        false
    }

    pub fn as_boolean(&self) -> Fallible<bool> {
        if let ValueData::Boolean(b) = self.data {
            return Ok(b);
        }
        bail!("runtime error: attempted to use a non-boolean value in boolean context")
    }

    pub fn as_integer(&self) -> Fallible<i64> {
        if let ValueData::Integer(i) = self.data {
            return Ok(i);
        }
        bail!("runtime error: attempted to use a non-integer value in integer context")
    }

    pub fn as_float(&self) -> Fallible<Float> {
        if let ValueData::Float(f) = self.data {
            return Ok(f);
        }
        bail!("runtime error: attempted to use a non-float value in float context")
    }

    pub fn as_string(&self) -> Fallible<String> {
        if let ValueData::String(ref s) = self.data {
            return Ok(s.to_owned());
        }
        bail!("runtime error: attempted to use a non-stringvalue in string context")
    }

    pub fn as_path_component(&self) -> Fallible<String> {
        match self.data {
            ValueData::Integer(i) => Ok(i.to_string()),
            ValueData::Boolean(b) => Ok(b.to_string()),
            ValueData::String(ref s) => Ok(s.to_owned()),
            ValueData::Float(_) => {
                bail!("runtime error: a float value cannot be used as a path component")
            }
            ValueData::Path(_) => bail!("runtime error: did not expect a path as path component"),
        }
    }

    // Devirtualize and return all concrete paths, if this is a path.
    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<ValueType> {
        trace!("Value::find_all_possible_inputs: {}", self);
        if let ValueData::Path(ref path) = self.data {
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
        Ok(match &self.data {
            ValueData::Boolean(_) => ValueType::BOOLEAN,
            ValueData::Float(_) => ValueType::FLOAT,
            ValueData::Integer(_) => ValueType::INTEGER,
            ValueData::String(_) => ValueType::STRING,
            ValueData::Path(_) => panic!("typeflow error: we already filtered out path"),
        })
    }
}

impl<'a> From<&'a str> for Value {
    fn from(t: &str) -> Value {
        Value {
            data: ValueData::String(t.to_owned()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.data {
            ValueData::Boolean(b) => write!(f, "{}", b),
            ValueData::Integer(i) => write!(f, "{}i64", i),
            ValueData::Float(v) => write!(f, "{}f64", v),
            ValueData::String(ref s) => write!(f, "\"{}\"", s),
            ValueData::Path(ref p) => write!(f, "{}", p),
        }
    }
}
