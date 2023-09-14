use components::app::AppComponent;
use eddy_workspace::Workspace;
use gflux::{ComponentHandle, ComponentTree, Obs};
use glib::{clone, ExitCode};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

mod color;
mod components;
mod theme;
mod widgets;

fn main() -> ExitCode {
    env_logger::init();
    gtk::init().expect("gtk init");

    // Create the global application state
    let workspace = Rc::new(RefCell::new(Obs::new(Workspace::new())));

    // Create the root of the component tree
    let mut ctree = ComponentTree::new(workspace.clone());

    workspace.borrow_mut().observe(clone!(@strong ctree => move |_| {
        glib::source::idle_add_local_once(clone!(@strong ctree => move || ctree.exec_rebuilds()));
    }));

    let appc: ComponentHandle<AppComponent> = ctree.new_component(|s| s, |s| s, ());

    // Run the application
    appc.widget().run()
}
