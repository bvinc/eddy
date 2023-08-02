use eddy_workspace::style::Color;
use gtk::gdk;

pub fn text_theme_to_gdk(c: Color) -> gdk::RGBA {
    let mut ret = gdk::RGBA::BLACK;
    ret.set_red(c.r_f32());
    ret.set_green(c.g_f32());
    ret.set_blue(c.b_f32());
    ret
}

pub fn pango_to_gdk(c: pango::Color) -> gdk::RGBA {
    let mut ret = gdk::RGBA::BLACK;
    ret.set_red(c.red() as f32 / 65536.0);
    ret.set_green(c.green() as f32 / 65536.0);
    ret.set_blue(c.blue() as f32 / 65536.0);
    ret
}
