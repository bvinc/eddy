use crate::lsp::{self, LanguageServerClient, ResultQueue};
use crate::style::AttrSpan;
use crate::style::Theme;
use crate::Buffer;
use crate::{Msg, MsgSender};
use anyhow::Context;
use ropey::RopeSlice;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use url::Url;

pub type BufferId = usize;
pub type ViewId = usize;

pub struct Callbacks {
    pub ls_initialized: Box<dyn 'static + Sync + Send + FnMut(Msg)>,
}
// pub struct Callbacks<CB: 'static + Send + Fn()> {
//     ls_initialized: CB,
// }

pub trait SpawnLocal {
    fn spawn_local<F: Future<Output = ()> + 'static>(&self, f: F);
}

pub struct Workspace {
    next_view_id: ViewId,
    next_buf_id: BufferId,
    views: HashMap<ViewId, BufferId>,
    buffers: HashMap<BufferId, Rc<RefCell<Buffer>>>,
    pub theme: Theme,
    ls_client: Option<Arc<Mutex<LanguageServerClient>>>,
    msg_sender: Arc<Mutex<MsgSender>>,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            next_view_id: 0,
            next_buf_id: 0,
            views: HashMap::new(),
            buffers: HashMap::new(),
            theme: Theme::new(),
            ls_client: None,
            msg_sender: Arc::new(Mutex::new(MsgSender::new())),
        }
    }

    pub fn set_callback(&mut self, c: impl FnMut(Msg) + 'static + Sync + Send) {
        self.msg_sender.lock().unwrap().callback = Some(Box::new(c));
    }

    pub fn new_view(&mut self, path: Option<&Path>) -> Result<ViewId, io::Error> {
        let view_id = self.next_view_id;
        self.next_view_id += 1;
        let buf_id = self.next_buf_id;
        self.next_buf_id += 1;
        self.views.insert(view_id, buf_id);
        let mut buffer = if let Some(path) = path {
            let buf = Buffer::from_file(buf_id, path, self.msg_sender.clone())?;

            dbg!(path);
            if let Some("rs") = path.extension().and_then(OsStr::to_str) {
                // let mut child = Command::new("rust-analyzer")
                //     .stdin(Stdio::piped())
                //     .stdout(Stdio::piped())
                //     .spawn()
                //     .expect("failed to execute process");
                // let stdin = child.stdin.take().expect("stdin take");

                let result_queue = ResultQueue::new();
                let mut ls_client = lsp::start_new_server(
                    "rust-analyzer".to_string(),
                    vec![],
                    vec!["rs".into()],
                    "rust",
                    result_queue,
                )
                .expect("lsp");
                // let mut ls_client = LanguageServerClient::new(
                //     Box::new(stdin),
                //     result_queue,
                //     "rust".into(),
                //     vec!["rs".into()],
                // );
                let root_url = Url::parse(&format!("{}{}", "file://", "/home/brain/src/donut"))
                    .expect("bad url");

                let document_uri = Url::from_file_path(path).expect("url from path");

                // let cb = self.callback.clone();
                let msg_sender = self.msg_sender.clone();
                let document_text = buf.to_string();
                ls_client.lock().expect("lock lsp").send_initialize(
                    Some(root_url),
                    move |ls_client, res| {
                        ls_client.is_initialized = true;
                        println!("sending init");
                        ls_client.send_initialized();
                        println!("sending init done");
                        msg_sender.lock().unwrap().send(Msg::LsInitialized);
                        ls_client.send_did_open(view_id, document_uri, document_text);
                    },
                );

                self.ls_client = Some(ls_client);
            }
            buf
        } else {
            Buffer::new(buf_id, self.msg_sender.clone())
        };
        buffer.init_view(view_id);
        self.buffers.insert(buf_id, Rc::new(RefCell::new(buffer)));

        Ok(view_id)
    }

    pub fn ls_initialized(&mut self) {
        if let Some(ref mut ls_client) = self.ls_client {
            let mut ls_client = ls_client.lock().expect("lsp");
            ls_client.is_initialized = true;
        }
    }

    pub fn buffer(&mut self, view_id: usize) -> Rc<RefCell<Buffer>> {
        self.buffers.get_mut(&view_id).unwrap().clone()
    }

    pub fn buffer_and_theme(&mut self, view_id: usize) -> (Rc<RefCell<Buffer>>, &Theme) {
        (self.buffers.get_mut(&view_id).unwrap().clone(), &self.theme)
    }

    pub fn save(&mut self, view_id: usize) -> Result<(), anyhow::Error> {
        self.buffer(view_id).borrow_mut().save()?;

        Ok(())
    }

    pub fn save_as(&mut self, view_id: usize, path: &Path) -> Result<(), anyhow::Error> {
        dbg!(path);
        self.buffer(view_id).borrow_mut().save_as(path)?;
        Ok(())
    }

    pub fn insert(&mut self, view_id: ViewId, text: &str) {
        self.buffer(view_id).borrow_mut().insert(view_id, text);
    }

    pub fn insert_newline(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().insert_newline(view_id);
    }

    pub fn insert_tab(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().insert_tab(view_id);
    }

    pub fn delete_forward(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().delete_forward(view_id);
    }

    pub fn delete_backward(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().delete_backward(view_id);
    }

    pub fn move_left(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().move_left(view_id);
    }

    pub fn move_right(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().move_right(view_id);
    }

    pub fn move_up(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().move_up(view_id);
    }
    pub fn move_up_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_up_and_modify_selection(view_id);
    }

    pub fn move_down(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().move_down(view_id);
    }

    pub fn move_down_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_down_and_modify_selection(view_id);
    }

    pub fn move_word_left(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().move_word_left(view_id);
    }
    pub fn move_word_right(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().move_word_right(view_id);
    }

    pub fn move_left_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_left_and_modify_selection(view_id);
    }

    pub fn move_right_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_right_and_modify_selection(view_id);
    }

    pub fn move_word_left_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_word_left_and_modify_selection(view_id);
    }
    pub fn move_word_right_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_word_right_and_modify_selection(view_id);
    }
    pub fn move_to_left_end_of_line(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_left_end_of_line(view_id);
    }
    pub fn move_to_right_end_of_line(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_right_end_of_line(view_id);
    }
    pub fn move_to_left_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_left_end_of_line_and_modify_selection(view_id);
    }
    pub fn move_to_right_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_right_end_of_line_and_modify_selection(view_id);
    }
    pub fn move_to_beginning_of_document(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_beginning_of_document(view_id);
    }

    pub fn move_to_end_of_document(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_end_of_document(view_id);
    }
    pub fn move_to_beginning_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_beginning_of_document_and_modify_selection(view_id);
    }
    pub fn move_to_end_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .move_to_end_of_document_and_modify_selection(view_id);
    }
    pub fn page_down(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().page_down(view_id);
    }
    pub fn page_up(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().page_up(view_id);
    }
    pub fn page_up_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .page_up_and_modify_selection(view_id);
    }
    pub fn page_down_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .borrow_mut()
            .page_down_and_modify_selection(view_id);
    }
    pub fn select_all(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().select_all(view_id);
    }
    pub fn undo(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().undo(view_id);
    }
    pub fn redo(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().redo(view_id);
    }

    pub fn cut(&mut self, view_id: ViewId) -> Option<String> {
        self.buffer(view_id).borrow_mut().cut(view_id)
    }
    pub fn copy(&mut self, view_id: ViewId) -> Option<String> {
        self.buffer(view_id).borrow_mut().copy(view_id)
    }
    pub fn paste(&mut self, view_id: ViewId) {
        self.buffer(view_id).borrow_mut().paste(view_id);
    }

    pub fn gesture_point_select(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        self.buffer(view_id)
            .borrow_mut()
            .gesture_point_select(view_id, line_idx, line_byte_idx);
    }
    pub fn drag(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        self.buffer(view_id)
            .borrow_mut()
            .drag(view_id, line_idx, line_byte_idx);
    }
}
