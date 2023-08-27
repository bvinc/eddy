use crate::widgets::gutter::Gutter;

use super::window::WindowComponent;
use eddy_workspace::{ViewId, Workspace};
use gflux::{Component, ComponentCtx, ComponentHandle};
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[allow(dead_code)]
pub struct GutterComponent {
    gutter: Gutter,
}

impl Component for GutterComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = Gutter;
    type Params = ViewId;

    fn widget(&self) -> Self::Widget {
        self.gutter.clone()
    }

    fn build(ctx: ComponentCtx<Self>, view_id: ViewId) -> Self {
        let gutter = Gutter::new(ctx.clone(), view_id);

        Self { gutter }
    }

    fn rebuild(&mut self, _ctx: ComponentCtx<Self>) {
        self.gutter.queue_draw();
    }
}
