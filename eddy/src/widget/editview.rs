use crate::scrollable_drawing_area::ScrollableDrawingArea;
// use crate::scrollable_drawing_area_orig::ScrollableDrawingArea;
use crate::theme::{set_source_color, Theme};
use crate::MainState;
use cairo::Context;
use eddy_workspace::style::{Attr, AttrSpan, Color};
use eddy_workspace::{Buffer, Workspace};
use gdk::keys::constants as key;
use gdk::EventMask;
use gdk::*;
use glib::clone;
use gtk::prelude::*;
use gtk::{self, *};
use gtk::{prelude::WidgetExtManual, BoxExt, Inhibit, WidgetExt};
use log::debug;
use log::*;
use pango::{self, *};
use pangocairo::functions::*;
use relm::{connect, EventStream, Relm, Update, Widget};
use relm_derive::Msg;
use ropey::RopeSlice;
use serde_json::Value;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use std::u32;

#[derive(Clone)]
pub struct Model {
    view_id: usize,
    file_name: Option<PathBuf>,
    pristine: bool,
    parent_es: EventStream<crate::Msg>,
    workspace: Rc<RefCell<Workspace>>,
    theme: Theme,
}

#[derive(Msg)]
pub enum Msg {
    ButtonPress(EventButton),
    ConfigChanged(Value),
    FindNext,
    FindPrev,
    FindStatus(Value),
    KeyPress(EventKey),
    MotionNotify(EventMotion),
    Replace,
    ReplaceAll,
    SizeAllocate(i32, i32),
    ScrollEvent(EventScroll),
    ScrollTo(usize, usize),
    ScrollToSelections,
    SearchChanged(Option<String>),
    StopSearch,
    Update,
}

struct State {
    model: Model,
    pub line_da: ScrollableDrawingArea,
    pub da: ScrollableDrawingArea,
    pub root_widget: gtk::Box,
    pub tab_widget: gtk::Box,
    pub label: Label,
    pub close_button: Button,
    search_bar: SearchBar,
    search_entry: SearchEntry,
    replace_expander: Expander,
    replace_revealer: Revealer,
    replace_entry: Entry,
    find_status_label: Label,
    hadj: Adjustment,
    vadj: Adjustment,
    font_desc: FontDescription,
    visible_lines: Range<u64>,
    font_height: f64,
    font_width: f64,
    font_ascent: f64,
    font_descent: f64,
}

pub struct EditView {
    state: Rc<RefCell<State>>,
}

impl Update for EditView {
    type Model = Model;
    type ModelParam = (
        usize,
        Option<PathBuf>,
        bool,
        EventStream<crate::Msg>,
        Rc<RefCell<Workspace>>,
    );
    type Msg = Msg;

    fn model(
        _: &Relm<Self>,
        (view_id, file_name, pristine, parent_es, workspace): (
            usize,
            Option<PathBuf>,
            bool,
            EventStream<crate::Msg>,
            Rc<RefCell<Workspace>>,
        ),
    ) -> Model {
        Model {
            view_id,
            file_name,
            pristine,
            parent_es,
            workspace,
            theme: Theme::default(),
        }
    }

    fn update(&mut self, event: Msg) {
        let mut state = self.state.borrow_mut();
        match event {
            Msg::ButtonPress(eb) => self.handle_button_press(&mut state, &eb),
            // Msg::FindNext => self.find_next(&mut state),
            // Msg::FindPrev => self.find_prev(&mut state),
            // Msg::FindStatus(queries) => self.find_status(&mut state, &queries),
            Msg::KeyPress(ek) => self.handle_key_press_event(&mut state, &ek),
            // Msg::ConfigChanged(changes) => self.config_changed(&mut state, &changes),
            Msg::MotionNotify(em) => self.handle_drag(&mut state, &em),
            Msg::ScrollEvent(es) => self.handle_scroll(&mut state, &es),
            Msg::ScrollTo(line, col) => self.scroll_to(&mut state, line, col),
            Msg::ScrollToSelections => self.scroll_to_selections(&mut state),
            Msg::SizeAllocate(w, h) => self.da_size_allocate(&mut state, w, h),
            // Msg::SearchChanged(s) => self.search_changed(&mut state, s),
            // Msg::StopSearch => self.stop_search(&mut state),
            // Msg::Replace => self.replace(&mut state),
            // Msg::ReplaceAll => self.replace_all(&mut state),
            Msg::Update => self.on_text_change(&mut state),
            _ => {}
        }
    }
}

impl Widget for EditView {
    type Root = gtk::Box;

    fn root(&self) -> Self::Root {
        let state = self.state.borrow();
        state.root_widget.clone()
    }

    fn init_view(&mut self) {
        // self.model.draw_handler.init(&self.sda);
        // self.sda.add_events(EventMask::POINTER_MOTION_MASK);
    }

    fn view(relm: &Relm<Self>, model: Self::Model) -> Self {
        let view_id = model.view_id;
        let da = ScrollableDrawingArea::new();
        let line_da = ScrollableDrawingArea::new();
        line_da.set_size_request(100, 100);
        let sw_hadj: Option<&Adjustment> = None;
        let sw_vadj: Option<&Adjustment> = None;
        let scrolled_window = ScrolledWindow::new(sw_hadj, sw_vadj);
        scrolled_window.add(&da);

        let hadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let vadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        vadj.set_step_increment(1.0);

        scrolled_window.set_hadjustment(Some(&hadj));
        scrolled_window.set_vadjustment(Some(&vadj));
        scrolled_window.set_kinetic_scrolling(true);

        da.set_events(
            EventMask::BUTTON_PRESS_MASK
                | EventMask::BUTTON_RELEASE_MASK
                | EventMask::BUTTON_MOTION_MASK
                | EventMask::SCROLL_MASK
                | EventMask::SMOOTH_SCROLL_MASK,
        );
        debug!("events={:?}", da.get_events());
        da.set_can_focus(true);

        let find_rep_src = include_str!("../ui/find_replace.glade");
        let find_rep_builder = Builder::from_string(find_rep_src);
        let search_bar: SearchBar = find_rep_builder.get_object("search_bar").unwrap();
        let replace_expander: Expander = find_rep_builder.get_object("replace_expander").unwrap();
        let replace_revealer: Revealer = find_rep_builder.get_object("replace_revealer").unwrap();
        let replace_entry: Entry = find_rep_builder.get_object("replace_entry").unwrap();
        let replace_button: Button = find_rep_builder.get_object("replace_button").unwrap();
        let replace_all_button: Button = find_rep_builder.get_object("replace_all_button").unwrap();
        let find_status_label: Label = find_rep_builder.get_object("find_status_label").unwrap();

        // let overlay: Overlay = frame_builder.get_object("overlay").unwrap();
        // let search_revealer: Revealer = frame_builder.get_object("revealer").unwrap();
        // let frame: Frame = frame_builder.get_object("frame").unwrap();
        let search_entry: SearchEntry = find_rep_builder.get_object("search_entry").unwrap();
        let go_down_button: Button = find_rep_builder.get_object("go_down_button").unwrap();
        let go_up_button: Button = find_rep_builder.get_object("go_up_button").unwrap();

        // let style_context = frame.get_style_context().unwrap();
        // style_context.add_provider(&css_provider, 1);

        let line_hbox = Box::new(Orientation::Horizontal, 0);
        line_hbox.pack_start(&line_da, false, false, 0);
        line_hbox.pack_start(&scrolled_window, true, true, 0);

        let main_vbox = Box::new(Orientation::Vertical, 0);
        main_vbox.pack_start(&search_bar, false, false, 0);
        main_vbox.pack_start(&line_hbox, true, true, 0);

        main_vbox.show_all();

        // Make the widgets for the tab
        let tab_hbox = gtk::Box::new(Orientation::Horizontal, 5);
        let label = Label::new(Some(""));
        tab_hbox.add(&label);
        let close_button = Button::from_icon_name(Some("window-close"), IconSize::SmallToolbar);
        tab_hbox.add(&close_button);
        tab_hbox.show_all();

        use std::ffi::CString;
        unsafe {
            let fonts_dir = CString::new("fonts").unwrap();
            let ret = fontconfig::fontconfig::FcConfigAppFontAddDir(
                fontconfig::fontconfig::FcConfigGetCurrent(),
                fonts_dir.as_ptr() as *const u8,
            );
            debug!("fc ret = {}", ret);
        }

        let font_desc = FontDescription::from_string("Inconsolata 20");
        let pango_ctx = da.get_pango_context();
        for family in pango_ctx.list_families() {
            if !family.is_monospace() {
                continue;
            }
            debug!(
                "font family {:?} monospace: {}",
                family.get_name(),
                family.is_monospace()
            );
        }
        pango_ctx.set_font_description(&font_desc);
        let language = pango_ctx
            .get_language()
            .expect("failed to get pango language");
        let fontset = pango_ctx
            .load_fontset(&font_desc, &language)
            .expect("failed to load font set");
        let metrics = fontset.get_metrics().expect("failed to load font metrics");
        debug!("metrics: {}", metrics.get_approximate_digit_width());
        let gutter_pango_ctx = line_da.get_pango_context();
        gutter_pango_ctx.set_font_description(&font_desc);

        // cr.select_font_face("Inconsolata", ::cairo::enums::FontSlant::Normal, ::cairo::enums::FontWeight::Normal);
        // cr.set_font_size(16.0);
        // let font_extents = cr.font_extents();

        let layout = pango::Layout::new(&pango_ctx);
        layout.set_text("a");
        let (_, log_extents) = layout.get_extents();
        debug!("size: {:?}", log_extents);
        debug!("layout_to_pos: {}", layout.index_to_pos(1).x);
        debug!(
            "layout_to_x: {}",
            layout.get_line(0).unwrap().index_to_x(1, false)
        );

        let font_height = f64::from(log_extents.height) / f64::from(pango::SCALE);
        let font_width = f64::from(log_extents.width) / f64::from(pango::SCALE);
        let font_ascent = f64::from(metrics.get_ascent()) / f64::from(pango::SCALE);
        let font_descent = f64::from(metrics.get_descent()) / f64::from(pango::SCALE);

        debug!(
            "font metrics: {} {} {} {}",
            font_width, font_height, font_ascent, font_descent
        );

        // edit_view.borrow_mut().update_title();

        // line_da.connect_draw(clone!(@strong edit_view => move |_,ctx| {
        //     edit_view.borrow_mut().handle_line_draw(&ctx)
        // }));

        connect!(
            relm,
            da,
            connect_button_press_event(_, eb),
            return (Some(Msg::ButtonPress(eb.clone())), Inhibit(false))
        );

        // da.connect_draw(clone!(@strong edit_view => move |_,ctx| {
        //     edit_view.borrow_mut().handle_draw(&ctx)
        // }));

        connect!(
            relm,
            da,
            connect_key_press_event(_, ek),
            return (Some(Msg::KeyPress(ek.clone())), Inhibit(true))
        );

        connect!(
            relm,
            da,
            connect_motion_notify_event(_, em),
            return (Some(Msg::MotionNotify(em.clone())), Inhibit(false))
        );

        da.connect_realize(|w| {
            // Set the text cursor
            if let Some(disp) = DisplayManager::get().get_default_display() {
                let cur = Cursor::new_for_display(&disp, CursorType::Xterm);
                if let Some(win) = w.get_window() {
                    win.set_cursor(Some(&cur))
                }
            }
            w.grab_focus();
        });

        connect!(
            relm,
            da,
            connect_scroll_event(_, es),
            return (Some(Msg::ScrollEvent(es.clone())), Inhibit(false))
        );

        connect!(
            relm,
            da,
            connect_size_allocate(_, alloc),
            Msg::SizeAllocate(alloc.width, alloc.height)
        );

        connect!(
            relm,
            search_entry,
            connect_search_changed(w),
            Msg::SearchChanged(Some(w.get_text().as_str().to_owned()))
        );

        connect!(relm, search_entry, connect_activate(_), Msg::FindNext);

        connect!(relm, search_entry, connect_stop_search(_), Msg::StopSearch);

        replace_expander.connect_property_expanded_notify(
            clone!(@strong replace_revealer => move|w| {
                if w.get_expanded() {
                    replace_revealer.set_reveal_child(true);
                } else {
                    replace_revealer.set_reveal_child(false);
                }
            }),
        );

        connect!(relm, replace_button, connect_clicked(_), Msg::Replace);

        connect!(
            relm,
            replace_all_button,
            connect_clicked(_),
            Msg::ReplaceAll
        );

        connect!(relm, go_down_button, connect_clicked(_), Msg::FindNext);

        connect!(relm, go_up_button, connect_clicked(_), Msg::FindPrev);

        let core = model.workspace.clone();
        let state = Rc::new(RefCell::new(State {
            model,
            line_da: line_da.clone(),
            da: da.clone(),
            root_widget: main_vbox.clone(),
            tab_widget: tab_hbox.clone(),
            label: label.clone(),
            close_button: close_button.clone(),
            search_bar: search_bar.clone(),
            search_entry: search_entry.clone(),
            replace_expander: replace_expander.clone(),
            replace_revealer: replace_revealer.clone(),
            replace_entry: replace_entry.clone(),
            find_status_label: find_status_label.clone(),
            hadj: hadj.clone(),
            vadj: vadj.clone(),
            font_desc,
            visible_lines: 0..1,
            font_height,
            font_width,
            font_ascent,
            font_descent,
        }));

        line_da.connect_draw(clone!(@strong state => move |_, cr| {
            let mut state = state.borrow_mut();
            handle_gutter_draw(&mut state, cr);
            Inhibit(false)
        }));
        da.connect_draw(clone!(@strong state => move |_, cr| {
            let mut state = state.borrow_mut();
            handle_draw(&mut state, cr);
            Inhibit(false)
        }));

        // Subscribe to buffer change events.  Add a callback to queue drawing
        // on our drawing areas.
        {
            let state_ref = state.borrow();
            let mut workspace = state_ref.model.workspace.borrow_mut();
            let buffer = workspace.buffer(state_ref.model.view_id);
            let stream = relm.stream().clone();
            buffer.connect_update(move || stream.emit(Msg::Update));
        }

        {
            let state_ref = state.borrow();
            let mut workspace = state_ref.model.workspace.borrow_mut();
            let buffer = workspace.buffer(state_ref.model.view_id);
            let stream = relm.stream().clone();
            buffer.connect_scroll_to_selections(view_id, move || {
                stream.emit(Msg::ScrollToSelections)
            });
        }

        let ev = EditView { state };

        // do a bunch of initialization that happens when text changes
        {
            let mut state = ev.state.borrow_mut();
            ev.on_text_change(&mut *state);
        }

        ev
    }
}

impl EditView {
    fn handle_key_press_event(&self, state: &mut State, ek: &EventKey) {
        debug!(
            "key press keyval={:?}, state={:?}, length={:?} group={:?} uc={:?}",
            ek.get_keyval(),
            ek.get_state(),
            ek.get_length(),
            ek.get_group(),
            ek.get_keyval().to_unicode(),
        );
        let view_id = state.model.view_id;
        let ch = ek.get_keyval().to_unicode();

        let alt = ek.get_state().contains(ModifierType::MOD1_MASK);
        let ctrl = ek.get_state().contains(ModifierType::CONTROL_MASK);
        let meta = ek.get_state().contains(ModifierType::META_MASK);
        let shift = ek.get_state().contains(ModifierType::SHIFT_MASK);
        let norm = !alt && !ctrl && !meta;

        match ek.get_keyval() {
            key::Delete if norm => state.model.workspace.borrow_mut().delete_forward(view_id),
            key::BackSpace if norm => state.model.workspace.borrow_mut().delete_backward(view_id),
            key::Return | key::KP_Enter => {
                state.model.workspace.borrow_mut().insert_newline(view_id);
            }
            key::Tab if norm && !shift => state.model.workspace.borrow_mut().insert_tab(view_id),
            key::Up if norm && !shift => state.model.workspace.borrow_mut().move_up(view_id),
            key::Down if norm && !shift => state.model.workspace.borrow_mut().move_down(view_id),
            key::Left if norm && !shift => state.model.workspace.borrow_mut().move_left(view_id),
            key::Right if norm && !shift => state.model.workspace.borrow_mut().move_right(view_id),
            key::Up if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_up_and_modify_selection(view_id);
            }
            key::Down if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_down_and_modify_selection(view_id);
            }
            key::Left if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_left_and_modify_selection(view_id);
            }
            key::Right if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_right_and_modify_selection(view_id);
            }
            key::Left if ctrl && !shift => {
                state.model.workspace.borrow_mut().move_word_left(view_id);
            }
            key::Right if ctrl && !shift => {
                state.model.workspace.borrow_mut().move_word_right(view_id);
            }
            key::Left if ctrl && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_word_left_and_modify_selection(view_id);
            }
            key::Right if ctrl && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_word_right_and_modify_selection(view_id);
            }
            key::Home if norm && !shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_left_end_of_line(view_id);
            }
            key::End if norm && !shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_right_end_of_line(view_id);
            }
            key::Home if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_left_end_of_line_and_modify_selection(view_id);
            }
            key::End if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_right_end_of_line_and_modify_selection(view_id);
            }
            key::Home if ctrl && !shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_beginning_of_document(view_id);
            }
            key::End if ctrl && !shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_end_of_document(view_id);
            }
            key::Home if ctrl && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_beginning_of_document_and_modify_selection(view_id);
            }
            key::End if ctrl && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .move_to_end_of_document_and_modify_selection(view_id);
            }
            key::Page_Up if norm && !shift => {
                state.model.workspace.borrow_mut().page_up(view_id);
            }
            key::Page_Down if norm && !shift => {
                state.model.workspace.borrow_mut().page_down(view_id);
            }
            key::Page_Up if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .page_up_and_modify_selection(view_id);
            }
            key::Page_Down if norm && shift => {
                state
                    .model
                    .workspace
                    .borrow_mut()
                    .page_down_and_modify_selection(view_id);
            }
            _ => {
                if let Some(ch) = ch {
                    match ch {
                        'a' if ctrl => {
                            state.model.workspace.borrow_mut().select_all(view_id);
                        }
                        'c' if ctrl => {
                            self.do_copy(state);
                        }
                        'f' if ctrl => {
                            // self.start_search(state);
                        }
                        'v' if ctrl => {
                            self.do_paste(state);
                        }
                        't' if ctrl => {
                            // TODO new tab
                        }
                        'x' if ctrl => {
                            self.do_cut(state);
                        }
                        'z' if ctrl => {
                            state.model.workspace.borrow_mut().undo(view_id);
                        }
                        'Z' if ctrl && shift => {
                            state.model.workspace.borrow_mut().redo(view_id);
                        }
                        c if (norm) && c >= '\u{0020}' => {
                            debug!("inserting key");
                            state
                                .model
                                .workspace
                                .borrow_mut()
                                .insert(view_id, &c.to_string());
                        }
                        _ => {
                            debug!("unhandled key: {:?}", ch);
                        }
                    }
                }
            }
        };
    }

    fn da_size_allocate(&self, state: &mut State, da_width: i32, da_height: i32) {
        let vadj = state.vadj.clone();
        vadj.set_page_size(f64::from(da_height));
        let hadj = state.hadj.clone();
        hadj.set_page_size(f64::from(da_width));

        // self.on_text_change(state);

        // self.update_visible_scroll_region(state);
    }

    fn handle_scroll(&self, _state: &mut State, _es: &EventScroll) {
        // // self.da.grab_focus();
        // // // let amt = self.font_height * 3.0;

        // // if let ScrollDirection::Smooth = es.get_direction() {
        // //     error!("Smooth scroll!");
        // // }

        // // debug!("handle scroll {:?}", es);
        // // let vadj = self.vadj.clone();
        // // let hadj = self.hadj.clone();
        // // match es.get_direction() {
        // //     ScrollDirection::Up => vadj.set_value(vadj.get_value() - amt),
        // //     ScrollDirection::Down => vadj.set_value(vadj.get_value() + amt),
        // //     ScrollDirection::Left => hadj.set_value(hadj.get_value() - amt),
        // //     ScrollDirection::Right => hadj.set_value(hadj.get_value() + amt),
        // //     ScrollDirection::Smooth => debug!("scroll Smooth"),
        // //     _ => {},
        // // }

        // self.update_visible_scroll_region(state);
    }

    fn scroll_to(&self, state: &mut State, line: usize, col: usize) {
        let cur_top = state.font_height * ((line + 1) as f64) - state.font_ascent;
        let cur_bottom = cur_top + state.font_ascent + state.font_descent;
        let vadj = state.vadj.clone();

        let cur_left = state.font_width * (col as f64) - state.font_ascent;
        let cur_right = cur_left + state.font_width * 2.0;
        let hadj = state.hadj.clone();

        if cur_top < vadj.get_value() {
            vadj.set_value(cur_top);
        } else if cur_bottom > vadj.get_value() + vadj.get_page_size()
            && vadj.get_page_size() != 0.0
        {
            vadj.set_value(cur_bottom - vadj.get_page_size());
        }

        if cur_left < hadj.get_value() {
            hadj.set_value(cur_left);
        } else if cur_right > hadj.get_value() + hadj.get_page_size() && hadj.get_page_size() != 0.0
        {
            let new_value = cur_right - hadj.get_page_size();
            if new_value + hadj.get_page_size() > hadj.get_upper() {
                hadj.set_upper(new_value + hadj.get_page_size());
            }
            hadj.set_value(new_value);
        }

        // self.update_visible_scroll_region(state);
    }

    fn scroll_to_selections(&self, state: &mut State) {
        let view_id = state.model.view_id;
        let mut workspace = state.model.workspace.borrow_mut();
        let (buffer, text_theme) = workspace.buffer_and_theme(state.model.view_id);

        let mut min_line = None;
        let mut max_line = None;

        let mut min_x_pos = None;
        let mut max_x_pos = None;

        let sels = buffer.selections(view_id);
        for sel in sels {
            // let start_line = buffer.char_to_line(sel.start);
            // min_line = Some(min_line.map_or(start_line, |y| min(y, start_line)));
            // max_line = Some(max_line.map_or(start_line, |y| max(y, start_line)));
            let end_line = buffer.char_to_line(sel.end);
            min_line = Some(min_line.map_or(end_line, |y| min(y, end_line)));
            max_line = Some(max_line.map_or(end_line, |y| max(y, end_line)));

            let end_line_byte = buffer.char_to_byte(sel.end) - buffer.line_to_byte(end_line);

            // let line_num = (y / state.font_height) as usize;
            if let Some((line, _)) =
                buffer.get_line_with_attributes(state.model.view_id, end_line, &text_theme)
            {
                let pango_ctx = state.da.get_pango_context();

                let layout = create_layout_for_line(state, &pango_ctx, &line, &[]);
                let (_, x_pos) = layout.index_to_line_x(end_line_byte as i32, false);
                let x_pos = x_pos / pango::SCALE;
                min_x_pos = Some(min_x_pos.map_or(x_pos, |y| min(y, x_pos)));
                max_x_pos = Some(max_x_pos.map_or(x_pos, |y| max(y, x_pos)));
            }
        }

        if min_line.is_none() || max_line.is_none() || min_x_pos.is_none() || max_x_pos.is_none() {
            return;
        }

        let min_line = min_line.unwrap();
        let max_line = max_line.unwrap();
        let min_x_pos = min_x_pos.unwrap();
        let max_x_pos = max_x_pos.unwrap();

        let top = state.font_height * ((min_line + 1) as f64) - state.font_ascent;
        let bottom = state.font_height * ((max_line + 1) as f64) + state.font_height;
        let vadj = state.vadj.clone();

        let left = (min_x_pos as f64) - state.font_width;
        let right = (max_x_pos as f64) + state.font_width * 2.0;
        let hadj = state.hadj.clone();

        if top < vadj.get_value() {
            vadj.set_value(top);
        } else if bottom > vadj.get_value() + vadj.get_page_size() && vadj.get_page_size() != 0.0 {
            vadj.set_value(bottom - vadj.get_page_size());
        }

        if left < hadj.get_value() {
            hadj.set_value(left);
        } else if right > hadj.get_value() + hadj.get_page_size() && hadj.get_page_size() != 0.0 {
            let new_value = right - hadj.get_page_size();
            if new_value + hadj.get_page_size() > hadj.get_upper() {
                hadj.set_upper(new_value + hadj.get_page_size());
            }
            hadj.set_value(new_value);
        }
    }

    fn handle_drag(&self, state: &mut State, em: &EventMotion) {
        if em.get_state() != ModifierType::BUTTON1_MASK {
            return;
        }
        let view_id = state.model.view_id;
        let mut workspace = state.model.workspace.borrow_mut();
        let (buffer, text_theme) = workspace.buffer_and_theme(state.model.view_id);

        let (x, y) = em.get_position();
        let (line, byte_idx) = { Self::da_px_to_line_byte_idx(state, buffer, text_theme, x, y) };

        workspace.drag(view_id, line, byte_idx);

        state.line_da.queue_draw();
        state.da.queue_draw();
    }

    fn do_cut(&self, state: &State) {
        let view_id = state.model.view_id;
        let mut workspace = state.model.workspace.borrow_mut();

        if let Some(text) = workspace.cut(view_id) {
            Clipboard::get(&SELECTION_CLIPBOARD).set_text(&text);
        }
    }

    fn do_copy(&self, state: &State) {
        let view_id = state.model.view_id;
        let mut workspace = state.model.workspace.borrow_mut();
        if let Some(text) = workspace.copy(view_id) {
            Clipboard::get(&SELECTION_CLIPBOARD).set_text(&text);
        }
    }

    fn do_paste(&self, state: &State) {
        let view_id = state.model.view_id;
        let workspace = state.model.workspace.clone();
        Clipboard::get(&SELECTION_CLIPBOARD).request_text(move |_, text| {
            if let Some(text) = text {
                // workspace.borrow_mut().view(view_id).paste(text);
                workspace.borrow_mut().insert(view_id, text);
            }
        });
        state.line_da.queue_draw();
        state.da.queue_draw();
    }

    fn do_paste_primary(&self, state: &State, view_id: usize, line: usize, byte_idx: usize) {
        let view_id = state.model.view_id;
        let workspace = state.model.workspace.clone();
        Clipboard::get(&SELECTION_PRIMARY).request_text(move |_, text| {
            if let Some(text) = text {
                workspace
                    .borrow_mut()
                    .gesture_point_select(view_id, line, byte_idx);
                workspace.borrow_mut().insert(view_id, text);
            }
        });
    }

    fn on_text_change(&self, state: &mut State) {
        let mut workspace = state.model.workspace.borrow_mut();
        let (buffer, text_theme) = workspace.buffer_and_theme(state.model.view_id);
        // if let Some(pristine) = update["pristine"].as_bool() {
        //     if state.model.pristine != pristine {
        //         state.model.pristine = pristine;
        //         self.update_title(state);
        //     }
        // }

        state.line_da.queue_draw();
        state.da.queue_draw();

        // let (text_width, text_height) = self.get_text_size(state);
        let text_height = buffer.len_lines() as f64 * state.font_height;
        let vadj = state.vadj.clone();
        let hadj = state.hadj.clone();

        // update scrollbars to the new text width and height
        state.vadj.set_lower(0f64);
        state.vadj.set_upper(text_height as f64);

        // If the last line was removed, scroll up so we're not overscrolled
        if vadj.get_value() + vadj.get_page_size() > vadj.get_upper() {
            vadj.set_value(vadj.get_upper() - vadj.get_page_size())
        }

        // self.update_visible_scroll_region(state)

        // hadj.set_lower(0f64);
        // hadj.set_upper(text_width as f64);
        // if hadj.get_value() + hadj.get_page_size() > hadj.get_upper() {
        //     hadj.set_value(hadj.get_upper() - hadj.get_page_size())
        // }
    }

    fn handle_button_press(&self, state: &mut State, eb: &EventButton) {
        let view_id = state.model.view_id;
        let mut workspace = state.model.workspace.borrow_mut();
        let (buffer, text_theme) = workspace.buffer_and_theme(state.model.view_id);
        state.da.grab_focus();

        let (x, y) = eb.get_position();
        let (line, byte_idx) = { Self::da_px_to_line_byte_idx(state, buffer, text_theme, x, y) };

        match eb.get_button() {
            1 => {
                if eb.get_state().contains(ModifierType::SHIFT_MASK) {
                    buffer.gesture_range_select(view_id, line, byte_idx);
                } else if eb.get_state().contains(ModifierType::CONTROL_MASK) {
                    buffer.gesture_toggle_sel(view_id, line, byte_idx);
                } else if eb.get_event_type() == EventType::DoubleButtonPress {
                    buffer.gesture_word_select(view_id, line, byte_idx);
                } else if eb.get_event_type() == EventType::TripleButtonPress {
                    buffer.gesture_line_select(view_id, line);
                } else {
                    buffer.gesture_point_select(view_id, line, byte_idx);
                }
            }
            2 => {
                self.do_paste_primary(state, view_id, line, byte_idx);
            }
            _ => {}
        }
    }

    fn da_px_to_line_byte_idx(
        state: &State,
        buffer: &Buffer,
        text_theme: &eddy_workspace::style::Theme,
        x: f64,
        y: f64,
    ) -> (usize, usize) {
        // let first_line = (vadj.get_value() / font_extents.height) as usize;
        let x = x + state.hadj.get_value();
        let y = y + state.vadj.get_value();
        let mut y = y - state.font_descent;
        if y < 0.0 {
            y = 0.0;
        }

        let line_num = (y / state.font_height) as usize;
        if let Some((line, attrs)) =
            buffer.get_line_with_attributes(state.model.view_id, line_num, &text_theme)
        {
            let pango_ctx = state.da.get_pango_context();

            let layout = create_layout_for_line(state, &pango_ctx, &line, &[]);
            let (_, index, trailing) = layout.xy_to_index(x as i32 * pango::SCALE, 0);
            let index = index + trailing;
            (line_num, index as usize)
        } else {
            (line_num, 0)
        }
    }
}
fn handle_gutter_draw(state: &mut State, cr: &Context) -> Inhibit {
    let da = state.line_da.clone();

    let da_width = da.get_allocated_width();
    let da_height = da.get_allocated_height();

    let mut workspace = state.model.workspace.borrow_mut();
    let (buffer, text_theme) = workspace.buffer_and_theme(state.model.view_id);

    let num_lines = buffer.len_lines();

    let vadj = state.vadj.clone();
    // let hadj = self.hadj.clone();
    trace!("drawing.  vadj={}, {}", vadj.get_value(), vadj.get_upper());

    let first_line = (vadj.get_value() / state.font_height) as usize;
    let last_line = ((vadj.get_value() + f64::from(da_height)) / state.font_height) as usize + 1;
    let last_line = min(last_line, num_lines);

    // Calculate ordinal or max line length
    let padding: usize = format!("{}", num_lines).len();

    // Just get the gutter size
    let mut gutter_size = 0.0;
    let pango_ctx = da.get_pango_context();
    let linecount_layout = create_layout_for_linecount(&pango_ctx, 0, padding);
    update_layout(cr, &linecount_layout);
    // show_layout(cr, &linecount_layout);

    let linecount_offset = (linecount_layout.get_extents().1.width / pango::SCALE) as f64;
    if linecount_offset > gutter_size {
        gutter_size = linecount_offset;
    }
    let gutter_size = gutter_size as i32;

    da.set_size_request(gutter_size, 0);

    // Draw the gutter background
    // set_source_color(cr, state.model.theme.gutter);
    cr.set_source_rgba(
        text_theme.bg.r_f64(),
        text_theme.bg.g_f64(),
        text_theme.bg.b_f64(),
        1.0,
    );
    cr.rectangle(0.0, 0.0, f64::from(da_width), f64::from(da_height));
    cr.fill();

    for i in first_line..last_line {
        // Keep track of the starting x position
        cr.move_to(0.0, state.font_height * (i as f64) - vadj.get_value());

        // set_source_color(cr, state.model.theme.gutter_foreground);
        cr.set_source_rgba(
            text_theme.fg.r_f64(),
            text_theme.fg.g_f64(),
            text_theme.fg.b_f64(),
            1.0,
        );

        let pango_ctx = da.get_pango_context();
        let linecount_layout = create_layout_for_linecount(&pango_ctx, i + 1, padding);
        update_layout(cr, &linecount_layout);
        show_layout(cr, &linecount_layout);
    }

    Inhibit(false)
}
/// Creates a pango layout for a particular line number
fn create_layout_for_linecount(
    pango_ctx: &pango::Context,
    n: usize,
    padding: usize,
) -> pango::Layout {
    let line_view = format!("{:>offset$} ", n, offset = padding);
    let layout = pango::Layout::new(pango_ctx);
    layout.set_text(line_view.as_str());
    layout
}

fn handle_draw(state: &mut State, cr: &Context) -> Inhibit {
    let draw_start = Instant::now();
    // let foreground = self.model.main_state.borrow().theme.foreground;
    let theme = &state.model.theme;
    let da = state.da.clone();

    let da_width = state.da.get_allocated_width();
    let da_height = state.da.get_allocated_height();

    let mut workspace = state.model.workspace.borrow_mut();
    let view_id = state.model.view_id;
    let (buffer, text_theme) = workspace.buffer_and_theme(view_id);

    //debug!("Drawing");
    // cr.select_font_face("Mono", ::cairo::enums::FontSlant::Normal, ::cairo::enums::FontWeight::Normal);
    // let mut font_options = cr.get_font_options();
    // debug!("font options: {:?} {:?} {:?}", font_options, font_options.get_antialias(), font_options.get_hint_style());
    // font_options.set_hint_style(HintStyle::Full);

    // let (text_width, text_height) = self.get_text_size();
    let num_lines = buffer.len_lines();

    let vadj = state.vadj.clone();
    let hadj = state.hadj.clone();
    trace!("drawing.  vadj={}, {}", vadj.get_value(), vadj.get_upper());

    let first_line = (vadj.get_value() / state.font_height) as usize;
    let last_line = ((vadj.get_value() + f64::from(da_height)) / state.font_height) as usize + 1;
    let last_line = min(last_line, num_lines);
    let visible_lines = first_line..last_line;

    let pango_ctx = state.da.get_pango_context();

    // Draw background
    // set_source_color(cr, text_theme.background);
    cr.set_source_rgba(
        text_theme.bg.r_f64(),
        text_theme.bg.g_f64(),
        text_theme.bg.b_f64(),
        1.0,
    );
    cr.rectangle(0.0, 0.0, f64::from(da_width), f64::from(da_height));
    cr.fill();

    // set_source_color(cr, theme.foreground);
    cr.set_source_rgba(
        text_theme.fg.r_f64(),
        text_theme.fg.g_f64(),
        text_theme.fg.b_f64(),
        1.0,
    );

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

            cr.move_to(
                -hadj.get_value(),
                state.font_height * (i as f64) - vadj.get_value(),
            );

            cr.set_source_rgba(
                text_theme.fg.r_f64(),
                text_theme.fg.g_f64(),
                text_theme.fg.b_f64(),
                1.0,
            );

            let layout = create_layout_for_line(state, &pango_ctx, &line, &attrs);
            max_width = max(max_width, layout.get_extents().1.width);
            update_layout(cr, &layout);
            show_layout(cr, &layout);

            let layout_line = layout.get_line(0);
            if layout_line.is_none() {
                continue;
            }
            let layout_line = layout_line.unwrap();

            // Draw the cursors
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
                    (x as f64) - hadj.get_value(),
                    (((state.font_height) as usize) * i) as f64 - vadj.get_value(),
                    CURSOR_WIDTH,
                    state.font_height,
                );
                cr.fill();
            }
        }
    }

    // Now that we know actual length of the text, adjust the scrollbar properly.
    // But we need to make sure we don't make the upper value smaller than the current viewport
    let mut h_upper = f64::from(max_width / pango::SCALE);
    let cur_h_max = hadj.get_value() + hadj.get_page_size();
    if cur_h_max > h_upper {
        h_upper = cur_h_max;
    }

    if hadj.get_upper() != h_upper {
        hadj.set_upper(h_upper);
        // If I don't signal that the value changed, sometimes the overscroll "shadow" will stick
        // This seems to make sure to tell the viewport that something has changed so it can
        // reevaluate its need for a scroll shadow.
        hadj.value_changed();
    }

    let draw_end = Instant::now();
    debug!("drawing took {}ms", (draw_end - draw_start).as_millis());

    Inhibit(false)
}

/// Creates a pango layout for a particular line in the linecache
fn create_layout_for_line(
    state: &State,
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
        if let Some(ref mut pattr) = pattr {
            pattr.set_start_index(aspan.start_idx as u32);
            pattr.set_end_index(aspan.end_idx as u32);
        }
        if let Some(pattr) = pattr {
            attr_list.insert(pattr);
        }
    }

    layout.set_attributes(Some(&attr_list));
    layout
}

/*
impl EditView {
    // pub fn set_file(&mut self, file_name: &str) {
    //     let mut state = self.state.borrow_mut();

    //     state.model.file_name = Some(file_name.to_string());
    //     self.update_title();
    // }

    fn update_title(&self, state: &mut State) {
        let title = match state.model.file_name {
            Some(ref f) => f
                .split(::std::path::MAIN_SEPARATOR)
                .last()
                .unwrap_or("Untitled")
                .to_string(),
            None => "Untitled".to_string(),
        };

        let mut full_title = String::new();
        if !state.model.pristine {
            full_title.push('*');
        }
        full_title.push_str(&title);

        trace!("setting title to {}", full_title);
        state.label.set_text(&full_title);
    }

    fn handle_update(&self, state: &mut State, params: &Value) {
        let update = &params["update"];

        state.line_cache.apply_update(update);

        if let Some(pristine) = update["pristine"].as_bool() {
            if state.model.pristine != pristine {
                state.model.pristine = pristine;
                self.update_title(state);
            }
        }

        state.line_da.queue_draw();
        state.da.queue_draw();

        let (text_width, text_height) = self.get_text_size(state);
        let vadj = state.vadj.clone();
        let hadj = state.hadj.clone();

        // update scrollbars to the new text width and height
        state.vadj.set_lower(0f64);
        state.vadj.set_upper(text_height as f64);
        if vadj.get_value() + vadj.get_page_size() > vadj.get_upper() {
            vadj.set_value(vadj.get_upper() - vadj.get_page_size())
        }

        self.update_visible_scroll_region(state)

        // hadj.set_lower(0f64);
        // hadj.set_upper(text_width as f64);
        // if hadj.get_value() + hadj.get_page_size() > hadj.get_upper() {
        //     hadj.set_value(hadj.get_upper() - hadj.get_page_size())
        // }
    }

    fn config_changed(&self, state: &mut State, changes: &Value) {
        if let Some(map) = changes.as_object() {
            for (name, value) in map {
                match name.as_ref() {
                    "font_size" => {
                        if let Some(font_size) = value.as_u64() {
                            state.font_desc.set_size(font_size as i32 * pango::SCALE);
                        }
                    }
                    "font_face" => {
                        if let Some(font_face) = value.as_str() {
                            if font_face == "InconsolataGo" {
                                // TODO This shouldn't be necessary, but the only font I've found
                                // to bundle is "Inconsolata"
                                state.font_desc.set_family("Inconsolata");
                            } else {
                                state.font_desc.set_family(font_face);
                            }
                        }
                    }
                    _ => {
                        error!("unhandled config option {}", name);
                    }
                }
            }
        }
    }

    // pub fn update_notification(&mut self, params: &Value) {
    //     let update = &params["update"];
    //     let (text_width, text_height, vadj, hadj) = {
    //         self.line_cache.apply_update(update);

    //         if let Some(pristine) = update["pristine"].as_bool() {
    //             if self.model.pristine != pristine {
    //                 self.model.pristine = pristine;
    //                 self.update_title();
    //             }
    //         }

    //         self.line_da.queue_draw();
    //         self.da.queue_draw();

    //         let (text_width, text_height) = self.get_text_size();
    //         let vadj = self.vadj.clone();
    //         let hadj = self.hadj.clone();

    //         (text_width, text_height, vadj, hadj)
    //     };
    //     // update scrollbars to the new text width and height
    //     vadj.set_lower(0f64);
    //     vadj.set_upper(text_height as f64);
    //     if vadj.get_value() + vadj.get_page_size() > vadj.get_upper() {
    //         vadj.set_value(vadj.get_upper() - vadj.get_page_size())
    //     }

    //     self.update_visible_scroll_region()

    //     // hadj.set_lower(0f64);
    //     // hadj.set_upper(text_width as f64);
    //     // if hadj.get_value() + hadj.get_page_size() > hadj.get_upper() {
    //     //     hadj.set_value(hadj.get_upper() - hadj.get_page_size())
    //     // }
    // }

    fn create_layout_for_line(
        &self,
        state: &State,
        pango_ctx: &pango::Context,
        line: &Line,
    ) -> pango::Layout {
        super::editview::create_layout_for_line(state, pango_ctx, line)
    }

    fn da_px_to_cell(&self, state: &mut State, x: f64, y: f64) -> (u64, u64) {
        // let first_line = (vadj.get_value() / font_extents.height) as usize;
        let x = x + state.hadj.get_value();
        let y = y + state.vadj.get_value();

        let mut y = y - state.font_descent;
        if y < 0.0 {
            y = 0.0;
        }
        let line_num = (y / state.font_height) as u64;
        let index = if let Some(line) = state.line_cache.get_line(line_num) {
            let pango_ctx = state
                .da
                .get_pango_context()
                .expect("failed to get pango ctx");

            let layout = self.create_layout_for_line(state, &pango_ctx, line);
            let (_, index, trailing) = layout.xy_to_index(x as i32 * pango::SCALE, 0);
            index + trailing
        } else {
            0
        };
        (index as u64, (y / state.font_height) as u64)
    }

    fn da_size_allocate(&self, state: &mut State, da_width: i32, da_height: i32) {
        // debug!("DA SIZE ALLOCATE");
        let vadj = state.vadj.clone();
        vadj.set_page_size(f64::from(da_height));
        let hadj = state.hadj.clone();
        hadj.set_page_size(f64::from(da_width));

        self.update_visible_scroll_region(state);
    }

    /// Inform core that the visible scroll region has changed
    fn update_visible_scroll_region(&self, state: &mut State) {
        let da_height = state.da.get_allocated_height();
        let (_, first_line) = self.da_px_to_cell(state, 0.0, 0.0);
        let (_, last_line) = self.da_px_to_cell(state, 0.0, f64::from(da_height));
        let last_line = last_line + 1;
        let visible_lines = first_line..last_line;
        if visible_lines != state.visible_lines {
            state.visible_lines = visible_lines;
            debug!("visible lines: [{}-{}]", first_line, last_line);
            self.core
                .scroll(&state.model.view_id, first_line, last_line);
        }
    }

    fn get_text_size(&self, state: &mut State) -> (f64, f64) {
        let da_width = f64::from(state.da.get_allocated_width());
        let da_height = f64::from(state.da.get_allocated_height());
        let num_lines = state.line_cache.height();

        let all_text_height = num_lines as f64 * state.font_height + state.font_descent;
        let height = if da_height > all_text_height {
            da_height
        } else {
            all_text_height
        };

        let all_text_width = state.line_cache.width() as f64 * state.font_width;
        let width = if da_width > all_text_width {
            da_width
        } else {
            all_text_width
        };
        (width, height)
    }

    fn scroll_to(&self, state: &mut State, line: u64, col: u64) {
        let cur_top = state.font_height * ((line + 1) as f64) - state.font_ascent;
        let cur_bottom = cur_top + state.font_ascent + state.font_descent;
        let vadj = state.vadj.clone();

        let cur_left = state.font_width * (col as f64) - state.font_ascent;
        let cur_right = cur_left + state.font_width * 2.0;
        let hadj = state.hadj.clone();

        if cur_top < vadj.get_value() {
            vadj.set_value(cur_top);
        } else if cur_bottom > vadj.get_value() + vadj.get_page_size()
            && vadj.get_page_size() != 0.0
        {
            vadj.set_value(cur_bottom - vadj.get_page_size());
        }

        if cur_left < hadj.get_value() {
            hadj.set_value(cur_left);
        } else if cur_right > hadj.get_value() + hadj.get_page_size() && hadj.get_page_size() != 0.0
        {
            let new_value = cur_right - hadj.get_page_size();
            if new_value + hadj.get_page_size() > hadj.get_upper() {
                hadj.set_upper(new_value + hadj.get_page_size());
            }
            hadj.set_value(new_value);
        }

        self.update_visible_scroll_region(state);
    }

    fn handle_drag(&self, state: &mut State, em: &EventMotion) {
        let (x, y) = em.get_position();
        let (col, line) = { self.da_px_to_cell(state, x, y) };
        state.model.core.drag(
            &state.model.view_id,
            line,
            col,
            convert_gtk_modifier(em.get_state()),
        );
    }

    fn handle_scroll(&self, state: &mut State, _es: &EventScroll) {
        // self.da.grab_focus();
        // // let amt = self.font_height * 3.0;

        // if let ScrollDirection::Smooth = es.get_direction() {
        //     error!("Smooth scroll!");
        // }

        // debug!("handle scroll {:?}", es);
        // let vadj = self.vadj.clone();
        // let hadj = self.hadj.clone();
        // match es.get_direction() {
        //     ScrollDirection::Up => vadj.set_value(vadj.get_value() - amt),
        //     ScrollDirection::Down => vadj.set_value(vadj.get_value() + amt),
        //     ScrollDirection::Left => hadj.set_value(hadj.get_value() - amt),
        //     ScrollDirection::Right => hadj.set_value(hadj.get_value() + amt),
        //     ScrollDirection::Smooth => debug!("scroll Smooth"),
        //     _ => {},
        // }

        self.update_visible_scroll_region(state);
    }

    fn handle_key_press_event(&self, state: &mut State, ek: &EventKey) {
        debug!(
            "key press keyval={:?}, state={:?}, length={:?} group={:?} uc={:?}",
            ek.get_keyval(),
            ek.get_state(),
            ek.get_length(),
            ek.get_group(),
            ::gdk::keyval_to_unicode(ek.get_keyval())
        );
        let view_id = &state.model.view_id;
        let ch = ::gdk::keyval_to_unicode(ek.get_keyval());

        let alt = ek.get_state().contains(ModifierType::MOD1_MASK);
        let ctrl = ek.get_state().contains(ModifierType::CONTROL_MASK);
        let meta = ek.get_state().contains(ModifierType::META_MASK);
        let shift = ek.get_state().contains(ModifierType::SHIFT_MASK);
        let norm = !alt && !ctrl && !meta;

        match ek.get_keyval() {
            key::Delete if norm => self.core.delete_forward(view_id),
            key::BackSpace if norm => self.core.delete_backward(view_id),
            key::Return | key::KP_Enter => {
                self.core.insert_newline(&view_id);
            }
            key::Tab if norm && !shift => self.core.insert_tab(view_id),
            key::Up if norm && !shift => self.core.move_up(view_id),
            key::Down if norm && !shift => self.core.move_down(view_id),
            key::Left if norm && !shift => self.core.move_left(view_id),
            key::Right if norm && !shift => self.core.move_right(view_id),
            key::Up if norm && shift => {
                self.core.move_up_and_modify_selection(view_id);
            }
            key::Down if norm && shift => {
                self.core.move_down_and_modify_selection(view_id);
            }
            key::Left if norm && shift => {
                self.core.move_left_and_modify_selection(view_id);
            }
            key::Right if norm && shift => {
                self.core.move_right_and_modify_selection(view_id);
            }
            key::Left if ctrl && !shift => {
                self.core.move_word_left(view_id);
            }
            key::Right if ctrl && !shift => {
                self.core.move_word_right(view_id);
            }
            key::Left if ctrl && shift => {
                self.core.move_word_left_and_modify_selection(view_id);
            }
            key::Right if ctrl && shift => {
                self.core.move_word_right_and_modify_selection(view_id);
            }
            key::Home if norm && !shift => {
                self.core.move_to_left_end_of_line(view_id);
            }
            key::End if norm && !shift => {
                self.core.move_to_right_end_of_line(view_id);
            }
            key::Home if norm && shift => {
                self.core
                    .move_to_left_end_of_line_and_modify_selection(view_id);
            }
            key::End if norm && shift => {
                self.core
                    .move_to_right_end_of_line_and_modify_selection(view_id);
            }
            key::Home if ctrl && !shift => {
                self.core.move_to_beginning_of_document(view_id);
            }
            key::End if ctrl && !shift => {
                self.core.move_to_end_of_document(view_id);
            }
            key::Home if ctrl && shift => {
                self.core
                    .move_to_beginning_of_document_and_modify_selection(view_id);
            }
            key::End if ctrl && shift => {
                self.core
                    .move_to_end_of_document_and_modify_selection(view_id);
            }
            key::Page_Up if norm && !shift => {
                self.core.page_up(view_id);
            }
            key::Page_Down if norm && !shift => {
                self.core.page_down(view_id);
            }
            key::Page_Up if norm && shift => {
                self.core.page_up_and_modify_selection(view_id);
            }
            key::Page_Down if norm && shift => {
                self.core.page_down_and_modify_selection(view_id);
            }
            _ => {
                if let Some(ch) = ch {
                    match ch {
                        'a' if ctrl => {
                            self.core.select_all(view_id);
                        }
                        'c' if ctrl => {
                            self.do_copy(view_id);
                        }
                        'f' if ctrl => {
                            self.start_search(state);
                        }
                        'v' if ctrl => {
                            self.do_paste(view_id);
                        }
                        't' if ctrl => {
                            // TODO new tab
                        }
                        'x' if ctrl => {
                            self.do_cut(view_id);
                        }
                        'z' if ctrl => {
                            self.core.undo(view_id);
                        }
                        'Z' if ctrl && shift => {
                            self.core.redo(view_id);
                        }
                        c if (norm) && c >= '\u{0020}' => {
                            debug!("inserting key");
                            self.core.insert(view_id, &c.to_string());
                        }
                        _ => {
                            debug!("unhandled key: {:?}", ch);
                        }
                    }
                }
            }
        };
    }

    fn do_cut(&self, view_id: &str) {
        if let Some(text) = self.core.cut(view_id) {
            Clipboard::get(&SELECTION_CLIPBOARD).set_text(&text);
        }
    }

    fn do_copy(&self, view_id: &str) {
        if let Some(text) = self.core.copy(view_id) {
            Clipboard::get(&SELECTION_CLIPBOARD).set_text(&text);
        }
    }

    fn do_paste(&self, view_id: &str) {
        let view_id2 = view_id.to_string().clone();
        let core = self.core.clone();
        Clipboard::get(&SELECTION_CLIPBOARD).request_text(move |_, text| {
            if let Some(text) = text {
                core.paste(&view_id2, &text);
            }
        });
    }

    fn do_paste_primary(&self, view_id: &str, line: u64, col: u64) {
        let view_id2 = view_id.to_string().clone();
        let core = self.core.clone();
        Clipboard::get(&SELECTION_PRIMARY).request_text(move |_, text| {
            if let Some(text) = text {
                core.gesture_point_select(&view_id2, line, col);
                core.insert(&view_id2, text);
            }
        });
    }

    fn start_search(&self, state: &mut State) {
        state.search_bar.set_search_mode(true);
        state.replace_expander.set_expanded(false);
        state.replace_revealer.set_reveal_child(false);
        state.search_entry.grab_focus();
        let needle = state
            .search_entry
            .get_text()
            .map(|gs| gs.as_str().to_owned())
            .unwrap_or_default();
        state
            .model
            .core
            .find(&state.model.view_id, needle, false, Some(false));
    }

    fn stop_search(&self, state: &mut State) {
        state.search_bar.set_search_mode(false);
        state.da.grab_focus();
    }

    fn find_status(&self, state: &mut State, queries: &Value) {
        if let Some(queries) = queries.as_array() {
            for query in queries {
                if let Some(query_obj) = query.as_object() {
                    if let Some(matches) = query_obj["matches"].as_u64() {
                        state
                            .find_status_label
                            .set_text(&format!("{} Results", matches));
                    }
                }
                debug!("query {}", query);
            }
        }
    }

    fn find_next(&self, state: &mut State) {
        self.core
            .find_next(&state.model.view_id, Some(true), Some(true));
    }

    fn find_prev(&self, state: &mut State) {
        self.core.find_previous(&state.model.view_id, Some(true));
    }

    fn search_changed(&self, state: &mut State, s: Option<String>) {
        let needle = s.unwrap_or_default();
        self.core
            .find(&state.model.view_id, needle, false, Some(false));
    }

    fn replace(&self, state: &mut State) {
        let replace_chars = state
            .replace_entry
            .get_text()
            .map(|gs| gs.as_str().to_owned())
            .unwrap_or_default();
        self.core
            .replace(&state.model.view_id, &replace_chars, false);
        self.core.replace_next(&state.model.view_id);
    }

    fn replace_all(&self, state: &mut State) {
        let replace_chars = state
            .replace_entry
            .get_text()
            .map(|gs| gs.as_str().to_owned())
            .unwrap_or_default();
        self.core
            .replace(&state.model.view_id, &replace_chars, false);
        self.core.replace_all(&state.model.view_id);
    }
}

fn convert_gtk_modifier(mt: ModifierType) -> u32 {
    let mut ret = 0;
    if mt.contains(ModifierType::SHIFT_MASK) {
        ret |= rpc::XI_SHIFT_KEY_MASK;
    }
    if mt.contains(ModifierType::CONTROL_MASK) {
        ret |= rpc::XI_CONTROL_KEY_MASK;
    }
    if mt.contains(ModifierType::MOD1_MASK) {
        ret |= rpc::XI_ALT_KEY_MASK;
    }
    ret
}

fn handle_line_draw(state: &mut State, cr: &Context) -> Inhibit {
    // let foreground = self.model.main_state.borrow().theme.foreground;
    let main_state = state.model.main_state.borrow();
    let theme = &main_state.theme;
    let da = state.line_da.clone();

    let da_width = da.get_allocated_width();
    let da_height = da.get_allocated_height();

    let num_lines = state.line_cache.height();

    let vadj = state.vadj.clone();
    // let hadj = self.hadj.clone();
    trace!("drawing.  vadj={}, {}", vadj.get_value(), vadj.get_upper());

    let first_line = (vadj.get_value() / state.font_height) as u64;
    let last_line = ((vadj.get_value() + f64::from(da_height)) / state.font_height) as u64 + 1;
    let last_line = min(last_line, num_lines);

    // Find missing lines
    let mut found_missing = false;
    for i in first_line..last_line {
        if state.line_cache.get_line(i).is_none() {
            debug!("missing line {}", i);
            found_missing = true;
        }
    }

    // We've already missed our chance to draw these lines, but we need to request them for the
    // next frame.  This needs to be improved to prevent flashing.
    if found_missing {
        debug!(
            "didn't have some lines, requesting, lines {}-{}",
            first_line, last_line
        );
        // self.model
        //     .core
        //     .request_lines(&self.model.view_id, first_line as u64, last_line as u64);
    }

    // Calculate ordinal or max line length
    let padding: usize = format!("{}", num_lines.saturating_sub(1)).len();

    // Just get the gutter size
    let mut gutter_size = 0.0;
    let pango_ctx = da.get_pango_context().expect("failed to get pango ctx");
    let linecount_layout = create_layout_for_linecount(&pango_ctx, &main_state, 0, padding);
    update_layout(cr, &linecount_layout);
    // show_layout(cr, &linecount_layout);

    let linecount_offset = (linecount_layout.get_extents().1.width / pango::SCALE) as f64;
    if linecount_offset > gutter_size {
        gutter_size = linecount_offset;
    }
    let gutter_size = gutter_size as i32;

    da.set_size_request(gutter_size, 0);

    // Draw the gutter background
    set_source_color(cr, theme.gutter);
    cr.rectangle(0.0, 0.0, f64::from(da_width), f64::from(da_height));
    cr.fill();

    for i in first_line..last_line {
        // Keep track of the starting x position
        if let Some(_) = state.line_cache.get_line(i) {
            cr.move_to(0.0, state.font_height * (i as f64) - vadj.get_value());

            set_source_color(cr, theme.gutter_foreground);
            let pango_ctx = da.get_pango_context().expect("failed to get pango ctx");
            let linecount_layout = create_layout_for_linecount(&pango_ctx, &main_state, i, padding);
            update_layout(cr, &linecount_layout);
            show_layout(cr, &linecount_layout);
        }
    }

    Inhibit(false)
}

fn handle_draw(state: &mut State, cr: &Context) -> Inhibit {
    // let foreground = self.model.main_state.borrow().theme.foreground;
    let main_state = state.model.main_state.borrow();
    let theme = &main_state.theme;
    let da = state.da.clone();

    let da_width = state.da.get_allocated_width();
    let da_height = state.da.get_allocated_height();

    //debug!("Drawing");
    // cr.select_font_face("Mono", ::cairo::enums::FontSlant::Normal, ::cairo::enums::FontWeight::Normal);
    // let mut font_options = cr.get_font_options();
    // debug!("font options: {:?} {:?} {:?}", font_options, font_options.get_antialias(), font_options.get_hint_style());
    // font_options.set_hint_style(HintStyle::Full);

    // let (text_width, text_height) = self.get_text_size();
    let num_lines = state.line_cache.height();

    let vadj = state.vadj.clone();
    let hadj = state.hadj.clone();
    trace!("drawing.  vadj={}, {}", vadj.get_value(), vadj.get_upper());

    let first_line = (vadj.get_value() / state.font_height) as u64;
    let last_line = ((vadj.get_value() + f64::from(da_height)) / state.font_height) as u64 + 1;
    let last_line = min(last_line, num_lines);

    // debug!("line_cache {} {} {}", self.line_cache.n_invalid_before, self.line_cache.lines.len(), self.line_cache.n_invalid_after);
    // let missing = self.line_cache.get_missing(first_line, last_line);

    // Find missing lines
    let mut found_missing = false;
    for i in first_line..last_line {
        if state.line_cache.get_line(i).is_none() {
            debug!("missing line {}", i);
            found_missing = true;
        }
    }

    // We've already missed our chance to draw these lines, but we need to request them for the
    // next frame.  This needs to be improved to prevent flashing.
    if found_missing {
        debug!(
            "didn't have some lines, requesting, lines {}-{}",
            first_line, last_line
        );
        state
            .model
            .core
            .request_lines(&state.model.view_id, first_line as u64, last_line as u64);
    }

    let pango_ctx = state.da.get_pango_context().unwrap();

    // Draw background
    set_source_color(cr, theme.background);
    cr.rectangle(0.0, 0.0, f64::from(da_width), f64::from(da_height));
    cr.fill();

    set_source_color(cr, theme.foreground);

    // Highlight cursor lines
    // for i in first_line..last_line {
    //     cr.set_source_rgba(0.8, 0.8, 0.8, 1.0);
    //     if let Some(line) = self.line_cache.get_line(i) {

    //         if !line.cursor().is_empty() {
    //             cr.set_source_rgba(0.23, 0.23, 0.23, 1.0);
    //             cr.rectangle(0f64,
    //                 font_extents.height*((i+1) as f64) - font_extents.ascent - vadj.get_value(),
    //                 da_width as f64,
    //                 font_extents.ascent + font_extents.descent);
    //             cr.fill();
    //         }
    //     }
    // }

    const CURSOR_WIDTH: f64 = 2.0;
    // Calculate ordinal or max line length
    let padding: usize = format!("{}", num_lines.saturating_sub(1)).len();

    let mut max_width = 0;

    let main_state = state.model.main_state.borrow();

    for i in first_line..last_line {
        // Keep track of the starting x position
        if let Some(line) = state.line_cache.get_line(i) {
            cr.move_to(
                -hadj.get_value(),
                state.font_height * (i as f64) - vadj.get_value(),
            );

            // let pango_ctx = self
            //     .da
            //     .get_pango_context()
            //     .expect("failed to get pango ctx");

            set_source_color(cr, theme.foreground);
            let layout = create_layout_for_line(state, &pango_ctx, line);
            max_width = max(max_width, layout.get_extents().1.width);
            // debug!("width={}", layout.get_extents().1.width);
            update_layout(cr, &layout);
            show_layout(cr, &layout);

            let layout_line = layout.get_line(0);
            if layout_line.is_none() {
                continue;
            }
            let layout_line = layout_line.unwrap();

            // Draw the cursor
            set_source_color(cr, theme.caret);

            for c in line.cursor() {
                let x = layout_line.index_to_x(*c as i32, false) / pango::SCALE;
                cr.rectangle(
                    (x as f64) - hadj.get_value(),
                    (((state.font_height) as u64) * i) as f64 - vadj.get_value(),
                    CURSOR_WIDTH,
                    state.font_height,
                );
                cr.fill();
            }
        }
    }

    // Now that we know actual length of the text, adjust the scrollbar properly.
    // But we need to make sure we don't make the upper value smaller than the current viewport
    let mut h_upper = f64::from(max_width / pango::SCALE);
    let cur_h_max = hadj.get_value() + hadj.get_page_size();
    if cur_h_max > h_upper {
        h_upper = cur_h_max;
    }

    if hadj.get_upper() != h_upper {
        hadj.set_upper(h_upper);
        // If I don't signal that the value changed, sometimes the overscroll "shadow" will stick
        // This seems to make sure to tell the viewport that something has changed so it can
        // reevaluate its need for a scroll shadow.
        hadj.value_changed();
    }

    Inhibit(false)
}

/// Creates a pango layout for a particular line number
fn create_layout_for_linecount(
    pango_ctx: &pango::Context,
    main_state: &MainState,
    n: u64,
    padding: usize,
) -> pango::Layout {
    let line_view = format!("{:>offset$} ", n, offset = padding);
    let layout = pango::Layout::new(pango_ctx);
    layout.set_text(line_view.as_str());
    layout
}

/// Creates a pango layout for a particular line in the linecache
fn create_layout_for_line(state: &State, pango_ctx: &pango::Context, line: &Line) -> pango::Layout {
    let main_state = state.model.main_state.borrow();

    let line_view = if line.text().ends_with('\n') {
        &line.text()[0..line.text().len() - 1]
    } else {
        &line.text()
    };

    // let layout = create_layout(cr).unwrap();
    let layout = pango::Layout::new(pango_ctx);
    layout.set_text(line_view);

    let mut ix = 0;
    let attr_list = pango::AttrList::new();
    for style in &line.styles {
        let start_index = (ix + style.start) as u32;
        let end_index = (ix + style.start + style.len as i64) as u32;

        let foreground = main_state.styles.get(style.id).and_then(|s| s.fg_color);
        if let Some(foreground) = foreground {
            let mut attr = Attribute::new_foreground(
                foreground.r_u16(),
                foreground.g_u16(),
                foreground.b_u16(),
            )
            .unwrap();
            attr.set_start_index(start_index);
            attr.set_end_index(end_index);
            attr_list.insert(attr);
        }

        let background = main_state.styles.get(style.id).and_then(|s| s.bg_color);
        if let Some(background) = background {
            let mut attr = Attribute::new_background(
                background.r_u16(),
                background.g_u16(),
                background.b_u16(),
            )
            .unwrap();
            attr.set_start_index(start_index);
            attr.set_end_index(end_index);
            attr_list.insert(attr);
        }

        let weight = main_state.styles.get(style.id).and_then(|s| s.weight);
        if let Some(weight) = weight {
            let mut attr = Attribute::new_weight(pango::Weight::__Unknown(weight as i32)).unwrap();
            attr.set_start_index(start_index);
            attr.set_end_index(end_index);
            attr_list.insert(attr);
        }

        let italic = main_state.styles.get(style.id).and_then(|s| s.italic);
        if let Some(italic) = italic {
            let mut attr = if italic {
                Attribute::new_style(pango::Style::Italic).unwrap()
            } else {
                Attribute::new_style(pango::Style::Normal).unwrap()
            };
            attr.set_start_index(start_index);
            attr.set_end_index(end_index);
            attr_list.insert(attr);
        }

        let underline = main_state.styles.get(style.id).and_then(|s| s.underline);
        if let Some(underline) = underline {
            let mut attr = if underline {
                Attribute::new_underline(pango::Underline::Single).unwrap()
            } else {
                Attribute::new_underline(pango::Underline::None).unwrap()
            };
            attr.set_start_index(start_index);
            attr.set_end_index(end_index);
            attr_list.insert(attr);
        }

        ix += style.start + style.len as i64;
    }

    layout.set_attributes(Some(&attr_list));
    layout
}
*/
