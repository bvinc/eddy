use eddy_ts::{Node, TextProvider};
use ropey::Rope;

pub struct RopeTextProvider<'a> {
    rope: &'a Rope,
}

impl<'a> RopeTextProvider<'a> {
    pub fn new(rope: &'a Rope) -> Self {
        Self { rope }
    }
}

impl<'a> TextProvider<'a> for RopeTextProvider<'a> {
    type I = BoundedRopeChunkIter<'a>;
    fn text(&mut self, node: Node) -> Self::I {
        let (chunks, byte_index, _, _) = self.rope.chunks_at_byte(node.byte_range().start);
        BoundedRopeChunkIter::new(chunks, byte_index, node.byte_range().len())
    }
}

pub struct BoundedRopeChunkIter<'a> {
    chunks: ropey::iter::Chunks<'a>,
    first_byte_index: usize,
    length: usize,
    first: bool,
}

impl<'a> BoundedRopeChunkIter<'a> {
    pub fn new(chunks: ropey::iter::Chunks<'a>, first_byte_index: usize, length: usize) -> Self {
        Self {
            chunks,
            first_byte_index,
            length,
            first: true,
        }
    }
}

impl<'a> ::std::iter::Iterator for BoundedRopeChunkIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().and_then(|c| {
            let mut byte_start = 0;
            if self.first {
                byte_start = self.first_byte_index;
                self.first = false;
            }
            if byte_start > c.len() {
                return None;
            }

            let mut byte_end = c.len();
            if self.length < c.len() - byte_start {
                byte_end = byte_start + self.length;
            }
            self.length -= byte_end - byte_start;

            Some(&c.as_bytes()[byte_start..byte_end])
        })
    }
}
