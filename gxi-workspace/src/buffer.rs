use crate::language::rust::RustLayer;
use crate::style::{Attr, AttrSpan, Theme};
use crate::Range;
use crate::Selection;
use crate::ViewId;
use eddy_ts::{language, Parser, Tree};
use ropey::{str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader};
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

// #[derive(Debug)]
pub struct Buffer {
    path: Option<PathBuf>,
    history_ix: usize,
    history: Vec<Rope>,
    selections: HashMap<ViewId, Vec<Selection>>,
    parser: eddy_ts::Parser,
    tree: Option<Tree>,
    layer: RustLayer,
}

fn print_tree(node: eddy_ts::Node, level: u32) {
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

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
            history_ix: 0,
            history: vec![Rope::new()],
            selections: HashMap::new(),
            parser: Parser::new(),
            tree: None,
            layer: RustLayer::new(),
        }
    }
    pub fn from_file(path: &Path) -> Result<Self, io::Error> {
        let rope = Rope::from_reader(BufReader::new(File::open(path)?))?;

        Ok(Self {
            path: Some(path.to_owned()),
            history_ix: 0,
            history: vec![rope],
            selections: HashMap::new(),
            parser: Parser::new(),
            tree: None,
            layer: RustLayer::new(),
        })
    }

    pub fn init_view(&mut self, view_id: ViewId) {
        self.selections.insert(
            view_id,
            vec![Selection {
                start: 0,
                end: 0,
                horiz: None,
            }],
        );
    }

    /// This is called before changes are made to create a snapshot in the undo
    /// history.
    pub fn save_undo(&mut self) {
        // Save the current state in the history
        self.history.truncate(self.history_ix + 1);
        let rope_clone = self.history[self.history_ix].clone();
        self.history.push(rope_clone);
        self.history_ix = self.history.len() - 1;
    }

    /// Get selections that are part of a view
    pub fn selections(&self, view_id: ViewId) -> &[Selection] {
        self.selections.get(&view_id).unwrap()
    }

    /// Removes a range of text from the buffer
    pub fn remove(&mut self, char_range: Range) {
        debug_assert!(char_range.start <= char_range.end);

        if char_range.start == char_range.end {
            return;
        }

        let rope = &mut self.history[self.history_ix];
        rope.remove(char_range);

        // Update all the selections
        let size = char_range.end - char_range.start;
        for sels in self.selections.values_mut() {
            for sel in sels {
                if char_range.contains(&sel.start) {
                    // collapse points inside the removal to the beginning
                    sel.start = char_range.start;
                } else if sel.start >= char_range.end {
                    // shift points after the removal to the left
                    sel.start -= size;
                }

                if char_range.contains(&sel.end) {
                    // collapse points inside the removal to the beginning
                    sel.end = char_range.start;
                } else if sel.end >= char_range.end {
                    // shift points after the removal to the left
                    sel.end -= size;
                }
            }
        }
    }

    /// Insert text at every selection location in a view
    pub fn insert(&mut self, view_id: ViewId, text: &str) {
        self.save_undo();

        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            self.remove(sel.range());
        }
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            self.insert_at(sel.cursor(), text);
        }

        self.parser.set_language(language::rust());
        let rope = &self.history[self.history_ix].clone();
        self.tree = self.parser.parse_with(
            &mut |byte_idx, pos| {
                if byte_idx > rope.len_bytes() {
                    return [].as_ref();
                }
                let (s, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte_idx);
                let ret = &s.as_bytes()[byte_idx - chunk_byte_idx..];
                // println!("asked for {} {}, returned {:?}", byte_idx, pos, ret);
                ret
            },
            None, //self.tree.as_ref(),
        );
        if let Some(tree) = &self.tree {
            print_tree(tree.root_node(), 0);
            self.layer.update_highlights(rope, tree.root_node());
        }
    }

    /// Insert text into the buffer at a character index
    pub fn insert_at(&mut self, char_idx: usize, text: &str) {
        let rope = &mut self.history[self.history_ix];
        rope.insert(char_idx, text);
        let size = text.chars().count();
        for sels in &mut self.selections.values_mut() {
            for sel in sels {
                if sel.start >= char_idx {
                    sel.start += size;
                }
                if sel.end >= char_idx {
                    sel.end += size;
                }
            }
        }
    }

    /// Insert a newline at every selection point of a view
    pub fn insert_newline(&mut self, view_id: ViewId) {
        self.insert(view_id, "\n")
    }

    /// Insert a tab at every selection point of a view
    pub fn insert_tab(&mut self, view_id: ViewId) {
        self.insert(view_id, "\t")
    }

    /// Delete the character after the cursor, or the highlighted region.  This
    /// is normally what happens when the delete key is pressed.
    pub fn delete_forward(&mut self, view_id: ViewId) {
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            let len_chars = self.history[self.history_ix].len_chars();
            if sel.is_caret() {
                if sel.cursor() < len_chars {
                    // Remove the character in front of the cursor
                    self.remove(Range {
                        start: sel.cursor(),
                        end: sel.cursor() + 1,
                    });
                }
            } else {
                // Just remove the selection
                self.remove(sel.range());
            }
        }
    }

    /// Delete the character before the cursor, or the highlighted region.  This
    /// is normally what happens when the backspace key is pressed.
    pub fn delete_backward(&mut self, view_id: ViewId) {
        // Delete all selection regions
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            if sel.is_caret() {
                if sel.cursor() != 0 {
                    // Remove the character before the cursor
                    self.remove(Range {
                        start: sel.cursor() - 1,
                        end: sel.cursor(),
                    });
                }
            } else {
                self.remove(sel.range());
            }
        }
    }

    /// Move the cursor to the left, or collapse selection region to the left
    pub fn move_left(&mut self, view_id: ViewId) {
        for sel in self.selections.entry(view_id).or_default() {
            if sel.is_caret() {
                // move cursor to the left
                if sel.start > 0 {
                    sel.start -= 1;
                    sel.end -= 1;
                }
            } else {
                // collapse selection to the left
                let left = sel.left();
                sel.start = left;
                sel.end = left;
            }
        }
    }

    /// Move the cursor to the right, or collapse selection region to the right
    pub fn move_right(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        let len_chars = rope.len_chars();
        for sel in self.selections.entry(view_id).or_default() {
            if sel.start != sel.end {
                // collapse selection to the right
                let right = max(sel.start, sel.end);
                sel.start = right;
                sel.end = right;
            } else {
                // move cursor to the right
                if sel.start < len_chars {
                    sel.start += 1;
                }
                if sel.end < len_chars {
                    sel.end += 1;
                }
            }
        }
    }

    fn up(rope: &Rope, char_idx: usize) -> (usize, Option<usize>) {
        let line = rope.char_to_line(char_idx);
        let line_home = rope.line_to_char(line);
        let x_diff = char_idx - line_home;
        let prev_line = line.saturating_sub(1);
        let prev_line_home = rope.line_to_char(prev_line);
        let prev_line_end = line_home.saturating_sub(1);
        let final_char = min(prev_line_end, prev_line_home + x_diff);
        (final_char, Some(x_diff))
    }

    /// Move the cursor up
    pub fn move_up(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::up(rope, sel.cursor());
            sel.horiz = horiz;
            sel.start = final_char;
            sel.end = final_char;
        }
    }

    /// Move the cursor up while modifying the selection region
    pub fn move_up_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::up(rope, sel.cursor());
            sel.horiz = horiz;
            sel.end = final_char;
        }
    }

    /// Move the cursor up
    pub fn move_down(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        let len_lines = rope.len_lines();
        let len_chars = rope.len_chars();
        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            let line_home = rope.line_to_char(line);
            let x_diff = sel.cursor() - line_home;
            if line == len_lines - 1 {
                // There is no next line
                if sel.cursor() == len_chars {
                    // Only if we're already at the end of the line, set the
                    // horiz. This is what gnome gedit does.
                    sel.horiz = Some(x_diff);
                    return;
                }
                // Move the cursor to the last character on the line
                sel.start = len_chars;
                sel.end = len_chars;
                return;
            }
            //let next_line = if line < len_lines { line + 1 } else { line };
            let next_line = line + 1;
            let next_line_home = rope.line_to_char(next_line);
            let next_line_end = if next_line == len_lines - 1 {
                // There's no line after next, so the end is the last char of
                // the buffer
                len_chars
            } else {
                rope.line_to_char(next_line + 1) - 1
            };
            // char_want is the ideal location
            let char_want = next_line_home + x_diff;
            let final_char = min(next_line_end, max(next_line_home, char_want));
            sel.start = final_char;
            sel.end = final_char;
            sel.horiz = Some(x_diff);
        }
    }

    /// Move the cursor down while modifying the selection region
    pub fn move_down_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        let len_lines = rope.len_lines();
        let len_chars = rope.len_chars();
        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            let line_home = rope.line_to_char(line);
            let x_diff = sel.cursor() - line_home;
            if line == len_lines - 1 {
                // There is no next line
                if sel.cursor() == len_chars {
                    // Only if we're already at the end of the line, set the
                    // horiz. This is what gnome gedit does.
                    sel.horiz = Some(x_diff);
                    return;
                }
                // Move the cursor to the last character on the line
                sel.end = len_chars;
                return;
            }
            //let next_line = if line < len_lines { line + 1 } else { line };
            let next_line = line + 1;
            let next_line_home = rope.line_to_char(next_line);
            let next_line_end = if next_line == len_lines - 1 {
                // There's no line after next, so the end is the last char of
                // the buffer
                len_chars
            } else {
                rope.line_to_char(next_line + 1) - 1
            };
            // char_want is the ideal location
            let char_want = next_line_home + x_diff;
            let final_char = min(next_line_end, max(next_line_home, char_want));
            sel.end = final_char;
            sel.horiz = Some(x_diff);
        }
    }

    /// TODO move the cursor to the left to the next word boundry
    pub fn move_word_left(&mut self, view_id: ViewId) {}
    /// TODO move the cursor to the right to the next word boundry
    pub fn move_word_right(&mut self, view_id: ViewId) {}

    /// Move the cursor left while modifying the selection region
    pub fn move_left_and_modify_selection(&mut self, view_id: ViewId) {
        for sel in self.selections.entry(view_id).or_default() {
            if sel.end > 0 {
                sel.end -= 1;
            }
        }
    }

    /// Move the cursor right while modifying the selection region
    pub fn move_right_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        let len_chars = rope.len_chars();

        for sel in self.selections.entry(view_id).or_default() {
            if sel.end < len_chars {
                sel.end += 1;
            }
        }
    }

    /// TODO move the cursor to the left to the next word boundry while
    /// modifying the seleciton region
    pub fn move_word_left_and_modify_selection(&mut self, view_id: ViewId) {}
    /// TODO move the cursor to the right to the next word boundry while
    /// modifying the seleciton region
    pub fn move_word_right_and_modify_selection(&mut self, view_id: ViewId) {}

    pub fn move_to_left_end_of_line(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            let line_home = rope.line_to_char(line);
            sel.start = line_home;
            sel.end = line_home;
            sel.horiz = None;
        }
    }

    pub fn move_to_right_end_of_line(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        let len_lines = rope.len_lines();
        let end_of_doc = rope.len_chars();
        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            if line == len_lines - 1 {
                sel.start = end_of_doc;
                sel.end = end_of_doc;
                sel.horiz = None;
                continue;
            }
            let line_end = rope.line_to_char(line + 1) - 1;
            sel.start = line_end;
            sel.end = line_end;
            sel.horiz = None;
        }
    }

    pub fn move_to_left_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            let line_home = rope.line_to_char(line);
            sel.end = line_home;
            sel.horiz = None;
        }
    }

    pub fn move_to_right_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        let len_lines = rope.len_lines();
        let end_of_doc = rope.len_chars();
        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            if line == len_lines - 1 {
                sel.end = end_of_doc;
                sel.horiz = None;
                continue;
            }
            let line_end = rope.line_to_char(line + 1) - 1;
            sel.end = line_end;
            sel.horiz = None;
        }
    }

    pub fn move_to_beginning_of_document(&mut self, view_id: ViewId) {
        for sel in self.selections.entry(view_id).or_default() {
            sel.start = 0;
            sel.end = 0;
        }
    }

    pub fn move_to_end_of_document(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        for sel in self.selections.entry(view_id).or_default() {
            let end_of_doc = rope.len_chars();
            sel.start = end_of_doc;
            sel.end = end_of_doc;
        }
    }
    pub fn move_to_beginning_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        for sel in self.selections.entry(view_id).or_default() {
            let end_of_doc = rope.len_chars();
            sel.end = end_of_doc;
        }
    }
    pub fn move_to_end_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        for sel in self.selections.entry(view_id).or_default() {
            let end_of_doc = rope.len_chars();
            sel.end = end_of_doc;
        }
    }
    pub fn page_down(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_down(view_id);
        }
    }
    pub fn page_up(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_up(view_id);
        }
    }
    pub fn page_up_and_modify_selection(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_up_and_modify_selection(view_id);
        }
    }
    pub fn page_down_and_modify_selection(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_down_and_modify_selection(view_id);
        }
    }
    pub fn select_all(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        let len_chars = rope.len_chars();
        let mut sel = Selection::new();
        sel.start = 0;
        sel.end = len_chars;
        self.selections.insert(view_id, vec![sel]);
    }
    pub fn undo(&mut self) {
        if self.history_ix > 0 {
            self.history_ix -= 1;
        }

        self.fix_selections();
    }
    pub fn redo(&mut self) {
        if self.history_ix < self.history.len() - 1 {
            self.history_ix += 1;
        }
    }

    // currently the only thing this does is ensure that all selections are not
    // out of bounds
    // TODO make sure selection regions never intersect, if so, merge them
    pub fn fix_selections(&mut self) {
        let rope = &self.history[self.history_ix];
        let len_chars = rope.len_chars();

        for (_, sels) in &mut self.selections {
            for sel in sels {
                if sel.start > len_chars {
                    sel.start = len_chars
                }
                if sel.end > len_chars {
                    sel.end = len_chars
                }
            }
        }
    }

    pub fn len_bytes(&self) -> usize {
        let rope = &self.history[self.history_ix];
        rope.len_bytes()
    }
    pub fn len_chars(&self) -> usize {
        let rope = &self.history[self.history_ix];
        rope.len_chars()
    }
    pub fn len_lines(&self) -> usize {
        let rope = &self.history[self.history_ix];
        rope.len_lines()
    }
    pub fn line(&self, line_idx: usize) -> RopeSlice {
        let rope = &self.history[self.history_ix];
        rope.line(line_idx)
    }
    pub fn rope_clone(&self) -> Rope {
        let rope = &self.history[self.history_ix];
        rope.clone()
    }
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        let rope = &self.history[self.history_ix];
        rope.char_to_line(char_idx)
    }
    pub fn line_to_char(&self, char_idx: usize) -> usize {
        let rope = &self.history[self.history_ix];
        rope.line_to_char(char_idx)
    }
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        let rope = &self.history[self.history_ix];
        rope.char_to_byte(char_idx)
    }
    pub fn line_to_byte(&self, char_idx: usize) -> usize {
        let rope = &self.history[self.history_ix];
        rope.line_to_byte(char_idx)
    }

    pub fn get_line_with_attributes(
        &self,
        line_idx: usize,
        theme: &Theme,
    ) -> (RopeSlice, Vec<AttrSpan>) {
        let rope = &self.history[self.history_ix];
        let line = rope.line(line_idx);
        let len_lines = rope.len_lines();
        let line_start = rope.line_to_byte(line_idx);
        let line_end = if line_idx == len_lines - 1 {
            // There is no next line
            rope.len_bytes()
        } else {
            rope.line_to_byte(line_idx + 1)
        };

        let mut spans = Vec::new();
        if let Some(tree) = &self.tree {
            let mut cur = tree.walk();
            loop {
                let mut relevant = false;
                let mut moved = false;
                if cur.node().start_byte() < line_end && cur.node().end_byte() > line_start {
                    let start_byte = max(line_start, cur.node().start_byte()) - line_start;
                    let end_byte = min(line_end, cur.node().end_byte()) - line_start;

                    if let Some(capture) = self.layer.capture_from_node(cur.node().id()) {
                        if let Some(attrs) = theme.attributes(capture).clone() {
                            if let Some(fg) = attrs.fg {
                                spans.push(AttrSpan {
                                    start_idx: start_byte,
                                    end_idx: end_byte,
                                    attr: Attr::ForegroundColor(fg),
                                });
                            }
                            if let Some(bg) = attrs.bg {
                                spans.push(AttrSpan {
                                    start_idx: start_byte,
                                    end_idx: end_byte,
                                    attr: Attr::BackgroundColor(bg),
                                });
                            }
                        }
                    }
                    relevant = true;
                }
                if relevant {
                    moved = cur.goto_first_child()
                }
                if !moved {
                    moved = cur.goto_next_sibling();
                }
                if !moved {
                    moved = cur.goto_parent() && cur.goto_next_sibling();
                }
                if !moved {
                    break;
                }
            }
        }
        (line, spans)
    }

    /// Finds the previous grapheme boundary before the given char position.
    fn prev_grapheme_boundary(&self, char_idx: usize) -> usize {
        let slice = &self.history[self.history_ix];

        // Bounds check
        debug_assert!(char_idx <= slice.len_chars());

        // We work with bytes for this, so convert.
        let byte_idx = slice.char_to_byte(char_idx);

        // Get the chunk with our byte index in it.
        let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

        // Set up the grapheme cursor.
        let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

        // Find the previous grapheme cluster boundary.
        loop {
            match gc.prev_boundary(chunk, chunk_byte_idx) {
                Ok(None) => return 0,
                Ok(Some(n)) => {
                    let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                    return chunk_char_idx + tmp;
                }
                Err(GraphemeIncomplete::PrevChunk) => {
                    let (a, b, c, _) = slice.chunk_at_byte(chunk_byte_idx - 1);
                    chunk = a;
                    chunk_byte_idx = b;
                    chunk_char_idx = c;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                    gc.provide_context(ctx_chunk, n - ctx_chunk.len());
                }
                _ => unreachable!(),
            }
        }
    }

    /// Finds the next grapheme boundary after the given char position.
    fn next_grapheme_boundary(&self, char_idx: usize) -> usize {
        let slice = &self.history[self.history_ix];

        // Bounds check
        debug_assert!(char_idx <= slice.len_chars());

        // We work with bytes for this, so convert.
        let byte_idx = slice.char_to_byte(char_idx);

        // Get the chunk with our byte index in it.
        let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

        // Set up the grapheme cursor.
        let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);

        // Find the next grapheme cluster boundary.
        loop {
            match gc.next_boundary(chunk, chunk_byte_idx) {
                Ok(None) => return slice.len_chars(),
                Ok(Some(n)) => {
                    let tmp = byte_to_char_idx(chunk, n - chunk_byte_idx);
                    return chunk_char_idx + tmp;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    chunk_byte_idx += chunk.len();
                    let (a, _, c, _) = slice.chunk_at_byte(chunk_byte_idx);
                    chunk = a;
                    chunk_char_idx = c;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let ctx_chunk = slice.chunk_at_byte(n - 1).0;
                    gc.provide_context(ctx_chunk, n - ctx_chunk.len());
                }
                _ => unreachable!(),
            }
        }
    }
}
