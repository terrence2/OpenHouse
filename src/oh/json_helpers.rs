// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{bail, err_msg, Fallible};
use json::{object::Object, Array, JsonValue};

pub trait ValueHelper {
    fn to_object(&self) -> Fallible<&Object>;
    fn to_array(&self) -> Fallible<&Array>;
    fn to_str(&self) -> Fallible<&str>;
    fn to_int(&self) -> Fallible<i64>;
    fn to_bool(&self) -> Fallible<bool>;
}

impl ValueHelper for JsonValue {
    fn to_object(&self) -> Fallible<&Object> {
        match self {
            JsonValue::Object(obj) => Ok(obj),
            _ => bail!("value is not an object"),
        }
    }

    fn to_array(&self) -> Fallible<&Array> {
        match self {
            JsonValue::Array(arr) => Ok(arr),
            _ => bail!("value is not an array"),
        }
    }

    fn to_str(&self) -> Fallible<&str> {
        match self {
            JsonValue::Short(short) => Ok(short.as_str()),
            JsonValue::String(s) => Ok(s),
            _ => bail!("value is not a string"),
        }
    }

    fn to_int(&self) -> Fallible<i64> {
        match self {
            JsonValue::Number(n) => {
                let f: f64 = (*n).into();
                Ok(f as i64)
            }
            _ => bail!("value is not a number"),
        }
    }

    fn to_bool(&self) -> Fallible<bool> {
        match self {
            JsonValue::Boolean(b) => Ok(*b),
            _ => bail!("value is not boolean"),
        }
    }
}

pub trait ObjectHelper {
    fn fetch(&self, key: &str) -> Fallible<&JsonValue>;
}

impl ObjectHelper for Object {
    fn fetch(&self, key: &str) -> Fallible<&JsonValue> {
        self.get(key)
            .ok_or_else(|| err_msg(format!("missing key: {}", key)))
    }
}
