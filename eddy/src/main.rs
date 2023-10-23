// Get rid of this when my changes to pango get in
#![allow(invalid_reference_casting)]
use components::app::AppComponent;
use eddy_workspace::Workspace;
use gflux::{ComponentHandle, ComponentTree, Obs};
use glib::{clone, ControlFlow, ExitCode, MainContext, Priority};
use gtk::prelude::*;
use std::cell::RefCell;
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;

mod color;
mod components;
mod theme;
mod widgets;

fn main() -> ExitCode {
    env_logger::init();
    gtk::init().expect("gtk init");

    let main_context = MainContext::default();
    let (event_sender, event_receiver) = MainContext::channel::<()>(Priority::DEFAULT);
    let (model_update_sender, model_update_receiver) =
        MainContext::channel::<()>(Priority::DEFAULT);
    // let wakeup = clone!(@strong sender => move || sender.send(()).expect("failure to wake up main context"););

    // Create the global application state
    let workspace = Rc::new(RefCell::new(Obs::new(Workspace::new(Arc::new(
        // wakeup.clone(),
        move || {
            event_sender
                .send(())
                .expect("failed to notify main thread of events")
        },
    )))));

    // Create the root of the component tree
    let mut ctree = ComponentTree::new(workspace.clone());

    // workspace.borrow_mut().observe(clone!(@strong ctree => move |_| {
    //     MainContext::default().wakeup();
    //     ctree.exec_rebuilds()
    // }));

    workspace
        .borrow_mut()
        .observe(clone!(@strong ctree => move |_| {
            // wakeup()
            model_update_sender.send(()).expect("failure to wake up main context");
            // glib::source::idle_add_local_once(clone!(@strong ctree => move || ctree.exec_rebuilds()));
        }));

    let appc: ComponentHandle<AppComponent> = ctree.new_component(|s| s, |s| s, ());

    event_receiver.attach(
        None,
        clone!(@strong ctree, @strong workspace => move |_| {
            println!("EVENT I JUST WOKE UP");
            workspace.borrow_mut().get_mut().handle_events();
            ControlFlow::Continue
        }),
    );

    model_update_receiver.attach(
        None,
        clone!(@strong ctree, @strong workspace => move |_| {
            println!("MODEL I JUST WOKE UP");
            ctree.exec_rebuilds();
            ControlFlow::Continue
        }),
    );

    // Run the application
    appc.widget().run()
}
