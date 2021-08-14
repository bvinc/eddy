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

use super::{CodeViewText, Gutter};
use crate::app::Action;
use crate::theme::Theme;

pub struct CodeViewPrivate {
    cvt: CodeViewText,
    gutter: Gutter,
    hadj: Adjustment,
    vadj: Adjustment,
    sender: OnceCell<Sender<Action>>,
    workspace: OnceCell<Rc<RefCell<Workspace>>>,
    view_id: usize,
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
        let sender = OnceCell::new();
        let workspace = OnceCell::new();
        let cvt = CodeViewText::new();
        let gutter = Gutter::new();
        let view_id = 0;
        let theme = Theme::default();

        let hadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let vadj = Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        Self {
            sender,
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
    fn constructed(&self, obj: &Self::Type) {
        dbg!("cv constructed");
        self.parent_constructed(obj);

        self.cvt.set_hadjust(&self.hadj);
        self.cvt.set_vadjust(&self.vadj);

        // obj.set_homogeneous(true);
        let scrolled_window = gtk::ScrolledWindow::builder()
            .hadjustment(&self.hadj)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vadjustment(&self.vadj)
            .vscrollbar_policy(gtk::PolicyType::Always)
            .min_content_width(360)
            .child(&self.cvt)
            .build();

        obj.append(&self.gutter);
        obj.append(&scrolled_window);

        self.cvt.set_hscroll_policy(gtk::ScrollablePolicy::Natural);

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
        self.cvt.buffer_changed();
        self.gutter.buffer_changed();
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
    pub fn new(sender: Sender<Action>, workspace: Rc<RefCell<Workspace>>) -> Self {
        let code_view = glib::Object::new::<Self>(&[]).unwrap();
        let code_view_priv = CodeViewPrivate::from_instance(&code_view);

        let _ = code_view_priv.sender.set(sender.clone());
        code_view_priv.cvt.set_sender(sender.clone());
        code_view_priv.gutter.set_sender(sender.clone());

        let _ = code_view_priv.workspace.set(workspace.clone());
        dbg!(code_view_priv.workspace.get().is_none());
        code_view_priv.cvt.set_workspace(workspace.clone());
        code_view_priv.gutter.set_workspace(workspace.clone());

        // Subscribe to buffer change events.  Add a callback to queue drawing
        // on our drawing areas.
        {
            let mut workspace = code_view_priv.workspace.get().unwrap().borrow_mut();
            let sender2 = code_view_priv.sender.get().unwrap().clone();
            let buffer = workspace.buffer(code_view_priv.view_id);
            let view_id = code_view_priv.view_id;
            buffer.connect_update(move || {
                if let Err(err) = sender2.send(Action::BufferChange { view_id }) {
                    error!("buffer changed: {}", err);
                };
            });
        }

        code_view.setup_widgets();
        // code_view.setup_signals();
        code_view
    }

    pub fn view_id(&self) -> usize {
        let code_view_priv = CodeViewPrivate::from_instance(&self);
        code_view_priv.view_id
    }

    pub fn buffer_changed(&self) {
        let cv_priv = CodeViewPrivate::from_instance(&self);
        cv_priv.buffer_changed()
    }

    fn setup_widgets(&self) {}

    // fn setup_signals(&self) {}

    fn button_pressed(&self, n_pressed: i32, x: f64, y: f64) {}
}
