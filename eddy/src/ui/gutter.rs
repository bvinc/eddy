use eddy_workspace::style::{Attr, AttrSpan, Color};
use eddy_workspace::Workspace;
use gdk::Key;
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
use std::cell::{Cell, RefCell};
use std::cmp::{max, min};
use std::rc::Rc;
use std::time::Instant;

use crate::app::Action;
use crate::theme::Theme;

pub struct GutterPrivate {
    vadj: RefCell<Adjustment>,
    sender: OnceCell<Sender<Action>>,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: usize,
    theme: Theme,
    gutter_nchars: Cell<usize>,
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
        let vadj = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let gutter_nchars = Cell::new(0);

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

        let nchars = std::cmp::max(self.gutter_nchars.get(), 2) + 2;

        let pango_ctx = obj.pango_context();
        if let Some(metrics) = pango_ctx.metrics(None, None) {
            let font_width = metrics.approximate_digit_width() as f64 / pango::SCALE as f64;
            let minimum_size = nchars as i32 * font_width as i32;
            let natural_size = nchars as i32 * font_width as i32;
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
        if nchars != self.gutter_nchars.get() {
            self.gutter_nchars.set(nchars);
            obj.queue_resize();
        }
    }

    fn buffer_changed(&self, gutter: &Gutter) {
        let mut workspace = self.workspace.get().unwrap().borrow_mut();
        let view_id = self.view_id;
        let (buffer, _) = workspace.buffer_and_theme(view_id);
        let max_line_num = buffer.len_lines();
        self.gutter_nchars.set(format!("{}", max_line_num).len());

        gutter.queue_draw();
        gutter.queue_resize();
    }

    fn handle_draw(&self, cv: &Gutter, snapshot: &gtk::Snapshot) {
        let draw_start = Instant::now();

        let theme = &self.theme;

        let da_width = cv.allocated_width();
        let da_height = cv.allocated_height();

        let mut workspace = self.workspace.get().unwrap().borrow_mut();
        let view_id = self.view_id;
        let (buffer, text_theme) = workspace.buffer_and_theme(view_id);

        // let (text_width, text_height) = self.get_text_size();
        let num_lines = buffer.len_lines();

        let vadj = self.vadj.borrow().clone();

        // We round the values from the scrollbars, because if we don't, rectangles
        // will be antialiased and lines will show up inbetween highlighted lines
        // of text.
        let vadj_value = f64::round(vadj.value());
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
        // debug!("visible lines {} {}", first_line, last_line);

        let pango_ctx = cv.pango_context();

        // Draw background
        // need to set color to text_theme.background?
        let mut bg_color = gdk::RGBA::WHITE;
        change_to_color(&mut bg_color, text_theme.gutter.bg);

        let rect_node = gtk::gsk::ColorNode::new(
            &bg_color,
            &graphene::Rect::new(0.0, 0.0, da_width as f32, da_height as f32),
        );
        snapshot.append_node(&rect_node);

        // Highlight cursor lines
        let mut highlight_bg_color = gdk::RGBA::WHITE;
        change_to_color(&mut highlight_bg_color, text_theme.gutter.bg);
        change_to_color(&mut highlight_bg_color, text_theme.gutter_line_highlight.bg);
        for i in first_line..last_line {
            for sel in buffer.selections(view_id) {
                if buffer.char_to_line(sel.cursor()) != i {
                    continue;
                }

                let rect_node = gtk::gsk::ColorNode::new(
                    &highlight_bg_color,
                    &graphene::Rect::new(
                        0.0,
                        font_height as f32 * (i as f32) - vadj_value as f32,
                        da_width as f32,
                        font_height as f32,
                    ),
                );

                let clip_node = gtk::gsk::ClipNode::new(
                    &rect_node,
                    &graphene::Rect::new(0.0, 0.0, da_width as f32, da_height as f32),
                );

                snapshot.append_node(&clip_node);
                break;
            }
        }

        // Calculate ordinal or max line length
        let nchars: usize = std::cmp::max(format!("{}", num_lines).len(), 2);

        for i in visible_lines {
            let mut fg_color = gdk::RGBA::BLACK;
            change_to_color(&mut fg_color, text_theme.gutter.fg);

            for sel in buffer.selections(view_id) {
                if buffer.char_to_line(sel.cursor()) != i {
                    continue;
                }
                change_to_color(&mut fg_color, text_theme.gutter_line_highlight.fg);
                break;
            }

            // Keep track of the starting x position
            if let Some((_, _)) = buffer.get_line_with_attributes(view_id, i, &text_theme) {
                self.append_text_to_snapshot(
                    cv,
                    fg_color,
                    snapshot,
                    &format!("{:>offset$}", i + 1, offset = nchars + 1),
                    pango::AttrList::new(),
                    0.0,
                    font_ascent as f32 + font_height as f32 * (i as f32) - vadj_value as f32,
                );
            }
        }

        let draw_end = Instant::now();
        debug!(
            "drawing gutter took {}ms",
            (draw_end - draw_start).as_millis()
        );
    }

    fn append_text_to_snapshot(
        &self,
        cv: &Gutter,
        color: gdk::RGBA,
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

            // dbg!(color.red, color.green, color.blue);
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
    pub fn new(workspace: Rc<RefCell<Workspace>>, sender: Sender<Action>) -> Self {
        let gutter = glib::Object::new::<Self>(&[]).unwrap();
        let gutter_priv = GutterPrivate::from_instance(&gutter);

        let _ = gutter_priv.workspace.set(workspace);
        let _ = gutter_priv.sender.set(sender);

        gutter_priv.buffer_changed(&gutter);

        gutter
    }

    pub fn set_vadjust(&self, adj: &Adjustment) {
        let gutter_priv = GutterPrivate::from_instance(self);
        gutter_priv.vadj.replace(adj.clone());
        gutter_priv
            .vadj
            .borrow()
            .connect_value_changed(clone!(@weak self as gutter => move |_| {
                gutter.queue_draw();
            }));
    }

    pub fn buffer_changed(&self) {
        let code_view_priv = GutterPrivate::from_instance(self);
        code_view_priv.buffer_changed(self);
    }
}

fn change_to_color(gc: &mut gdk::RGBA, c: Option<Color>) {
    if let Some(c) = c {
        gc.set_red(c.r_f32());
        gc.set_green(c.g_f32());
        gc.set_blue(c.b_f32());
    }
}
