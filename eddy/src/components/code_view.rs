use crate::widgets::code_view_text::CodeViewText;

use super::code_view_text::CodeViewTextComponent;
use super::gutter::GutterComponent;
use super::window::WindowComponent;
use eddy_workspace::{ViewId, Workspace};
use gflux::{Component, ComponentCtx, ComponentHandle};
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

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

        let cvt: ComponentHandle<CodeViewTextComponent> = ctx.create_child(|ws| ws, view_id);
        // let cvt = CodeViewText::new();
        // cvt.set_hadjust(&hadj); TODO
        // cvt.set_vadjust(&vadj); TODO

        let gutter: ComponentHandle<GutterComponent> = ctx.create_child(|ws| ws, view_id);
        // gutter.set_vadjust(&vadj); TODO

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

    fn rebuild(&mut self, _ctx: ComponentCtx<Self>) {
        println!("cv rebuild");
        self.gutter.rebuild();
    }
}
