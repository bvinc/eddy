use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevTrack<T> {
    data: T,
    revision: usize,
}

impl<T> RevTrack<T> {
    pub fn new(data: T) -> Self {
        Self { data, revision: 0 }
    }

    pub fn revision(&self) -> usize {
        self.revision
    }

    pub fn get(&self) -> &T {
        &self.data
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.revision += 1;
        &mut self.data
    }
}

impl<T: Debug> RevTrack<T> {
    pub fn debug(&self) -> String {
        format!("{:?}", self.data)
    }
}
