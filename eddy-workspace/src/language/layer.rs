use super::go::GoLayer;
use super::rust::RustLayer;
use crate::language::capture::Capture;
use crate::Range;
use eddy_ts::Tree;
use ropey::Rope;
use std::path::Path;
pub trait Layer {
    fn capture(&self, idx: usize) -> Option<Capture>;
    fn capture_from_node(&self, id: usize) -> Option<Capture>;
    fn update_highlights(&mut self, rope: &Rope);
    fn tree(&self) -> Option<&Tree>;
    /// edit the tree, so tree-sitter can know what changed. All units are in code points.
    fn edit_tree(&mut self, rope: &Rope, start: usize, old_end: usize, new_end: usize);
    /// edit the tree, so tree-sitter can know what changed. All units are in code points.
    fn edit_tree_remove(&mut self, rope: &Rope, start: usize, old_end: usize) {
        self.edit_tree(rope, start, old_end, start);
    }
    /// edit the tree, so tree-sitter can know what changed. All units are in code points.
    fn edit_tree_insert(&mut self, rope: &Rope, start: usize, new_end: usize) {
        self.edit_tree(rope, start, start, new_end);
    }
}

pub fn layer_from_path(path: &Path) -> Box<dyn Layer> {
    if let Some(ext) = path.extension() {
        if ext == "rs" {
            return Box::new(RustLayer::new());
        }
        if ext == "go" {
            return Box::new(GoLayer::new());
        }
    }
    return Box::new(NilLayer::new());
}

pub struct NilLayer {}

impl NilLayer {
    pub fn new() -> Self {
        NilLayer {}
    }
}

impl Layer for NilLayer {
    fn capture(&self, _idx: usize) -> Option<Capture> {
        None
    }
    fn capture_from_node(&self, _id: usize) -> Option<Capture> {
        None
    }
    fn update_highlights(&mut self, _rope: &Rope) {}
    fn tree(&self) -> Option<&Tree> {
        None
    }
    fn edit_tree(&mut self, rope: &Rope, start: usize, old_end: usize, new_end: usize) {}
}

pub fn print_tree(node: eddy_ts::Node, level: u32) {
    let mut cur = node.walk();
    println!(
        "{}{} {} {}-{}",
        // indent 4 spaces for each level
        (0..level * 4).map(|_| " ").collect::<String>(),
        cur.node().kind(),
        cur.node().kind_id(),
        cur.node().start_position(),
        cur.node().end_position()
    );
    if cur.goto_first_child() {
        print_tree(cur.node(), level + 1);
    }
    while cur.goto_next_sibling() {
        print_tree(cur.node(), level + 1);
    }
}
