use eddy_workspace::style::{Attr, AttrSpan};
use eddy_workspace::Workspace;
use gdk::keys::Key;
use gdk::ModifierType;
use glib::clone;
use glib::Sender;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib::subclass, Adjustment};
use log::*;
use lru_cache::LruCache;
use once_cell::unsync::OnceCell;
use pango::Attribute;
use ropey::RopeSlice;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;
use std::time::Instant;

use crate::app::Action;
use crate::theme::Theme;

pub struct GutterPrivate {
    vadj: Adjustment,
    sender: OnceCell<Sender<Action>>,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: usize,
    theme: Theme,
    gutter_nchars: usize,
}

#[glib::object_subclass]
impl ObjectSubclass for GutterPrivate {
    const NAME: &'static str = "Gutter";
    type Type = Gutter;
    type ParentType = gtk::Widget;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    fn new() -> Self {
        let sender = OnceCell::new();
        let workspace = OnceCell::new();
        let view_id = 0;
        let theme = Theme::default();
        let vadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let gutter_nchars = 3;

        Self {
            vadj,
            sender,
            workspace,
            view_id,
            theme,
            gutter_nchars,
        }
    }
}

impl ObjectImpl for GutterPrivate {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        let pango_ctx = obj.pango_context();
        let mut font_desc = pango::FontDescription::new();
        font_desc.set_family("Hack, Mono");
        font_desc.set_size(16384);
        pango_ctx.set_font_description(&font_desc);
    }
}
impl WidgetImpl for GutterPrivate {
    fn snapshot(&self, gutter: &Gutter, snapshot: &gtk::Snapshot) {
        // snapshot.render_layout(&ctx, 10.0, 10.0, &layout);
        // snapshot.render_background(&ctx, 10.0, 10.0, 30.0, 20.0);
        self.handle_draw(gutter, snapshot);
    }
    fn compute_expand(&self, obj: &Self::Type, hexpand: &mut bool, vexpand: &mut bool) {
        self.parent_compute_expand(obj, hexpand, vexpand);
        debug!("gutter compute expand");
        dbg!(hexpand, vexpand);
    }
    fn map(&self, obj: &Self::Type) {
        self.parent_map(obj);
        debug!("gutter cvt map");
    }
    fn measure(
        &self,
        obj: &Self::Type,
        orientation: gtk::Orientation,
        for_size: i32,
    ) -> (i32, i32, i32, i32) {
        self.parent_measure(obj, orientation, for_size);
        dbg!(orientation, for_size);

        let pango_ctx = obj.pango_context();
        if let Some(metrics) = pango_ctx.metrics(None, None) {
            let font_width = metrics.approximate_digit_width() as f64 / pango::SCALE as f64;
            let minimum_size = self.gutter_nchars as i32 * font_width as i32;
            let natural_size = self.gutter_nchars as i32 * font_width as i32;
            let minimum_baseline = -1;
            let natural_baseline = -1;
            (
                minimum_size,
                natural_size,
                minimum_baseline,
                natural_baseline,
            )
        } else {
            (0, 0, -1, -1)
        }
    }
    fn show(&self, _: &Self::Type) {
        debug!("gutter cvt show");
    }
    fn size_allocate(&self, obj: &Self::Type, w: i32, h: i32, bl: i32) {
        self.parent_size_allocate(obj, w, h, bl);
        // dbg!(w, h, bl);
        debug!("gutter cvt size allocate");
    }
}

impl GutterPrivate {
    fn change_gutter_nchars(&mut self, obj: &Gutter, nchars: usize) {
        if nchars != self.gutter_nchars {
            self.gutter_nchars = nchars;
            obj.queue_resize();
        }
    }

    fn buffer_changed(&self, gutter: &Gutter) {
        gutter.queue_draw();
        gutter.queue_resize();
    }

    fn handle_draw(&self, cv: &Gutter, snapshot: &gtk::Snapshot) {
        let draw_start = Instant::now();

        // let css_provider = gtk::CssProvider::new();
        // css_provider.load_from_data("* { background-color: #000000; }".as_bytes());
        // let ctx = cv.style_context();
        // ctx.add_provider(&css_provider, 1);
        // let foreground = self.model.main_state.borrow().theme.foreground;
        let theme = &self.theme;

        let da_width = cv.allocated_width();
        let da_height = cv.allocated_height();

        let mut workspace = self.workspace.get().unwrap().borrow_mut();
        let view_id = self.view_id;
        let (buffer, text_theme) = workspace.buffer_and_theme(view_id);

        //debug!("Drawing");
        // cr.select_font_face("Mono", ::cairo::enums::FontSlant::Normal, ::cairo::enums::FontWeight::Normal);
        // let mut font_options = cr.get_font_options();
        // debug!("font options: {:?} {:?} {:?}", font_options, font_options.get_antialias(), font_options.get_hint_style());
        // font_options.set_hint_style(HintStyle::Full);

        // let (text_width, text_height) = self.get_text_size();
        let num_lines = buffer.len_lines();

        // Calculate ordinal or max line length
        let padding: usize = format!("{}", num_lines).len();
        // dbg!(padding);

        let vadj = self.vadj.clone();

        // We round the values from the scrollbars, because if we don't, rectangles
        // will be antialiased and lines will show up inbetween highlighted lines
        // of text.
        let vadj_value = f64::round(vadj.value());
        let hadj_value = 0f64;
        trace!("gutter drawing.  vadj={}, {}", vadj.value(), vadj.upper());

        // TESTING
        let pango_ctx = cv.pango_context();
        let mut font_height = 15.0;
        let mut font_ascent = 15.0;
        let mut font_width = 15.0;
        if let Some(metrics) = pango_ctx.metrics(None, None) {
            font_height = metrics.height() as f64 / pango::SCALE as f64;
            font_ascent = metrics.ascent() as f64 / pango::SCALE as f64;
            font_width = metrics.approximate_digit_width() as f64 / pango::SCALE as f64;
        }

        // cv.size_allocate(Rectangle::new(), -1);

        let first_line = (vadj_value / font_height) as usize;
        let last_line = ((vadj_value + f64::from(da_height)) / font_height) as usize + 1;
        let last_line = min(last_line, num_lines);
        let visible_lines = first_line..last_line;

        let pango_ctx = cv.pango_context();

        // Draw background
        // need to set color to text_theme.background?
        let mut bg_color = gdk::RGBA::white();
        if let Some(gutter_bg) = text_theme.gutter.bg {
            bg_color.red = gutter_bg.r_f32();
            bg_color.green = gutter_bg.g_f32();
            bg_color.blue = gutter_bg.b_f32();
        }

        let rect_node = gtk::gsk::ColorNode::new(
            &bg_color,
            &graphene::Rect::new(0.0, 0.0, da_width as f32, da_height as f32),
        );
        snapshot.append_node(&rect_node);

        /*
        // set_source_color(cr, theme.foreground);
        cr.set_source_rgba(
            text_theme.fg.r_f64(),
            text_theme.fg.g_f64(),
            text_theme.fg.b_f64(),
            1.0,
        );
        */

        // Highlight cursor lines
        // for i in first_line..last_line {
        //     cr.set_source_rgba(0.8, 0.8, 0.8, 1.0);
        //     if let Some(line) = self.line_cache.get_line(i) {
        //         if !line.cursor().is_empty() {
        //             cr.set_source_rgba(0.23, 0.23, 0.23, 1.0);
        //             cr.rectangle(
        //                 0f64,
        //                 font_extents.height * ((i + 1) as f64) - font_extents.ascent - vadj.get_value(),
        //                 da_width as f64,
        //                 font_extents.ascent + font_extents.descent,
        //             );
        //             cr.fill();
        //         }
        //     }
        // }

        const CURSOR_WIDTH: f64 = 2.0;
        let mut max_width = 0;
        for i in visible_lines {
            // Keep track of the starting x position
            if let Some((line, attrs)) = buffer.get_line_with_attributes(view_id, i, &text_theme) {
                // let line = buffer.line(i);

                /*
                cr.move_to(-hadj_value, state.font_height * (i as f64) - vadj_value);

                cr.set_source_rgba(
                    text_theme.fg.r_f64(),
                    text_theme.fg.g_f64(),
                    text_theme.fg.b_f64(),
                    1.0,
                );
                 */

                self.append_text_to_snapshot(
                    cv,
                    text_theme,
                    snapshot,
                    &format!("{:>offset$} ", i + 1, offset = padding),
                    pango::AttrList::new(),
                    -hadj_value as f32,
                    font_ascent as f32 + font_height as f32 * (i as f32) - vadj_value as f32,
                );
                // Draw the cursors
                /*
                cr.set_source_rgba(
                    text_theme.cursor.r_f64(),
                    text_theme.cursor.g_f64(),
                    text_theme.cursor.b_f64(),
                    1.0,
                );

                for sel in buffer.selections(view_id) {
                    if buffer.char_to_line(sel.cursor()) != i {
                        continue;
                    }
                    let line_byte = buffer.char_to_byte(sel.cursor()) - buffer.line_to_byte(i);
                    let x = layout_line.index_to_x(line_byte as i32, false) / pango::SCALE;
                    cr.rectangle(
                        (x as f64) - hadj_value,
                        (((state.font_height) as usize) * i) as f64 - vadj_value,
                        CURSOR_WIDTH,
                        state.font_height,
                    );
                    cr.fill();
                }
                */
            }
        }

        /*
        if hadj.get_upper() != h_upper {
            hadj.set_upper(h_upper);
            // If I don't signal that the value changed, sometimes the overscroll "shadow" will stick
            // This seems to make sure to tell the viewport that something has changed so it can
            // reevaluate its need for a scroll shadow.
            hadj.value_changed();
        }
        */

        let draw_end = Instant::now();
        debug!("drawing took {}ms", (draw_end - draw_start).as_millis());
    }

    /// Creates a pango attr list from eddy attributes
    fn create_pango_attr_list(&self, attr_spans: &[AttrSpan]) -> pango::AttrList {
        let attr_list = pango::AttrList::new();
        for aspan in attr_spans {
            let mut pattr = match aspan.attr {
                Attr::ForegroundColor(color) => {
                    Attribute::new_foreground(color.r_u16(), color.g_u16(), color.b_u16())
                }
                Attr::BackgroundColor(color) => {
                    Attribute::new_background(color.r_u16(), color.g_u16(), color.b_u16())
                }
            };
            pattr.set_start_index(aspan.start_idx as u32);
            pattr.set_end_index(aspan.end_idx as u32);
            attr_list.insert(pattr);
        }

        attr_list
    }

    /// Creates a pango layout for a particular line
    fn create_layout_for_line(
        &self,
        pango_ctx: &pango::Context,
        line: &RopeSlice,
        attr_spans: &[AttrSpan],
    ) -> pango::Layout {
        let layout = pango::Layout::new(pango_ctx);
        let text: Cow<str> = (*line).into();
        layout.set_text(&text);

        let attr_list = pango::AttrList::new();
        for aspan in attr_spans {
            let mut pattr = match aspan.attr {
                Attr::ForegroundColor(color) => {
                    Attribute::new_foreground(color.r_u16(), color.g_u16(), color.b_u16())
                }
                Attr::BackgroundColor(color) => {
                    Attribute::new_background(color.r_u16(), color.g_u16(), color.b_u16())
                }
            };
            pattr.set_start_index(aspan.start_idx as u32);
            pattr.set_end_index(aspan.end_idx as u32);
            attr_list.insert(pattr);
        }

        layout.set_attributes(Some(&attr_list));
        layout
    }

    fn append_text_to_snapshot(
        &self,
        cv: &Gutter,
        text_theme: &eddy_workspace::style::Theme,
        snapshot: &gtk::Snapshot,
        text: &str,
        attrs: pango::AttrList,
        x: f32,
        y: f32,
    ) {
        let pango_ctx = cv.pango_context();

        let items = pango::itemize_with_base_dir(
            &pango_ctx,
            pango::Direction::Ltr,
            text,
            0,
            text.len() as i32,
            &attrs,
            None,
        );

        let mut x_off = 0.0;
        for item in items {
            let mut glyphs = pango::GlyphString::new();
            let item_text = unsafe {
                std::str::from_utf8_unchecked(
                    &text.as_bytes()
                        [item.offset() as usize..item.offset() as usize + item.length() as usize],
                )
            };
            // dbg!(item_text);
            let mut color = gdk::RGBA::black();
            if let Some(gutter_fg) = text_theme.gutter.fg {
                color.red = gutter_fg.r_f32();
                color.green = gutter_fg.g_f32();
                color.blue = gutter_fg.b_f32();
            }
            // dbg!(color.red, color.green, color.blue);
            for attr in &item.analysis().extra_attrs() {
                // dbg!(
                //     attr.get_start_index(),
                //     attr.get_end_index(),
                //     attr.get_attr_class().type_()
                // );
                if let Some(ca) = attr.downcast_ref::<pango::AttrColor>() {
                    let pc = ca.color();
                    color.red = pc.red() as f32 / 65536.0;
                    color.green = pc.green() as f32 / 65536.0;
                    color.blue = pc.blue() as f32 / 65536.0;
                }
                // dbg!(format!("{}", ca.color()));
            }
            pango::shape_full(item_text, None, item.analysis(), &mut glyphs);
            // this calculates width
            let width = glyphs.width() as f32 / pango::SCALE as f32;

            if let Some(text_node) = gtk::gsk::TextNode::new(
                &item.analysis().font(),
                &mut glyphs,
                &color,
                &graphene::Point::new(x + x_off, y),
            ) {
                let width = cv.allocated_width();
                let height = cv.allocated_height();
                let clip_node = gtk::gsk::ClipNode::new(
                    &text_node,
                    &graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
                );

                snapshot.append_node(&clip_node);
            }
            x_off += width;
        }
    }
}

glib::wrapper! {
    pub struct Gutter(ObjectSubclass<GutterPrivate>)
    @extends gtk::Widget;
}

impl Gutter {
    pub fn new() -> Self {
        let gutter = glib::Object::new::<Self>(&[]).unwrap();
        let gutter_priv = GutterPrivate::from_instance(&gutter);

        gutter
    }

    pub fn set_sender(&self, sender: Sender<Action>) {
        let code_view_priv = GutterPrivate::from_instance(self);
        let _ = code_view_priv.sender.set(sender);
    }

    pub fn set_workspace(&self, workspace: Rc<RefCell<Workspace>>) {
        let code_view_priv = GutterPrivate::from_instance(self);
        let _ = code_view_priv.workspace.set(workspace);
    }

    pub fn buffer_changed(&self) {
        let code_view_priv = GutterPrivate::from_instance(self);
        code_view_priv.buffer_changed(self);
    }
}
