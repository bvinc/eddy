// Range is basically just like the std `Range<usize>` except its specifically
// for this purpose and not an iterator, so I can implement `Copy`.
#[derive(Debug, Copy, Clone)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl std::ops::RangeBounds<usize> for Range {
    fn start_bound(&self) -> std::ops::Bound<&usize> {
        std::ops::Bound::Included(&self.start)
    }
    fn end_bound(&self) -> std::ops::Bound<&usize> {
        std::ops::Bound::Excluded(&self.end)
    }
}
