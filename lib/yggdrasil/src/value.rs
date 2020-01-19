// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use crate::{
    float::Float,
    path::{ConcretePath, ScriptPath},
    tokenizer::Token,
    tree::Tree,
};
use failure::{bail, ensure, Fallible};
use std::{convert::From, fmt};
use tracing::trace;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueData {
    Boolean(bool),
    Float(Float),
    Integer(i64),
    Path(ScriptPath),
    String(String),
    InputFlag, // Our Any type
}

fn latch<T>(lhs: &Value, rhs: &Value, a: T, b: T) -> T {
    if lhs.generation() >= rhs.generation() {
        a
    } else {
        b
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub data: ValueData,
    generation: usize,
}

impl Value {
    pub fn from_boolean(b: bool) -> Self {
        Self {
            data: ValueData::Boolean(b),
            generation: 0,
        }
    }

    pub fn from_integer(i: i64) -> Self {
        Self {
            data: ValueData::Integer(i),
            generation: 0,
        }
    }

    pub fn from_float(f: Float) -> Self {
        Self {
            data: ValueData::Float(f),
            generation: 0,
        }
    }

    pub fn from_string(s: String) -> Self {
        Self {
            data: ValueData::String(s),
            generation: 0,
        }
    }

    pub fn new_str(s: &str) -> Self {
        Self {
            data: ValueData::String(s.to_owned()),
            generation: 0,
        }
    }

    pub fn from_path(p: ScriptPath) -> Self {
        Self {
            data: ValueData::Path(p),
            generation: 0,
        }
    }

    pub fn input_flag() -> Self {
        Self {
            data: ValueData::InputFlag,
            generation: 0,
        }
    }

    pub fn generation(&self) -> usize {
        self.generation
    }

    pub fn set_generation(&mut self, generation: usize) {
        self.generation = generation
    }

    pub fn with_generation(mut self, generation: usize) -> Self {
        self.generation = generation;
        self
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
            ValueData::Boolean(_) => Self::apply_boolean(tok, self, other)?,
            ValueData::Integer(_) => Self::apply_integer(tok, self, other)?,
            ValueData::Float(_) => Self::apply_float(tok, self, other)?,
            ValueData::String(_) => Self::apply_string(tok, self, other)?,
            _ => bail!("runtime error: apply reached a path node"),
        })
    }

    pub(super) fn apply_boolean(tok: &Token, lhs: &Value, rhs: &Value) -> Fallible<Value> {
        let a = lhs.as_boolean()?;
        let b = rhs.as_boolean()?;
        let next = match tok {
            Token::Or => a || b,
            Token::And => a && b,
            Token::Latch => latch(lhs, rhs, a, b),
            Token::Equals => a == b,
            Token::NotEquals => a != b,
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a bool",
                tok
            ),
        };
        Ok(Value::from_boolean(next).with_generation(lhs.generation().max(rhs.generation())))
    }

    pub(super) fn apply_integer(tok: &Token, lhs: &Value, rhs: &Value) -> Fallible<Value> {
        let a = lhs.as_integer()?;
        let b = rhs.as_integer()?;
        let data = match tok {
            Token::Add => ValueData::Integer(a + b),
            Token::Subtract => ValueData::Integer(a - b),
            Token::Multiply => ValueData::Integer(a * b),
            Token::Divide => ValueData::Float(Float::new(a as f64)? / Float::new(b as f64)?),
            Token::Modulo => ValueData::Integer(a % b),
            Token::Latch => ValueData::Integer(latch(lhs, rhs, a, b)),
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
        Ok(Value {
            data,
            generation: lhs.generation().max(rhs.generation()),
        })
    }

    pub(super) fn apply_float(tok: &Token, lhs: &Value, rhs: &Value) -> Fallible<Value> {
        let a = lhs.as_float()?;
        let b = rhs.as_float()?;
        let data = match tok {
            Token::Add => ValueData::Float(a + b),
            Token::Subtract => ValueData::Float(a - b),
            Token::Multiply => ValueData::Float(a * b),
            Token::Divide => ValueData::Float(a / b),
            Token::Latch => ValueData::Float(latch(lhs, rhs, a, b)),
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
        Ok(Value {
            data,
            generation: lhs.generation().max(rhs.generation()),
        })
    }

    pub(super) fn apply_string(tok: &Token, lhs: &Value, rhs: &Value) -> Fallible<Value> {
        let a = lhs.as_string()?;
        let b = rhs.as_string()?;
        let s = match tok {
            Token::Add => a + &b,
            Token::Latch => latch(lhs, rhs, a, b),
            _ => bail!(
                "runtime error: {:?} is not a valid operation on a string",
                tok
            ),
        };
        Ok(Value::from_string(s).with_generation(lhs.generation().max(rhs.generation())))
    }

    pub fn is_path(&self) -> bool {
        if let ValueData::Path(_) = self.data {
            return true;
        }
        false
    }

    pub fn is_input_flag(&self) -> bool {
        self.data == ValueData::InputFlag
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
            ValueData::InputFlag => bail!("runtime error: input flag in as_path_component"),
        }
    }

    // Devirtualize and return all concrete paths, if this is a path.
    pub fn find_all_possible_inputs(
        &self,
        tree: &Tree,
        out: &mut Vec<ConcretePath>,
    ) -> Fallible<()> {
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

            // Devirtualization is eager; expansions may result in paths that do not actually
            // exist in the tree. This may be fine, depending on what inputs real devices produce.
            // It does mean that we have to filter out impossible paths at this layer.
            let mut direct_inputs = path
                .devirtualize(tree)?
                .drain(..)
                .filter(|path| tree.lookup_path(path).is_ok())
                .collect::<Vec<ConcretePath>>();

            // Collect both direct and indirect inputs at this value.
            out.append(&mut direct_inputs);
            out.append(&mut concrete_inputs);
        }
        Ok(())
    }
}

impl<'a> From<&'a str> for Value {
    fn from(t: &str) -> Value {
        Value {
            data: ValueData::String(t.to_owned()),
            generation: 0,
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
            ValueData::InputFlag => write!(f, "InputFlag"),
        }
    }
}
