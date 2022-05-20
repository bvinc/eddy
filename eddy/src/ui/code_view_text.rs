use crate::app::Action;
use crate::color::{pango_to_gdk, text_theme_to_gdk};
use crate::theme::Theme;
use crate::ui::{Layout, LayoutItem, LayoutLine};
use cairo::glib::{ParamSpecEnum, ParamSpecObject};
use eddy_workspace::style::{Attr, AttrSpan, Color};
use eddy_workspace::Workspace;
use gdk::Key;
use gdk::ModifierType;
use glib::clone;
use glib::ParamSpec;
use glib::Sender;
use gtk::glib;
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, Adjustment};
use log::*;
use lru_cache::LruCache;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;
use pango::{AttrColor, Attribute, FontDescription};
use ropey::RopeSlice;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cell::{Cell, RefMut};
use std::cmp::{max, min};
use std::collections::hash_map;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

pub struct CodeViewTextPrivate {
    hadj: RefCell<Adjustment>,
    hscroll_policy: gtk::ScrollablePolicy,
    vadj: RefCell<Adjustment>,
    vscroll_policy: gtk::ScrollablePolicy,
    sender: OnceCell<Sender<Action>>,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: usize,
    theme: Theme,
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

        Self {
            hadj,
            hscroll_policy: gtk::ScrollablePolicy::Minimum,
            vadj,
            vscroll_policy: gtk::ScrollablePolicy::Minimum,
            sender,
            workspace,
            view_id,
            theme,
        }
    }
}

impl ObjectImpl for CodeViewTextPrivate {
    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            vec![
                ParamSpecObject::new(
                    "hadjustment",
                    "Horizontal Adjustment",
                    "Horizontal `GtkAdjustment` of the scrollable widget",
                    gtk::Adjustment::static_type(),
                    glib::ParamFlags::READWRITE,
                ),
                ParamSpecEnum::new(
                    "hscroll-policy",
                    "Horizontal Scroll Policy",
                    "Determines when horizontal scrolling should start",
                    gtk::ScrollablePolicy::static_type(),
                    0,
                    glib::ParamFlags::READWRITE,
                ),
                ParamSpecObject::new(
                    "vadjustment",
                    "Vertical Adjustment",
                    "Vertical `GtkAdjustment` of the scrollable widget",
                    gtk::Adjustment::static_type(),
                    glib::ParamFlags::READWRITE,
                ),
                ParamSpecEnum::new(
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
        // debug!("get property {}", pspec.name());
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

        // let event_controller_scroll = gtk::EventControllerScroll::builder()
        //     // .flags(
        //     //     gtk::EventControllerScrollFlags::BOTH_AXES
        //     //         | gtk::EventControllerScrollFlags::KINETIC,
        //     // )
        //     // .name("codeviewtext")
        //     // .propagation_limit(gtk::PropagationLimit::SameNative)
        //     // .propagation_phase(gtk::PropagationPhase::Target)
        //     .build();
        // // event_controller_scroll.connect_decelerate(clone!(@strong obj as this => move |_,a,b| {
        // //     dbg!("connect_decelerate", a, b);
        // // }));
        // // event_controller_scroll.connect_scroll(clone!(@strong obj as this => move |_,a,b| {
        // //     dbg!("connect_scroll", a, b);
        // //     gtk::Inhibit(true)
        // // }));
        // // event_controller_scroll.connect_scroll_begin(clone!(@strong obj as this => move |_| {
        // //     dbg!("connect_scroll");
        // // }));
        // // event_controller_scroll.connect_scroll_end(clone!(@strong obj as this => move |_| {
        // //     dbg!("connect_scroll_end");
        // // }));
        // // event_controller_scroll.connect_flags_notify(clone!(@strong obj as this => move |_| {
        // //     dbg!("connect_flags_notify");
        // // }));
        // obj.add_controller(&event_controller_scroll);
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
        debug!("cvt size allocate {} {} {}", w, h, bl);

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
    fn font_height(&self, cvt: &CodeViewText) -> (f64, f64) {
        let pango_ctx = cvt.pango_context();
        let mut font_height = 15.0;
        let mut font_ascent = 15.0;
        if let Some(metrics) = pango_ctx.metrics(None, None) {
            font_height = metrics.height() as f64 / pango::SCALE as f64;
            font_ascent = metrics.ascent() as f64 / pango::SCALE as f64;
        }
        (font_height, font_ascent)
    }

    fn buffer_changed(&self, cvt: &CodeViewText) {
        cvt.queue_draw();

        let mut workspace = self.workspace.get().unwrap().borrow_mut();
        let view_id = self.view_id;
        let (buffer, text_theme) = workspace.buffer_and_theme(view_id);

        let (font_height, font_ascent) = self.font_height(cvt);
        // let (text_width, text_height) = self.get_text_size(state);
        let text_height = buffer.len_lines() as f64 * font_height;
        let da_height = f64::from(cvt.allocated_height());

        self.scroll_to_carets(&mut workspace, cvt);

        self.set_adj_upper(&self.vadj, da_height as f64, text_height);
    }

    fn scroll_to_carets(&self, workspace: &mut RefMut<Workspace>, cvt: &CodeViewText) {
        let (buffer, _) = workspace.buffer_and_theme(self.view_id);
        let (font_height, font_ascent) = self.font_height(cvt);
        let selections = buffer.selections(self.view_id);

        if selections.len() == 0 {
            return;
        }
        let mut min_line = buffer.char_to_line(selections[0].cursor());
        let mut max_line = buffer.char_to_line(selections[0].cursor());
        let mut min_x = 
        for sel in selections {
            let line = buffer.char_to_line(sel.cursor());
            min_line = std::cmp::min(min_line, line);
            max_line = std::cmp::max(max_line, line);
        }

        let min = min_line as f64 * font_height;
        let max = max_line as f64 * font_height + font_height;
        if max - min < self.vadj.borrow().page_size() {
            if min < self.vadj.borrow().value() {
                self.vadj.borrow().set_value(min);
            } else if max > self.vadj.borrow().value() + self.vadj.borrow().page_size() {
                self.vadj
                    .borrow()
                    .set_value(max - self.vadj.borrow().page_size())
            }
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

        let vadj = self.vadj.clone();
        let hadj = self.hadj.clone();

        // We round the values from the scrollbars, because if we don't, rectangles
        // will be antialiased and lines will show up inbetween highlighted lines
        // of text.
        let vadj_value = f64::round(vadj.borrow().value());
        let hadj_value = f64::round(hadj.borrow().value());
        trace!(
            "drawing cvt.  height={} width={}, vadj={}, {}",
            da_height,
            da_width,
            vadj.borrow().value(),
            vadj.borrow().upper()
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
        let bg_color = text_theme_to_gdk(text_theme.bg);
        let rect_node = gtk::gsk::ColorNode::new(
            &bg_color,
            &graphene::Rect::new(0.0, 0.0, da_width as f32, da_height as f32),
        );
        snapshot.append_node(&rect_node);

        // Highlight cursor lines
        let mut highlight_bg_color = gdk::RGBA::WHITE;
        change_to_color(&mut highlight_bg_color, Some(text_theme.bg));
        change_to_color(&mut highlight_bg_color, text_theme.line_highlight.bg);
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

        // Loop through the visible lines
        const CURSOR_WIDTH: f64 = 2.0;
        let mut max_width = 0;
        for line_num in visible_lines {
            let mut layout_line = LayoutLine::new();
            // Keep track of the starting x position
            if let Some((line, attrs)) =
                buffer.get_line_with_attributes(view_id, line_num, &text_theme)
            {
                let text: Cow<str> = line.into();

                let pango_attrs = self.create_pango_attr_list(&attrs);
                let line_x = -hadj_value as f32;
                let line_y =
                    font_ascent as f32 + font_height as f32 * (line_num as f32) - vadj_value as f32;
                let pango_ctx = cv.pango_context();

                // Itemize
                let items = pango::itemize_with_base_dir(
                    &pango_ctx,
                    pango::Direction::Ltr,
                    &text,
                    0,
                    text.len() as i32,
                    &pango_attrs,
                    None,
                );

                // Loop through the items
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
                    let mut fg_color = text_theme_to_gdk(text_theme.fg);
                    // dbg!(color.red, color.green, color.blue);
                    for attr in &item.analysis().extra_attrs() {
                        if attr.type_() == pango::AttrType::Foreground {
                            if let Some(ca) = attr.downcast_ref::<pango::AttrColor>() {
                                fg_color = pango_to_gdk(ca.color());
                            }
                        }
                        if attr.type_() == pango::AttrType::Background {
                            if let Some(ca) = attr.downcast_ref::<pango::AttrColor>() {
                                bg_color = Some(pango_to_gdk(ca.color()));
                            }
                        }
                    }
                    pango::shape_full(item_text, None, item.analysis(), &mut glyphs);
                    self.adjust_glyph_tabs(&text, &item, &mut glyphs);
                    let item_width = glyphs.width();

                    // Append text background node to snapshot
                    if let Some(bg_color) = bg_color {
                        let rect_node = gtk::gsk::ColorNode::new(
                            &bg_color,
                            &graphene::Rect::new(
                                line_x + (x_off as f32 / pango::SCALE as f32) as f32,
                                line_y - font_ascent as f32,
                                item_width as f32 / pango::SCALE as f32,
                                font_height as f32,
                            ),
                        );
                        append_clipped_node(snapshot, rect_node, da_width as f32, da_height as f32);
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
                        append_clipped_node(snapshot, text_node, da_width as f32, da_height as f32);
                    }

                    layout_line.push(LayoutItem {
                        text: item_text.to_string(),
                        item,
                        glyphs,
                        x_off,
                    });

                    x_off += item_width;
                    if x_off > max_width {
                        max_width = x_off;
                    }
                }

                // Draw the cursors on the line
                for sel in buffer.selections(view_id) {
                    if buffer.char_to_line(sel.cursor()) != line_num {
                        continue;
                    }
                    let line_byte =
                        buffer.char_to_byte(sel.cursor()) - buffer.line_to_byte(line_num);
                    let x = layout_line.index_to_x(line_byte) as f32 / pango::SCALE as f32;

                    let color = text_theme_to_gdk(text_theme.fg);

                    let rect_node = gtk::gsk::ColorNode::new(
                        &color,
                        &graphene::Rect::new(
                            x - hadj_value as f32,
                            line_y - font_ascent as f32,
                            CURSOR_WIDTH as f32,
                            font_height as f32,
                        ),
                    );

                    append_clipped_node(snapshot, rect_node, da_width as f32, da_height as f32);
                }
            }
        }

        // Now that we know actual length of the text, adjust the scrollbar properly.
        let h_upper = f64::from(max_width / pango::SCALE);
        self.set_adj_upper(&self.hadj, da_width as f64, h_upper);

        let draw_end = Instant::now();
        debug!("drawing took {}ms", (draw_end - draw_start).as_millis());
    }

    fn set_adj_upper(&self, adj: &RefCell<Adjustment>, da_length: f64, content_length: f64) {
        let adj = adj.borrow();
        let mut upper: f64 = if da_length > content_length {
            da_length as f64
        } else {
            content_length as f64
        };

        if adj.value() + adj.page_size() > upper {
            upper = adj.value() + adj.page_size()
        }

        if upper != adj.upper() {
            adj.set_upper(upper);
        }
    }

    /// Creates a pango attr list from eddy attributes
    fn create_pango_attr_list(&self, attr_spans: &[AttrSpan]) -> pango::AttrList {
        let attr_list = pango::AttrList::new();
        for aspan in attr_spans {
            let mut pattr = match aspan.attr {
                Attr::ForegroundColor(color) => {
                    AttrColor::new_foreground(color.r_u16(), color.g_u16(), color.b_u16())
                }
                Attr::BackgroundColor(color) => {
                    AttrColor::new_background(color.r_u16(), color.g_u16(), color.b_u16())
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
                    AttrColor::new_foreground(color.r_u16(), color.g_u16(), color.b_u16())
                }
                Attr::BackgroundColor(color) => {
                    AttrColor::new_background(color.r_u16(), color.g_u16(), color.b_u16())
                }
            };
            pattr.set_start_index(aspan.start_idx as u32);
            pattr.set_end_index(aspan.end_idx as u32);
            attr_list.insert(pattr);
        }

        layout.set_attributes(Some(&attr_list));
        layout
    }

    fn adjust_glyph_tabs(
        &self,
        text: &Cow<str>,
        item: &pango::Item,
        glyphs: &mut pango::GlyphString,
    ) {
        let glyph_info = glyphs.glyph_info_mut();
        // dbg!(&glyph_info);
        if glyph_info.len() == 0 {
            return;
        }
        // dbg!(&text, item.offset());
        if text.bytes().nth(item.offset() as usize) == Some(b'\t') {
            dbg!("adjusting tab at", item.offset());
            glyph_info[0].geometry_mut().set_width(1024 * 100);
        }
        // for gi in &mut glyphs.glyph_info() {
        // }
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
    pub fn new(workspace: Rc<RefCell<Workspace>>, sender: Sender<Action>) -> Self {
        let obj = glib::Object::new::<Self>(&[]).unwrap();
        let imp = CodeViewTextPrivate::from_instance(&obj);

        imp.workspace.set(workspace);
        imp.sender.set(sender);

        // code_view.setup_widgets();
        // code_view.setup_signals();
        obj
    }

    // fn setup_widgets(&self) {}

    // fn setup_signals(&self) {}

    pub fn set_hadjust(&self, adj: &Adjustment) {
        let cvt_priv = CodeViewTextPrivate::from_instance(self);
        cvt_priv.hadj.replace(adj.clone());
        cvt_priv
            .hadj
            .borrow()
            .connect_value_changed(clone!(@weak self as cvt => move |_| {
                cvt.queue_draw();
            }));
    }

    pub fn set_vadjust(&self, adj: &Adjustment) {
        let cvt_priv = CodeViewTextPrivate::from_instance(self);
        cvt_priv.vadj.replace(adj.clone());
        cvt_priv
            .vadj
            .borrow()
            .connect_value_changed(clone!(@weak self as cvt => move |_| {
                cvt.queue_draw();
            }));
    }

    pub fn buffer_changed(&self) {
        let code_view_priv = CodeViewTextPrivate::from_instance(self);
        code_view_priv.buffer_changed(self);
    }

    fn button_pressed(&self, n_pressed: i32, x: f64, y: f64) {}

    fn key_pressed(&self, key: Key, keycode: u32, state: ModifierType) {
        let self_ = CodeViewTextPrivate::from_instance(self);
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
            Key::Delete if norm => buffer.delete_forward(view_id),
            Key::BackSpace if norm => buffer.delete_backward(view_id),
            Key::Return | Key::KP_Enter => {
                buffer.insert_newline(view_id);
            }
            Key::Tab if norm && !shift => buffer.insert_tab(view_id),
            Key::Up if norm && !shift => buffer.move_up(view_id),
            Key::Down if norm && !shift => buffer.move_down(view_id),
            Key::Left if norm && !shift => buffer.move_left(view_id),
            Key::Right if norm && !shift => buffer.move_right(view_id),
            Key::Up if norm && shift => {
                buffer.move_up_and_modify_selection(view_id);
            }
            Key::Down if norm && shift => {
                buffer.move_down_and_modify_selection(view_id);
            }
            Key::Left if norm && shift => {
                buffer.move_left_and_modify_selection(view_id);
            }
            Key::Right if norm && shift => {
                buffer.move_right_and_modify_selection(view_id);
            }
            Key::Left if ctrl && !shift => {
                buffer.move_word_left(view_id);
            }
            Key::Right if ctrl && !shift => {
                buffer.move_word_right(view_id);
            }
            Key::Left if ctrl && shift => {
                buffer.move_word_left_and_modify_selection(view_id);
            }
            Key::Right if ctrl && shift => {
                buffer.move_word_right_and_modify_selection(view_id);
            }
            Key::Home if norm && !shift => {
                buffer.move_to_left_end_of_line(view_id);
            }
            Key::End if norm && !shift => {
                buffer.move_to_right_end_of_line(view_id);
            }
            Key::Home if norm && shift => {
                buffer.move_to_left_end_of_line_and_modify_selection(view_id);
            }
            Key::End if norm && shift => {
                buffer.move_to_right_end_of_line_and_modify_selection(view_id);
            }
            Key::Home if ctrl && !shift => {
                buffer.move_to_beginning_of_document(view_id);
            }
            Key::End if ctrl && !shift => {
                buffer.move_to_end_of_document(view_id);
            }
            Key::Home if ctrl && shift => {
                buffer.move_to_beginning_of_document_and_modify_selection(view_id);
            }
            Key::End if ctrl && shift => {
                buffer.move_to_end_of_document_and_modify_selection(view_id);
            }
            Key::Page_Up if norm && !shift => {
                buffer.page_up(view_id);
            }
            Key::Page_Down if norm && !shift => {
                buffer.page_down(view_id);
            }
            Key::Page_Up if norm && shift => {
                buffer.page_up_and_modify_selection(view_id);
            }
            Key::Page_Down if norm && shift => {
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

fn append_clipped_node<P: AsRef<gtk::gsk::RenderNode>>(
    snapshot: &gtk::Snapshot,
    node: P,
    w: f32,
    h: f32,
) {
    let clip_node = gtk::gsk::ClipNode::new(&node, &graphene::Rect::new(0.0, 0.0, w, h));
    snapshot.append_node(&clip_node);
}

fn change_to_color(gc: &mut gdk::RGBA, c: Option<Color>) {
    if let Some(c) = c {
        gc.set_red(c.r_f32());
        gc.set_green(c.g_f32());
        gc.set_blue(c.b_f32());
    }
}
