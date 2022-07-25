use anyhow::*;
use cairo::glib::translate::FromGlib;
use eddy_workspace::Workspace;
use gio::ApplicationFlags;
use glib::{subclass, WeakRef};
use glib::{Receiver, Sender};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::application::GtkApplicationImpl;
use gtk::subclass::prelude::ApplicationImpl;
use gtk::subclass::prelude::*;
use gtk::{ButtonsType, DialogFlags, MessageDialog, MessageType};
use log::*;
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use crate::ui::EddyApplicationWindow;

#[derive(Clone, Debug)]
pub enum Action {
    Open(PathBuf),
    BufferChange { view_id: usize },
    ScrollToCarets { view_id: usize },
}

pub struct EddyApplicationPrivate {
    pub sender: Sender<Action>,
    receiver: RefCell<Option<Receiver<Action>>>,
    pub workspace: Rc<RefCell<Workspace>>,
    window: OnceCell<WeakRef<EddyApplicationWindow>>,
}

#[glib::object_subclass]
impl ObjectSubclass for EddyApplicationPrivate {
    const NAME: &'static str = "EddyApplication";
    type Type = EddyApplication;
    type ParentType = gtk::Application;
    type Instance = subclass::basic::InstanceStruct<Self>;
    type Class = subclass::basic::ClassStruct<Self>;

    fn new() -> Self {
        let (sender, r) = glib::MainContext::channel(unsafe { glib::Priority::from_glib(200) });
        let receiver = RefCell::new(Some(r));
        let workspace = Rc::new(RefCell::new(Workspace::new()));
        let window = OnceCell::new();
        Self {
            sender,
            receiver,
            workspace,
            window,
        }
    }
}

impl ObjectImpl for EddyApplicationPrivate {}
impl GtkApplicationImpl for EddyApplicationPrivate {}
impl ApplicationImpl for EddyApplicationPrivate {
    fn activate(&self, _: &Self::Type) {
        debug!("activate");

        let app = self.instance().downcast::<EddyApplication>().unwrap();

        debug!("setup");
        app.setup();

        let window = app.create_window();
        window.present();
        self.window.set(window.downgrade()).unwrap();
        info!("created window");

        let receiver = self.receiver.borrow_mut().take().unwrap();
        receiver.attach(None, move |action| app.process_action(action));
    }
}

glib::wrapper! {
    pub struct EddyApplication(ObjectSubclass<EddyApplicationPrivate>)
        @extends gio::Application, gtk::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl EddyApplication {
    pub fn run() {
        info!("run");

        let app = glib::Object::new::<Self>(&[
            ("application-id", &Some("com.github.bvinc.eddy")),
            ("flags", &ApplicationFlags::empty()),
        ])
        .unwrap();

        let args: Vec<String> = env::args().collect();
        app.run_with_args(&args);
    }

    fn setup(&self) {}

    fn create_window(&self) -> EddyApplicationWindow {
        let window = EddyApplicationWindow::new(self.clone());

        window
    }

    fn get_main_window(&self) -> EddyApplicationWindow {
        let self_ = EddyApplicationPrivate::from_instance(self);
        self_.window.get().unwrap().clone().upgrade().unwrap()
    }
    /*
        pub fn workspace(&self) -> &mut Workspace {
            let self_ = EddyApplicationPrivate::from_instance(self);
            self_.workspace.get().unwrap().borrow_mut().deref_mut()
        }
    */
    fn process_action(&self, action: Action) -> glib::Continue {
        debug!("{:?}", &action);
        match action {
            Action::Open(pb) => self.show_err(self.action_open(&pb)),
            Action::BufferChange { view_id } => self.action_buffer_change(view_id),
            Action::ScrollToCarets { view_id } => self.action_scroll_to_carets(view_id),
        }
        glib::Continue(true)
    }

    fn action_open(&self, path: &Path) -> Result<(), anyhow::Error> {
        if !path.is_absolute() {
            bail!("path is not absolute");
        }
        let self_ = EddyApplicationPrivate::from_instance(self);
        let view_id = self_.workspace.borrow_mut().new_view(Some(path))?;
        let window = self.get_main_window();
        window.new_view(view_id, Some(path)).context("new_view")?;
        Ok(())
    }

    fn action_buffer_change(&self, view_id: usize) {
        let window = self.get_main_window();
        window.buffer_changed(view_id);
    }

    fn action_scroll_to_carets(&self, view_id: usize) {
        let window = self.get_main_window();
        window.scroll_to_carets(view_id);
    }

    fn show_err(&self, res: Result<(), anyhow::Error>) {
        if let Err(e) = res {
            dbg!(&e);
            let dialog = MessageDialog::new(
                Some(&self.get_main_window()),
                DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
                MessageType::Error,
                ButtonsType::Ok,
                &format!("{}", e),
            );
            dialog.connect_response(|w, _| w.hide());
            dialog.show();
        }
    }
}
