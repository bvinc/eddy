use crate::color::{pango_to_gdk, text_theme_to_gdk};
use crate::theme::Theme;
use crate::ui::{Layout, LayoutItem, LayoutLine};
use cairo::glib::{ParamSpecEnum, ParamSpecObject};
use eddy_workspace::style::{Attr, AttrSpan, Color};
use eddy_workspace::{Buffer, Event, Selection, Workspace};
use gdk::{Key, ModifierType};
use glib::{clone, ParamSpec, Sender};
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, Adjustment, Gesture};
use log::*;
use lru_cache::LruCache;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;
use pango::{AttrColor, AttrList, Attribute, FontDescription, GlyphGeometry, GlyphInfo};
use ropey::RopeSlice;
use std::borrow::Cow;
use std::cell::{Cell, RefCell, RefMut};
use std::cmp::{max, min};
use std::collections::{hash_map, HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

const CURSOR_WIDTH: f64 = 2.0;

pub struct CodeViewTextPrivate {
    hadj: RefCell<Adjustment>,
    hscroll_policy: gtk::ScrollablePolicy,
    vadj: RefCell<Adjustment>,
    vscroll_policy: gtk::ScrollablePolicy,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: Cell<usize>,
    font_metrics: RefCell<FontMetrics>,
    theme: Theme,
    gesture_drag: gtk::GestureDrag,
    // When starting a double-click drag or triple-click drag, the initial
    // selection is saved here.
    drag_anchor: Selection,
    highlighted_lines: RefCell<HashSet<usize>>,
}

#[derive(Copy, Clone, Debug, Default)]
struct FontMetrics {
    space_width: f64,
    space_glyph: u32,
    font_height: f64,
    font_ascent: f64,
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
        let workspace = OnceCell::new();
        let view_id = Cell::new(0);
        let font_metrics = RefCell::new(FontMetrics::default());
        let theme = Theme::default();

        let hadj = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
        let vadj = RefCell::new(Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));

        Self {
            hadj,
            hscroll_policy: gtk::ScrollablePolicy::Minimum,
            vadj,
            vscroll_policy: gtk::ScrollablePolicy::Minimum,
            workspace,
            view_id,
            font_metrics,
            theme,
            gesture_drag: gtk::GestureDrag::new(),
            drag_anchor: Selection::new(),
            highlighted_lines: RefCell::new(HashSet::new()),
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

    fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {}

    fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        // debug!("get property {}", pspec.name());
        match pspec.name() {
            "hadjustment" => self.hadj.borrow().to_value(),
            "hscroll-policy" => self.hscroll_policy.to_value(),
            "vadjustment" => self.vadj.borrow().to_value(),
            "vscroll-policy" => self.vscroll_policy.to_value(),
            _ => 0.to_value(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        let pango_ctx = obj.pango_context();
        let mut font_desc = pango::FontDescription::new();
        font_desc.set_family("Hack, Mono");
        font_desc.set_size(16384);
        pango_ctx.set_font_description(Some(&font_desc));
        CodeViewTextPrivate::from_instance(&obj).on_font_change(&obj);

        obj.set_focusable(true);
        obj.set_can_focus(true);

        obj.set_valign(gtk::Align::Fill);
        obj.set_halign(gtk::Align::Fill);
        obj.set_vexpand(true);
        obj.set_hexpand(true);

        let gesture_click = gtk::GestureClick::new();
        gesture_click.connect_pressed(clone!(@strong obj as this => move |gc, n_press, x, y| {
            this.grab_focus();
            let this_ = CodeViewTextPrivate::from_instance(&this);
            this_.button_pressed(&this, gc, n_press, x, y);
            // gc.set_state(gtk::EventSequenceState::Claimed);
        }));
        obj.add_controller(&gesture_click);

        self.gesture_drag.set_button(gdk::BUTTON_PRIMARY);
        // self.gesture_drag.connect_drag_begin(|gd, x, y| {
        //     dbg!("drag begin");
        // });
        self.gesture_drag
            .connect_drag_end(clone!(@strong obj as this => move |gd, _, _| {
                this.drag_end(gd);
            }));
        self.gesture_drag
            .connect_drag_update(clone!(@strong obj as this => move |gd, _, _| {
                this.drag_update(gd);
                // gd.set_state(gtk::EventSequenceState::Claimed);
            }));

        obj.add_controller(&self.gesture_drag);

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
    fn snapshot(&self, snapshot: &gtk::Snapshot) {
        // snapshot.render_layout(&ctx, 10.0, 10.0, &layout);
        // snapshot.render_background(&ctx, 10.0, 10.0, 30.0, 20.0);
        self.handle_draw(&self.obj(), snapshot);
    }
    fn size_allocate(&self, w: i32, h: i32, bl: i32) {
        self.parent_size_allocate(w, h, bl);
        debug!("cvt size allocate {} {} {}", w, h, bl);

        let vadj = self.vadj.borrow().clone();
        vadj.set_page_size(f64::from(h));
        let hadj = self.hadj.borrow().clone();
        hadj.set_page_size(f64::from(w));

        self.reset_vadj_upper(&self.obj());

        self.obj().grab_focus();
    }
}
impl BoxImpl for CodeViewTextPrivate {}
impl ScrollableImpl for CodeViewTextPrivate {}

impl CodeViewTextPrivate {
    fn on_font_change(&self, cvt: &CodeViewText) {
        // let pango_attrs = self.create_pango_attr_list(&attrs);
        let pango_ctx = cvt.pango_context();

        let metrics = pango_ctx.metrics(None, None);
        self.font_metrics.borrow_mut().font_height = metrics.height() as f64 / pango::SCALE as f64;
        self.font_metrics.borrow_mut().font_ascent = metrics.ascent() as f64 / pango::SCALE as f64;

        let space = " ";
        // Itemize
        let items = pango::itemize_with_base_dir(
            &pango_ctx,
            pango::Direction::Ltr,
            space,
            0,
            space.len() as i32,
            &AttrList::new(),
            None,
        );

        if items.len() > 0 && items[0].offset() == 0 && items[0].length() == 1 {
            let item = &items[0];
            let mut glyphs = pango::GlyphString::new();
            pango::shape_full(space, None, item.analysis(), &mut glyphs);
            let glyph_info = glyphs.glyph_info();
            if glyph_info.len() > 0 {
                self.font_metrics.borrow_mut().space_glyph = glyph_info[0].glyph();
                self.font_metrics.borrow_mut().space_width =
                    glyph_info[0].geometry().width() as f64 / pango::SCALE as f64;
            }
        }
    }

    fn get_buffer(&self) -> Rc<RefCell<Buffer>> {
        let workspace = self.workspace.get().unwrap();
        let (buffer, _) = workspace.borrow_mut().buffer_and_theme(self.view_id.get());
        buffer
    }

    fn get_buffer_and_theme(&self) -> (Rc<RefCell<Buffer>>, eddy_workspace::style::Theme) {
        let workspace = self.workspace.get().unwrap();
        let (buffer, theme) = workspace.borrow_mut().buffer_and_theme(self.view_id.get());
        (buffer, theme)
    }

    fn reset_vadj_upper(&self, cvt: &CodeViewText) {
        let buffer = self.get_buffer();

        let font_height = self.font_metrics.borrow().font_height;
        let text_height = buffer.borrow().len_lines() as f64 * font_height;
        let da_height = f64::from(cvt.allocated_height());

        self.set_adj_upper(&self.vadj, da_height as f64, text_height);
    }

    fn buffer_changed(&self, cvt: &CodeViewText) {
        cvt.queue_draw();

        self.reset_vadj_upper(cvt);
        self.scroll_to_carets(cvt);
    }

    fn scroll_to_carets(&self, cvt: &CodeViewText) {
        let buffer = self.get_buffer();
        let font_height = self.font_metrics.borrow().font_height;
        let buffer = buffer.borrow();
        let selections = buffer.selections(self.view_id.get());

        if selections.len() == 0 {
            return;
        }

        let mut min_x = None;
        let mut max_x = None;
        let mut min_y = None;
        let mut max_y = None;
        for sel in selections {
            let line = buffer.char_to_line(sel.cursor());
            let line_min_y = line as f64 * font_height;
            let line_max_y = line as f64 * font_height + font_height;
            min_y = Some(line_min_y.min(min_y.unwrap_or(line_min_y)));
            max_y = Some(line_max_y.max(max_y.unwrap_or(line_max_y)));

            let line_byte = buffer.char_to_byte(sel.cursor()) - buffer.line_to_byte(line);
            let layout_line = self.make_layout_line(cvt, line);
            let x = layout_line.index_to_x(line_byte) as f64 / pango::SCALE as f64;
            let cur_min_x = x;
            let cur_max_x = x + CURSOR_WIDTH;
            min_x = Some(cur_min_x.min(min_x.unwrap_or(cur_min_x)));
            max_x = Some(cur_max_x.max(max_x.unwrap_or(cur_max_x)));
        }

        // If the cursors can fit on the screen
        if let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (min_x, max_x, min_y, max_y) {
            if max_x - min_x < self.hadj.borrow().page_size()
                && max_y - min_y < self.vadj.borrow().page_size()
            {
                // If we need to scroll up/down
                if min_y < self.vadj.borrow().value() {
                    self.vadj.borrow().set_value(min_y);
                } else if max_y > self.vadj.borrow().value() + self.vadj.borrow().page_size() {
                    self.vadj
                        .borrow()
                        .set_value(max_y - self.vadj.borrow().page_size())
                }

                // If we need to scroll left/right
                if min_x < self.hadj.borrow().value() {
                    self.hadj.borrow().set_value(min_x);
                } else if max_x > self.hadj.borrow().value() + self.hadj.borrow().page_size() {
                    self.hadj
                        .borrow()
                        .set_value(max_x - self.hadj.borrow().page_size())
                }
            }
        }
    }

    fn make_layout_line(&self, cvt: &CodeViewText, line_num: usize) -> LayoutLine {
        let (buffer, text_theme) = self.get_buffer_and_theme();
        let mut layout_line = LayoutLine::new();
        // Keep track of the starting x position
        if let Some((line, attrs)) =
            buffer
                .borrow()
                .get_line_with_attributes(self.view_id.get(), line_num, &text_theme)
        {
            let text: Cow<str> = line.into();

            let pango_attrs = self.create_pango_attr_list(&attrs);
            let pango_ctx = cvt.pango_context();

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

                pango::shape_full(item_text, None, item.analysis(), &mut glyphs);
                self.adjust_glyph_tabs(&text, &item, &mut glyphs);
                let width = glyphs.width();

                layout_line.push(LayoutItem {
                    text: item_text.to_string(),
                    inner: item,
                    glyphs,
                    x_off,
                    width,
                });

                x_off += width;
            }
        }
        layout_line
    }

    fn xy_to_line_idx(&self, cvt: &CodeViewText, x: f64, y: f64) -> (usize, usize) {
        // We round the values from the scrollbars, because if we don't,
        // rectangles will be antialiased and lines will show up inbetween
        // highlighted lines of text.  Since they're rounded for drawing I
        // guess they should be rounded here.
        let vadj_value = f64::round(self.vadj.borrow().value());
        let font_height = self.font_metrics.borrow().font_height;

        let line = ((vadj_value + y) / font_height) as usize;
        let layout_line = self.make_layout_line(cvt, line);
        let idx = layout_line.x_to_index(x as i32 * pango::SCALE);

        (line, idx)
    }

    fn button_pressed(
        &self,
        cvt: &CodeViewText,
        gc: &gtk::GestureClick,
        n_press: i32,
        x: f64,
        y: f64,
    ) {
        // dbg!(n_press);
        let sequence = gc.current_sequence(); // Can be None
        let button = gc.current_button();
        let event = gc.last_event(sequence.as_ref()).unwrap();

        let shift = gc.current_event().map_or(false, |ev| {
            ev.modifier_state().contains(gdk::ModifierType::SHIFT_MASK)
        });
        let ctrl = gc.current_event().map_or(false, |ev| {
            ev.modifier_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
        });
        // dbg!(ctrl);

        if n_press == 1 && event.triggers_context_menu() {
            // TODO context menu?
        } else if button == gdk::BUTTON_MIDDLE {
            // TODO middle click paste
        } else if button == gdk::BUTTON_PRIMARY {
        }

        let buffer = self
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer(self.view_id.get());
        let (line, idx) = self.xy_to_line_idx(cvt, x, y);

        match n_press {
            1 => {
                if ctrl {
                    let mut buffer = buffer.borrow_mut();
                    buffer.gesture_toggle_sel(self.view_id.get(), line, idx);
                } else {
                    let mut buffer = buffer.borrow_mut();
                    buffer.gesture_point_select(self.view_id.get(), line, idx);
                }
            }

            2 => {
                let mut buffer = buffer.borrow_mut();
                buffer.gesture_word_select(self.view_id.get(), line, idx);
            }
            3 => {
                let mut buffer = buffer.borrow_mut();
                buffer.gesture_line_select(self.view_id.get(), line);
            }
            _ => {}
        };
    }

    fn gesture_toggle_sel(&self, cvt: &CodeViewText, x: f64, y: f64) {
        let buffer = self
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer(self.view_id.get());
        let (line, byte_idx) = self.xy_to_line_idx(cvt, x, y);
        buffer
            .borrow_mut()
            .gesture_toggle_sel(self.view_id.get(), line, byte_idx);
    }

    fn gesture_drag(&self, cvt: &CodeViewText, x: f64, y: f64) {
        let buffer = self
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer(self.view_id.get());
        let (line, idx) = self.xy_to_line_idx(cvt, x, y);
        buffer
            .borrow_mut()
            .drag_update(self.view_id.get(), line, idx);
    }

    fn drag_end(&self, _cvt: &CodeViewText) {
        let buffer = self
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer(self.view_id.get());
        buffer.borrow_mut().drag_end(self.view_id.get());
    }

    /// Determines how many lines page up or down should use
    fn page_lines(&self, cvt: &CodeViewText) -> usize {
        let font_height = self.font_metrics.borrow().font_height;
        std::cmp::max(
            2,
            ((cvt.allocated_height() as f64 / font_height) - 2.0) as usize,
        )
    }

    fn handle_draw(&self, cvt: &CodeViewText, snapshot: &gtk::Snapshot) {
        let draw_start = Instant::now();

        let _theme = &self.theme;

        let da_width = cvt.allocated_width();
        let da_height = cvt.allocated_height();

        let view_id = self.view_id.get();
        let (buffer, text_theme) = self
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer_and_theme(view_id);

        //debug!("Drawing");
        // cr.select_font_face("Mono", ::cairo::enums::FontSlant::Normal, ::cairo::enums::FontWeight::Normal);
        // let mut font_options = cr.get_font_options();
        // debug!("font options: {:?} {:?} {:?}", font_options, font_options.get_antialias(), font_options.get_hint_style());
        // font_options.set_hint_style(HintStyle::Full);

        // let (text_width, text_height) = self.get_text_size();
        let num_lines = buffer.borrow().len_lines();

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

        let font_height = self.font_metrics.borrow().font_height;
        let font_ascent = self.font_metrics.borrow().font_ascent;

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

        // Figure out which of our lines need highlighting
        let mut highlighted_lines = self.highlighted_lines.borrow_mut();
        highlighted_lines.clear();
        for sel in buffer.borrow().selections(view_id) {
            let line = buffer.borrow().char_to_line(sel.cursor());
            if sel.is_caret() && visible_lines.contains(&line) {
                highlighted_lines.insert(line);
            }
        }

        // Highlight cursor lines
        let mut highlight_bg_color = gdk::RGBA::WHITE;
        change_to_color(&mut highlight_bg_color, Some(text_theme.bg));
        change_to_color(&mut highlight_bg_color, text_theme.line_highlight.bg);
        for line in first_line..last_line {
            if !highlighted_lines.contains(&line) {
                continue;
            }

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

        // Loop through the visible lines
        let mut max_width = 0;
        for line_num in visible_lines {
            let line_x = -hadj_value as f32;
            let line_y =
                font_ascent as f32 + font_height as f32 * (line_num as f32) - vadj_value as f32;

            let mut layout_line = self.make_layout_line(cvt, line_num);
            // Loop through the items
            for item in &mut layout_line.items {
                let mut bg_color: Option<gdk::RGBA> = None;
                let mut fg_color = text_theme_to_gdk(text_theme.fg);

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

                // Append text background node to snapshot
                if let Some(bg_color) = bg_color {
                    let rect_node = gtk::gsk::ColorNode::new(
                        &bg_color,
                        &graphene::Rect::new(
                            line_x + (item.x_off as f32 / pango::SCALE as f32) as f32,
                            line_y - font_ascent as f32,
                            item.width as f32 / pango::SCALE as f32,
                            font_height as f32,
                        ),
                    );
                    append_clipped_node(snapshot, rect_node, da_width, da_height);
                }

                // Append text node to snapshot
                if let Some(text_node) = gtk::gsk::TextNode::new(
                    &item.analysis().font(),
                    &item.glyphs,
                    &fg_color,
                    &graphene::Point::new(
                        line_x + (item.x_off as f32 / pango::SCALE as f32) as f32,
                        line_y,
                    ),
                ) {
                    append_clipped_node(snapshot, text_node, da_width, da_height);
                }

                if item.x_off > max_width {
                    max_width = item.x_off;
                }
            }

            // Draw the cursors on the line
            for sel in buffer.borrow().selections(view_id) {
                if buffer.borrow().char_to_line(sel.cursor()) != line_num {
                    continue;
                }
                let line_byte = buffer.borrow().char_to_byte(sel.cursor())
                    - buffer.borrow().line_to_byte(line_num);
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

                append_clipped_node(snapshot, rect_node, da_width, da_height);
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
        if glyph_info.len() == 0 {
            return;
        }
        if text.bytes().nth(item.offset() as usize) == Some(b'\t') {
            glyph_info[0].set_glyph(self.font_metrics.borrow().space_glyph);
            glyph_info[0]
                .geometry_mut()
                .set_width((self.font_metrics.borrow().space_width * 4.0) as i32 * pango::SCALE);
        }
    }
}

glib::wrapper! {
    pub struct CodeViewText(ObjectSubclass<CodeViewTextPrivate>)
    @extends gtk::Widget,
    @implements gtk::Scrollable;
}

impl CodeViewText {
    pub fn new(workspace: Rc<RefCell<Workspace>>, view_id: usize) -> Self {
        let obj = glib::Object::new::<Self>(&[]);
        let imp = CodeViewTextPrivate::from_instance(&obj);

        imp.view_id.set(view_id);
        imp.workspace.set(workspace);

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

    pub fn scroll_to_carets(&self) {
        let code_view_priv = CodeViewTextPrivate::from_instance(self);
        code_view_priv.scroll_to_carets(self);
    }

    fn drag_update(&self, gd: &gtk::GestureDrag) {
        self.grab_focus();
        let self_ = CodeViewTextPrivate::from_instance(self);

        let (start_x, start_y) = gd.start_point().unwrap();
        let (off_x, off_y) = gd.offset().unwrap();
        let x = start_x + off_x;
        let y = start_y + off_y;

        self_.gesture_drag(self, x, y);
    }

    fn drag_end(&self, _gd: &gtk::GestureDrag) {
        let self_ = CodeViewTextPrivate::from_instance(self);
        self_.drag_end(self);
    }

    // fn middle_button_pressed(&self, n_pressed: i32, x: f64, y: f64) {
    //     self.grab_focus();
    //     let self_ = CodeViewTextPrivate::from_instance(self);

    //     let (col, line) = { self.da_px_to_cell(&main_state, x, y) };
    //     self.do_paste_primary(&self.view_id, line, col);
    // }

    fn key_pressed(&self, key: Key, _keycode: u32, state: ModifierType) {
        let self_ = CodeViewTextPrivate::from_instance(self);
        debug!(
            "key press keyval={:?}, state={:?}, uc={:?}",
            key,
            state,
            key.to_unicode(),
        );
        let (buffer, _) = self_
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer_and_theme(self_.view_id.get());

        let view_id = self_.view_id.get();
        let ch = key.to_unicode();

        let alt = state.contains(ModifierType::ALT_MASK);
        let ctrl = state.contains(ModifierType::CONTROL_MASK);
        let meta = state.contains(ModifierType::META_MASK);
        let shift = state.contains(ModifierType::SHIFT_MASK);
        let norm = !alt && !ctrl && !meta;

        match key {
            Key::Delete if norm => buffer.borrow_mut().delete_forward(view_id),
            Key::BackSpace if norm => buffer.borrow_mut().delete_backward(view_id),
            Key::Return | Key::KP_Enter => {
                buffer.borrow_mut().insert_newline(view_id);
            }
            Key::Tab if norm && !shift => buffer.borrow_mut().insert_tab(view_id),
            Key::Up if norm && !shift => buffer.borrow_mut().move_up(view_id),
            Key::Down if norm && !shift => buffer.borrow_mut().move_down(view_id),
            Key::Left if norm && !shift => buffer.borrow_mut().move_left(view_id),
            Key::Right if norm && !shift => buffer.borrow_mut().move_right(view_id),
            Key::Up if norm && shift => {
                buffer.borrow_mut().move_up_and_modify_selection(view_id);
            }
            Key::Down if norm && shift => {
                buffer.borrow_mut().move_down_and_modify_selection(view_id);
            }
            Key::Left if norm && shift => {
                buffer.borrow_mut().move_left_and_modify_selection(view_id);
            }
            Key::Right if norm && shift => {
                buffer.borrow_mut().move_right_and_modify_selection(view_id);
            }
            Key::Left if ctrl && !shift => {
                buffer.borrow_mut().move_word_left(view_id);
            }
            Key::Right if ctrl && !shift => {
                buffer.borrow_mut().move_word_right(view_id);
            }
            Key::Left if ctrl && shift => {
                buffer
                    .borrow_mut()
                    .move_word_left_and_modify_selection(view_id);
            }
            Key::Right if ctrl && shift => {
                buffer
                    .borrow_mut()
                    .move_word_right_and_modify_selection(view_id);
            }
            Key::Home if norm && !shift => {
                buffer.borrow_mut().move_to_left_end_of_line(view_id);
            }
            Key::End if norm && !shift => {
                buffer.borrow_mut().move_to_right_end_of_line(view_id);
            }
            Key::Home if norm && shift => {
                buffer
                    .borrow_mut()
                    .move_to_left_end_of_line_and_modify_selection(view_id);
            }
            Key::End if norm && shift => {
                buffer
                    .borrow_mut()
                    .move_to_right_end_of_line_and_modify_selection(view_id);
            }
            Key::Home if ctrl && !shift => {
                buffer.borrow_mut().move_to_beginning_of_document(view_id);
            }
            Key::End if ctrl && !shift => {
                buffer.borrow_mut().move_to_end_of_document(view_id);
            }
            Key::Home if ctrl && shift => {
                buffer
                    .borrow_mut()
                    .move_to_beginning_of_document_and_modify_selection(view_id);
            }
            Key::End if ctrl && shift => {
                buffer
                    .borrow_mut()
                    .move_to_end_of_document_and_modify_selection(view_id);
            }
            Key::Page_Up if norm && !shift => {
                buffer.borrow_mut().page_up(view_id, self_.page_lines(self));
            }
            Key::Page_Down if norm && !shift => {
                buffer
                    .borrow_mut()
                    .page_down(view_id, self_.page_lines(self));
            }
            Key::Page_Up if norm && shift => {
                buffer
                    .borrow_mut()
                    .page_up_and_modify_selection(view_id, self_.page_lines(self));
            }
            Key::Page_Down if norm && shift => {
                buffer
                    .borrow_mut()
                    .page_down_and_modify_selection(view_id, self_.page_lines(self));
            }
            _ => {
                if let Some(ch) = ch {
                    match ch {
                        'a' if ctrl => {
                            buffer.borrow_mut().select_all(view_id);
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
                        's' if ctrl => {
                            buffer.borrow_mut().save();
                        }
                        't' if ctrl => {
                            // TODO new tab
                        }
                        'x' if ctrl => {
                            // self.do_cut(state);
                        }
                        'z' if ctrl => {
                            buffer.borrow_mut().undo(view_id);
                        }
                        'Z' if ctrl && shift => {
                            buffer.borrow_mut().redo(view_id);
                        }
                        c if (norm) && c >= '\u{0020}' => {
                            buffer.borrow_mut().insert(view_id, &c.to_string());
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
    w: i32,
    h: i32,
) {
    let clip_node =
        gtk::gsk::ClipNode::new(&node, &graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
    snapshot.append_node(&clip_node);
}

fn change_to_color(gc: &mut gdk::RGBA, c: Option<Color>) {
    if let Some(c) = c {
        gc.set_red(c.r_f32());
        gc.set_green(c.g_f32());
        gc.set_blue(c.b_f32());
    }
}
