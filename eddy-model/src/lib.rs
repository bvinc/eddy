#![allow(dead_code)]
#![allow(unused_imports)]
#![warn(missing_debug_implementations, rust_2018_idioms)]

mod backend;
mod buffer;
pub mod files;
pub(crate) mod graphemes;
mod history;
mod language;
mod line_ending;
mod lsp;
mod point;
mod project;
mod range;
mod selection;
pub mod style;
mod tab_mode;
mod window;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

pub use buffer::*;
pub use point::*;
pub use range::*;
pub use selection::*;
use style::Theme;
pub use window::*;

#[derive(Debug)]
pub enum Command {}

pub struct Model {
    next_win_id: u64,
    pub wins: BTreeMap<u64, Window>,
    pub theme: Theme,
    wakeup: Arc<dyn Fn() + Send + Sync>,
}

impl fmt::Debug for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Model")
            .field("next_win_id", &self.next_win_id)
            .field("wins", &self.wins)
            .field("theme", &self.theme)
            .finish()
    }
}

impl Model {
    #[allow(clippy::new_without_default)]
    pub fn new(wakeup: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            next_win_id: 0,
            wins: BTreeMap::new(),
            theme: Theme::default(),
            wakeup: wakeup.clone(),
        }
    }

    pub fn new_win(&mut self) -> u64 {
        let win_id = self.next_win_id;
        self.next_win_id += 1;
        let win = Window::new(self.wakeup.clone());
        self.wins.insert(win_id, win);
        win_id
    }

    pub fn has_events(&self) -> bool {
        self.wins.values().any(|w| w.has_events())
    }

    pub fn handle_events(&mut self) {
        for win in self.wins.values_mut() {
            win.handle_events();
        }
    }
}
