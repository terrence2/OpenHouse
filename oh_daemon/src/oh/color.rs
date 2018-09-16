// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Fallible;
use regex::Regex;

#[derive(Debug, Eq, PartialEq)]
pub struct BHS {
    pub brightness: u8,
    pub hue: u16,
    pub saturation: u8,
}

impl BHS {
    pub fn new(brightness: u8, hue: u16, saturation: u8) -> Fallible<Self> {
        Ok(Self {
            brightness,
            hue,
            saturation,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RGB {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RGB {
    pub fn new(red: u8, green: u8, blue: u8) -> Fallible<RGB> {
        Ok(RGB { red, green, blue })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Mired {
    pub color_temp: u8,
}

impl Mired {
    pub fn new(color_temp: u8) -> Fallible<Self> {
        ensure!(
            color_temp >= 40,
            "mired: color temp less than 40 is not meaningful"
        );
        ensure!(
            color_temp <= 200,
            "mired: color temp greater than 200 is not meaningful"
        );
        Ok(Self { color_temp })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Color {
    BHS(BHS),
    RGB(RGB),
    Mired(Mired),
}

impl Color {
    pub fn parse(s: &str) -> Fallible<Color> {
        lazy_static! {
            static ref REGEX_MIRED: Regex = Regex::new(r"\s*mired\(\s*(\d+)\s*\)\s*").unwrap();
            static ref REGEX_RGB: Regex =
                Regex::new(r"\s*rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)\s*").unwrap();
            static ref REGEX_BHS: Regex =
                Regex::new(r"\s*bhs\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)\s*").unwrap();
        }
        if let Some(captures) = REGEX_BHS.captures(s) {
            return Ok(Color::BHS(BHS::new(
                captures[1].parse()?,
                captures[2].parse()?,
                captures[3].parse()?,
            )?));
        } else if let Some(captures) = REGEX_RGB.captures(s) {
            return Ok(Color::RGB(RGB::new(
                captures[1].parse()?,
                captures[2].parse()?,
                captures[3].parse()?,
            )?));
        } else if let Some(captures) = REGEX_MIRED.captures(s) {
            return Ok(Color::Mired(Mired::new(captures[1].parse()?)?));
        }
        bail!("color: not a color: '{}'", s);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_mired() -> Fallible<()> {
        assert_eq!(Color::parse("mired(40)")?, Color::Mired(Mired::new(40)?));
        assert_eq!(Color::parse(" mired(40)")?, Color::Mired(Mired::new(40)?));
        assert_eq!(Color::parse("mired(40) ")?, Color::Mired(Mired::new(40)?));
        assert_eq!(Color::parse(" mired(40) ")?, Color::Mired(Mired::new(40)?));
        assert_eq!(Color::parse(" mired( 40) ")?, Color::Mired(Mired::new(40)?));
        assert_eq!(Color::parse(" mired(40 ) ")?, Color::Mired(Mired::new(40)?));
        assert_eq!(
            Color::parse(" mired( 40 ) ")?,
            Color::Mired(Mired::new(40)?)
        );
        assert!(Color::parse("  mired(  39  )  ").is_err());
        assert!(Color::parse("  mired(  201  )  ").is_err());
        assert!(Color::parse("  mired(  256  )  ").is_err());
        Ok(())
    }

    #[test]
    fn test_parse_rgb() -> Fallible<()> {
        assert_eq!(Color::parse("rgb(0,0,0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse(" rgb(0,0,0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb( 0,0,0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb(0 ,0,0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb(0, 0,0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb(0,0 ,0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb(0,0, 0)")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb(0,0,0 )")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(Color::parse("rgb(0,0,0) ")?, Color::RGB(RGB::new(0, 0, 0)?));
        assert_eq!(
            Color::parse(" rgb( 0 , 0 , 0 ) ")?,
            Color::RGB(RGB::new(0, 0, 0)?)
        );
        assert!(Color::parse("rgb(-1,0,0)").is_err());
        assert!(Color::parse("rgb(0,-1,0)").is_err());
        assert!(Color::parse("rgb(0,0,-1)").is_err());
        assert!(Color::parse("rgb(256,0,0)").is_err());
        assert!(Color::parse("rgb(0,256,0)").is_err());
        assert!(Color::parse("rgb(0,0,256)").is_err());
        Ok(())
    }

    #[test]
    fn test_parse_bhs() -> Fallible<()> {
        assert_eq!(Color::parse("bhs(0,0,0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse(" bhs(0,0,0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs( 0,0,0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs(0 ,0,0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs(0, 0,0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs(0,0 ,0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs(0,0, 0)")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs(0,0,0 )")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(Color::parse("bhs(0,0,0) ")?, Color::BHS(BHS::new(0, 0, 0)?));
        assert_eq!(
            Color::parse(" bhs( 0 , 0 , 0 ) ")?,
            Color::BHS(BHS::new(0, 0, 0)?)
        );
        assert!(Color::parse("bhs(-1,0,0)").is_err());
        assert!(Color::parse("bhs(0,-1,0)").is_err());
        assert!(Color::parse("bhs(0,0,-1)").is_err());
        assert!(Color::parse("bhs(256,0,0)").is_err());
        assert!(Color::parse("bhs(0,65566,0)").is_err());
        assert!(Color::parse("bhs(0,0,256)").is_err());
        Ok(())
    }
}
