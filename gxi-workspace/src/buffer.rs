use crate::Range;
use crate::Selection;
use ropey::{str_utils::byte_to_char_idx, Rope, RopeSlice};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader};
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

pub type ViewId = u64;

#[derive(Debug)]
pub struct Buffer {
    path: Option<PathBuf>,
    rope: Rope,
    selections: HashMap<ViewId, Vec<Selection>>,
    last_view_id: ViewId,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            path: None,
            rope: Rope::new(),
            selections: HashMap::new(),
            last_view_id: 0,
        }
    }
    pub fn from_file(path: &Path) -> Result<Self, io::Error> {
        let mut rope = Rope::from_reader(BufReader::new(File::open("my_great_book.txt")?))?;

        Ok(Self {
            path: Some(path.to_owned()),
            rope,
            selections: HashMap::new(),
            last_view_id: 0,
        })
    }

    pub fn new_view(&mut self) -> ViewId {
        self.last_view_id += 1;
        return self.last_view_id;
    }

    pub fn remove(&mut self, char_range: Range) {
        debug_assert!(char_range.start <= char_range.end);

        if char_range.start == char_range.end {
            return;
        }

        self.rope.remove(char_range);

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

    pub fn insert(&mut self, view_id: ViewId, text: &str) {
        for sel in self.selections.entry(view_id).or_default() {
            self.remove(sel.range());
        }
        for sel in self.selections.entry(view_id).or_default() {
            self.insert_at(sel.cursor(), text);
        }
    }

    pub fn insert_at(&mut self, char_idx: usize, text: &str) {
        self.rope.insert(char_idx, text);
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

    pub fn delete_forward(&mut self, view_id: ViewId) {
        for sel in self.selections.entry(view_id).or_default() {
            let len_chars = self.len_chars();
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

    pub fn delete_backward(&mut self, view_id: ViewId) {
        // Delete all selection regions
        for sel in self.selections.entry(view_id).or_default() {
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

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }
    pub fn line(&self, line_idx: usize) -> RopeSlice {
        self.rope.line(line_idx)
    }
    pub fn rope_clone(&self) -> Rope {
        self.rope.clone()
    }
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }
    pub fn line_to_char(&self, char_idx: usize) -> usize {
        self.rope.line_to_char(char_idx)
    }

    /// Finds the previous grapheme boundary before the given char position.
    fn prev_grapheme_boundary(&self, char_idx: usize) -> usize {
        let slice = &self.rope;

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
        let slice = &self.rope;

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

impl std::fmt::Display for Buffer {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.rope)?;
        Ok(())
    }
}
