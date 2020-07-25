use crate::buffer::Buffer;
use crate::view::View;
use std::path::Path;

pub struct Workspace {
    views: Vec<View>,
    // buffers: Vec<Buffer>,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            views: vec![],
            // buffers: vec![],
        }
    }

    pub fn new_view(&mut self, path: Option<&Path>) -> usize {
        self.views.push(View::new());
        self.views.len() - 1
    }

    pub fn view(&mut self, view_id: usize) -> &mut View {
        &mut self.views[view_id]
    }

    pub fn save(&mut self, view_id: usize, path: &Path) {}
}
