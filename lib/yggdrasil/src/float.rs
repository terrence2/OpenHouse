// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::{ensure, Fallible};
use std::{
    cmp::Ordering,
    fmt,
    ops::{Add, Div, Mul, Neg, Sub},
};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Float {
    pub value: f64,
}

impl Float {
    pub fn new(value: f64) -> Fallible<Float> {
        ensure!(!value.is_infinite(), "numerical error: glimpsed infinity");
        ensure!(!value.is_nan(), "numerical error: not a number");
        Ok(Float { value })
    }

    pub fn checked_add(self, rhs: Float) -> Fallible<Float> {
        Float::new(self.value + rhs.value)
    }

    pub fn checked_div(self, rhs: Float) -> Fallible<Float> {
        Float::new(self.value / rhs.value)
    }

    pub fn checked_mul(self, rhs: Float) -> Fallible<Float> {
        Float::new(self.value * rhs.value)
    }

    pub fn checked_neg(self) -> Fallible<Float> {
        Float::new(-self.value)
    }

    pub fn checked_sub(self, rhs: Float) -> Fallible<Float> {
        Float::new(self.value - rhs.value)
    }
}

impl Add for Float {
    type Output = Float;
    fn add(self, rhs: Float) -> Self::Output {
        self.checked_add(rhs).unwrap()
    }
}

impl Div for Float {
    type Output = Float;
    fn div(self, rhs: Float) -> Self::Output {
        self.checked_div(rhs).unwrap()
    }
}

impl Eq for Float {}

impl Mul for Float {
    type Output = Float;
    fn mul(self, rhs: Float) -> Self::Output {
        self.checked_mul(rhs).unwrap()
    }
}

impl Neg for Float {
    type Output = Float;
    fn neg(self) -> Self::Output {
        self.checked_neg().unwrap()
    }
}

impl Ord for Float {
    fn cmp(&self, other: &Float) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Sub for Float {
    type Output = Float;
    fn sub(self, rhs: Float) -> Self::Output {
        self.checked_sub(rhs).unwrap()
    }
}

impl fmt::Display for Float {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic]
    fn test_float_inf() {
        Float::new(1.0f64 / 0.0f64).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_float_nan() {
        #[allow(clippy::eq_op, clippy::zero_divided_by_zero)]
        Float::new(0.0f64 / 0.0f64).unwrap();
    }

    #[test]
    fn test_float_arith() {
        let two = Float::new(2.0f64).unwrap();
        let three = Float::new(3.0f64).unwrap();
        let four = Float::new(4.0f64).unwrap();
        let five = Float::new(5.0f64).unwrap();
        let six = Float::new(6.0f64).unwrap();
        assert_eq!(two + three, five);
        assert_eq!(two * three, six);
        assert_eq!(six - two, four);
        assert_eq!(six / two, three);
    }
}
