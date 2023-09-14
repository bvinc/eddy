use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use components::app::AppComponent;
use glib::clone;
use gtk::prelude::*;

use gflux::*;

pub mod components;

#[derive(Clone, Debug, Default)]
pub struct AppState {
    tasks: Tasks,
}

#[derive(Clone, Debug, Default)]
pub struct Tasks {
    map: BTreeMap<u64, Task>,
}

impl Tasks {
    fn add_task(&mut self, name: &str) {
        let id = self.map.keys().last().copied().unwrap_or_default() + 1;
        let task = Task {
            id,
            name: name.to_string(),
            done: false,
        };
        self.map.insert(id, task);
    }

    fn remove_task(&mut self, id: u64) {
        self.map.remove(&id);
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    id: u64,
    name: String,
    done: bool,
}

fn main() -> glib::ExitCode {
    let mut tasks = Tasks::default();
    tasks.add_task("Take out the trash");
    tasks.add_task("Wash the dishes");

    // Create the global application state
    let global = Rc::new(RefCell::new(Obs::new(AppState { tasks })));

    // Create the root of the component tree
    let mut ctree = ComponentTree::new(global.clone());

    global.borrow_mut().observe(clone!(@strong ctree => move |_| {
        glib::source::idle_add_local_once(clone!(@strong ctree => move || ctree.exec_rebuilds()));
    }));

    let appc: ComponentHandle<AppComponent> = ctree.new_component(|s| s, |s| s, ());

    // Run the application
    appc.widget().run()
}
