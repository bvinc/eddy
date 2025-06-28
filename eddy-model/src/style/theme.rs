use super::Color;
use crate::language::capture::Capture;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThemeAttributes {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: Option<bool>,
}

impl ThemeAttributes {
    fn from_file_attrs(tfa: ThemeFileAttributes) -> Self {
        let fg = tfa.fg.and_then(|s| Color::from_str(&s).ok());
        let bg = tfa.bg.and_then(|s| Color::from_str(&s).ok());
        ThemeAttributes {
            fg,
            bg,
            bold: tfa.bold,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub gutter: ThemeAttributes,
    pub gutter_line_highlight: ThemeAttributes,
    pub line_highlight: ThemeAttributes,
    pub selection: ThemeAttributes,
    pub cursor: Color,
    highlights: HashMap<Capture, ThemeAttributes>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_str(
            r##"
fg     = "#fdf4c1"
bg     = "#282828"
line_highlight = {bg="#3c3836"}
gutter = {bg="#3c3836", fg="#7c6f64"}
gutter_line_highlight = {bg="#504945", fg="#fe8019"}
cursor = "#fdf4c1"
selection = {bg = "#4e4e4e"}
line_number = {fg = "#7c6f64"}

[highlights]
"attribute"             = {fg = "#fe8019"}
"comment"               = {fg = "#7c6f64"}
"constant"              = {fg = "#d3869b"}
"constant.builtin"      = {fg = "#fe8019"}
"constructor"           = {fg = "#d3869b"}
"escape"                = {fg = "#8ec07c"}
"function"              = {fg = "#fabd2f"}
"function.macro"        = {fg = "#fe8019"}
"function.method"       = {fg = "#fabd2f"}
"keyword"               = {fg = "#fb4933"}
"label"                 = {fg = "#83a598"}
"operator"              = {fg = "#fdf4c1"}
"property"              = {fg = "#83a598"}
"punctuation.bracket"   = {fg = "#fdf4c1"}
"punctuation.delimiter" = {fg = "#fdf4c1"}
"string"                = {fg = "#b8bb26"}
"type"                  = {fg = "#d3869b"}
"type.builtin"          = {fg = "#fe8019"}
"variable.builtin"      = {fg = "#fe8019"}
"variable.parameter"    = {fg = "#83a598"}
"##,
        )
        .unwrap()
    }
}
impl FromStr for Theme {
    type Err = Box<dyn Error>;
    fn from_str(s: &str) -> Result<Theme, Box<dyn Error>> {
        let tf: ThemeFile = toml::from_str(s)?;
        let gutter = ThemeAttributes::from_file_attrs(tf.gutter);
        let gutter_line_highlight = ThemeAttributes::from_file_attrs(tf.gutter_line_highlight);
        let line_highlight = ThemeAttributes::from_file_attrs(tf.line_highlight);
        let selection = ThemeAttributes::from_file_attrs(tf.selection);
        let mut highlights = HashMap::new();
        for (name, value) in tf.highlights {
            let cap = Capture::from_name(&name);
            let theme_attributes = ThemeAttributes::from_file_attrs(value);
            if let Some(cap) = cap {
                highlights.insert(cap, theme_attributes);
            }
        }
        Ok(Theme {
            fg: Color::from_str(&tf.fg)?,
            bg: Color::from_str(&tf.bg)?,
            gutter,
            gutter_line_highlight,
            line_highlight,
            cursor: Color::from_str(&tf.cursor)?,
            selection,
            highlights,
        })
    }
}
impl Theme {
    pub fn attributes(&self, c: Capture) -> Option<ThemeAttributes> {
        self.highlights.get(&c).copied()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ThemeFile {
    pub fg: String,
    pub bg: String,
    pub gutter: ThemeFileAttributes,
    pub gutter_line_highlight: ThemeFileAttributes,
    pub line_highlight: ThemeFileAttributes,
    pub cursor: String,
    pub selection: ThemeFileAttributes,
    pub highlights: HashMap<String, ThemeFileAttributes>,
}
#[derive(Debug, Clone, Deserialize)]
struct ThemeFileAttributes {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: Option<bool>,
}
