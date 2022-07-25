/// Represents a point in the buffer
#[derive(Debug, Copy, Clone)]
pub struct Point {
    /// The total byte index
    pub byte: usize,
    /// The total code point index
    pub char: usize,
    /// The line number
    pub line: usize,
    /// The number of code points from the beginning of the line
    pub col: usize,
}
