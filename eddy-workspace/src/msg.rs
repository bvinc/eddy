use crate::BufferId;

#[derive(Copy, Clone, Debug)]
pub enum Msg {
    LsInitialized,
    PathChanged(BufferId),
}

pub struct MsgSender {
    pub callback: Option<Box<dyn 'static + Sync + Send + FnMut(Msg)>>,
}

impl MsgSender {
    pub fn new() -> MsgSender {
        MsgSender { callback: None }
    }
    pub fn send(&mut self, msg: Msg) {
        if let Some(ref mut cb) = self.callback {
            cb(Msg::LsInitialized);
        }
    }
}
