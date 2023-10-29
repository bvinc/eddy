use super::window::WindowComponent;
use eddy_model::Model;
use gflux::{Component, ComponentCtx, ComponentHandle};
use glib::clone;
use gtk::prelude::*;
use std::collections::{HashMap, HashSet};

#[allow(dead_code)]
pub struct AppComponent {
    app: gtk::Application,
    // win_components: Rc<RefCell<Vec<ComponentHandle<WindowComponent>>>>,
    wins: HashMap<u64, ComponentHandle<WindowComponent>>,
    last_wins: HashSet<u64>,
}

impl Component for AppComponent {
    type GlobalModel = Model;
    type Model = Model;
    type Widget = gtk::Application;
    type Params = ();

    fn widget(&self) -> Self::Widget {
        self.app.clone()
    }

    fn build(ctx: ComponentCtx<Self>, _params: ()) -> Self {
        let app = gtk::Application::builder()
            .application_id("com.github.bvinc.eddy")
            .build();

        // let win_components = Rc::new(RefCell::new(vec![]));

        // app.connect_activate(clone!(@strong win_components => move |app| {
        //     let c: ComponentHandle<WindowComponent> =
        //         ctx.create_child(|s: &Model| s, |s: &mut Model| s, app.clone());

        //     c.widget().present();

        //     win_components.borrow_mut().push(c);
        // }));

        app.connect_activate(clone!(@strong ctx => move |_app| {
            ctx.with_model_mut(|m| m.new_win());
            ctx.rebuild();
        }));

        Self {
            app,
            wins: HashMap::new(),
            last_wins: HashSet::new(),
        }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        let wins: HashSet<u64> = ctx.with_model(|m| m.wins.keys().copied().collect());
        let last_wins: HashSet<u64> = self.last_wins.clone();

        // Remove old windows
        for win_id in last_wins.difference(&wins) {
            self.wins.remove(win_id);
        }

        // Add new windows
        for win_id in wins.difference(&last_wins).copied() {
            let c: ComponentHandle<WindowComponent> = ctx.create_child(
                move |m: &Model| m.wins.get(&win_id).unwrap(),
                move |m: &mut Model| m.wins.get_mut(&win_id).unwrap(),
                self.app.clone(),
            );

            c.widget().present();

            self.wins.insert(win_id, c);
        }

        self.last_wins = wins;

        ctx.rebuild_children();
    }
}
