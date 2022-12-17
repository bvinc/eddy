use crate::{BufferId, Event};

#[derive(Copy, Clone, Debug)]
pub enum BufferUpdate {
    LsInitialized,
    PathChanged(BufferId),
    PristineChanged(BufferId),
}

pub struct EventSender {
    pub callback: Option<Box<dyn 'static + Send + FnMut(Event)>>,
}

impl EventSender {
    pub fn new() -> Self {
        Self { callback: None }
    }
    pub fn send(&mut self, msg: Event) {
        if let Some(ref mut cb) = self.callback {
            cb(msg);
        }
    }
}
