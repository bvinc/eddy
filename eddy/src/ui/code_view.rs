use super::{CodeViewText, Gutter};
use crate::theme::Theme;
use eddy_workspace::style::{Attr, AttrSpan};
use eddy_workspace::{BufferId, Event, ViewId, Workspace};
use gdk::{Key, ModifierType};
use glib::{clone, Sender};
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, Adjustment};
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

pub struct CodeViewPrivate {
    cvt: OnceCell<CodeViewText>,
    gutter: OnceCell<Gutter>,
    hadj: Adjustment,
    vadj: Adjustment,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: Cell<usize>,
    theme: Theme,
    scrolled_window: OnceCell<gtk::ScrolledWindow>,
}

#[glib::object_subclass]
impl ObjectSubclass for CodeViewPrivate {
    const NAME: &'static str = "CodeView";
    type Type = CodeView;
    type ParentType = gtk::Box;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    fn new() -> Self {
        let workspace = OnceCell::new();
        let cvt = OnceCell::new();
        let gutter = OnceCell::new();
        let view_id = Cell::new(0);
        let theme = Theme::default();

        let hadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let vadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        Self {
            workspace,
            cvt,
            gutter,
            hadj,
            vadj,
            view_id,
            theme,
            scrolled_window: OnceCell::new(),
        }
    }
}

impl ObjectImpl for CodeViewPrivate {
    fn constructed(&self) {
        dbg!("cv constructed");
        self.parent_constructed();

        // obj.set_focusable(true);
        // obj.set_can_focus(true);

        // let gesture_click = gtk::GestureClick::new();
        // gesture_click.connect_pressed(clone!(@strong obj as this => move |_w, n_press, x, y| {
        //     this.button_pressed(n_press, x, y);
        //     this.grab_focus();
        //     debug!("clicked");
        // }));
        // obj.add_controller(&gesture_click);
    }
}
impl WidgetImpl for CodeViewPrivate {}
impl BoxImpl for CodeViewPrivate {}

impl CodeViewPrivate {
    fn buffer_changed(&self) {
        self.cvt.get().unwrap().buffer_changed();
        self.gutter.get().unwrap().buffer_changed();
    }

    fn scroll_to_carets(&self) {
        self.cvt.get().unwrap().scroll_to_carets();
    }

    //fn get_text_node(&self,
    fn button_pressed() {
        debug!("button pressed");
    }
}

glib::wrapper! {
    pub struct CodeView(ObjectSubclass<CodeViewPrivate>)
    @extends gtk::Box, gtk::Widget;
}

impl CodeView {
    pub fn new(workspace: Rc<RefCell<Workspace>>, view_id: ViewId) -> Self {
        let obj = glib::Object::new::<Self>();
        let imp = CodeViewPrivate::from_obj(&obj);
        imp.view_id.set(view_id);

        let cvt = CodeViewText::new(workspace.clone(), view_id);
        cvt.set_hadjust(&imp.hadj);
        cvt.set_vadjust(&imp.vadj);
        let _ = imp.cvt.set(cvt.clone());

        let gutter = Gutter::new(workspace.clone(), view_id);
        gutter.set_vadjust(&imp.vadj);
        let _ = imp.gutter.set(gutter.clone());

        // obj.set_homogeneous(true);
        let scrolled_window = gtk::ScrolledWindow::builder()
            .hadjustment(&imp.hadj)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vadjustment(&imp.vadj)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_width(360)
            .child(&cvt.clone())
            .build();

        obj.append(&gutter);
        obj.append(&scrolled_window);

        cvt.set_hscroll_policy(gtk::ScrollablePolicy::Natural);

        let _ = imp.workspace.set(workspace.clone());
        // dbg!(code_view_priv.workspace.get().is_none());

        obj.setup_widgets();
        // code_view.setup_signals();
        obj
    }

    pub fn process_event(&self, event: &Event) {
        let bid = self
            .imp()
            .workspace
            .get()
            .unwrap()
            .borrow()
            .buffer(self.imp().view_id.get())
            .borrow()
            .id;
        match event {
            Event::ScrollToCarets { buffer_id } if bid == *buffer_id => self.scroll_to_carets(),
            Event::BufferChange { buffer_id } if bid == *buffer_id => self.buffer_changed(),
            _ => {}
        }
    }

    fn bu_ls_initialized(&self) {
        debug!("ls initialized");
    }

    fn bu_path_changed(&self, buf_id: BufferId) {
        debug!("path changed");
    }

    fn bu_pristine_changed(&self, buf_id: BufferId) {
        debug!("pristine changed");
        self.emit_by_name::<()>("pristine-changed", &[]);
    }

    pub fn view_id(&self) -> usize {
        let code_view_priv = CodeViewPrivate::from_obj(&self);
        code_view_priv.view_id.get()
    }

    pub fn buffer_changed(&self) {
        let cv_priv = CodeViewPrivate::from_obj(&self);
        cv_priv.buffer_changed()
    }

    pub fn scroll_to_carets(&self) {
        let cv_priv = CodeViewPrivate::from_obj(&self);
        cv_priv.scroll_to_carets()
    }

    fn setup_widgets(&self) {}

    // fn setup_signals(&self) {}

    fn button_pressed(&self, n_pressed: i32, x: f64, y: f64) {}
}
