use eddy_workspace::style::{Attr, AttrSpan};
use eddy_workspace::Workspace;
use gdk::keys::Key;
use gdk::ModifierType;
use glib::clone;
use glib::ParamSpec;
use glib::Sender;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib::subclass, Adjustment};
use log::*;
use lru_cache::LruCache;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;
use pango::{Attribute, FontDescription};
use ropey::RopeSlice;
use std::borrow::Cow;
use std::cell::Cell;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::collections::hash_map;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::app::Action;
use crate::theme::Theme;
use crate::ui::{Layout, LayoutItem, LayoutLine};

pub struct CodeViewTextPrivate {
    hadj: RefCell<Adjustment>,
    hscroll_policy: gtk::ScrollablePolicy,
    vadj: RefCell<Adjustment>,
    vscroll_policy: gtk::ScrollablePolicy,
    sender: OnceCell<Sender<Action>>,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: usize,
    theme: Theme,
    layout: Rc<RefCell<Layout>>,
}

#[glib::object_subclass]
impl ObjectSubclass for CodeViewTextPrivate {
    const NAME: &'static str = "CodeViewText";
    type Type = CodeViewText;
    type ParentType = gtk::Widget;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;
    type Interfaces = (gtk::Scrollable,);

    fn new() -> Self {
        let sender = OnceCell::new();
        let workspace = OnceCell::new();
        let view_id = 0;
        let theme = Theme::default();

        let hadj = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let vadj = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let layout = Rc::new(RefCell::new(Layout::new()));

        Self {
            hadj,
            hscroll_policy: gtk::ScrollablePolicy::Minimum,
            vadj,
            vscroll_policy: gtk::ScrollablePolicy::Minimum,
            sender,
            workspace,
            view_id,
            theme,
            layout,
        }
    }
}

impl ObjectImpl for CodeViewTextPrivate {
    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            vec![
                ParamSpec::new_object(
                    "hadjustment",
                    "Horizontal Adjustment",
                    "Horizontal `GtkAdjustment` of the scrollable widget",
                    gtk::Adjustment::static_type(),
                    glib::ParamFlags::READWRITE,
                ),
                ParamSpec::new_enum(
                    "hscroll-policy",
                    "Horizontal Scroll Policy",
                    "Determines when horizontal scrolling should start",
                    gtk::ScrollablePolicy::static_type(),
                    0,
                    glib::ParamFlags::READWRITE,
                ),
                ParamSpec::new_object(
                    "vadjustment",
                    "Vertical Adjustment",
                    "Vertical `GtkAdjustment` of the scrollable widget",
                    gtk::Adjustment::static_type(),
                    glib::ParamFlags::READWRITE,
                ),
                ParamSpec::new_enum(
                    "vscroll-policy",
                    "Vertical Scroll Policy",
                    "Determines when vertical scrolling should start",
                    gtk::ScrollablePolicy::static_type(),
                    0,
                    glib::ParamFlags::READWRITE,
                ),
            ]
        });
        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        editable: &Self::Type,
        id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
    }

    fn property(&self, editable: &Self::Type, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        dbg!(pspec.name());
        match pspec.name() {
            "hadjustment" => self.hadj.borrow().to_value(),
            "hscroll-policy" => self.hscroll_policy.to_value(),
            "vadjustment" => self.vadj.borrow().to_value(),
            "vscroll-policy" => self.vscroll_policy.to_value(),
            _ => 0.to_value(),
        }
    }

    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        let pango_ctx = obj.pango_context();
        let mut font_desc = pango::FontDescription::new();
        font_desc.set_family("Hack, Mono");
        font_desc.set_size(16384);
        pango_ctx.set_font_description(&font_desc);

        obj.set_focusable(true);
        obj.set_can_focus(true);

        obj.set_valign(gtk::Align::Fill);
        obj.set_halign(gtk::Align::Fill);
        obj.set_vexpand(true);
        obj.set_hexpand(true);
        dbg!(obj.valign(), obj.halign());

        let gesture_click = gtk::GestureClick::new();
        gesture_click.connect_pressed(clone!(@strong obj as this => move |_w, n_press, x, y| {
            this.button_pressed(n_press, x, y);
            this.grab_focus();
            debug!("cvt clicked");
        }));
        obj.add_controller(&gesture_click);

        let event_controller_key = gtk::EventControllerKey::new();
        event_controller_key.connect_key_pressed(
            clone!(@strong obj as this => move |_,key, code, state| {
                this.key_pressed(key, code, state);
                gtk::Inhibit(true)
            }),
        );
        obj.add_controller(&event_controller_key);

        let event_controller_scroll = gtk::EventControllerScroll::builder()
            .flags(
                gtk::EventControllerScrollFlags::BOTH_AXES
                    | gtk::EventControllerScrollFlags::KINETIC,
            )
            .name("codeviewtext")
            .propagation_limit(gtk::PropagationLimit::SameNative)
            .propagation_phase(gtk::PropagationPhase::Target)
            .build();
        event_controller_scroll.connect_decelerate(clone!(@strong obj as this => move |_,a,b| {
            dbg!("connect_decelerate", a, b);
        }));
        event_controller_scroll.connect_scroll(clone!(@strong obj as this => move |_,a,b| {
            dbg!("connect_scroll", a, b);
            gtk::Inhibit(true)
        }));
        event_controller_scroll.connect_scroll_begin(clone!(@strong obj as this => move |_| {
            dbg!("connect_scroll");
        }));
        event_controller_scroll.connect_scroll_end(clone!(@strong obj as this => move |_| {
            dbg!("connect_scroll_end");
        }));
        event_controller_scroll.connect_flags_notify(clone!(@strong obj as this => move |_| {
            dbg!("connect_flags_notify");
        }));
        obj.add_controller(&event_controller_scroll);
    }
}
impl WidgetImpl for CodeViewTextPrivate {
    fn snapshot(&self, code_view: &CodeViewText, snapshot: &gtk::Snapshot) {
        // snapshot.render_layout(&ctx, 10.0, 10.0, &layout);
        // snapshot.render_background(&ctx, 10.0, 10.0, 30.0, 20.0);
        self.handle_draw(code_view, snapshot);
    }
    // fn compute_expand(&self, obj: &Self::Type, hexpand: &mut bool, vexpand: &mut bool) {
    //     self.parent_compute_expand(obj, hexpand, vexpand);
    //     debug!("compute expand");
    //     dbg!(hexpand, vexpand);
    // }
    // fn map(&self, obj: &Self::Type) {
    //     self.parent_map(obj);
    //     debug!("cvt map");
    // }
    fn measure(
        &self,
        obj: &Self::Type,
        orientation: gtk::Orientation,
        for_size: i32,
    ) -> (i32, i32, i32, i32) {
        self.parent_measure(obj, orientation, for_size);
        debug!("cvt measure {}", orientation);
        (100, 100, -1, -1)
    }
    // fn show(&self, _: &Self::Type) {
    //     debug!("cvt show");
    // }
    fn size_allocate(&self, obj: &Self::Type, w: i32, h: i32, bl: i32) {
        self.parent_size_allocate(obj, w, h, bl);
        dbg!(w, h, bl);
        debug!("cvt size allocate");

        let vadj = self.vadj.borrow().clone();
        vadj.set_page_size(f64::from(h));
        let hadj = self.hadj.borrow().clone();
        hadj.set_page_size(f64::from(w));

        self.buffer_changed(obj);
    }
}
impl BoxImpl for CodeViewTextPrivate {}
impl ScrollableImpl for CodeViewTextPrivate {
    // fn border(&self, _: &Self::Type) -> Option<gtk::Border> {
    //     dbg!("cvt border");
    //     Some(gtk::Border::builder().right(10).bottom(10).build())
    // }
}

impl CodeViewTextPrivate {
    fn buffer_changed(&self, cvt: &CodeViewText) {
        cvt.queue_draw();

        let mut workspace = self.workspace.get().unwrap().borrow_mut();
        let view_id = self.view_id;
        let (buffer, text_theme) = workspace.buffer_and_theme(view_id);

        let pango_ctx = cvt.pango_context();
        let mut font_height = 15.0;
        let mut font_ascent = 15.0;
        if let Some(metrics) = pango_ctx.metrics(None, None) {
            font_height = metrics.height() as f64 / pango::SCALE as f64;
            font_ascent = metrics.ascent() as f64 / pango::SCALE as f64;
        }

        // let (text_width, text_height) = self.get_text_size(state);
        let text_height = buffer.len_lines() as f64 * font_height;
        let da_height = f64::from(cvt.allocated_height());
        let vadj = self.vadj.borrow().clone();
        let hadj = self.hadj.borrow().clone();

        // update scrollbars to the new text width and height
        vadj.set_lower(0f64);
        let upper = if da_height > text_height {
            da_height
        } else {
            text_height
        };
        vadj.set_upper(upper);

        // If the last line was removed, scroll up so we're not overscrolled
        if vadj.value() + vadj.page_size() > vadj.upper() {
            vadj.set_value(vadj.upper() - vadj.page_size())
        }
    }

    fn handle_draw(&self, cv: &CodeViewText, snapshot: &gtk::Snapshot) {
        let draw_start = Instant::now();

        // let css_provider = gtk::CssProvider::new();
        // css_provider.load_from_data("* { background-color: #000000; }".as_bytes());
        let ctx = cv.style_context();
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

        let vadj = self.vadj.borrow().clone();
        let hadj = self.hadj.borrow().clone();

        // We round the values from the scrollbars, because if we don't, rectangles
        // will be antialiased and lines will show up inbetween highlighted lines
        // of text.
        let vadj_value = f64::round(vadj.value());
        let hadj_value = f64::round(hadj.value());
        trace!(
            "drawing cvt.  height={} width={}, vadj={}, {}",
            da_height,
            da_width,
            vadj.value(),
            vadj.upper()
        );

        // TESTING
        let pango_ctx = cv.pango_context();
        let mut font_height = 15.0;
        let mut font_ascent = 15.0;
        if let Some(metrics) = pango_ctx.metrics(None, None) {
            font_height = metrics.height() as f64 / pango::SCALE as f64;
            font_ascent = metrics.ascent() as f64 / pango::SCALE as f64;
        }

        let first_line = (vadj_value / font_height) as usize;
        let last_line = ((vadj_value + f64::from(da_height)) / font_height) as usize + 1;
        let last_line = min(last_line, num_lines);
        let visible_lines = first_line..last_line;

        // Draw background
        // need to set color to text_theme.background?
        let mut bg_color = gdk::RGBA::black();
        bg_color.red = text_theme.bg.r_f32();
        bg_color.green = text_theme.bg.g_f32();
        bg_color.blue = text_theme.bg.b_f32();

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

        let mut layout = self.layout.borrow_mut();
        layout.clear();
        let mut filtered_line = String::new();

        const CURSOR_WIDTH: f64 = 2.0;
        let mut max_width = 0;
        for line_num in visible_lines {
            let mut layout_line = LayoutLine::new();
            // Keep track of the starting x position
            if let Some((line, attrs)) =
                buffer.get_line_with_attributes(view_id, line_num, &text_theme)
            {
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

                let text: Cow<str> = line.into();
                buffer.filter_line_to_display(&text, &mut filtered_line);
                let text = &filtered_line;

                let pango_attrs = self.create_pango_attr_list(&attrs);
                let line_x = -hadj_value as f32;
                let line_y =
                    font_ascent as f32 + font_height as f32 * (line_num as f32) - vadj_value as f32;
                // let text: Cow<str> = line.into();
                // self.append_text_to_snapshot(
                //     cv,
                //     text_theme,
                //     snapshot,
                //     &text,
                //     self.create_pango_attr_list(&attrs),
                //     -hadj_value as f32,
                //     font_ascent as f32 + font_height as f32 * (i as f32) - vadj_value as f32,
                // );

                let pango_ctx = cv.pango_context();

                let items = pango::itemize_with_base_dir(
                    &pango_ctx,
                    pango::Direction::Ltr,
                    &text,
                    0,
                    text.len() as i32,
                    &pango_attrs,
                    None,
                );

                let mut x_off = 0;
                for item in items {
                    let mut glyphs = pango::GlyphString::new();
                    let item_text = unsafe {
                        std::str::from_utf8_unchecked(
                            &text.as_bytes()[item.offset() as usize
                                ..item.offset() as usize + item.length() as usize],
                        )
                    };
                    // dbg!(item_text);
                    // if let Some(metrics) = item.analysis().font().metrics(None) {
                    //     dbg!(metrics.height(), metrics.ascent(), metrics.descent());
                    // }
                    let mut bg_color: Option<gdk::RGBA> = None;
                    let mut fg_color = gdk::RGBA::black();
                    fg_color.red = text_theme.fg.r_f32();
                    fg_color.green = text_theme.fg.g_f32();
                    fg_color.blue = text_theme.fg.b_f32();
                    // dbg!(color.red, color.green, color.blue);
                    for attr in &item.analysis().extra_attrs() {
                        if attr.type_() == pango::AttrType::Foreground {
                            if let Some(ca) = attr.downcast_ref::<pango::AttrColor>() {
                                let pc = ca.color();
                                fg_color.red = pc.red() as f32 / 65536.0;
                                fg_color.green = pc.green() as f32 / 65536.0;
                                fg_color.blue = pc.blue() as f32 / 65536.0;
                            }
                        }
                        if attr.type_() == pango::AttrType::Background {
                            if let Some(ca) = attr.downcast_ref::<pango::AttrColor>() {
                                let pc = ca.color();
                                let mut bgc = gdk::RGBA::black();
                                bgc.red = pc.red() as f32 / 65536.0;
                                bgc.green = pc.green() as f32 / 65536.0;
                                bgc.blue = pc.blue() as f32 / 65536.0;
                                bg_color = Some(bgc);
                            }
                        }
                    }
                    pango::shape_full(item_text, None, item.analysis(), &mut glyphs);
                    // this calculates width
                    let width = glyphs.width();

                    // Append text background node to snapshot
                    if let Some(bg_color) = bg_color {
                        let rect_node = gtk::gsk::ColorNode::new(
                            &bg_color,
                            &graphene::Rect::new(
                                line_x + (x_off as f32 / pango::SCALE as f32) as f32,
                                line_y - font_ascent as f32,
                                width as f32 / pango::SCALE as f32,
                                font_height as f32,
                            ),
                        );
                        snapshot.append_node(&rect_node);
                    }

                    // Append text node to snapshot
                    if let Some(text_node) = gtk::gsk::TextNode::new(
                        &item.analysis().font(),
                        &mut glyphs,
                        &fg_color,
                        &graphene::Point::new(
                            line_x + (x_off as f32 / pango::SCALE as f32) as f32,
                            line_y,
                        ),
                    ) {
                        // Lets clip the text node to the widget area
                        let width = cv.allocated_width();
                        let height = cv.allocated_height();
                        let clip_node = gtk::gsk::ClipNode::new(
                            &text_node,
                            &graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
                        );

                        snapshot.append_node(&clip_node);
                    }

                    layout_line.push(LayoutItem {
                        text: item_text.to_string(),
                        item,
                        glyphs,
                        x_off,
                    });
                    // match layout.entry(line_num) {
                    //     hash_map::Entry::Occupied(e) => ,
                    //     hash_map::Entry::Vacant(e) => ,
                    // }
                    // layout.entry(line_num).or_insert(vec![]).push(LayoutItem {
                    //     text: item_text.to_string(),
                    //     item,
                    //     glyphs,
                    //     x_off,
                    // });

                    // layout.insert(
                    //     line_num,
                    //     LayoutItem {
                    //         text: item_text.to_string(),
                    //         item,
                    //         glyphs,
                    //         x_off,
                    //     },
                    // );
                    x_off += width;
                }

                // Draw the cursors
                for sel in buffer.selections(view_id) {
                    if buffer.char_to_line(sel.cursor()) != line_num {
                        continue;
                    }
                    let line_byte =
                        buffer.char_to_byte(sel.cursor()) - buffer.line_to_byte(line_num);
                    let x = layout_line.index_to_x(line_byte) as f32 / pango::SCALE as f32;
                    // let x = 10;

                    let mut color = gdk::RGBA::black();
                    color.red = text_theme.fg.r_f32();
                    color.green = text_theme.fg.g_f32();
                    color.blue = text_theme.fg.b_f32();

                    let rect_node = gtk::gsk::ColorNode::new(
                        &color,
                        &graphene::Rect::new(
                            x - hadj_value as f32,
                            line_y - font_ascent as f32,
                            CURSOR_WIDTH as f32,
                            font_height as f32,
                        ),
                    );
                    snapshot.append_node(&rect_node);
                    // cr.rectangle(
                    //     (x as f64) - hadj_value,
                    //     (((state.font_height) as usize) * i) as f64 - vadj_value,
                    //     CURSOR_WIDTH,
                    //     state.font_height,
                    // );
                    // cr.fill();
                }

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

        // Now that we know actual length of the text, adjust the scrollbar properly.
        // But we need to make sure we don't make the upper value smaller than the current viewport
        let mut h_upper = f64::from(max_width / pango::SCALE);
        let cur_h_max = hadj_value + hadj.page_size();
        if cur_h_max > h_upper {
            h_upper = cur_h_max;
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

        // let font_desc_attr = Attribute::new_font_desc(&self.font_desc);
        // attr_list.insert(font_desc_attr);

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
        cv: &CodeViewText,
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
            // if let Some(metrics) = item.analysis().font().metrics(None) {
            //     dbg!(metrics.height(), metrics.ascent(), metrics.descent());
            // }
            let mut color = gdk::RGBA::black();
            color.red = text_theme.fg.r_f32();
            color.green = text_theme.fg.g_f32();
            color.blue = text_theme.fg.b_f32();
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

    //fn get_text_node(&self,
    fn button_pressed() {
        debug!("button pressed");
    }
}

glib::wrapper! {
    pub struct CodeViewText(ObjectSubclass<CodeViewTextPrivate>)
    @extends gtk::Widget,
    @implements gtk::Scrollable;
}

impl CodeViewText {
    pub fn new() -> Self {
        let code_view = glib::Object::new::<Self>(&[]).unwrap();
        let code_view_priv = CodeViewTextPrivate::from_instance(&code_view);

        // code_view.setup_widgets();
        // code_view.setup_signals();
        code_view
    }

    // fn setup_widgets(&self) {}

    // fn setup_signals(&self) {}

    pub fn set_sender(&self, sender: Sender<Action>) {
        let code_view_priv = CodeViewTextPrivate::from_instance(self);
        let _ = code_view_priv.sender.set(sender);
    }

    pub fn set_workspace(&self, workspace: Rc<RefCell<Workspace>>) {
        let code_view_priv = CodeViewTextPrivate::from_instance(self);
        let _ = code_view_priv.workspace.set(workspace);
    }

    pub fn set_hadjust(&self, adj: &Adjustment) {
        let mut code_view_priv = CodeViewTextPrivate::from_instance(self);
        code_view_priv.hadj.replace(adj.clone());
    }

    pub fn set_vadjust(&self, adj: &Adjustment) {
        let mut code_view_priv = CodeViewTextPrivate::from_instance(self);
        code_view_priv.vadj.replace(adj.clone());
    }

    pub fn buffer_changed(&self) {
        let code_view_priv = CodeViewTextPrivate::from_instance(self);
        code_view_priv.buffer_changed(self);
    }

    fn button_pressed(&self, n_pressed: i32, x: f64, y: f64) {}

    fn key_pressed(&self, key: Key, keycode: u32, state: ModifierType) {
        let self_ = CodeViewTextPrivate::from_instance(self);
        use gdk::keys::constants;
        debug!(
            "key press keyval={:?}, state={:?}, uc={:?}",
            key,
            state,
            key.to_unicode(),
        );
        let mut workspace = self_.workspace.get().unwrap().borrow_mut();
        let (buffer, _) = workspace.buffer_and_theme(self_.view_id);

        let view_id = self_.view_id;
        let ch = key.to_unicode();

        let alt = state.contains(ModifierType::ALT_MASK);
        let ctrl = state.contains(ModifierType::CONTROL_MASK);
        let meta = state.contains(ModifierType::META_MASK);
        let shift = state.contains(ModifierType::SHIFT_MASK);
        let norm = !alt && !ctrl && !meta;

        match key {
            constants::Delete if norm => buffer.delete_forward(view_id),
            constants::BackSpace if norm => buffer.delete_backward(view_id),
            constants::Return | constants::KP_Enter => {
                buffer.insert_newline(view_id);
            }
            constants::Tab if norm && !shift => buffer.insert_tab(view_id),
            constants::Up if norm && !shift => buffer.move_up(view_id),
            constants::Down if norm && !shift => buffer.move_down(view_id),
            constants::Left if norm && !shift => buffer.move_left(view_id),
            constants::Right if norm && !shift => buffer.move_right(view_id),
            constants::Up if norm && shift => {
                buffer.move_up_and_modify_selection(view_id);
            }
            constants::Down if norm && shift => {
                buffer.move_down_and_modify_selection(view_id);
            }
            constants::Left if norm && shift => {
                buffer.move_left_and_modify_selection(view_id);
            }
            constants::Right if norm && shift => {
                buffer.move_right_and_modify_selection(view_id);
            }
            constants::Left if ctrl && !shift => {
                buffer.move_word_left(view_id);
            }
            constants::Right if ctrl && !shift => {
                buffer.move_word_right(view_id);
            }
            constants::Left if ctrl && shift => {
                buffer.move_word_left_and_modify_selection(view_id);
            }
            constants::Right if ctrl && shift => {
                buffer.move_word_right_and_modify_selection(view_id);
            }
            constants::Home if norm && !shift => {
                buffer.move_to_left_end_of_line(view_id);
            }
            constants::End if norm && !shift => {
                buffer.move_to_right_end_of_line(view_id);
            }
            constants::Home if norm && shift => {
                buffer.move_to_left_end_of_line_and_modify_selection(view_id);
            }
            constants::End if norm && shift => {
                buffer.move_to_right_end_of_line_and_modify_selection(view_id);
            }
            constants::Home if ctrl && !shift => {
                buffer.move_to_beginning_of_document(view_id);
            }
            constants::End if ctrl && !shift => {
                buffer.move_to_end_of_document(view_id);
            }
            constants::Home if ctrl && shift => {
                buffer.move_to_beginning_of_document_and_modify_selection(view_id);
            }
            constants::End if ctrl && shift => {
                buffer.move_to_end_of_document_and_modify_selection(view_id);
            }
            constants::Page_Up if norm && !shift => {
                buffer.page_up(view_id);
            }
            constants::Page_Down if norm && !shift => {
                buffer.page_down(view_id);
            }
            constants::Page_Up if norm && shift => {
                buffer.page_up_and_modify_selection(view_id);
            }
            constants::Page_Down if norm && shift => {
                buffer.page_down_and_modify_selection(view_id);
            }
            _ => {
                if let Some(ch) = ch {
                    match ch {
                        'a' if ctrl => {
                            buffer.select_all(view_id);
                        }
                        'c' if ctrl => {
                            // self.do_copy(state);
                        }
                        'f' if ctrl => {
                            // self.start_search(state);
                        }
                        'v' if ctrl => {
                            // self.do_paste(state);
                        }
                        't' if ctrl => {
                            // TODO new tab
                        }
                        'x' if ctrl => {
                            // self.do_cut(state);
                        }
                        'z' if ctrl => {
                            buffer.undo(view_id);
                        }
                        'Z' if ctrl && shift => {
                            buffer.redo(view_id);
                        }
                        c if (norm) && c >= '\u{0020}' => {
                            debug!("inserting key");
                            buffer.insert(view_id, &c.to_string());
                        }
                        _ => {
                            debug!("unhandled key: {:?}", ch);
                        }
                    }
                }
            }
        };
    }
}
