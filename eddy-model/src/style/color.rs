use std::fmt;
use std::str::FromStr;

#[derive(Debug, Copy, Clone)]
pub struct ParseColorError;

impl std::error::Error for ParseColorError {}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "color parse error")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };

    pub fn r_u8(&self) -> u8 {
        self.r
    }
    pub fn g_u8(&self) -> u8 {
        self.g
    }
    pub fn b_u8(&self) -> u8 {
        self.b
    }

    pub fn r_u16(&self) -> u16 {
        (self.r as u16) << 8
    }
    pub fn g_u16(&self) -> u16 {
        (self.g as u16) << 8
    }
    pub fn b_u16(&self) -> u16 {
        (self.b as u16) << 8
    }

    pub fn r_f32(&self) -> f32 {
        (self.r as f32) / 255.0
    }
    pub fn g_f32(&self) -> f32 {
        (self.g as f32) / 255.0
    }
    pub fn b_f32(&self) -> f32 {
        (self.b as f32) / 255.0
    }

    pub fn r_f64(&self) -> f64 {
        (self.r as f64) / 255.0
    }
    pub fn g_f64(&self) -> f64 {
        (self.g as f64) / 255.0
    }
    pub fn b_f64(&self) -> f64 {
        (self.b as f64) / 255.0
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 7 || s.chars().count() != 7 {
            return Err(ParseColorError);
        }
        if let Some(b'#') = s.as_bytes().first().copied() {
        } else {
            return Err(ParseColorError);
        }

        let (mut r, mut b, mut g) = (0, 0, 0);
        if let Ok(red) = i64::from_str_radix(&s[1..3], 16) {
            r = red as u8;
        }
        if let Ok(green) = i64::from_str_radix(&s[3..5], 16) {
            g = green as u8;
        }
        if let Ok(blue) = i64::from_str_radix(&s[5..7], 16) {
            b = blue as u8;
        }

        Ok(Color { r, g, b })
    }
}
