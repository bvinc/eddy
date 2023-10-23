use super::Color;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Attr {
    ForegroundColor(Color),
    BackgroundColor(Color),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttrSpan {
    pub attr: Attr,
    /// in bytes
    pub start_idx: usize,
    /// in bytes.  The character at this index is not included
    pub end_idx: usize,
}
