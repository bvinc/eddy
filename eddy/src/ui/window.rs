use crate::app::{EddyApplication, EddyApplicationPrivate};
use crate::ui::{CodeView, DirBar, TabLabel};
use eddy_workspace::{BufferUpdate, Event, ViewId, Workspace};
use glib::Sender;
use gtk::glib::subclass;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, Orientation};
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

struct Page {
    view_id: usize,
    pristine: bool,
}

pub struct EddyApplicationWindowPrivate {
    app: OnceCell<EddyApplication>,
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
        let window = glib::Object::new::<Self>(&[("application", &app)]);

        app.add_window(&window);
        window.setup_widgets();
        window.setup_signals();
        window
    }

    fn setup_widgets(&self) {
        let app: EddyApplication = self.application().unwrap().downcast().unwrap();
        let app_private = EddyApplicationPrivate::from_instance(&app);
        let self_ = EddyApplicationWindowPrivate::from_instance(self);

        self.set_default_size(1150, 750);
        self.set_icon_name(Some("text-x-generic"));

        let header_bar = gtk::HeaderBar::new();
        // let open_button = gtk::Button::new();
        let new_button = gtk::Button::new();
        let menu_button = gtk::MenuButton::new();
        // let save_button = gtk::Button::new();

        header_bar.pack_start(&new_button);
        header_bar.pack_end(&menu_button);

        let dir_bar = DirBar::new();
        dir_bar.init(app_private.workspace.clone());

        let sidebar_scrolled_window = gtk::ScrolledWindow::builder()
            // .hadjustment(&sidebar_hadj)
            // .hscrollbar_policy(gtk::PolicyType::Automatic)
            // .propagate_natural_width(true)
            // .hexpand(true)
            // .hexpand_set(true)
            // .min_content_width(0)
            // .vadjustment(&sidebar_vadj)
            // .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&dir_bar)
            .build();

        let sidebar_paned = gtk::Paned::new(Orientation::Horizontal);
        sidebar_paned.set_start_child(Some(&sidebar_scrolled_window));
        sidebar_paned.set_position(200);
        sidebar_paned.set_resize_start_child(false);
        sidebar_paned.set_shrink_start_child(true);

        sidebar_paned.set_end_child(Some(&self_.notebook));
        sidebar_paned.set_resize_end_child(true);
        sidebar_paned.set_shrink_end_child(false);

        self.set_child(Some(&sidebar_paned));
        self.set_titlebar(Some(&header_bar));
    }

    fn setup_signals(&self) {}

    pub fn new_view(&self, view_id: ViewId) -> Result<(), anyhow::Error> {
        let app: EddyApplication = self.application().unwrap().downcast().unwrap();
        let app_private = EddyApplicationPrivate::from_instance(&app);
        let page_num = self.imp().notebook.append_page(
            &CodeView::new(app_private.workspace.clone(), view_id),
            Some(&TabLabel::new(app_private.workspace.clone(), view_id)),
        );
        self.imp().notebook.set_page(page_num as i32);
        self.imp().pages.borrow_mut().push(Page {
            view_id,
            pristine: true,
        });
        dbg!("open");
        Ok(())
    }

    pub fn process_event(&self, event: &Event) {
        for page_num in 0..self.imp().notebook.n_pages() {
            if let Some(cv) = self.imp().notebook.nth_page(Some(page_num)) {
                if let Some(tl) = self.imp().notebook.tab_label(&cv) {
                    let tl: TabLabel = tl.downcast().unwrap();
                    tl.process_event(event);
                }
                if let Some(cv) = cv.downcast_ref::<CodeView>() {
                    cv.process_event(event);
                }
            }
        }
    }
}
