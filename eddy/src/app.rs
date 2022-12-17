use crate::ui::EddyApplicationWindow;
use anyhow::*;
use cairo::glib::translate::FromGlib;
use eddy_workspace::{BufferId, BufferUpdate, Event, ViewId, Workspace};
use gio::ApplicationFlags;
use glib::subclass::Signal;
use glib::{subclass, Receiver, Sender, WeakRef};
use gtk::prelude::*;
use gtk::subclass::application::GtkApplicationImpl;
use gtk::subclass::prelude::{ApplicationImpl, *};
use gtk::{glib, ButtonsType, DialogFlags, MessageDialog, MessageType};
use log::*;
use once_cell::sync::Lazy;
use once_cell::unsync::OnceCell;
use std::cell::RefCell;
use std::env;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct EddyApplicationPrivate {
    pub sender: Sender<Event>,
    receiver: RefCell<Option<Receiver<Event>>>,
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
        let (sender, r) = glib::MainContext::channel(glib::PRIORITY_DEFAULT_IDLE);
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

impl ObjectImpl for EddyApplicationPrivate {
    fn signals() -> &'static [Signal] {
        static SIGNALS: Lazy<Vec<Signal>> =
            Lazy::new(|| vec![Signal::builder("pristine-changed").build()]);
        SIGNALS.as_ref()
    }
}
impl GtkApplicationImpl for EddyApplicationPrivate {}
impl ApplicationImpl for EddyApplicationPrivate {
    fn activate(&self) {
        debug!("activate");

        let app = self.obj().clone();

        debug!("setup");
        app.setup();

        let window = app.create_window();
        window.present();
        self.window.set(window.downgrade()).unwrap();
        info!("created window");

        let receiver = self.receiver.borrow_mut().take().unwrap();
        receiver.attach(None, move |event| app.process_event(&event));

        let sender = self.sender.clone();
        self.workspace
            .borrow_mut()
            .set_event_callback(move |event| {
                sender.send(event).expect("send error");
            });
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
        ]);

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
    fn process_event(&self, event: &Event) -> glib::Continue {
        debug!("{:?}", &event);
        match event {
            Event::NewView { view_id } => self.show_err(self.event_new_view(*view_id)),
            e => self.get_main_window().process_event(e),
        }
        glib::Continue(true)
    }

    fn event_new_view(&self, view_id: ViewId) -> Result<(), anyhow::Error> {
        // if !path.is_absolute() {
        //     bail!("path is not absolute");
        // }
        let self_ = EddyApplicationPrivate::from_instance(self);
        let window = self.get_main_window();
        window.new_view(view_id).context("new_view")?;
        Ok(())
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

impl EddyApplicationPrivate {}
