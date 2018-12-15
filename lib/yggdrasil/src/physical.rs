// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use approx::relative_eq;
use failure::{ensure, Fallible};

#[derive(Clone, Copy, Debug)]
pub enum Length {
    Meters(f64),
    Imperial(i64, f64), // feet+inches
}

impl Length {
    pub fn from_str(s: &str) -> Fallible<Self> {
        if s.contains('\'') {
            let parts = s.splitn(2, '\'').collect::<Vec<&str>>();
            let ft = parts[0].parse::<i64>()?;
            if parts.len() == 1 || parts[1].is_empty() {
                Ok(Length::Imperial(ft, 0.))
            } else {
                ensure!(s.ends_with('"'), "expected \" to be at end");
                let stripped = &parts[1][0..parts[1].len() - 1];
                Ok(Length::Imperial(ft, stripped.parse::<f64>()?))
            }
        } else if s.contains('"') {
            ensure!(s.ends_with('"'), "expected \" to be at end");
            let stripped = &s[0..s.len() - 1];
            Ok(Length::Imperial(0, stripped.parse::<f64>()?))
        } else if s.ends_with('m') {
            let stripped = &s[0..s.len() - 1];
            Ok(Length::Meters(stripped.parse::<f64>()?))
        } else {
            Ok(Length::Meters(s.parse::<f64>()?))
        }
    }

    pub fn meters(&self) -> f64 {
        match *self {
            Length::Meters(meters) => meters,
            Length::Imperial(feet, inches) => ((feet as f64) + (inches / 12.)) * 0.3048,
        }
    }
}
impl PartialEq for Length {
    fn eq(&self, other: &Length) -> bool {
        relative_eq!(self.meters(), other.meters())
    }
}
impl Eq for Length {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dimension2 {
    x_len: Length,
    y_len: Length,
}

impl Dimension2 {
    pub fn from_str(s: &str) -> Fallible<Self> {
        assert!(!s.starts_with('@'));
        assert!(!s.starts_with('<'));
        assert!(!s.starts_with('>'));
        let parts = s.splitn(2, 'x').collect::<Vec<&str>>();
        ensure!(parts.len() == 2, "invalid dimension: no x in middle");
        ensure!(!parts[0].is_empty(), "invalid dimension: empty X part");
        ensure!(!parts[1].is_empty(), "invalid dimension: empty Y part");
        Ok(Dimension2 {
            x_len: Length::from_str(parts[0])?,
            y_len: Length::from_str(parts[1])?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_meters() {
        let d = Dimension2 {
            x_len: Length::Meters(1.),
            y_len: Length::Meters(1.),
        };
        assert_eq!(Dimension2::from_str("1x1").unwrap(), d);
        assert_eq!(Dimension2::from_str("1.x1").unwrap(), d);
        assert_eq!(Dimension2::from_str("1x1.0").unwrap(), d);
        assert_eq!(Dimension2::from_str("1mx1m").unwrap(), d);
        assert_eq!(Dimension2::from_str("1mx1.m").unwrap(), d);
        assert_eq!(Dimension2::from_str("1mx1.0m").unwrap(), d);
        assert_eq!(Dimension2::from_str("1x1m").unwrap(), d);
        assert_eq!(Dimension2::from_str("1mx1").unwrap(), d);

        let d = Dimension2 {
            x_len: Length::Meters(-1.),
            y_len: Length::Meters(-1.),
        };
        assert_eq!(Dimension2::from_str("-1x-1").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1.x-1").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1x-1.0").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1mx-1m").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1mx-1.m").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1mx-1.0m").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1x-1m").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1mx-1").unwrap(), d);
    }

    #[test]
    fn test_parse_feet() {
        let d = Dimension2 {
            x_len: Length::Imperial(1, 0.),
            y_len: Length::Imperial(1, 0.),
        };
        assert_eq!(Dimension2::from_str("1'x1'").unwrap(), d);

        let d = Dimension2 {
            x_len: Length::Imperial(-1, 0.),
            y_len: Length::Imperial(-1, 0.),
        };
        assert_eq!(Dimension2::from_str("-1'x-1'").unwrap(), d);
    }

    #[test]
    fn test_parse_inches() {
        let d = Dimension2 {
            x_len: Length::Imperial(0, 1.),
            y_len: Length::Imperial(0, 1.),
        };
        assert_eq!(Dimension2::from_str("1\"x1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("1.\"x1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("1.0\"x1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("1.0000\"x1\"").unwrap(), d);

        let d = Dimension2 {
            x_len: Length::Imperial(0, -1.),
            y_len: Length::Imperial(0, -1.),
        };
        assert_eq!(Dimension2::from_str("-1\"x-1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1.\"x-1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1.0\"x-1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("-1.0000\"x-1\"").unwrap(), d);
    }

    #[test]
    fn test_parse_imperial() {
        let d = Dimension2 {
            x_len: Length::Imperial(2, 1.),
            y_len: Length::Imperial(0, 1.),
        };
        assert_eq!(Dimension2::from_str("2'1\"x1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("2'1.\"x1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("2'1.0\"x1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("2'1.0000\"x1\"").unwrap(), d);

        let d = Dimension2 {
            x_len: Length::Imperial(-2, -1.),
            y_len: Length::Imperial(0, -1.),
        };
        assert_eq!(Dimension2::from_str("-2'-1\"x-1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("-2'-1.\"x-1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("-2'-1.0\"x-1\"").unwrap(), d);
        assert_eq!(Dimension2::from_str("-2'-1.0000\"x-1\"").unwrap(), d);
    }
}
