use eddy_model::{Model, ViewId, Window};
use gflux::{Component, ComponentCtx};
use glib::clone;
use gtk::{prelude::*, Orientation};

#[allow(dead_code)]
pub struct TabLabelComponent {
    view_id: ViewId,
    hbox: gtk::Box,
    label: gtk::Label,
}

impl Component for TabLabelComponent {
    type GlobalModel = Model;
    type Model = Window;
    type Widget = gtk::Box;
    type Params = ViewId;

    fn widget(&self) -> Self::Widget {
        self.hbox.clone()
    }

    fn build(ctx: ComponentCtx<Self>, view_id: ViewId) -> Self {
        let label = gtk::Label::new(None);

        let button = gtk::Button::new();
        button.set_icon_name("window-close");
        button.connect_clicked(clone!(@strong ctx as ctx => move |_| {
            ctx.with_model_mut(|ws| ws.close_view(view_id));
        }));

        let hbox = gtk::Box::new(Orientation::Horizontal, 8);
        hbox.append(&label);
        hbox.append(&button);

        Self {
            view_id,
            hbox,
            label,
        }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        let view_id = self.view_id;
        let name = ctx.with_model(|ws| ws.display_name(view_id));
        let pristine = ctx.with_model(|ws| ws.buffer(view_id).pristine);
        let name = format!("{}{}", if pristine { "" } else { "*" }, name);

        self.label.set_text(&name);
        ctx.rebuild_children();
    }
}
