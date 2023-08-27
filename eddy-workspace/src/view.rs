use crate::Buffer;
use crate::Range;
use crate::Selection;
use crate::ViewID;
use log::*;
use ropey::Rope;
use std::cell::RefCell;
use std::cmp::{max, min, Ordering};
use std::collections::BTreeMap;
use std::ops::RangeBounds;
use std::rc::Rc;

#[derive(Debug)]
pub struct View {
    id: ViewID,
    buffer: Buffer,
    // pub selections: Vec<Selection>,
}

impl View {
    pub fn new(buffer: Rc<RefCell<Buffer>>) -> Self {
        let id = buffer.borrow_mut().new_view();
        Self {
            id,
            buffer: buffer.clone(),
            selections: vec![Selection::new()],
        }
    }

    pub fn insert(&mut self, text: &str) {
        // for i in 0..self.selections.len() {
        //     let sel = self.selections[i].clone();
        //     self.remove_range(sel.range());
        // }
        // for i in 0..self.selections.len() {
        //     let sel = self.selections[i].clone();
        //     self.insert_at(sel.cursor(), text);
        // }
        self.buffer.borrow_mut().insert(self.id, text)
    }

    pub fn insert_newline(&mut self) {
        self.insert("\n")
    }

    pub fn insert_tab(&mut self) {
        self.insert("\t")
    }

    pub fn delete_forward(&mut self) {
        // Delete all selection regions
        for i in 0..self.selections.len() {
            let sel = self.selections[i].clone();
            let len_chars = self.buffer.borrow().len_chars();
            if sel.is_caret() {
                if sel.cursor() < len_chars {
                    // Remove the character in front of the cursor
                    self.remove_range(Range {
                        start: sel.cursor(),
                        end: sel.cursor() + 1,
                    });
                }
            } else {
                // Just remove the selection
                self.remove_range(sel.range());
            }
        }
    }
    pub fn delete_backward(&mut self) {
        // Delete all selection regions
        for i in 0..self.selections.len() {
            let sel = self.selections[i].clone();
            if sel.is_caret() {
                if sel.cursor() != 0 {
                    // Remove the character before the cursor
                    self.remove_range(Range {
                        start: sel.cursor() - 1,
                        end: sel.cursor(),
                    });
                }
            } else {
                self.remove_range(sel.range());
            }
        }
    }

    pub fn move_left(&mut self) {
        for sel in &mut self.selections {
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

    pub fn move_right(&mut self) {
        let len_chars = self.buffer.borrow().len_chars();
        for sel in &mut self.selections {
            if sel.start != sel.end {
                // collapse selection to the right
                sel.start = sel.end
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

    fn up(buffer: Rc<RefCell<Buffer>>, char_idx: usize) -> (usize, Option<usize>) {
        let buffer = buffer.borrow();
        let line = buffer.char_to_line(char_idx);
        let line_home = buffer.line_to_char(line);
        let x_diff = char_idx - line_home;
        let prev_line = line.saturating_sub(1);
        let prev_line_home = buffer.line_to_char(prev_line);
        let prev_line_end = line_home.saturating_sub(1);
        let final_char = min(prev_line_end, prev_line_home + x_diff);
        (final_char, Some(x_diff))
    }
    pub fn move_up(&mut self) {
        for sel in &mut self.selections {
            let (final_char, horiz) = Self::up(self.buffer.clone(), sel.cursor());
            sel.horiz = horiz;
            sel.start = final_char;
            sel.end = final_char;
        }
    }
    pub fn move_up_and_modify_selection(&mut self) {
        for sel in &mut self.selections {
            let (final_char, horiz) = Self::up(self.buffer.clone(), sel.cursor());
            sel.horiz = horiz;
            sel.end = final_char;
        }
    }

    pub fn move_down(&mut self) {
        let len_lines = self.buffer.borrow().len_lines();
        let len_chars = self.buffer.borrow().len_chars();
        for sel in &mut self.selections {
            let line = self.buffer.borrow().char_to_line(sel.cursor());
            let line_home = self.buffer.borrow().line_to_char(line);
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
            let next_line_home = self.buffer.borrow().line_to_char(next_line);
            let next_line_end = if next_line == len_lines - 1 {
                // There's no line after next, so the end is the last char of
                // the buffer
                len_chars
            } else {
                self.buffer.borrow().line_to_char(next_line + 1) - 1
            };
            // char_want is the ideal location
            let char_want = next_line_home + x_diff;
            let final_char = min(next_line_end, max(next_line_home, char_want));
            sel.start = final_char;
            sel.end = final_char;
            sel.horiz = Some(x_diff);
        }
    }

    pub fn move_down_and_modify_selection(&mut self) {
        let len_lines = self.buffer.borrow().len_lines();
        let len_chars = self.buffer.borrow().len_chars();
        for sel in &mut self.selections {
            let line = self.buffer.borrow().char_to_line(sel.cursor());
            let line_home = self.buffer.borrow().line_to_char(line);
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
            let next_line_home = self.buffer.borrow().line_to_char(next_line);
            let next_line_end = if next_line == len_lines - 1 {
                // There's no line after next, so the end is the last char of
                // the buffer
                len_chars
            } else {
                self.buffer.borrow().line_to_char(next_line + 1) - 1
            };
            // char_want is the ideal location
            let char_want = next_line_home + x_diff;
            let final_char = min(next_line_end, max(next_line_home, char_want));
            sel.end = final_char;
            sel.horiz = Some(x_diff);
        }
    }

    pub fn move_word_left(&mut self) {}
    pub fn move_word_right(&mut self) {}

    pub fn move_left_and_modify_selection(&mut self) {
        for sel in &mut self.selections {
            if sel.end > 0 {
                sel.end -= 1;
            }
        }
    }

    pub fn move_right_and_modify_selection(&mut self) {
        let len_chars = self.buffer.borrow().len_chars();

        for sel in &mut self.selections {
            if sel.end < len_chars {
                sel.end += 1;
            }
        }
    }

    pub fn move_word_left_and_modify_selection(&mut self) {}
    pub fn move_word_right_and_modify_selection(&mut self) {}
    pub fn move_to_left_end_of_line(&mut self) {
        for sel in &mut self.selections {
            let line = self.buffer.borrow().char_to_line(sel.cursor());
            let line_home = self.buffer.borrow().line_to_char(line);
            sel.start = line_home;
            sel.end = line_home;
            sel.horiz = None;
        }
    }
    pub fn move_to_right_end_of_line(&mut self) {}
    pub fn move_to_left_end_of_line_and_modify_selection(&mut self) {}
    pub fn move_to_right_end_of_line_and_modify_selection(&mut self) {}
    pub fn move_to_beginning_of_document(&mut self) {
        for sel in &mut self.selections {
            sel.start = 0;
            sel.end = 0;
        }
    }

    pub fn move_to_end_of_document(&mut self) {
        for sel in &mut self.selections {
            let end_of_doc = self.buffer.borrow().len_chars();
            sel.start = end_of_doc;
            sel.end = end_of_doc;
        }
    }
    pub fn move_to_beginning_of_document_and_modify_selection(&mut self) {
        for sel in &mut self.selections {
            let end_of_doc = self.buffer.borrow().len_chars();
            sel.end = end_of_doc;
        }
    }
    pub fn move_to_end_of_document_and_modify_selection(&mut self) {
        for sel in &mut self.selections {
            let end_of_doc = self.buffer.borrow().len_chars();
            sel.end = end_of_doc;
        }
    }
    pub fn page_down(&mut self) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_down();
        }
    }
    pub fn page_up(&mut self) {
        // TODO base on visible lines
        for _ in 0..10 {
            self.move_up();
        }
    }
    pub fn page_up_and_modify_selection(&mut self) {}
    pub fn page_down_and_modify_selection(&mut self) {}
    pub fn select_all(&mut self) {}
    pub fn undo(&mut self) {}
    pub fn redo(&mut self) {}

    // removes the given range of text, adjusting selections accordingly.
    fn remove_range(&mut self, char_range: Range) {
        debug_assert!(char_range.start <= char_range.end);

        if char_range.start == char_range.end {
            return;
        }

        let size = char_range.end - char_range.start;
        self.buffer.borrow_mut().remove(char_range);
        for sel in &mut self.selections {
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

    // inserts text to the given index, adjusting selections accordingly.
    fn insert_at(&mut self, char_idx: usize, text: &str) {
        let size = text.chars().count();
        self.buffer.borrow_mut().insert(char_idx, text);
        for sel in &mut self.selections {
            if sel.start >= char_idx {
                sel.start += size;
            }
            if sel.end >= char_idx {
                sel.end += size;
            }
        }
    }

    pub fn rope_clone(&self) -> Rope {
        self.buffer.borrow().rope_clone()
    }
    pub fn len_chars(&self) -> usize {
        self.buffer.borrow().len_chars()
    }
    pub fn len_lines(&self) -> usize {
        self.buffer.borrow().len_lines()
    }
}

impl std::fmt::Display for View {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", *self.buffer.borrow())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut view = View::new();
        view.insert("a");
        assert_eq!(view.to_string(), "a");
    }
    #[test]
    fn test_insert2() {
        let mut view = View::new();
        view.insert("a");
        view.insert("b");
        view.insert("cd");
        assert_eq!(view.to_string(), "abcd");
    }

    #[test]
    fn test_move_left() {
        let mut view = View::new();
        view.insert("a");
        view.insert("b");
        view.move_left();
        view.insert("cd");
        assert_eq!(view.to_string(), "acdb");
    }
    #[test]
    fn test_move_left_right() {
        let mut view = View::new();
        view.insert("a");
        view.insert("b");
        view.move_left();
        view.move_right();
        view.insert("cd");
        assert_eq!(view.to_string(), "abcd");
    }

    #[test]
    fn test_move_left_too_far() {
        let mut view = View::new();
        view.move_left();
        view.move_left();
        view.move_left();
        view.insert("abc");
        assert_eq!(view.to_string(), "abc");
    }
    #[test]
    fn test_move_right_too_far() {
        let mut view = View::new();
        view.move_right();
        view.move_right();
        view.move_right();
        view.insert("abc");
        assert_eq!(view.to_string(), "abc");
    }

    #[test]
    fn test_move_left_and_modify_selection() {
        let mut view = View::new();
        view.insert("abc");
        view.move_left_and_modify_selection();
        view.move_left_and_modify_selection();
        view.insert("de");
        assert_eq!(view.to_string(), "ade");
        view.move_left_and_modify_selection();
        view.insert("f");
        assert_eq!(view.to_string(), "adf");
    }
    #[test]
    fn test_move_right_and_modify_selection() {
        let mut view = View::new();
        view.insert("abc");
        view.move_left();
        view.move_left();
        view.move_right_and_modify_selection();
        view.move_right_and_modify_selection();
        view.insert("de");
        assert_eq!(view.to_string(), "ade");
    }
    #[test]
    fn test_move_up() {
        let mut view = View::new();
        view.insert("abc\ndef");
        view.move_left();
        view.move_up();
        view.insert("ge");
        assert_eq!(view.to_string(), "abgec\ndef");
    }
    #[test]
    fn test_move_up2() {
        let mut view = View::new();
        view.insert("a\nbcd");
        view.move_up();
        view.insert("ge");
        assert_eq!(view.to_string(), "age\nbcd");
    }
    #[test]
    fn test_move_up_to_tab_0() {
        let mut view = View::new();
        view.insert("\tabc");
        view.insert_newline();
        view.move_up();
        view.insert("de");
        assert_eq!(view.to_string(), "de\tabc");
    }
    #[test]
    fn test_move_up_to_tab_4() {
        let mut view = View::new();
        view.insert("\tabc");
        view.insert_newline();
        view.insert("    ");
        view.move_up();
        view.insert("de");
        assert_eq!(view.to_string(), "de\tabc");
    }
    #[test]
    fn test_move_up_to_tab_8() {
        let mut view = View::new();
        view.insert("\tabc");
        view.insert_newline();
        view.insert("        ");
        view.move_up();
        view.insert("de");
        assert_eq!(view.to_string(), "\tdeabc");
    }
    #[test]
    fn test_move_up_to_tab_9() {
        let mut view = View::new();
        view.insert("\tabc");
        view.insert_newline();
        view.insert("         ");
        view.move_up();
        view.insert("de");
        assert_eq!(view.to_string(), "\tadebc");
    }
    #[test]
    fn test_move_down() {
        let mut view = View::new();
        view.insert("abc\ndef");
        view.move_left();
        view.move_up();
        view.move_down();
        assert_eq!(
            view.selections,
            vec![Selection {
                start: 6,
                end: 6,
                horiz: Some(2),
            }]
        );
    }
    #[test]
    fn test_move_down2() {
        let mut view = View::new();
        view.insert("abc\nd");
        view.move_left();
        view.move_left();
        view.move_down();
        assert_eq!(
            view.selections,
            vec![Selection {
                start: 5,
                end: 5,
                horiz: Some(3),
            }]
        );
    }
    #[test]
    fn test_move_down3() {
        let mut view = View::new();
        view.insert("abc");
        view.move_left();
        view.move_down();
        assert_eq!(
            view.selections,
            vec![Selection {
                start: 3,
                end: 3,
                horiz: None,
            }]
        );
    }
}
