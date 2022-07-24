/// This represents a point in the buffer, containing a byte index, a line, and
/// a column, which is defined as the number of code points from the beginning
/// of the line
#[derive(Debug, Copy, Clone)]
pub struct Point {
    pub byte: usize,
    pub line: usize,
    pub col: usize,
}
