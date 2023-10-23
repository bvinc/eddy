use std::collections::{HashMap, HashSet};

use eddy_model::{ViewId, Workspace};
use gflux::{Component, ComponentCtx, ComponentHandle};
use gio::SimpleAction;
use glib::clone;
use gtk::ffi::GtkFileChooserDialog;
use gtk::{
    prelude::*, ApplicationWindow, ButtonsType, FileChooserDialog, MessageDialog, MessageType,
    Orientation, ResponseType,
};

use crate::components::tab_label::TabLabelComponent;

use super::code_view::CodeViewComponent;
use super::dirbar::DirBarComponent;

#[allow(dead_code)]
pub struct WindowComponent {
    window: gtk::ApplicationWindow,
    action_new: SimpleAction,
    action_close: SimpleAction,
    action_save: SimpleAction,
    action_save_as: SimpleAction,

    dir_bar: ComponentHandle<DirBarComponent>,
    code_views: HashMap<ViewId, ComponentHandle<CodeViewComponent>>,
    tab_labels: HashMap<ViewId, ComponentHandle<TabLabelComponent>>,
    notebook: gtk::Notebook,
    last_views: HashSet<ViewId>,
}

impl WindowComponent {
    fn build_popover_menu() -> gtk::PopoverMenu {
        let menu_model = gio::Menu::new();
        let item = gio::MenuItem::new(Some("New"), Some("app.new"));
        menu_model.append_item(&gio::MenuItem::new(Some("New"), Some("win.new")));
        menu_model.append_item(&gio::MenuItem::new(Some("Close"), Some("win.close_view")));
        menu_model.append_item(&gio::MenuItem::new(Some("Save"), Some("win.save")));
        menu_model.append_item(&gio::MenuItem::new(Some("Save As..."), Some("win.save_as")));
        gtk::PopoverMenu::builder().menu_model(&menu_model).build()
    }
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
        menu_button.set_icon_name("open-menu-symbolic");

        // let eddy_glade_str = include_str!("../ui/eddy.ui");
        // let eddy_glade_builder = gtk::Builder::from_string(eddy_glade_str);
        // let popover: gtk::PopoverMenu = eddy_glade_builder.object("hamburger_popover").unwrap();
        let popover_menu = WindowComponent::build_popover_menu();

        menu_button.set_popover(Some(&popover_menu));

        header_bar.pack_start(&new_button);
        header_bar.pack_end(&menu_button);

        let dir_bar = ctx.create_child(|s| s, |s| s, ());
        let sidebar_scrolled_window = gtk::ScrolledWindow::builder()
            .child(&dir_bar.widget())
            .build();

        let notebook = gtk::Notebook::new();
        let code_views = HashMap::new();
        let tab_labels = HashMap::new();

        let sidebar_paned = gtk::Paned::new(Orientation::Horizontal);
        sidebar_paned.set_start_child(Some(&sidebar_scrolled_window));
        sidebar_paned.set_position(200);
        sidebar_paned.set_resize_start_child(false);
        sidebar_paned.set_shrink_start_child(true);

        sidebar_paned.set_end_child(Some(&notebook));
        sidebar_paned.set_resize_end_child(true);
        sidebar_paned.set_shrink_end_child(false);

        // Create a window and set the title
        let window = ApplicationWindow::builder()
            .application(&app)
            .width_request(1150)
            .height_request(750)
            .icon_name("text-x-generic")
            .title("Eddy")
            .build();

        window.set_child(Some(&sidebar_paned));
        window.set_titlebar(Some(&header_bar));
        window.set_show_menubar(true);

        let action_new = SimpleAction::new("new", None);
        action_new.connect_activate(
            clone!(@weak notebook, @weak window, @strong ctx => move |_, _| {
                let res = ctx.with_model_mut(|ws| ws.new_view(None));
                show_res(&window, res);
            }),
        );
        window.add_action(&action_new);

        let action_close = SimpleAction::new("close_view", None);
        action_close.connect_activate(clone!(@weak notebook, @strong ctx => move |_, _| {
            if let Some(focused_view) = ctx.with_model(|ws| ws.focused_view) {
                ctx.with_model_mut(|ws| ws.close_view(focused_view))
            }
        }));
        window.add_action(&action_close);

        let action_save = SimpleAction::new("save", None);
        action_save.connect_activate(
            clone!(@weak notebook, @weak window, @strong ctx => move |_, _| {
                if let Some(focused_view) = ctx.with_model(|ws| ws.focused_view) {
                    let res = ctx.with_model_mut(|ws| ws.save(focused_view));
                    show_res(&window, res);
                }
            }),
        );
        window.add_action(&action_save);

        let action_save_as = SimpleAction::new("save_as", None);
        action_save_as.connect_activate(
            clone!(@weak notebook, @weak window, @strong ctx => move |_, _| {
                if let Some(focused_view) = ctx.with_model(|ws| ws.focused_view) {
                    let fcd = FileChooserDialog::new(
                        Some("Save File"),
                        Some(&window),
                        gtk::FileChooserAction::Save,
                        &[("_Cancel", ResponseType::Cancel), ("_Save", ResponseType::Accept)]);
                    fcd.set_modal(true);
                    fcd.connect_response(clone!(@strong ctx => move |chooser, response| {
                        if response == ResponseType::Accept {
                            if let Some(path) = chooser.file().and_then(|f| f.path()) {
                                let res = ctx.with_model_mut(|ws| ws.save_as(focused_view, &path));
                                show_res(&window, res);
                            }
                        }
                        chooser.destroy();
                    }));
                    fcd.present();
                }
            }),
        );
        window.add_action(&action_save_as);

        // Present window
        window.present();

        Self {
            window,
            action_new,
            action_close,
            action_save,
            action_save_as,
            dir_bar,
            code_views,
            tab_labels,
            notebook,
            last_views: HashSet::new(),
        }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        let focused_view = ctx.with_model(|ws| ws.focused_view);
        self.action_close.set_enabled(focused_view.is_some());
        self.action_save.set_enabled(focused_view.is_some());
        self.action_save_as.set_enabled(focused_view.is_some());

        // dbg!("window rebuild");
        let views: HashSet<ViewId> = ctx.with_model(|ws| ws.views.keys().copied().collect());
        let last_views: HashSet<ViewId> = self.last_views.clone();

        // Remove old views
        for view_id in last_views.difference(&views) {
            let page_num = self
                .notebook
                .page_num(&self.code_views.get(view_id).unwrap().widget());
            self.notebook.remove_page(page_num);
            self.code_views.remove(view_id);
            self.tab_labels.remove(view_id);
        }

        // Add new views
        for view_id in views.difference(&last_views).copied() {
            let cv_comp: ComponentHandle<CodeViewComponent> =
                ctx.create_child(|ws| ws, |ws| ws, view_id);
            let tl_comp: ComponentHandle<TabLabelComponent> =
                ctx.create_child(|ws| ws, |ws| ws, view_id);
            let page_num = dbg!(self
                .notebook
                .append_page(&cv_comp.widget(), Some(&tl_comp.widget())));
            self.notebook.set_page(page_num as i32);

            self.code_views.insert(view_id, cv_comp);
            self.tab_labels.insert(view_id, tl_comp);
        }

        self.last_views = views;

        ctx.rebuild_children();
    }
}

fn show_res<R>(window: &ApplicationWindow, res: Result<R, anyhow::Error>) {
    dbg!("show_res");
    if let Err(e) = res {
        show_err(window, e);
    }
}
// fn show_err<E: std::error::Error>(window: ApplicationWindow, e: E) {
//     let dialog = MessageDialog::builder()
//         .transient_for(&window)
//         .destroy_with_parent(true)
//         .modal(true)
//         .message_type(MessageType::Error)
//         .buttons(ButtonsType::Close)
//         .text(e.to_string())
//         .build();
//     dialog.connect_response(|dialog, _| dialog.destroy());
//     // (Some(&self.window), Dialog )
//     // e.to_string()
// }

fn show_err(window: &ApplicationWindow, e: anyhow::Error) {
    dbg!("show_err");
    let dialog = MessageDialog::builder()
        .transient_for(window)
        .destroy_with_parent(true)
        .modal(true)
        .message_type(MessageType::Error)
        .buttons(ButtonsType::Close)
        .text(e.to_string())
        .build();
    dialog.connect_response(|dialog, _| dialog.destroy());
    dialog.present();
    // (Some(&self.window), Dialog )
    // e.to_string()
}
