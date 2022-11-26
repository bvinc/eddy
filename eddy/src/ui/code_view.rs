use super::{CodeViewText, Gutter};
use crate::app::Event;
use crate::theme::Theme;
use eddy_workspace::style::{Attr, AttrSpan};
use eddy_workspace::{ViewId, Workspace};
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
    sender: OnceCell<Sender<Event>>,
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
        let sender = OnceCell::new();
        let workspace = OnceCell::new();
        let cvt = OnceCell::new();
        let gutter = OnceCell::new();
        let view_id = Cell::new(0);
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
    pub fn new(workspace: Rc<RefCell<Workspace>>, sender: Sender<Event>, view_id: ViewId) -> Self {
        let obj = glib::Object::new::<Self>(&[]);
        let imp = CodeViewPrivate::from_instance(&obj);
        imp.view_id.set(view_id);

        let cvt = CodeViewText::new(workspace.clone(), sender.clone(), view_id);
        cvt.set_hadjust(&imp.hadj);
        cvt.set_vadjust(&imp.vadj);
        let _ = imp.cvt.set(cvt.clone());

        let gutter = Gutter::new(workspace.clone(), sender.clone(), view_id);
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

        let _ = imp.sender.set(sender.clone());

        let _ = imp.workspace.set(workspace.clone());
        // dbg!(code_view_priv.workspace.get().is_none());

        // Subscribe to buffer change events.  Add a callback to queue drawing
        // on our drawing areas.
        {
            let workspace = imp.workspace.get().unwrap().borrow_mut();
            let sender2 = imp.sender.get().unwrap().clone();
            let buffer = workspace.buffer(imp.view_id.get());
            let view_id = imp.view_id.get();
            buffer.borrow_mut().connect_update(move || {
                if let Err(err) = sender2.send(Event::BufferChange { view_id }) {
                    error!("buffer changed: {}", err);
                };
            });

            let sender3 = imp.sender.get().unwrap().clone();
            buffer
                .borrow_mut()
                .connect_scroll_to_selections(view_id, move || {
                    if let Err(err) = sender3.send(Event::ScrollToCarets { view_id }) {
                        error!("scroll to selections: {}", err);
                    };
                });
        }

        obj.setup_widgets();
        // code_view.setup_signals();
        obj
    }

    pub fn view_id(&self) -> usize {
        let code_view_priv = CodeViewPrivate::from_instance(&self);
        code_view_priv.view_id.get()
    }

    pub fn buffer_changed(&self) {
        let cv_priv = CodeViewPrivate::from_instance(&self);
        cv_priv.buffer_changed()
    }

    pub fn scroll_to_carets(&self) {
        let cv_priv = CodeViewPrivate::from_instance(&self);
        cv_priv.scroll_to_carets()
    }

    fn setup_widgets(&self) {}

    // fn setup_signals(&self) {}

    fn button_pressed(&self, n_pressed: i32, x: f64, y: f64) {}
}
