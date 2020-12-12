#![recursion_limit = "128"]

// mod clipboard;
// mod edit_view;
// mod main_view;
// mod controller;
// mod dir_bar;
// mod main_win;
mod linecache;
mod scrollable_drawing_area;
mod theme;
mod widget;

use crate::widget::dir_bar::DirBar;
use crate::widget::editview::{self, EditView};
use crate::widget::tab::{self, Tab};
use anyhow::Context;
use eddy_workspace::{Callbacks, Workspace};
use gio::prelude::*;
use gio::ApplicationExt;
use gio::{ActionMapExt, ApplicationFlags, SimpleAction};
use glib::variant::Variant;
use gtk::prelude::*;
use gtk::{
    self, Application, ApplicationWindow, ButtonsType, Dialog, DialogFlags, FileChooserAction,
    FileChooserDialog, MessageDialog, MessageType, Notebook, Paned, ResponseType,
};
use log::*;
use relm::{connect, Channel, Relm, Update, Widget};
use relm_derive::Msg;
use serde_json::{json, Value};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env::{args, home_dir};
use std::include_str;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use syntect::highlighting::ThemeSettings;

pub struct MainState {
    pub avail_langs: Vec<String>,
    pub themes: Vec<String>,
    pub theme_name: String,
    pub theme: ThemeSettings,
}

#[derive(Msg)]
pub enum Msg {
    Activate,
    AutoIndent(SimpleAction, Option<Variant>),
    Close,
    CloseAll,
    CloseView(String),
    Find,
    New,
    Open,
    OpenPath(PathBuf),
    OpenFileDialog(ResponseType),
    Prefs,
    Save,
    SaveAs,
    SaveFileDialog(ResponseType),
    Shutdown,
    Workspace(::eddy_workspace::Msg),
    Quit,
}

pub struct Model {
    relm: Relm<Win>,
    workspace: Rc<RefCell<Workspace>>,
    // model: eddy_model::Model,
    application: Application,
}

pub struct Page {
    view_id: usize,
    pristine: bool,
    tab: relm::Component<Tab>,
    page: relm::Component<EditView>,
}

pub struct Win {
    model: Model,
    app_win: ApplicationWindow,
    notebook: Notebook,
    open_dialog: FileChooserDialog,
    save_dialog: FileChooserDialog,
    pages: Vec<Page>,
    view_to_page: HashMap<String, u32>,
    relm: Relm<Self>,
    dir_bar: relm::Component<DirBar>,
    workspace_chan: Channel<::eddy_workspace::Msg>,
}

impl Update for Win {
    // Specify the model used for this widget.
    type Model = Model;
    // Specify the model parameter used to init the model.
    type ModelParam = Application;
    // Specify the type of the messages sent to the update function.
    type Msg = Msg;

    fn model(relm: &Relm<Self>, application: Application) -> Model {
        let stream = relm.stream().clone();

        let mut config_dir = None;
        let mut plugin_dir = None;
        if let Some(home_dir) = home_dir() {
            let xi_config = home_dir.join(".config").join("xi");
            let xi_plugin = xi_config.join("plugins");
            config_dir = xi_config.to_str().map(|s| s.to_string());
            plugin_dir = xi_plugin.to_str().map(|s| s.to_string());
        }

        Model {
            relm: relm.clone(),
            workspace: Rc::new(RefCell::new(Workspace::new())),
            application,
        }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::AutoIndent(_, _) => self.auto_indent(),
            Msg::Activate => {}
            Msg::Close => self.close(),
            Msg::CloseAll => self.close_all(),
            Msg::CloseView(view_id) => self.close_view(&view_id),
            Msg::Find => self.find(),
            Msg::Prefs => self.prefs(),
            Msg::New => {
                let res = self.handle_new_button();
                self.show_err(res);
            }
            Msg::Open => self.handle_open_button(),
            Msg::OpenPath(ref path) => {
                let res = self.handle_open_path(path);
                self.show_err(res);
            }
            Msg::OpenFileDialog(rt) => {
                let res = self.handle_open_file(rt);
                self.show_err(res);
            }
            Msg::Save => {
                let res = self.save();
                self.show_err(res);
            }
            Msg::SaveAs => self.save_as(),
            Msg::SaveFileDialog(rt) => {
                let res = self.handle_save_file(rt);
                self.show_err(res);
            }
            Msg::Shutdown => {}
            Msg::Workspace(msg) => self.handle_workspace_msg(msg),
            Msg::Quit => self.model.application.quit(),
        }
    }
}

impl Widget for Win {
    type Root = ApplicationWindow;

    // Return the root widget.
    fn root(&self) -> Self::Root {
        self.app_win.clone()
    }

    fn view(relm: &Relm<Self>, model: Self::Model) -> Self {
        let application = model.application.clone();

        let stream = relm.stream().clone();
        let (workspace_chan, sender) = Channel::new(move |msg| {
            stream.emit(Msg::Workspace(msg));
        });
        let sender = Arc::new(Mutex::new(sender));
        let cb = move |msg| {
            let sender = sender.lock().expect("unlock sender");
            sender.send(msg).expect("send workspace msg");
        };
        {
            let mut workspace = model.workspace.borrow_mut();
            workspace.set_callback(cb);
        }

        connect!(relm, application, connect_activate(_), Msg::Activate);
        connect!(relm, application, connect_open(_, _, _), Msg::Open);
        connect!(relm, application, connect_shutdown(_), Msg::Shutdown);

        let glade_src = include_str!("ui/eddy.glade");
        let builder = gtk::Builder::from_string(glade_src);

        let app_win: ApplicationWindow = builder.get_object("appwindow").unwrap();
        let notebook: Notebook = builder.get_object("notebook").unwrap();
        // connect!(
        //     relm,
        //     notebook,
        //     connect_page_removed(_, _, i),
        //     Msg::PageRemoved(i)
        // );
        let sidebar_paned: Paned = builder.get_object("sidebar_paned").unwrap();
        let sidebar_box: gtk::Box = builder.get_object("sidebar_box").unwrap();
        let dir_bar =
            relm::init::<DirBar>(relm.clone()).expect("failed to create dir bar component");
        dir_bar.emit(crate::widget::dir_bar::Msg::SetDir(".".into()));

        // let dir_bar_id = DirBar::new(None, controller.clone());

        sidebar_paned.set_position(200);
        sidebar_paned.set_child_resize(&sidebar_box, false);
        sidebar_paned.set_child_resize(&notebook, true);
        sidebar_box.pack_start(dir_bar.widget(), true, true, 0);

        app_win.set_application(Some(&model.application.clone()));

        // Open dialog
        trace!("creating open dialog");
        let open_dialog = FileChooserDialog::new(None, Some(&app_win), FileChooserAction::Open);
        trace!("finished creating open dialog");
        open_dialog.set_transient_for(Some(&app_win));
        open_dialog.add_button("Open", ResponseType::Ok);
        open_dialog.set_default_response(ResponseType::Ok);
        open_dialog.set_select_multiple(true);
        connect!(
            relm,
            open_dialog,
            connect_response(_, rt),
            Msg::OpenFileDialog(rt)
        );
        connect!(
            relm,
            open_dialog,
            connect_delete_event(_, _),
            return (None, Inhibit(true))
        );

        // Save dialog
        let save_dialog = FileChooserDialog::new(None, Some(&app_win), FileChooserAction::Save);
        save_dialog.set_transient_for(Some(&app_win));
        save_dialog.add_button("Save", ResponseType::Ok);
        save_dialog.set_default_response(ResponseType::Ok);
        connect!(
            relm,
            save_dialog,
            connect_response(_, rt),
            Msg::SaveFileDialog(rt)
        );
        save_dialog.connect_delete_event(|w, _| {
            w.hide();
            Inhibit(true)
        });
        // connect!(
        //     relm,
        //     save_dialog,
        //     connect_delete_event(_, _),
        //     return (None, Inhibit(false))
        // );

        // connect!(
        //     relm,
        //     app_win,
        //     connect_delete_event(_, _),
        //     return (Some(Msg::Quit), Inhibit(false))
        // );

        trace!("view2");
        {
            let open_action = SimpleAction::new("open", None);
            connect!(relm, open_action, connect_activate(_, _), Msg::Open);
            app_win.add_action(&open_action);
        }
        {
            let new_action = SimpleAction::new("new", None);
            connect!(relm, new_action, connect_activate(_, _), Msg::New);
            app_win.add_action(&new_action);
        }
        {
            let prefs_action = SimpleAction::new("prefs", None);
            connect!(relm, prefs_action, connect_activate(_, _), Msg::Prefs);
            app_win.add_action(&prefs_action);
        }
        {
            let find_action = SimpleAction::new("find", None);
            connect!(relm, find_action, connect_activate(_, _), Msg::Find);
            app_win.add_action(&find_action);
        }
        {
            let save_action = SimpleAction::new("save", None);
            connect!(relm, save_action, connect_activate(_, _), Msg::Save);
            app_win.add_action(&save_action);
        }
        {
            let save_as_action = SimpleAction::new("save_as", None);
            connect!(relm, save_as_action, connect_activate(_, _), Msg::SaveAs);
            app_win.add_action(&save_as_action);
        }
        {
            let close_action = SimpleAction::new("close", None);
            connect!(relm, close_action, connect_activate(_, _), Msg::Close);
            app_win.add_action(&close_action);
        }
        {
            let close_all_action = SimpleAction::new("close_all", None);
            connect!(
                relm,
                close_all_action,
                connect_activate(_, _),
                Msg::CloseAll
            );
            app_win.add_action(&close_all_action);
        }
        trace!("view3");
        {
            let quit_action = SimpleAction::new("quit", None);
            connect!(relm, quit_action, connect_activate(_, _), Msg::Quit);
            app_win.add_action(&quit_action);
        }
        {
            let auto_indent_action =
                SimpleAction::new_stateful("auto_indent", None, &false.to_variant());
            connect!(
                relm,
                auto_indent_action,
                connect_change_state(action, value),
                Msg::AutoIndent(action.clone(), value.map(|v| v.clone()))
            );

            // auto_indent_action.connect_change_state(move |action, value| {
            //     if value.is_none() {
            //         return;
            //     }
            //     if let Some(value) = value.as_ref() {
            //         action.set_state(value);
            //         let value: bool = value.get().unwrap();
            //         debug!("auto indent {}", value);
            //         controller.borrow().set_auto_indent(value)
            //     }
            // });
            app_win.add_action(&auto_indent_action);
        }

        trace!("view4");
        connect!(
            relm,
            app_win,
            connect_delete_event(_, _),
            return (Some(Msg::Quit), Inhibit(false))
        );

        trace!("view5");

        app_win.show_all();

        trace!("view-last");
        Win {
            model,
            app_win,
            notebook,
            open_dialog,
            save_dialog,
            pages: vec![],
            view_to_page: HashMap::new(),
            relm: relm.clone(),
            dir_bar,
            workspace_chan,
        }
    }
}

impl Win {
    pub fn show_err(&mut self, res: Result<(), anyhow::Error>) {
        if let Err(e) = res {
            let dialog = MessageDialog::new(
                Some(&self.app_win),
                DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
                MessageType::Error,
                ButtonsType::Ok,
                &format!("{}", e),
            );
            dialog.connect_response(|w, _| w.hide());
            dialog.show();
        }
    }

    pub fn handle_new_button(&mut self) -> Result<(), anyhow::Error> {
        trace!("handle new button");
        let view_id = self.model.workspace.borrow_mut().new_view(None)?;
        self.new_view_response(view_id, None);
        Ok(())
    }

    pub fn handle_open_button(&mut self) {
        self.open_dialog.show_all();
    }

    pub fn auto_indent(&mut self) {
        debug!("auto_indent");
    }
    pub fn close(&mut self) {
        debug!("close")
    }
    pub fn close_all(&mut self) {
        debug!("close all")
    }
    pub fn prefs(&mut self) {
        debug!("prefs")
    }
    pub fn find(&mut self) {
        debug!("find")
    }
    pub fn save(&mut self) -> Result<(), anyhow::Error> {
        debug!("save");

        if let Some(idx) = self.notebook.get_current_page() {
            let view_id = self.pages[idx as usize].view_id;
            self.model.workspace.borrow_mut().save(view_id)?;
        }

        Ok(())
    }

    pub fn save_as(&mut self) {
        debug!("save_as");
        self.save_dialog.show_all();
    }

    pub fn handle_open_path(&mut self, path: &Path) -> Result<(), anyhow::Error> {
        debug!("handle open path {:?}", path);
        let view_id = self.model.workspace.borrow_mut().new_view(Some(&path))?;
        self.new_view_response(view_id, Some(path));
        Ok(())
    }

    // This is called in response to the FileChooserDialog
    pub fn handle_open_file(&mut self, rt: ResponseType) -> Result<(), anyhow::Error> {
        debug!("handle open file {:?}", rt);
        self.open_dialog.hide();
        if rt != ResponseType::Ok {
            return Ok(());
        }
        if let Some(filename) = self.open_dialog.get_filename() {
            let file_name = PathBuf::from(filename.to_string_lossy().to_string());
            let view_id = self
                .model
                .workspace
                .borrow_mut()
                .new_view(Some(&file_name))?;
            self.new_view_response(view_id, Some(&file_name));
        }
        Ok(())
    }

    // This is called in response to the FileChooserDialog
    pub fn handle_save_file(&mut self, rt: ResponseType) -> Result<(), anyhow::Error> {
        debug!("handle save file {:?}", rt);
        self.save_dialog.hide();
        if rt != ResponseType::Ok {
            return Ok(());
        }
        if let Some(filename) = self.save_dialog.get_filename() {
            if let Some(idx) = self.notebook.get_current_page() {
                let view_id = self.pages[idx as usize].view_id;
                self.model.workspace.borrow_mut().save_as(
                    view_id,
                    &PathBuf::from(&*filename.to_string_lossy().to_owned()),
                )?;
            }
        }
        Ok(())
    }

    pub fn handle_workspace_msg(&mut self, msg: eddy_workspace::Msg) {
        debug!("handle workspace msg {:?}", msg);
        self.model.workspace.borrow_mut().ls_initialized();
    }

    fn new_view_response(&mut self, view_id: usize, file_name: Option<&Path>) {
        let tab_comp = relm::init::<Tab>((
            self.relm.clone(),
            view_id,
            file_name.map(|p| p.to_path_buf()),
        ))
        .expect("failed to create tab component");
        let page_comp = relm::init::<EditView>((
            view_id,
            file_name.map(|p| p.to_path_buf()),
            true,
            self.relm.stream().clone(),
            self.model.workspace.clone(),
        ))
        .expect("failed to create page component");
        let page_num =
            self.notebook
                .insert_page(&*page_comp.widget(), Some(&*tab_comp.widget()), None);
        self.pages.push(Page {
            view_id,
            pristine: true,
            tab: tab_comp,
            page: page_comp,
        });
        self.view_to_page.insert(view_id.to_string(), page_num);
    }

    fn close_view(&mut self, view_id: &str) {
        debug!("close view");
        if let Some(page_num) = self.view_to_page.remove(view_id) {
            self.notebook.remove_page(Some(page_num));
            self.pages.remove(page_num as usize);
            // Adjust the page numbers accordingly
            for pn in self.view_to_page.values_mut() {
                if *pn > page_num {
                    *pn -= 1;
                }
            }
        }
    }
}

fn main() {
    env_logger::init();
    gtk::init().expect("gtk init");

    // let model = eddy_model::Model::new();
    // model.init();
    // let shared_queue = model.shared_queue();
    // let reader_raw_fd = {
    //     let sq = shared_queue.lock().expect("failed to lock shared queue");
    //     sq.reader_raw_fd()
    // };
    // let main_win = Mutex::new(None);

    // CONTROLLER.set(Controller::new(model.clone()));
    // let cont = Rc::new(RefCell::new(Controller::new(model.clone())));
    let application = Application::new(
        Some("com.github.bvinc.eddy"),
        ApplicationFlags::HANDLES_OPEN,
    )
    .expect("failed to create gtk application");

    let win = Rc::new(RefCell::new(None));

    application.connect_startup(move |application| {
        // let model = eddy_model::Model::new();
        // model.init();
        // let shared_queue = model.shared_queue();
        // let reader_raw_fd = {
        //     let sq = shared_queue.lock().expect("failed to lock shared queue");
        //     sq.reader_raw_fd()
        // };

        // // Implement a GSource based on our signaling pipe's reader FD
        // let source = new_source(QueueSource {
        //     queue: shared_queue.clone(),
        // });
        // unsafe {
        //     use glib::translate::ToGlibPtr;
        //     ::glib_sys::g_source_add_unix_fd(
        //         source.to_glib_none().0,
        //         reader_raw_fd,
        //         ::glib_sys::G_IO_IN,
        //     );
        // }

        // model.client_started();

        // // Attach it to the main context
        // let main_context = MainContext::default();
        // source.attach(Some(&main_context));

        {
            let mut w = win.borrow_mut();
            *w = Some(relm::init::<Win>(application.clone()));
        }
    });

    application.run(&args().collect::<Vec<_>>());
}
