use super::code_view_text::CodeViewTextComponent;
use super::gutter::GutterComponent;

use eddy_model::{ViewId, Workspace};
use gflux::{Component, ComponentCtx, ComponentHandle};

use gtk::prelude::*;

#[allow(dead_code)]
pub struct CodeViewComponent {
    hbox: gtk::Box,
    cvt: ComponentHandle<CodeViewTextComponent>,
    gutter: ComponentHandle<GutterComponent>,
}

impl Component for CodeViewComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = gtk::Box;
    type Params = ViewId;

    fn widget(&self) -> Self::Widget {
        self.hbox.clone()
    }

    fn build(ctx: ComponentCtx<Self>, view_id: ViewId) -> Self {
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let hadj = gtk::Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let vadj = gtk::Adjustment::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        let cvt: ComponentHandle<CodeViewTextComponent> =
            ctx.create_child(|ws| ws, |ws| ws, view_id);
        // let cvt = CodeViewText::new();
        cvt.widget().set_hadjust(&hadj);
        cvt.widget().set_vadjust(&vadj);

        let gutter: ComponentHandle<GutterComponent> = ctx.create_child(|ws| ws, |ws| ws, view_id);
        gutter.widget().set_vadjust(&vadj); // TODO

        let scrolled_window = gtk::ScrolledWindow::builder()
            .hadjustment(&hadj)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vadjustment(&vadj)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_width(360)
            .child(&cvt.widget())
            .build();

        hbox.append(&gutter.widget());
        hbox.append(&scrolled_window);

        // cvt.set_hscroll_policy(gtk::ScrollablePolicy::Natural); TODO
        Self { hbox, cvt, gutter }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        ctx.rebuild_children();
    }
}
