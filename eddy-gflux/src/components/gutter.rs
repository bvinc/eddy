use super::window::WindowComponent;
use eddy_workspace::Workspace;
use gflux::{Component, ComponentCtx, ComponentHandle};
use glib::clone;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[allow(dead_code)]
pub struct GutterComponent {
    label: gtk::Label,
}

impl Component for GutterComponent {
    type GlobalModel = Workspace;
    type Model = Workspace;
    type Widget = gtk::Label;
    type Params = ();

    fn widget(&self) -> Self::Widget {
        self.label.clone()
    }

    fn build(ctx: ComponentCtx<Self>, _params: ()) -> Self {
        let label = gtk::Label::new(Some(&"gutter"));

        Self { label }
    }

    fn rebuild(&mut self, _ctx: ComponentCtx<Self>) {}
}
