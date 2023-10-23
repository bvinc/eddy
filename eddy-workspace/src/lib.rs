#![allow(dead_code)]
#![allow(unused_imports)]
#![warn(missing_debug_implementations, rust_2018_idioms)]

mod backend;
mod buffer;
pub(crate) mod graphemes;
mod history;
mod language;
mod line_ending;
mod lsp;
mod point;
mod range;
mod selection;
pub mod style;
mod tab_mode;
mod workspace;

use std::path::PathBuf;

pub use buffer::*;
pub use point::*;
pub use range::*;
pub use selection::*;
pub use workspace::*;

#[derive(Debug)]
pub enum Command {}
