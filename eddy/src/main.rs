// Get rid of this when my changes to pango get in
#![allow(invalid_reference_casting)]
use components::app::AppComponent;
use eddy_model::Model;
use gflux::{ComponentHandle, ComponentTree, Obs};
use glib::{clone, ExitCode};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

mod color;
mod components;
mod objects;
mod theme;
mod widgets;

fn main() -> ExitCode {
    env_logger::init();
    gtk::init().expect("gtk init");

    // Create the global application state
    let model = Rc::new(RefCell::new(Obs::new(Model::new(Arc::new(|| {
        // This wakeup function is now handled by the observer below
    })))));

    // Create the root of the component tree
    let mut ctree = ComponentTree::new(model.clone());

    model.borrow_mut().observe(clone!(
        #[strong]
        ctree,
        #[strong]
        model,
        move |_| {
            glib::source::idle_add_local_once(clone!(
                #[strong]
                ctree,
                #[strong]
                model,
                move || {
                    println!("MODEL I JUST WOKE UP");
                    ctree.exec_rebuilds();

                    if model.borrow().get().has_events() {
                        model.borrow_mut().get_mut().handle_events();
                    }
                }
            ));
        }
    ));

    let appc: ComponentHandle<AppComponent> = ctree.new_component(|s| s, |s| s, ());

    // Run the application
    appc.widget().run()
}
