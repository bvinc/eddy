use crate::Range;
use std::cmp::{max, min};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selection {
    /// start of a selection region, in character indexes
    pub start: usize,
    /// end of a selection region, in character indexes.  This is where the cursor is.
    pub end: usize,
    /// saved horizontal position after up/down
    pub horiz: Option<usize>,
}

impl Selection {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn left(&self) -> usize {
        min(self.start, self.end)
    }
    pub fn right(&self) -> usize {
        max(self.start, self.end)
    }
    pub fn range(&self) -> Range {
        if self.start <= self.end {
            Range {
                start: self.start,
                end: self.end,
            }
        } else {
            Range {
                start: self.end,
                end: self.start,
            }
        }
    }
    pub fn is_caret(&self) -> bool {
        self.start == self.end
    }
    pub fn cursor(&self) -> usize {
        self.end
    }
}
