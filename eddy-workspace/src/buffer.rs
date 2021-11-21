use crate::graphemes::{
    next_grapheme_boundary, prev_grapheme_boundary, RopeGraphemes, RopeGraphemesRev,
};
use crate::history::History;
use crate::language::{self, Layer, NilLayer};
use crate::line_ending::LineEnding;
use crate::style::{Attr, AttrSpan, Theme};
use crate::tab_mode::TabMode;
use crate::{BufferId, Msg, MsgSender, Range, Selection, ViewId};
use anyhow::bail;
use log::*;
use ropey::{Rope, RopeSlice};
use std::borrow::Cow;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader};
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// #[derive(Debug)]
pub struct Buffer {
    id: BufferId,
    path: Option<PathBuf>,
    // history_ix: usize,
    // history: Vec<Rope>,
    rope: Rope,
    history: History,
    selections: HashMap<ViewId, Vec<Selection>>,
    layer: Box<dyn Layer>,
    line_ending: LineEnding,
    tab_mode: TabMode,
    tab_size: usize,
    text_change_cbs: Vec<Box<dyn Fn() + 'static>>,
    scroll_to_selections_cbs: HashMap<ViewId, Vec<Box<dyn Fn() + 'static>>>,
    msg_sender: Arc<Mutex<MsgSender>>,
}

impl Buffer {
    pub fn new(id: BufferId, msg_sender: Arc<Mutex<MsgSender>>) -> Self {
        let rope = Rope::new();
        Self {
            id,
            path: None,
            history: History::new(&rope),
            rope,
            selections: HashMap::new(),
            layer: Box::new(NilLayer::new()),
            line_ending: LineEnding::LF,
            tab_mode: TabMode::Spaces(4),
            tab_size: 8,
            text_change_cbs: Vec::new(),
            scroll_to_selections_cbs: HashMap::new(),
            msg_sender,
        }
    }
    pub fn from_file(
        id: BufferId,
        path: &Path,
        msg_sender: Arc<Mutex<MsgSender>>,
    ) -> Result<Self, io::Error> {
        let rope = Rope::from_reader(BufReader::new(File::open(path)?))?;

        let mut buffer = Buffer {
            id,
            path: Some(path.to_owned()),
            history: History::new(&rope),
            rope,
            selections: HashMap::new(),
            layer: language::layer_from_path(path),
            line_ending: LineEnding::LF,
            tab_mode: TabMode::Spaces(4),
            tab_size: 8,
            text_change_cbs: Vec::new(),
            scroll_to_selections_cbs: HashMap::new(),
            msg_sender,
        };
        buffer.on_text_change();
        Ok(buffer)
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

    /// Get selections that are part of a view
    pub fn selections(&self, view_id: ViewId) -> &[Selection] {
        self.selections.get(&view_id).unwrap()
    }

    /// Subscribe to buffer updates.  Whenever this buffer changes, call `cb`.
    pub fn connect_update<F: Fn() + 'static>(&mut self, cb: F) {
        self.text_change_cbs.push(Box::new(cb))
    }

    pub fn connect_scroll_to_selections<F: Fn() + 'static>(&mut self, view_id: ViewId, cb: F) {
        self.scroll_to_selections_cbs
            .entry(view_id)
            .or_insert(Vec::new())
            .push(Box::new(cb))
    }

    fn scroll_to_selections(&mut self, view_id: ViewId) {
        // Call the callbacks
        for cb in self
            .scroll_to_selections_cbs
            .get(&view_id)
            .unwrap_or(&Vec::new())
        {
            cb()
        }
    }

    fn on_text_change(&mut self) {
        let start = Instant::now();
        self.layer.update_highlights(&self.rope);
        debug!("update_highlights took {}ms", start.elapsed().as_millis());

        // Call the change callbacks
        for cb in &self.text_change_cbs {
            cb()
        }
    }

    fn on_selection_change(&mut self) {
        // Call the change callbacks
        for cb in &self.text_change_cbs {
            cb()
        }
    }

    /// Removes a range of text from the buffer
    /// `remove` and `insert_at` are the two base methods that all edits
    /// eventually call.
    pub fn remove(&mut self, char_range: Range) {
        debug_assert!(char_range.start <= char_range.end);

        if char_range.start == char_range.end {
            return;
        }

        let rope = &mut self.rope;
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

    /// Insert text into the buffer at a character index
    /// `remove` and `insert_at` are the two base methods that all edits
    /// eventually call.
    pub fn insert_at(&mut self, char_idx: usize, text: &str) {
        let rope = &mut self.rope;
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
        let sels_before = self.selections.get(&view_id).cloned().unwrap_or_default();

        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            self.remove(sel.range());
        }
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let mut sel = self.selections.get(&view_id).unwrap()[i];
            self.insert_at(sel.cursor(), text);
            sel.horiz = None;
        }

        let sels_after = self.selections.get(&view_id).cloned().unwrap_or_default();
        self.history.new_change(&self.rope, sels_before, sels_after);

        self.on_text_change();
        self.scroll_to_selections(view_id);
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
        let sels_before = self.selections.get(&view_id).cloned().unwrap_or_default();

        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            let len_chars = self.rope.len_chars();
            if sel.is_caret() {
                if sel.cursor() < len_chars {
                    // Remove the character in front of the cursor
                    self.remove(Range {
                        start: sel.cursor(),
                        end: next_grapheme_boundary(&self.rope, sel.start),
                    });
                }
            } else {
                // Just remove the selection
                self.remove(sel.range());
            }
        }

        let sels_after = self.selections.get(&view_id).cloned().unwrap_or_default();
        self.history.new_change(&self.rope, sels_before, sels_after);

        self.on_text_change();
        self.scroll_to_selections(view_id);
    }

    /// Delete the character before the cursor, or the highlighted region.  This
    /// is normally what happens when the backspace key is pressed.
    pub fn delete_backward(&mut self, view_id: ViewId) {
        let sels_before = self.selections.get(&view_id).cloned().unwrap_or_default();

        // Delete all selection regions
        for i in 0..self.selections.entry(view_id).or_default().len() {
            let sel = self.selections.get(&view_id).unwrap()[i];
            if sel.is_caret() {
                if sel.cursor() != 0 {
                    // Remove the character before the cursor
                    self.remove(Range {
                        start: prev_grapheme_boundary(&self.rope, sel.start),
                        end: sel.cursor(),
                    });
                }
            } else {
                self.remove(sel.range());
            }
        }

        let sels_after = self.selections.get(&view_id).cloned().unwrap_or_default();
        self.history.new_change(&self.rope, sels_before, sels_after);

        self.on_text_change();
        self.scroll_to_selections(view_id);
    }

    /// Move the cursor to the left, or collapse selection region to the left
    pub fn move_left(&mut self, view_id: ViewId) {
        let rope = &self.rope;

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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Move the cursor to the right, or collapse selection region to the right
    pub fn move_right(&mut self, view_id: ViewId) {
        let rope = &self.rope;

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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
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
        let rope = &self.rope;
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::up(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.start = final_char;
            sel.end = final_char;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Move the cursor up while modifying the selection region
    pub fn move_up_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::up(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.end = final_char;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
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
        let rope = &self.rope;
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::down(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.start = final_char;
            sel.end = final_char;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Move the cursor down while modifying the selection region
    pub fn move_down_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;
        for sel in self.selections.entry(view_id).or_default() {
            let (final_char, horiz) = Self::down(rope, sel.cursor(), sel.horiz, self.tab_size);
            sel.horiz = horiz;
            sel.end = final_char;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Given a character location, return a new character location to the next
    /// left-word-boundary
    fn word_left(rope: &Rope, char_idx: usize) -> usize {
        enum State {
            Whitespace,
            Symbols,
            Letters,
        };
        let mut state = State::Whitespace;

        let mut final_char = char_idx;
        for g in RopeGraphemesRev::new(&rope.slice(0..char_idx)) {
            let mut is_letter = false;
            let mut is_space = false;
            if g.len_chars() == 1 {
                let c = g.char(0);
                is_space = c.is_whitespace();
                is_letter = c.is_alphanumeric() || c == '_';
            }
            let is_symbol = !is_space && !is_letter;

            match state {
                State::Whitespace if is_space => {}
                State::Whitespace if is_letter => state = State::Letters,
                State::Whitespace => state = State::Symbols,
                State::Symbols if is_symbol => {}
                State::Symbols => {
                    return final_char;
                }
                State::Letters if is_letter => {}
                State::Letters => {
                    return final_char;
                }
            }
            final_char -= g.len_chars();
        }
        0
    }

    /// Given a character location, return a new character location to the next
    /// right-word-boundary
    fn word_right(rope: &Rope, char_idx: usize) -> usize {
        enum State {
            Whitespace,
            Symbols,
            Letters,
        }
        let mut state = State::Whitespace;

        let mut final_char = char_idx;
        for g in RopeGraphemes::new(&rope.slice(char_idx..rope.len_chars())) {
            let mut is_letter = false;
            let mut is_space = false;
            if g.len_chars() == 1 {
                let c = g.char(0);
                is_space = c.is_whitespace();
                is_letter = c.is_alphanumeric() || c == '_';
            }
            let is_symbol = !is_space && !is_letter;

            match state {
                State::Whitespace if is_space => {}
                State::Whitespace if is_letter => state = State::Letters,
                State::Whitespace => state = State::Symbols,
                State::Symbols if is_symbol => {}
                State::Symbols => {
                    return final_char;
                }
                State::Letters if is_letter => {}
                State::Letters => {
                    return final_char;
                }
            }
            final_char += g.len_chars();
        }
        rope.len_chars()
    }

    /// move the cursor to the left to the next word boundry
    pub fn move_word_left(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let word_right = Self::word_left(rope, sel.end);
            sel.start = word_right;
            sel.end = word_right;
            sel.horiz = None;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }
    /// move the cursor to the right to the next word boundry
    pub fn move_word_right(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let word_right = Self::word_right(rope, sel.end);
            sel.start = word_right;
            sel.end = word_right;
            sel.horiz = None;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Move the cursor left while modifying the selection region
    pub fn move_left_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            if sel.end > 0 {
                let left = prev_grapheme_boundary(rope, sel.end);
                sel.end = left;
                sel.horiz = None;
            }
        }

        self.on_selection_change();
    }

    /// Move the cursor right while modifying the selection region
    pub fn move_right_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;
        let len_chars = rope.len_chars();

        for sel in self.selections.entry(view_id).or_default() {
            if sel.end < len_chars {
                let right = next_grapheme_boundary(rope, sel.end);
                sel.end = right;
                sel.horiz = None;
            }
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// move the cursor to the left to the next word boundry while modifying
    /// the seleciton region
    pub fn move_word_left_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let word_right = Self::word_left(rope, sel.end);
            sel.end = word_right;
            sel.horiz = None;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// move the cursor to the right to the next word boundry while modifying
    /// the seleciton region
    pub fn move_word_right_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let word_right = Self::word_right(rope, sel.end);
            sel.end = word_right;
            sel.horiz = None;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_left_end_of_line(&mut self, view_id: ViewId) {
        let rope = &self.rope;
        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            let line_home = rope.line_to_char(line);
            sel.start = line_home;
            sel.end = line_home;
            sel.horiz = None;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_right_end_of_line(&mut self, view_id: ViewId) {
        let rope = &self.rope;

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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_left_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let line = rope.char_to_line(sel.cursor());
            let line_home = rope.line_to_char(line);
            sel.end = line_home;
            sel.horiz = None;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_right_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_beginning_of_document(&mut self, view_id: ViewId) {
        for sel in self.selections.entry(view_id).or_default() {
            sel.start = 0;
            sel.end = 0;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_end_of_document(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let end_of_doc = rope.len_chars();
            sel.start = end_of_doc;
            sel.end = end_of_doc;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_beginning_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            sel.end = 0;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn move_to_end_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        let rope = &self.rope;

        for sel in self.selections.entry(view_id).or_default() {
            let end_of_doc = rope.len_chars();
            sel.end = end_of_doc;
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn page_down(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_down(view_id);
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn page_up(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_up(view_id);
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn page_up_and_modify_selection(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_up_and_modify_selection(view_id);
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn page_down_and_modify_selection(&mut self, view_id: ViewId) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_down_and_modify_selection(view_id);
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Executed when a user clicks
    pub fn gesture_point_select(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        let rope = &self.rope;
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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Executed when a user shift-clicks
    pub fn gesture_range_select(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        let rope = &self.rope;
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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Executed when a user ctrl-clicks.  If a selection exists on that point,
    /// remove it.  Otherwise, add a new selection at that point.
    pub fn gesture_toggle_sel(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        let rope = &self.rope;
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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Executed when a user double-clicks
    pub fn gesture_word_select(&mut self, view_id: ViewId, line: usize, byte_idx: usize) {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        enum CharClass {
            Space,
            Letter,
            Symbol,
        }
        use CharClass::*;
        impl CharClass {
            fn from_rope(slice: RopeSlice) -> Self {
                if slice.len_chars() == 1 {
                    let c = slice.char(0);
                    if c.is_whitespace() {
                        return Space;
                    }
                    if c.is_alphanumeric() || c == '_' {
                        return Letter;
                    }
                }
                Symbol
            }
        }

        let rope = &self.rope;
        let line = min(line, rope.len_lines());
        let total_byte_idx = rope.line_to_byte(line) + byte_idx;
        let char_idx = rope.byte_to_char(total_byte_idx);

        let mut left_iter = RopeGraphemesRev::new(&rope.slice(0..char_idx));
        let mut right_iter = RopeGraphemes::new(&rope.slice(char_idx..rope.len_chars()));

        let left_char = left_iter
            .next()
            .map(|s| CharClass::from_rope(s))
            .unwrap_or(CharClass::Space);
        let right_char = right_iter
            .next()
            .map(|s| CharClass::from_rope(s))
            .unwrap_or(CharClass::Space);

        let mut word_class = Symbol;
        if left_char == Space || right_char == Space {
            word_class = Space;
        }
        if left_char == Letter || right_char == Letter {
            word_class = Letter;
        }

        let left_char_idx: usize = char_idx
            - RopeGraphemesRev::new(&rope.slice(0..char_idx))
                .take_while(|s| CharClass::from_rope(*s) == word_class)
                .map(|s| s.len_chars())
                .sum::<usize>();
        let right_char_idx: usize = char_idx
            + RopeGraphemes::new(&rope.slice(char_idx..rope.len_chars()))
                .take_while(|s| CharClass::from_rope(*s) == word_class)
                .map(|s| s.len_chars())
                .sum::<usize>();

        let sel = Selection {
            start: left_char_idx,
            end: right_char_idx,
            horiz: None,
        };
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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    /// Executed when a user triple-clicks
    pub fn gesture_line_select(&mut self, view_id: ViewId, line: usize) {
        let rope = &self.rope;
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

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn select_all(&mut self, view_id: ViewId) {
        let rope = &self.rope;
        let len_chars = rope.len_chars();
        let mut sel = Selection::new();
        sel.start = 0;
        sel.end = len_chars;
        self.selections.insert(view_id, vec![sel]);

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn replace_selections(&mut self, view_id: ViewId, sels: &[Selection]) {
        use std::collections::hash_map::Entry;
        match self.selections.entry(view_id) {
            Entry::Occupied(ref mut e) => {
                e.get_mut().clear();
                e.get_mut().extend_from_slice(sels);
            }
            Entry::Vacant(e) => {
                let mut v = Vec::new();
                v.extend_from_slice(sels);
                e.insert(v);
            }
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    pub fn undo(&mut self, view_id: ViewId) {
        if let Some((rope, sels)) = self.history.undo() {
            self.rope = rope;
            use std::collections::hash_map::Entry;
            match self.selections.entry(view_id) {
                Entry::Occupied(ref mut e) => {
                    e.get_mut().clear();
                    e.get_mut().extend_from_slice(sels);
                }
                Entry::Vacant(e) => {
                    let mut v = Vec::new();
                    v.extend_from_slice(sels);
                    e.insert(v);
                }
            }
        }

        self.fix_selections();
        self.on_text_change();
        self.scroll_to_selections(view_id);
    }

    pub fn redo(&mut self, view_id: ViewId) {
        if let Some((rope, sels)) = self.history.redo() {
            self.rope = rope;
            use std::collections::hash_map::Entry;
            match self.selections.entry(view_id) {
                Entry::Occupied(ref mut e) => {
                    e.get_mut().clear();
                    e.get_mut().extend_from_slice(sels);
                }
                Entry::Vacant(e) => {
                    let mut v = Vec::new();
                    v.extend_from_slice(sels);
                    e.insert(v);
                }
            }
        }

        self.fix_selections();
        self.on_text_change();
        self.scroll_to_selections(view_id);
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
                let rope = &self.rope;
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
        let rope = &self.rope;
        let byte_idx = if line_idx >= rope.len_lines() {
            rope.len_bytes()
        } else {
            min(
                rope.line_to_byte(line_idx) + line_byte_idx,
                rope.len_bytes(),
            )
        };
        for sel in self.selections.entry(view_id).or_default() {
            sel.end = rope.byte_to_char(byte_idx);
        }

        self.on_selection_change();
        self.scroll_to_selections(view_id);
    }

    // currently the only thing this does is ensure that all selections are not
    // out of bounds
    // TODO make sure selection regions never intersect, if so, merge them
    pub fn fix_selections(&mut self) {
        let rope = &self.rope;
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
        let rope = &self.rope;
        debug_assert!(self.selections.get(&view_id).unwrap().len() > 0);
        for sel in self.selections.entry(view_id).or_default() {
            // dbg!(
            //     rope,
            //     sel.start,
            //     rope.len_chars(),
            //     prev_grapheme_boundary(rope, sel.start),
            //     next_grapheme_boundary(rope, sel.start),
            //     prev_grapheme_boundary(rope, next_grapheme_boundary(rope, sel.start))
            // );
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
        let rope = &self.rope;
        rope.len_bytes()
    }
    pub fn len_chars(&self) -> usize {
        let rope = &self.rope;
        rope.len_chars()
    }
    pub fn len_lines(&self) -> usize {
        let rope = &self.rope;
        rope.len_lines()
    }
    pub fn line(&self, line_idx: usize) -> RopeSlice {
        let rope = &self.rope;
        rope.line(line_idx)
    }
    pub fn rope_clone(&self) -> Rope {
        self.rope.clone()
    }
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        let rope = &self.rope;
        rope.char_to_line(char_idx)
    }
    pub fn line_to_char(&self, char_idx: usize) -> usize {
        let rope = &self.rope;
        rope.line_to_char(char_idx)
    }
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        let rope = &self.rope;
        rope.char_to_byte(char_idx)
    }
    pub fn line_to_byte(&self, char_idx: usize) -> usize {
        let rope = &self.rope;
        rope.line_to_byte(char_idx)
    }

    pub fn filter_line_to_display(&self, text: &str, out: &mut String) {
        out.clear();
        for ch in text.chars() {
            if ch == '\t' {
                for _ in 0..self.tab_size {
                    out.push(' ');
                }
            } else if ch != '\n' && ch != '\r' {
                out.push(ch);
            }
        }
    }

    pub fn get_line_with_attributes(
        &self,
        view_id: ViewId,
        line_idx: usize,
        theme: &Theme,
    ) -> Option<(RopeSlice, Vec<AttrSpan>)> {
        let rope = &self.rope;
        if line_idx >= rope.len_lines() {
            return None;
        }
        let mut line = rope.line(line_idx);

        // Take off the newline at the end if one exists.  When we support
        // multiple line endings, this needs to change.
        if line.len_chars() > 0 && line.char(line.len_chars() - 1) == '\n' {
            line = line.slice(0..line.len_chars() - 1);
        }

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

    pub fn save(&mut self) -> Result<(), anyhow::Error> {
        if let Some(ref path) = self.path {
            let mut file = File::create(path)?;
            let rope = &self.rope;
            rope.write_to(&mut file)?;
        } else {
            bail!("cannot save, no known file path");
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), io::Error> {
        let mut file = File::create(path)?;
        let rope = &self.rope;
        rope.write_to(&mut file)?;

        self.path = Some(path.into());
        self.msg_sender
            .lock()
            .unwrap()
            .send(Msg::PathChanged(self.id));
        Ok(())
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rope.slice(..))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_insert() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "a");
        assert_eq!(buf.to_string(), "a");
    }
    #[test]
    fn test_insert2() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "a");
        buf.insert(0, "b");
        buf.insert(0, "cd");
        assert_eq!(buf.to_string(), "abcd");
    }

    #[test]
    fn test_move_left() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "a");
        buf.insert(0, "b");
        buf.move_left(0);
        buf.insert(0, "cd");
        assert_eq!(buf.to_string(), "acdb");
    }
    #[test]
    fn test_move_left_right() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.move_left(0);
        buf.move_left(0);
        buf.move_left(0);
        buf.insert(0, "abc");
        assert_eq!(buf.to_string(), "abc");
    }
    #[test]
    fn test_move_right_too_far() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.move_right(0);
        buf.move_right(0);
        buf.move_right(0);
        buf.insert(0, "abc");
        assert_eq!(buf.to_string(), "abc");
    }

    #[test]
    fn test_move_left_and_modify_selection() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "abc\ndef");
        buf.move_left(0);
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "ab_c\ndef");
    }
    #[test]
    fn test_move_up2() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "a\nbcd");
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "a_\nbcd");
    }
    #[test]
    fn test_move_up_to_tab_0() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "\tabc");
        buf.insert_newline(0);
        buf.move_up(0);
        buf.insert(0, "_");
        assert_eq!(buf.to_string(), "_\tabc\n");
    }
    #[test]
    fn test_move_up_to_tab_4() {
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
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
        let msg_sender = Arc::new(Mutex::new(MsgSender::new()));
        let mut buf = Buffer::new(0, msg_sender);
        buf.init_view(0);
        buf.insert(0, "abc");
        buf.move_left(0);
        buf.move_down(0);
        assert_eq!(
            buf.selections.get(&0).unwrap(),
            &vec![Selection {
                start: 3,
                end: 3,
                horiz: Some(2),
            }]
        );
    }
}
