use components::app::AppComponent;
use eddy_workspace::Workspace;
use gflux::{ComponentHandle, ComponentTree};
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
    let workspace = Rc::new(RefCell::new(Workspace::new()));

    // Create the root of the component tree
    let mut ctree = ComponentTree::new(workspace);

    // When the tree first moves from clean to dirty, use `idle_add_local_once`
    // to make sure that `ctree.exec_rebuilds()` later gets called from the gtk
    // main loop
    ctree.on_first_change(clone!(@strong ctree => move || {
        glib::source::idle_add_local_once(clone!(@strong ctree => move || ctree.exec_rebuilds()));
    }));

    let appc: ComponentHandle<AppComponent> = ctree.new_component(|s| s, ());

    // Run the application
    appc.widget().run()
}
