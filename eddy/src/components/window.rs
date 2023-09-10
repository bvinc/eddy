use std::collections::HashSet;

use eddy_workspace::{ViewId, Workspace};
use gflux::{Component, ComponentCtx, ComponentHandle};
use gtk::{prelude::*, Orientation};

use crate::components::tab_label::TabLabelComponent;

use super::code_view::CodeViewComponent;
use super::dirbar::DirBarComponent;

#[allow(dead_code)]
pub struct WindowComponent {
    window: gtk::ApplicationWindow,
    dir_bar: ComponentHandle<DirBarComponent>,
    code_views: Vec<ComponentHandle<CodeViewComponent>>,
    tab_labels: Vec<ComponentHandle<TabLabelComponent>>,
    notebook: gtk::Notebook,
    last_views: HashSet<ViewId>,
}

impl Component for WindowComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = gtk::ApplicationWindow;
    type Params = gtk::Application;

    fn widget(&self) -> Self::Widget {
        self.window.clone()
    }

    fn build(ctx: ComponentCtx<Self>, app: gtk::Application) -> Self {
        let header_bar = gtk::HeaderBar::new();
        let new_button = gtk::Button::new();
        let menu_button = gtk::MenuButton::new();

        header_bar.pack_start(&new_button);
        header_bar.pack_end(&menu_button);

        let dir_bar = ctx.create_child(|s| s, ());
        let sidebar_scrolled_window = gtk::ScrolledWindow::builder()
            .child(&dir_bar.widget())
            .build();

        let notebook = gtk::Notebook::new();
        let code_views = Vec::new();
        let tab_labels = Vec::new();

        let sidebar_paned = gtk::Paned::new(Orientation::Horizontal);
        sidebar_paned.set_start_child(Some(&sidebar_scrolled_window));
        sidebar_paned.set_position(200);
        sidebar_paned.set_resize_start_child(false);
        sidebar_paned.set_shrink_start_child(true);

        sidebar_paned.set_end_child(Some(&notebook));
        sidebar_paned.set_resize_end_child(true);
        sidebar_paned.set_shrink_end_child(false);

        // Create a window and set the title
        let window = gtk::ApplicationWindow::builder()
            .application(&app)
            .width_request(1150)
            .height_request(750)
            .icon_name("text-x-generic")
            .title("Eddy")
            .build();

        window.set_child(Some(&sidebar_paned));
        window.set_titlebar(Some(&header_bar));

        // Present window
        window.present();

        Self {
            window,
            dir_bar,
            code_views,
            tab_labels,
            notebook,
            last_views: HashSet::new(),
        }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        println!("window rebuild");
        let views: HashSet<ViewId> = ctx.with_model(|ws| ws.views.keys().copied().collect());
        let last_views: HashSet<ViewId> = self.last_views.clone();

        for view_id in views.difference(&last_views).copied() {
            let cv_comp: ComponentHandle<CodeViewComponent> = ctx.create_child(|ws| ws, view_id);
            let tl_comp: ComponentHandle<TabLabelComponent> = ctx.create_child(|ws| ws, view_id);
            let page_num = self
                .notebook
                .append_page(&cv_comp.widget(), Some(&tl_comp.widget()));
            self.notebook.set_page(page_num as i32);
            self.last_views.insert(view_id);

            self.code_views.push(cv_comp);
            self.tab_labels.push(tl_comp);
        }

        for tab_label in &self.tab_labels {
            tab_label.rebuild();
        }
        dbg!("open");
    }
}
