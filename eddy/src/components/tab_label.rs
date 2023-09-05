use eddy_workspace::{ViewId, Workspace};
use gflux::{Component, ComponentCtx};
use glib::clone;
use gtk::{prelude::*, Orientation};

#[allow(dead_code)]
pub struct TabLabelComponent {
    hbox: gtk::Box,
}

impl Component for TabLabelComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = gtk::Box;
    type Params = ViewId;

    fn widget(&self) -> Self::Widget {
        self.hbox.clone()
    }

    fn build(ctx: ComponentCtx<Self>, view_id: ViewId) -> Self {
        let name = ctx.with_model(|ws| ws.display_name(view_id));

        let label = gtk::Label::new(Some(&name));

        let button = gtk::Button::new();
        button.set_icon_name("window-close");
        button.connect_clicked(clone!(@strong ctx as ctx => move |_| {
            ctx.with_model(|ws| ws.close_view(view_id));
        }));

        let hbox = gtk::Box::new(Orientation::Horizontal, 8);
        hbox.append(&label);
        hbox.append(&button);

        Self { hbox }
    }

    fn rebuild(&mut self, _ctx: ComponentCtx<Self>) {}
}
