use crate::widgets::code_view_text::CodeViewText;

use eddy_workspace::{ViewId, Workspace};
use gflux::{Component, ComponentCtx};

use gtk::prelude::*;

#[allow(dead_code)]
pub struct CodeViewTextComponent {
    cvt: CodeViewText,
    view_id: ViewId,
}

impl Component for CodeViewTextComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = CodeViewText;
    type Params = ViewId;

    fn widget(&self) -> Self::Widget {
        self.cvt.clone()
    }

    fn build(ctx: ComponentCtx<Self>, view_id: ViewId) -> Self {
        // let cvt: ComponentHandle<CodeViewTextComponent> = ctx.create_child(|ws| ws, ());
        let cvt = CodeViewText::new(ctx.clone(), view_id);
        // cvt.set_hadjust(&hadj); TODO
        // cvt.set_vadjust(&vadj); TODO

        // cvt.set_hscroll_policy(gtk::ScrollablePolicy::Natural); TODO
        Self { cvt, view_id }
    }

    fn rebuild(&mut self, ctx: ComponentCtx<Self>) {
        self.cvt.queue_draw();
    }
}
