#![allow(dead_code)]
#![allow(unused_imports)]
#![warn(missing_debug_implementations, rust_2018_idioms)]

mod buffer;
pub(crate) mod graphemes;
mod history;
mod language;
mod line_ending;
mod lsp;
mod msg;
mod point;
mod range;
mod selection;
pub mod style;
mod tab_mode;
mod workspace;

use std::path::PathBuf;

pub use buffer::*;
pub use msg::*;
pub use point::*;
pub use range::*;
pub use selection::*;
pub use workspace::*;

#[derive(Clone, Debug)]
pub enum Event {
    NewView { view_id: ViewId },
    BufferChange { buffer_id: usize },
    ScrollToCarets { buffer_id: usize },
    BufferUpdate(BufferUpdate),
}
pub enum Command {}
