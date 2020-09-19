use cairo;

#[derive(Clone, Copy, Debug)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    pub fn from_u8s(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }
    pub fn from_u32_argb(c: u32) -> Color {
        Color::from_u8s((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
    }

    pub fn r_u16(&self) -> u16 {
        u16::from(self.r) << 8
    }
    pub fn g_u16(&self) -> u16 {
        u16::from(self.g) << 8
    }
    pub fn b_u16(&self) -> u16 {
        u16::from(self.b) << 8
    }
}

#[inline]
pub fn set_source_color(cr: &cairo::Context, c: Color) {
    cr.set_source_rgba(
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
        f64::from(c.a) / 255.0,
    );
}

#[derive(Clone, Debug)]
pub struct Theme {
    /// Text color for the view.
    pub foreground: Color,
    /// Backgound color of the view.
    pub background: Color,
    /// Color of the caret.
    pub caret: Color,
    /// Color of the line the caret is in.
    pub line_highlight: Option<Color>,

    /// Background color of regions matching the current search.
    pub find_highlight: Color,
    pub find_highlight_foreground: Option<Color>,

    /// Background color of the gutter.
    pub gutter: Color,
    /// The color of the line numbers in the gutter.
    pub gutter_foreground: Color,

    /// The background color of selections.
    pub selection: Color,
    /// text color of the selection regions.
    pub selection_foreground: Color,
    /// Color of the selection regions border.
    pub selection_border: Option<Color>,
    pub inactive_selection: Option<Color>,
    pub inactive_selection_foreground: Option<Color>,

    /// The color of the shadow used when a text area can be horizontally scrolled.
    pub shadow: Color,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            foreground: Color::from_u8s(50, 50, 50, 255),
            background: Color::WHITE,
            caret: Color::from_u8s(50, 50, 50, 255),
            line_highlight: Some(Color::from_u8s(245, 245, 245, 255)),
            find_highlight: Color::BLACK,
            find_highlight_foreground: Some(Color::from_u8s(50, 50, 50, 255)),
            gutter: Color::WHITE,
            gutter_foreground: Color::from_u8s(179, 179, 179, 255),
            selection: Color::from_u8s(248, 238, 199, 255),
            selection_foreground: Color::BLACK,
            selection_border: Some(Color::WHITE),
            inactive_selection: None,
            inactive_selection_foreground: None,
            shadow: Color::WHITE,
        }
    }
}
