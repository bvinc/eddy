use crate::language::go::GoLayer;
use crate::language::{self, Layer, NilLayer};
use crate::line_ending::LineEnding;
use crate::style::{Attr, AttrSpan, Theme};
use crate::tab_mode::TabMode;
use crate::Range;
use crate::Selection;
use crate::ViewId;
use eddy_ts::{Parser, Tree};
use ropey::{str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::borrow::Cow;
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
    layer: Box<dyn Layer>,
    line_ending: LineEnding,
    tab_mode: TabMode,
    tab_size: usize,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
            history_ix: 0,
            history: vec![Rope::new()],
            selections: HashMap::new(),
            layer: Box::new(NilLayer::new()),
            line_ending: LineEnding::LF,
            tab_mode: TabMode::Spaces(4),
            tab_size: 8,
        }
    }
    pub fn from_file(path: &Path) -> Result<Self, io::Error> {
        let rope = Rope::from_reader(BufReader::new(File::open(path)?))?;

        Ok(Self {
            path: Some(path.to_owned()),
            history_ix: 0,
            history: vec![rope],
            selections: HashMap::new(),
            layer: language::layer_from_path(path),
            line_ending: LineEnding::LF,
            tab_mode: TabMode::Spaces(4),
            tab_size: 8,
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
    /// `remove` and `insert_at` are the two base methods that all edits
    /// eventually call.
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

        self.layer.update_highlights(rope);
    }

    /// Insert text into the buffer at a character index
    /// `remove` and `insert_at` are the two base methods that all edits
    /// eventually call.
    pub fn insert_at(&mut self, char_idx: usize, text: &str) {
        let rope = &mut self.history[self.history_ix];
        let text = self.line_ending.normalize(text);
        rope.insert(char_idx, &text);
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

    /// Insert text at every selection location in a view
    pub fn insert(&mut self, view_id: ViewId, text: &str) {
        self.save_undo();

        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            self.remove(sel.range());
        }
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let mut sel = self.selections.get(&view_id).unwrap()[i];
            self.insert_at(sel.cursor(), text);
            sel.horiz = None;
        }

        let rope = &self.history[self.history_ix].clone();
        self.layer.update_highlights(rope);
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
                        end: next_grapheme_boundary(&self.history[self.history_ix], sel.start),
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
                        start: prev_grapheme_boundary(&self.history[self.history_ix], sel.start),
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
        let rope = &self.history[self.history_ix];

        for sel in self.selections.entry(view_id).or_default() {
            sel.horiz = None;
            if sel.is_caret() {
                // move cursor to the left
                if sel.start > 0 {
                    let left = prev_grapheme_boundary(rope, sel.start);
                    sel.start = left;
                    sel.end = left;
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
            sel.horiz = None;
            if sel.is_caret() {
                // move cursor to the right
                if sel.start < len_chars {
                    let right = next_grapheme_boundary(rope, sel.start);
                    sel.start = right;
                    sel.end = right;
                }
            } else {
                // collapse selection to the right
                let right = sel.right();
                sel.start = right;
                sel.end = right;
            }
        }
    }

    /// Given a character location, and a saved horizontal offset, return a new
    /// character location and a new saved horizontal offset.
    fn up(
        rope: &Rope,
        char_idx: usize,
        horiz: Option<usize>,
        tab_size: usize,
    ) -> (usize, Option<usize>) {
        let line = rope.char_to_line(char_idx);
        let line_home = rope.line_to_char(line);
        // If we don't currently have a horizontal alignment, calculate the
        // graphemes from the line start.
        let horiz = horiz.unwrap_or_else(|| {
            RopeGraphemes::new(&rope.slice(line_home..char_idx))
                .map(|slice| {
                    if slice.len_bytes() == 1 && slice.char(0) == '\t' {
                        8
                    } else {
                        1
                    }
                })
                .sum()
        });

        if char_idx == 0 {
            // Only if we're already at the end of the line, set the
            // horiz.
            return (char_idx, Some(0));
        }

        if line == 0 {
            // There is no next line
            // Move the cursor to the last character on the line
            return (0, Some(horiz));
        }

        let prev_line = line.saturating_sub(1);
        let prev_line_home = rope.line_to_char(prev_line);
        let prev_line_end = line_home.saturating_sub(1);

        // iterate through the line's characters to find where we end up
        let mut final_char = prev_line_home;
        let mut x_diff = 0;

        // Itearate the graphemes on the line above, come up with a left
        // candidate and right candidate position
        let mut left_cand = (prev_line_home, 0);
        let mut right_cand = None;
        for g in RopeGraphemes::new(&rope.slice(prev_line_home..prev_line_end)) {
            if x_diff <= horiz {
                left_cand = (final_char, x_diff);
            } else {
                right_cand = Some((final_char, x_diff));
                break;
            }

            if g.len_bytes() == 1 && g.char(0) == '\t' {
                x_diff += ((x_diff / tab_size) + 1) * tab_size;
            } else {
                x_diff += 1
            }
            final_char += g.len_chars();
        }
        if x_diff <= horiz {
            left_cand = (final_char, x_diff);
        } else {
            right_cand = Some((final_char, x_diff));
        }

        // Go to the closest position to our horizontal alignment
        // If it's a tie, the left one wins.
        if let Some(right_cand) = right_cand {
            if horiz - left_cand.1 <= right_cand.1 - horiz {
                (left_cand.0, Some(horiz))
            } else {
                (right_cand.0, Some(horiz))
            }
        } else {
            (left_cand.0, Some(horiz))
        }
    }

    /// Move the cursor up
    pub fn move_up(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::up(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.start = final_char;
            sel.end = final_char;
        }
    }

    /// Move the cursor up while modifying the selection region
    pub fn move_up_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::up(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.end = final_char;
        }
    }

    /// Given a character location, and a saved horizontal offset, return a new
    /// character location and a new saved horizontal offset.
    fn down(
        rope: &Rope,
        char_idx: usize,
        horiz: Option<usize>,
        tab_size: usize,
    ) -> (usize, Option<usize>) {
        let line = rope.char_to_line(char_idx);
        let len_lines = rope.len_lines();
        let len_chars = rope.len_chars();
        let line_home = rope.line_to_char(line);

        let cur_x_diff = RopeGraphemes::new(&rope.slice(line_home..char_idx))
            .map(|slice| {
                if slice.len_bytes() == 1 && slice.char(0) == '\t' {
                    8
                } else {
                    1
                }
            })
            .sum();

        if char_idx == len_chars {
            // Only if we're already at the end of the line, set the
            // horiz.
            return (char_idx, Some(cur_x_diff));
        }

        // If we don't currently have a horizontal alignment, calculate the
        // graphemes from the line start.
        let horiz = horiz.unwrap_or_else(|| cur_x_diff);

        if line == len_lines - 1 {
            // There is no next line
            // Move the cursor to the last character on the line
            return (len_chars, Some(horiz));
        }

        let next_line = line + 1;
        let next_line_home = rope.line_to_char(next_line);
        let next_line_end = if next_line == len_lines - 1 {
            // There's no line after next, so the end is the last char of
            // the buffer
            len_chars
        } else {
            rope.line_to_char(next_line + 1) - 1
        };

        // iterate through the line's characters to find where we end up
        let mut final_char = next_line_home;
        let mut x_diff = 0;

        // Itearate the graphemes on the line above, come up with a left
        // candidate and right candidate position
        let mut left_cand = (next_line_home, 0);
        let mut right_cand = None;
        for g in RopeGraphemes::new(&rope.slice(next_line_home..next_line_end)) {
            if x_diff <= horiz {
                left_cand = (final_char, x_diff);
            } else {
                right_cand = Some((final_char, x_diff));
                break;
            }

            if g.len_bytes() == 1 && g.char(0) == '\t' {
                x_diff += ((x_diff / tab_size) + 1) * tab_size;
            } else {
                x_diff += 1
            }
            final_char += g.len_chars();
        }
        if x_diff <= horiz {
            left_cand = (final_char, x_diff);
        } else {
            right_cand = Some((final_char, x_diff));
        }

        // Go to the closest position to our horizontal alignment
        // If it's a tie, the left one wins.
        if let Some(right_cand) = right_cand {
            if horiz - left_cand.1 <= right_cand.1 - horiz {
                (left_cand.0, Some(horiz))
            } else {
                (right_cand.0, Some(horiz))
            }
        } else {
            (left_cand.0, Some(horiz))
        }
    }

    /// Move the cursor down
    pub fn move_down(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::down(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.start = final_char;
            sel.end = final_char;
        }
    }

    /// Move the cursor down while modifying the selection region
    pub fn move_down_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::down(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.end = final_char;
        }
    }

    /// TODO move the cursor to the left to the next word boundry
    pub fn move_word_left(&mut self, view_id: ViewId) {}
    /// TODO move the cursor to the right to the next word boundry
    pub fn move_word_right(&mut self, view_id: ViewId) {}

    /// Move the cursor left while modifying the selection region
    pub fn move_left_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];

        for sel in self.selections.entry(view_id).or_default() {
            if sel.end > 0 {
                let left = prev_grapheme_boundary(rope, sel.end);
                sel.end = left;
            }
        }
    }

    /// Move the cursor right while modifying the selection region
    pub fn move_right_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        let len_chars = rope.len_chars();

        for sel in self.selections.entry(view_id).or_default() {
            if sel.end < len_chars {
                let right = next_grapheme_boundary(rope, sel.end);
                sel.end = right;
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

    /// Executed when a user clicks
    pub fn gesture_point_select(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        let rope = &self.history[self.history_ix];
        let line = min(line, rope.len_lines());
        let total_byte_idx = rope.line_to_byte(line) + byte_idx;
        let total_char_idx = rope.byte_to_char(total_byte_idx);
        let total_char_idx = min(total_char_idx, rope.len_chars());

        let mut sel = Selection::new();
        sel.start = total_char_idx;
        sel.end = total_char_idx;

        use std::collections::hash_map::Entry;
        match self.selections.entry(view_id) {
            Entry::Occupied(ref mut e) => {
                e.get_mut().clear();
                e.get_mut().push(sel);
            }
            Entry::Vacant(e) => {
                e.insert(vec![sel]);
            }
        }
    }

    /// Executed when a user shift-clicks
    pub fn gesture_range_select(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        let rope = &self.history[self.history_ix];
        let line = min(line, rope.len_lines());
        let total_byte_idx = rope.line_to_byte(line) + byte_idx;
        let total_char_idx = rope.byte_to_char(total_byte_idx);
        let total_char_idx = min(total_char_idx, rope.len_chars());

        let mut sel = Selection::new();
        sel.start = self
            .selections
            .entry(view_id)
            .or_default()
            .iter()
            .map(|&s| s.start)
            .min()
            .unwrap_or_default();
        sel.end = total_char_idx;

        use std::collections::hash_map::Entry;
        match self.selections.entry(view_id) {
            Entry::Occupied(ref mut e) => {
                e.get_mut().clear();
                e.get_mut().push(sel);
            }
            Entry::Vacant(e) => {
                e.insert(vec![sel]);
            }
        }
    }

    /// Executed when a user ctrl-clicks.  If a selection exists on that point,
    /// remove it.  Otherwise, add a new selection at that point.
    pub fn gesture_toggle_sel(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        let rope = &self.history[self.history_ix];
        let line = min(line, rope.len_lines());
        let total_byte_idx = rope.line_to_byte(line) + byte_idx;
        let total_char_idx = rope.byte_to_char(total_byte_idx);
        let total_char_idx = min(total_char_idx, rope.len_chars());

        let new_sel = Selection {
            start: total_char_idx,
            end: total_char_idx,
            horiz: None,
        };

        use std::collections::hash_map::Entry;
        match self.selections.entry(view_id) {
            Entry::Vacant(e) => {
                // This shouldn't happen, but if it does, add a cursor
                e.insert(vec![new_sel]);
            }
            Entry::Occupied(ref mut e) => {
                // Search for a selection where the user clicked
                match e.get().binary_search_by_key(&total_char_idx, |s| s.start) {
                    Ok(ix) => {
                        // We found one, remove it
                        e.get_mut().remove(ix);
                    }
                    Err(ix) => {
                        if ix > 0 && e.get()[ix - 1].end >= total_char_idx {
                            // The one before it overlaps where the user clicked
                            e.get_mut().remove(ix - 1);
                        } else {
                            e.get_mut().insert(ix, new_sel);
                        }
                    }
                }
            }
        };
    }

    /// Executed when a user double-clicks
    pub fn gesture_word_select(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {}

    /// Executed when a user triple-clicks
    pub fn gesture_line_select(&mut self, view_id: ViewId, line: usize) {
        let rope = &self.history[self.history_ix];
        let line = min(line, rope.len_lines());
        let line_char_idx = rope.line_to_char(line);
        let line_end_char_idx = {
            if line >= rope.len_lines() - 1 {
                rope.len_chars()
            } else {
                rope.line_to_char(line + 1)
            }
        };

        let mut sel = Selection::new();
        sel.start = line_char_idx;
        sel.end = line_end_char_idx;

        use std::collections::hash_map::Entry;
        match self.selections.entry(view_id) {
            Entry::Occupied(ref mut e) => {
                e.get_mut().clear();
                e.get_mut().push(sel);
            }
            Entry::Vacant(e) => {
                e.insert(vec![sel]);
            }
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
        if self.history_ix <= 0 {
            return;
        }

        self.history_ix -= 1;

        self.fix_selections();
        self.layer.update_highlights(&self.history[self.history_ix]);
    }
    pub fn redo(&mut self) {
        if self.history_ix < self.history.len() - 1 {
            self.history_ix += 1;
        }

        self.fix_selections();
        self.layer.update_highlights(&self.history[self.history_ix]);
    }
    pub fn cut(&mut self, view_id: ViewId) -> Option<String> {
        let ret = self.copy(view_id);
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            if !sel.is_caret() {
                // Just remove the selection
                self.remove(sel.range());
            }
        }
        ret
    }
    pub fn copy(&mut self, view_id: ViewId) -> Option<String> {
        let mut ret = String::new();
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            if !sel.is_caret() {
                // Just remove the selection
                let rope = &self.history[self.history_ix];
                let text: Cow<str> = rope.slice(sel.range()).into();
                if !ret.is_empty() {
                    ret.push('\n');
                }
                ret.push_str(&text);
            }
        }
        if ret.is_empty() {
            None
        } else {
            Some(ret)
        }
    }
    pub fn paste(&mut self, view_id: ViewId) {}

    pub fn drag(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        let rope = &self.history[self.history_ix];
        let byte_idx = if line_idx >= rope.len_lines() {
            rope.len_bytes()
        } else {
            min(
                rope.line_to_byte(line_idx) + line_byte_idx,
                rope.len_bytes(),
            )
        };
        for sel in self.selections.entry(view_id).or_default() {
            // sel.start = dbg!(rope.byte_to_char(byte_idx));
            sel.end = dbg!(rope.byte_to_char(byte_idx));
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

    pub fn check_invariants(&mut self, view_id: ViewId) {
        let rope = &self.history[self.history_ix];
        debug_assert!(self.selections.get(&view_id).unwrap().len() > 0);
        for sel in self.selections.entry(view_id).or_default() {
            dbg!(
                rope,
                sel.start,
                rope.len_chars(),
                prev_grapheme_boundary(rope, sel.start),
                next_grapheme_boundary(rope, sel.start),
                prev_grapheme_boundary(rope, next_grapheme_boundary(rope, sel.start))
            );
            debug_assert!(
                sel.start == rope.len_chars()
                    || sel.start
                        == prev_grapheme_boundary(rope, next_grapheme_boundary(rope, sel.start))
            );
            debug_assert!(
                sel.end == rope.len_chars()
                    || sel.end
                        == prev_grapheme_boundary(rope, next_grapheme_boundary(rope, sel.end))
            );
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
        view_id: ViewId,
        line_idx: usize,
        theme: &Theme,
    ) -> Option<(RopeSlice, Vec<AttrSpan>)> {
        let rope = &self.history[self.history_ix];
        if line_idx >= rope.len_lines() {
            return None;
        }
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
        if let Some(tree) = self.layer.tree() {
            let mut cur = tree.walk();
            loop {
                let mut relevant = false;
                let mut moved = false;
                // TODO should this be an || ?
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

        for sel in self.selections.get(&view_id).unwrap_or(&vec![]) {
            if !sel.is_caret() {
                let r = sel.range();
                let sel_start_byte = self.char_to_byte(r.start);
                let sel_end_byte = self.char_to_byte(r.end);
                let sel_min_byte = min(sel_start_byte, sel_end_byte);
                let sel_max_byte = max(sel_start_byte, sel_end_byte);

                if sel_min_byte < line_end && sel_max_byte > line_start {
                    let start_byte = max(line_start, sel_min_byte) - line_start;
                    let end_byte = min(line_end, sel_max_byte) - line_start;
                    let attrs = theme.selection.clone();
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
        }

        Some((line, spans))
    }
}

/// Finds the previous grapheme boundary before the given char position.
fn prev_grapheme_boundary(slice: &Rope, char_idx: usize) -> usize {
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
fn next_grapheme_boundary(slice: &Rope, char_idx: usize) -> usize {
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

/// An implementation of a graphemes iterator, for iterating over
/// the graphemes of a RopeSlice.
struct RopeGraphemes<'a> {
    text: RopeSlice<'a>,
    chunks: ropey::iter::Chunks<'a>,
    cur_chunk: &'a str,
    cur_chunk_start: usize,
    cursor: GraphemeCursor,
}

impl<'a> RopeGraphemes<'a> {
    fn new<'b>(slice: &RopeSlice<'b>) -> RopeGraphemes<'b> {
        let mut chunks = slice.chunks();
        let first_chunk = chunks.next().unwrap_or("");
        RopeGraphemes {
            text: *slice,
            chunks,
            cur_chunk: first_chunk,
            cur_chunk_start: 0,
            cursor: GraphemeCursor::new(0, slice.len_bytes(), true),
        }
    }
}

impl<'a> Iterator for RopeGraphemes<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        let a = self.cursor.cur_cursor();
        let b;
        loop {
            match self
                .cursor
                .next_boundary(self.cur_chunk, self.cur_chunk_start)
            {
                Ok(None) => {
                    return None;
                }
                Ok(Some(n)) => {
                    b = n;
                    break;
                }
                Err(GraphemeIncomplete::NextChunk) => {
                    self.cur_chunk_start += self.cur_chunk.len();
                    self.cur_chunk = self.chunks.next().unwrap_or("");
                }
                _ => unreachable!(),
            }
        }

        if a < self.cur_chunk_start {
            let a_char = self.text.byte_to_char(a);
            let b_char = self.text.byte_to_char(b);

            Some(self.text.slice(a_char..b_char))
        } else {
            let a2 = a - self.cur_chunk_start;
            let b2 = b - self.cur_chunk_start;
            Some((&self.cur_chunk[a2..b2]).into())
        }
    }
}
/*
struct GraphemeIterator {
    gc: GraphemeCursor,
}

impl GraphemeIterator {
    fn new(slice: &Rope, char_idx: usize) -> Self {
        // Bounds check
        debug_assert!(char_idx <= slice.len_chars());

        // We work with bytes for this, so convert.
        let byte_idx = slice.char_to_byte(char_idx);

        // Get the chunk with our byte index in it.
        let (mut chunk, mut chunk_byte_idx, mut chunk_char_idx, _) = slice.chunk_at_byte(byte_idx);

        // Set up the grapheme cursor.
        let mut gc = GraphemeCursor::new(byte_idx, slice.len_bytes(), true);
        GraphemeIterator {}
    }
}

impl Iterator for GraphemeIterator {
    type Item = usize;
    fn next() -> Option<Item> {}
}
*/
impl ToString for Buffer {
    #[inline]
    fn to_string(&self) -> String {
        self.history[self.history_ix].slice(..).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "a");
        assert_eq!(buf.to_string(), "a");
    }
    #[test]
    fn test_insert2() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "a");
        buf.insert(0, "b");
        buf.insert(0, "cd");
        assert_eq!(buf.to_string(), "abcd");
    }

    #[test]
    fn test_move_left() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "a");
        buf.insert(0, "b");
        buf.move_left(0);
        buf.insert(0, "cd");
        assert_eq!(buf.to_string(), "acdb");
    }
    #[test]
    fn test_move_left_right() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "a");
        buf.insert(0, "b");
        buf.move_left(0);
        buf.move_right(0);
        buf.insert(0, "cd");
        assert_eq!(buf.to_string(), "abcd");
    }

    #[test]
    fn test_move_left_too_far() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.move_left(0);
        buf.move_left(0);
        buf.move_left(0);
        buf.insert(0, "abc");
        assert_eq!(buf.to_string(), "abc");
    }
    #[test]
    fn test_move_right_too_far() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.move_right(0);
        buf.move_right(0);
        buf.move_right(0);
        buf.insert(0, "abc");
        assert_eq!(buf.to_string(), "abc");
    }

    #[test]
    fn test_move_left_and_modify_selection() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abc");
        buf.move_left_and_modify_selection(0);
        buf.move_left_and_modify_selection(0);
        buf.insert(0, "de");
        assert_eq!(buf.to_string(), "ade");
        buf.move_left_and_modify_selection(0);
        buf.insert(0, "f");
        assert_eq!(buf.to_string(), "adf");
    }
    #[test]
    fn test_move_right_and_modify_selection() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abc");
        buf.move_left(0);
        buf.move_left(0);
        buf.move_right_and_modify_selection(0);
        buf.move_right_and_modify_selection(0);
        buf.insert(0, "de");
        assert_eq!(buf.to_string(), "ade");
    }
    #[test]
    fn test_move_up() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abc\ndef");
        buf.move_left(0);
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "ab_c\ndef");
    }
    #[test]
    fn test_move_up2() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "a\nbcd");
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "a_\nbcd");
    }
    #[test]
    fn test_move_up_to_tab_0() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "\tabc");
        buf.insert_newline(0);
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "_\tabc\n");
    }
    #[test]
    fn test_move_up_to_tab_4() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "\tabc");
        buf.insert_newline(0);
        buf.insert(0, "    ");
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "_\tabc\n    ");
    }
    #[test]
    fn test_move_up_to_tab_8() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "\tabc");
        buf.insert_newline(0);
        buf.insert(0, "        ");
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "\t_abc\n        ");
    }
    #[test]
    fn test_move_up_to_tab_9() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "\tabc");
        buf.insert_newline(0);
        buf.insert(0, "         ");
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "\ta_bc\n         ");
    }
    #[test]
    fn test_move_up_from_tab() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abcdefghi");
        buf.insert_newline(0);
        buf.insert(0, "\t");
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "abcdefgh_i\n\t");
    }
    #[test]
    fn test_move_down() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abc\ndef");
        buf.move_left(0);
        buf.move_up(0);
        buf.move_down(0);
        assert_eq!(
            buf.selections.get(&0).unwrap(),
            &vec![Selection {
                start: 6,
                end: 6,
                horiz: Some(2),
            }]
        );
    }
    #[test]
    fn test_move_down2() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abc\nd");
        buf.move_left(0);
        buf.move_left(0);
        buf.move_down(0);
        assert_eq!(
            buf.selections.get(&0).unwrap(),
            &vec![Selection {
                start: 5,
                end: 5,
                horiz: Some(3),
            }]
        );
    }
    #[test]
    fn test_move_down3() {
        let mut buf = Buffer::new();
        buf.init_view(0);
        buf.insert(0, "abc");
        buf.move_left(0);
        buf.move_down(0);
        assert_eq!(
            buf.selections.get(&0).unwrap(),
            &vec![Selection {
                start: 3,
                end: 3,
                horiz: None,
            }]
        );
    }
}
