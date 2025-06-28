use crate::components::gutter::GutterComponent;
use crate::theme::Theme;
use eddy_model::style::Color;
use eddy_model::Buffer;

use gflux::ComponentCtx;
use glib::clone;
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, Adjustment};
use log::*;

use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};
use std::cmp::min;
use std::collections::HashSet;
use std::time::Instant;

pub struct GutterPrivate {
    vadj: RefCell<Adjustment>,
    ctx: OnceCell<ComponentCtx<GutterComponent>>,
    view_id: Cell<usize>,
    theme: Theme,
    gutter_nchars: Cell<usize>,
    highlighted_lines: RefCell<HashSet<usize>>,
}

#[glib::object_subclass]
impl ObjectSubclass for GutterPrivate {
    const NAME: &'static str = "Gutter";
    type Type = Gutter;
    type ParentType = gtk::Widget;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    fn new() -> Self {
        let ctx = OnceCell::new();
        let view_id = Cell::new(0);
        let theme = Theme::default();
        let vadj = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let gutter_nchars = Cell::new(0);

        Self {
            vadj,
            ctx,
            view_id,
            theme,
            gutter_nchars,
            highlighted_lines: RefCell::new(HashSet::new()),
        }
    }
}

impl ObjectImpl for GutterPrivate {
    fn constructed(&self) {
        self.parent_constructed();

        let pango_ctx = self.obj().pango_context();
        let mut font_desc = pango::FontDescription::new();
        font_desc.set_family("Hack, Mono");
        font_desc.set_size(16384);
        pango_ctx.set_font_description(Some(&font_desc));
    }
}
impl WidgetImpl for GutterPrivate {
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        // snapshot.render_layout(&ctx, 10.0, 10.0, &layout);
        // snapshot.render_background(&ctx, 10.0, 10.0, 30.0, 20.0);
        self.handle_draw(&self.obj(), snapshot);
    }
    fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
        self.parent_measure(orientation, for_size);

        let nchars = std::cmp::max(self.gutter_nchars.get(), 2) + 2;

        let pango_ctx = self.obj().pango_context();
        let metrics = pango_ctx.metrics(None, None);
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
    }
    fn size_allocate(&self, w: i32, h: i32, bl: i32) {
        self.parent_size_allocate(w, h, bl);
        // dbg!(w, h, bl);
        debug!("gutter cvt size allocate");
    }
}

impl GutterPrivate {
    fn with_buffer<F, R>(&self, f: F) -> R
    where
        F: Fn(&Buffer) -> R,
    {
        self.ctx
            .get()
            .unwrap()
            .with_model(|ws| f(ws.buffer(self.view_id.get())))
    }

    fn with_buffer_mut<F, R>(&self, f: F) -> R
    where
        F: Fn(&mut Buffer) -> R,
    {
        self.ctx
            .get()
            .unwrap()
            .with_model_mut(|ws| f(ws.buffer_mut(self.view_id.get())))
    }

    fn change_gutter_nchars(&mut self, obj: &Gutter, nchars: usize) {
        if nchars != self.gutter_nchars.get() {
            self.gutter_nchars.set(nchars);
            obj.queue_resize();
        }
    }

    fn buffer_changed(&self, gutter: &Gutter) {
        let _view_id = self.view_id.get();
        let max_line_num = self.with_buffer(|b| b.len_lines());
        self.gutter_nchars.set(format!("{max_line_num}").len());

        gutter.queue_draw();
        gutter.queue_resize();
    }

    fn handle_draw(&self, cv: &Gutter, snapshot: &gtk::Snapshot) {
        let draw_start = Instant::now();

        let _theme = &self.theme;

        let da_width = cv.allocated_width();
        let da_height = cv.allocated_height();

        let text_theme = self.ctx.get().unwrap().with_model(|ws| ws.theme.clone());
        let view_id = self.view_id.get();

        // let (text_width, text_height) = self.get_text_size();
        let num_lines = self.with_buffer(|b| b.len_lines());

        let vadj = self.vadj.borrow().clone();

        // We round the values from the scrollbars, because if we don't, rectangles
        // will be antialiased and lines will show up inbetween highlighted lines
        // of text.
        let vadj_value = f64::round(vadj.value());
        trace!("gutter drawing.  vadj={}, {}", vadj.value(), vadj.upper());

        // TESTING
        let pango_ctx = cv.pango_context();
        let metrics = pango_ctx.metrics(None, None);
        let font_height = metrics.height() as f64 / pango::SCALE as f64;
        let font_ascent = metrics.ascent() as f64 / pango::SCALE as f64;

        // cv.size_allocate(Rectangle::new(), -1);

        let first_line = (vadj_value / font_height) as usize;
        let last_line = ((vadj_value + f64::from(da_height)) / font_height) as usize + 1;
        let last_line = min(last_line, num_lines);
        let visible_lines = first_line..last_line;
        // debug!("visible lines {} {}", first_line, last_line);

        // Draw background
        // need to set color to text_theme.background?
        let mut bg_color = gdk::RGBA::WHITE;
        change_to_color(&mut bg_color, text_theme.gutter.bg);

        let rect_node = gtk::gsk::ColorNode::new(
            &bg_color,
            &graphene::Rect::new(0.0, 0.0, da_width as f32, da_height as f32),
        );
        snapshot.append_node(&rect_node);

        // Figure out which of our lines need highlighting
        let mut highlighted_lines = self.highlighted_lines.borrow_mut();
        highlighted_lines.clear();
        let selections = self.with_buffer(|b| b.selections(view_id).to_vec());
        for sel in selections {
            let line = self.with_buffer(|b| b.char_to_line(sel.cursor()));
            if visible_lines.contains(&line) {
                highlighted_lines.insert(line);
            }
        }

        // Highlight cursor lines
        let mut highlight_bg_color = gdk::RGBA::WHITE;
        change_to_color(&mut highlight_bg_color, text_theme.gutter.bg);
        change_to_color(&mut highlight_bg_color, text_theme.gutter_line_highlight.bg);
        for &line in highlighted_lines.iter() {
            let rect_node = gtk::gsk::ColorNode::new(
                &highlight_bg_color,
                &graphene::Rect::new(
                    0.0,
                    font_height as f32 * (line as f32) - vadj_value as f32,
                    da_width as f32,
                    font_height as f32,
                ),
            );
            append_clipped_node(snapshot, rect_node, da_width, da_height);
        }

        // Calculate ordinal or max line length
        let nchars: usize = std::cmp::max(format!("{num_lines}").len(), 2);

        for line in visible_lines {
            let mut fg_color = gdk::RGBA::BLACK;
            change_to_color(&mut fg_color, text_theme.gutter.fg);

            if highlighted_lines.contains(&line) {
                change_to_color(&mut fg_color, text_theme.gutter_line_highlight.fg);
            }

            self.append_text_to_snapshot(
                cv,
                fg_color,
                snapshot,
                &format!("{:>offset$}", line + 1, offset = nchars + 1),
                pango::AttrList::new(),
                0.0,
                font_ascent as f32 + font_height as f32 * (line as f32) - vadj_value as f32,
            );
        }

        let draw_end = Instant::now();
        debug!(
            "drawing gutter took {}ms",
            (draw_end - draw_start).as_millis()
        );
    }

    #[allow(clippy::too_many_arguments)]
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
                &glyphs,
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

fn append_clipped_node<P: AsRef<gtk::gsk::RenderNode>>(
    snapshot: &gtk::Snapshot,
    node: P,
    w: i32,
    h: i32,
) {
    let clip_node =
        gtk::gsk::ClipNode::new(&node, &graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
    snapshot.append_node(&clip_node);
}

glib::wrapper! {
    pub struct Gutter(ObjectSubclass<GutterPrivate>)
    @extends gtk::Widget;
}

impl Gutter {
    pub fn new(ctx: ComponentCtx<GutterComponent>, view_id: usize) -> Self {
        let gutter = glib::Object::new::<Self>();
        let gutter_priv = GutterPrivate::from_obj(&gutter);

        let _ = gutter_priv.ctx.set(ctx);
        gutter_priv.view_id.set(view_id);

        gutter_priv.buffer_changed(&gutter);

        gutter
    }

    pub fn set_vadjust(&self, adj: &Adjustment) {
        let gutter_priv = GutterPrivate::from_obj(self);
        gutter_priv.vadj.replace(adj.clone());
        gutter_priv.vadj.borrow().connect_value_changed(clone!(
            #[weak(rename_to = gutter)]
            self,
            move |_| {
                gutter.queue_draw();
            }
        ));
    }
}

fn change_to_color(gc: &mut gdk::RGBA, c: Option<Color>) {
    if let Some(c) = c {
        gc.set_red(c.r_f32());
        gc.set_green(c.g_f32());
        gc.set_blue(c.b_f32());
    }
}
