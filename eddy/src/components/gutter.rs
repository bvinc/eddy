use crate::widgets::gutter::Gutter;

use eddy_model::{Model, ViewId, Window};
use gflux::{Component, ComponentCtx};

use gtk::prelude::*;

#[allow(dead_code)]
pub struct GutterComponent {
    gutter: Gutter,
}

impl Component for GutterComponent {
    type GlobalModel = Model;
    type Model = Window;
    type Widget = Gutter;
    type Params = ViewId;

    fn widget(&self) -> Self::Widget {
        self.gutter.clone()
    }

    fn build(ctx: ComponentCtx<Self>, view_id: ViewId) -> Self {
        let gutter = Gutter::new(ctx.clone(), view_id);

        Self { gutter }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        self.gutter.queue_draw();
        ctx.rebuild_children();
    }
}
