use std::fmt::Debug;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rev<T> {
    data: T,
    rev: usize,
}

impl<T> Rev<T> {
    pub fn new(data: T) -> Self {
        Self { data, rev: 0 }
    }

    pub fn rev(&self) -> usize {
        self.rev
    }

    pub fn get(&self) -> &T {
        &self.data
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.rev += 1;
        &mut self.data
    }

    pub fn set(&mut self, data: T) {
        self.rev += 1;
        self.data = data;
    }
}
