use crate::BufferId;

#[derive(Copy, Clone, Debug)]
pub enum BufferUpdate {
    LsInitialized,
    PathChanged(BufferId),
    PristineChanged(BufferId),
}

pub struct BufferUpdateSender {
    pub callback: Option<Box<dyn 'static + Send + FnMut(BufferUpdate)>>,
}

impl BufferUpdateSender {
    pub fn new() -> BufferUpdateSender {
        BufferUpdateSender { callback: None }
    }
    pub fn send(&mut self, msg: BufferUpdate) {
        if let Some(ref mut cb) = self.callback {
            cb(msg);
        }
    }
}
