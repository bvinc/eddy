use glib::Sender;
use gtk::glib;
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::Orientation;
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::app::{Action, EddyApplication, EddyApplicationPrivate};
use crate::ui::{CodeView, DirBar};
use eddy_workspace::{ViewId, Workspace};

struct Page {
    view_id: usize,
    pristine: bool,
}

pub struct EddyApplicationWindowPrivate {
    app: OnceCell<EddyApplication>,
    sender: OnceCell<Sender<Action>>,
    notebook: gtk::Notebook,
    pages: RefCell<Vec<Page>>,
}

#[glib::object_subclass]
impl ObjectSubclass for EddyApplicationWindowPrivate {
    const NAME: &'static str = "EddyApplicationWindow";
    type Type = EddyApplicationWindow;
    type ParentType = gtk::ApplicationWindow;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    fn new() -> Self {
        Self {
            app: OnceCell::new(),
            sender: OnceCell::new(),
            notebook: gtk::Notebook::new(),
            pages: RefCell::new(vec![]),
        }
    }
}

impl ObjectImpl for EddyApplicationWindowPrivate {}
impl WidgetImpl for EddyApplicationWindowPrivate {}
impl WindowImpl for EddyApplicationWindowPrivate {}
impl gtk::subclass::prelude::ApplicationWindowImpl for EddyApplicationWindowPrivate {}

glib::wrapper! {
    pub struct EddyApplicationWindow(ObjectSubclass<EddyApplicationWindowPrivate>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow;
}

impl EddyApplicationWindow {
    pub fn new(app: EddyApplication) -> Self {
        let window = glib::Object::new::<Self>(&[("application", &app)]).unwrap();

        app.add_window(&window);
        window.setup_widgets();
        window.setup_signals();
        window
    }

    fn setup_widgets(&self) {
        let app: EddyApplication = self.application().unwrap().downcast().unwrap();
        let app_private = EddyApplicationPrivate::from_instance(&app);
        let self_ = EddyApplicationWindowPrivate::from_instance(self);
        let _ = self_.sender.set(app_private.sender.clone());

        self.set_default_size(1150, 750);
        self.set_icon_name(Some("text-x-generic"));

        let header_bar = gtk::HeaderBar::new();
        // let open_button = gtk::Button::new();
        let new_button = gtk::Button::new();
        let menu_button = gtk::MenuButton::new();
        // let save_button = gtk::Button::new();

        header_bar.pack_start(&new_button);
        header_bar.pack_end(&menu_button);
        // add to header_bar

        let sidebar_paned = gtk::Paned::new(Orientation::Horizontal);
        let dir_bar = DirBar::new();
        dir_bar.init(app_private.sender.clone());
        sidebar_paned.set_start_child(&dir_bar);
        sidebar_paned.set_resize_start_child(false);
        sidebar_paned.set_shrink_start_child(true);

        sidebar_paned.set_end_child(&self_.notebook);
        sidebar_paned.set_resize_end_child(true);
        sidebar_paned.set_shrink_end_child(false);

        self.set_child(Some(&sidebar_paned));
        self.set_titlebar(Some(&header_bar));
    }

    fn setup_signals(&self) {}

    pub fn new_view(&self, view_id: ViewId, path: Option<&Path>) -> Result<(), anyhow::Error> {
        let self_ = EddyApplicationWindowPrivate::from_instance(self);
        let app: EddyApplication = self.application().unwrap().downcast().unwrap();
        let app_private = EddyApplicationPrivate::from_instance(&app);
        let page_num = self_.notebook.append_page(
            &CodeView::new(app_private.workspace.clone(), app_private.sender.clone()),
            None::<&gtk::Widget>,
        );
        self_.pages.borrow_mut().push(Page {
            view_id,
            pristine: true,
        });
        dbg!("open");
        Ok(())
    }

    pub fn buffer_changed(&self, view_id: ViewId) {
        let self_ = EddyApplicationWindowPrivate::from_instance(self);

        // let pages = self_.pages.borrow();
        // for page in pages {

        // }

        for page_num in 0..self_.notebook.n_pages() {
            if let Some(cv) = self_.notebook.nth_page(Some(page_num)) {
                if let Some(cv) = cv.downcast_ref::<CodeView>() {
                    let cv_view_id = cv.view_id();
                    if view_id == cv_view_id {
                        cv.buffer_changed();
                    }
                }
            }
        }
    }
}