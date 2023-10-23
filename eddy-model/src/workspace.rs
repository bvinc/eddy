use crate::backend::Backend;
use crate::lsp::{self, LanguageServerClient, ResultQueue};
use crate::style::{AttrSpan, Theme};
use crate::Buffer;
use anyhow::Context;
use log::debug;
use ropey::RopeSlice;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use url::Url;

pub type BufferId = usize;
pub type ViewId = usize;

// pub struct JoinHandle<R>{};

pub struct Workspace {
    pub views: BTreeMap<ViewId, BufferId>,
    buffers: BTreeMap<BufferId, Buffer>,
    pub theme: Theme,
    ls_client: Option<Arc<Mutex<LanguageServerClient>>>,
    pub dir: PathBuf,
    pub focused_view: Option<ViewId>,
    pub wakeup: Arc<dyn Fn() + Send + Sync>,
    pub backend: Backend,
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("views", &self.views)
            .field("buffers", &self.buffers)
            .field("theme", &self.theme)
            .field("ls_client", &self.ls_client)
            .field("dir", &self.dir)
            .field("focused_view", &self.focused_view)
            .finish()
    }
}

impl Workspace {
    #[allow(clippy::new_without_default)]
    pub fn new(wakeup: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            views: BTreeMap::new(),
            buffers: BTreeMap::new(),
            theme: Theme::default(),
            ls_client: None,
            dir: std::env::current_dir().expect("cwd"),
            focused_view: None,
            wakeup: wakeup.clone(),
            backend: Backend::ssh("brain", "127.0.0.1:22", None, wakeup),
        }
    }

    pub fn new_view(&mut self, path: Option<&Path>) -> Result<ViewId, anyhow::Error> {
        println!("new view");
        self.backend.list_files(
            &PathBuf::from_str("/").unwrap(),
            Box::new(|s| println!("hi {:?}", s)),
        );

        let view_id = self.views.keys().max().copied().unwrap_or_default() + 1;
        let buf_id = self.buffers.keys().max().copied().unwrap_or_default() + 1;
        self.views.insert(view_id, buf_id);
        dbg!("new view", view_id, buf_id);
        let mut buffer = if let Some(path) = path {
            let buf = Buffer::from_file(buf_id, path)?;

            dbg!(path);
            if let Some("rs") = path.extension().and_then(OsStr::to_str) {
                // let mut child = Command::new("rust-analyzer")
                //     .stdin(Stdio::piped())
                //     .stdout(Stdio::piped())
                //     .spawn()
                //     .expect("failed to execute process");
                // let stdin = child.stdin.take().expect("stdin take");

                let result_queue = ResultQueue::new();
                let ls_client = lsp::start_new_server(
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
                let root_url = Url::parse(&format!("{}{}", "file://", "/home/brain/src/eddy-gtk4"))
                    .expect("bad url");

                let document_uri = Url::from_file_path(path).expect("url from path");

                let document_text = buf.to_string();
                ls_client.lock().expect("lock lsp").send_initialize(
                    Some(root_url),
                    move |ls_client, _res| {
                        ls_client.is_initialized = true;
                        println!("sending init");
                        ls_client.send_initialized();
                        println!("sending init done");
                        ls_client.send_did_open(view_id, document_uri, document_text);
                    },
                );

                self.ls_client = Some(ls_client);
            }
            buf
        } else {
            Buffer::new(buf_id)
        };
        buffer.init_view(view_id);
        self.buffers.insert(buf_id, buffer);

        Ok(view_id)
    }

    pub fn close_view(&mut self, view_id: usize) {
        debug!("close view {}", view_id);
        self.views.remove(&view_id);
        if self.focused_view == Some(view_id) {
            self.focused_view = None;
        }
    }

    pub fn have_events(&mut self) {
        self.backend.handle_responses();
    }

    pub fn handle_events(&mut self) {
        self.backend.handle_responses();
    }

    // pub async fn list_files(&self) -> Result<Vec<String>, anyhow::Error> {
    //     for entry in tokio::fs::read_dir(self.dir).await? {
    //         let entry = entry?;
    //         dbg!(&entry);
    //         let metadata = entry.metadata()?;
    //         files.push((metadata.is_dir(), entry.file_name()));
    //     }

    //     files.sort_unstable_by_key(|(is_dir, fname)| {
    //         (!is_dir, fname.to_string_lossy().to_uppercase())
    //     });
    //     for (is_dir, fname) in files {
    //         let node = tree_store.insert_with_values(
    //             ti,
    //             None,
    //             &[(0, &fname.to_string_lossy().to_string())],
    //         );
    //         if is_dir {
    //             tree_store.insert_with_values(Some(&node), None, &[(0, &".")]);
    //         }
    //     }
    // }

    pub fn display_name(&self, view_id: usize) -> String {
        let buf_id = self.views.get(&view_id).unwrap();
        self.buffers
            .get(buf_id)
            .and_then(|b| {
                b.path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or("Untitled".to_string())
    }

    pub fn ls_initialized(&mut self) {
        if let Some(ref mut ls_client) = self.ls_client {
            let mut ls_client = ls_client.lock().expect("lsp");
            ls_client.is_initialized = true;
        }
    }

    pub fn buffer(&self, view_id: usize) -> &Buffer {
        let buf_id = self.views.get(&view_id).unwrap();
        self.buffers.get(buf_id).unwrap()
    }

    pub fn buffer_mut(&mut self, view_id: usize) -> &mut Buffer {
        let buf_id = self.views.get(&view_id).unwrap();
        self.buffers.get_mut(buf_id).unwrap()
    }

    pub fn buffer_and_theme(&self, view_id: usize) -> (&Buffer, Theme) {
        let buf_id = self.views.get(&view_id).unwrap();
        (self.buffers.get(buf_id).unwrap(), self.theme.clone())
    }

    pub fn buffer_and_theme_mut(&mut self, view_id: usize) -> (&mut Buffer, Theme) {
        let buf_id = self.views.get(&view_id).unwrap();

        (self.buffers.get_mut(buf_id).unwrap(), self.theme.clone())
    }

    pub fn has_path(&self, view_id: usize) -> bool {
        self.buffer(view_id).path.is_some()
    }

    pub fn save(&mut self, view_id: usize) -> Result<(), anyhow::Error> {
        self.buffer_mut(view_id).save()?;

        Ok(())
    }

    pub fn save_as(&mut self, view_id: usize, path: &Path) -> Result<(), anyhow::Error> {
        dbg!(path);
        self.buffer_mut(view_id).save_as(path)?;
        Ok(())
    }

    pub fn insert(&mut self, view_id: ViewId, text: &str) {
        self.buffer_mut(view_id).insert(view_id, text);
    }

    pub fn insert_newline(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).insert_newline(view_id);
    }

    pub fn insert_tab(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).insert_tab(view_id);
    }

    pub fn delete_forward(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).delete_forward(view_id);
    }

    pub fn delete_backward(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).delete_backward(view_id);
    }

    pub fn move_left(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_left(view_id);
    }

    pub fn move_right(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_right(view_id);
    }

    pub fn move_up(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_up(view_id);
    }
    pub fn move_up_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_up_and_modify_selection(view_id);
    }

    pub fn move_down(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_down(view_id);
    }

    pub fn move_down_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_down_and_modify_selection(view_id);
    }

    pub fn move_word_left(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_word_left(view_id);
    }
    pub fn move_word_right(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_word_right(view_id);
    }

    pub fn move_left_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_left_and_modify_selection(view_id);
    }

    pub fn move_right_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_right_and_modify_selection(view_id);
    }

    pub fn move_word_left_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_word_left_and_modify_selection(view_id);
    }
    pub fn move_word_right_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_word_right_and_modify_selection(view_id);
    }
    pub fn move_to_left_end_of_line(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_to_left_end_of_line(view_id);
    }
    pub fn move_to_right_end_of_line(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_to_right_end_of_line(view_id);
    }
    pub fn move_to_left_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_to_left_end_of_line_and_modify_selection(view_id);
    }
    pub fn move_to_right_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_to_right_end_of_line_and_modify_selection(view_id);
    }
    pub fn move_to_beginning_of_document(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_to_beginning_of_document(view_id);
    }

    pub fn move_to_end_of_document(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).move_to_end_of_document(view_id);
    }
    pub fn move_to_beginning_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_to_beginning_of_document_and_modify_selection(view_id);
    }
    pub fn move_to_end_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id)
            .move_to_end_of_document_and_modify_selection(view_id);
    }
    pub fn page_down(&mut self, view_id: ViewId, lines_visible: usize) {
        self.buffer_mut(view_id).page_down(view_id, lines_visible);
    }
    pub fn page_up(&mut self, view_id: ViewId, lines_visible: usize) {
        self.buffer_mut(view_id).page_up(view_id, lines_visible);
    }
    pub fn page_up_and_modify_selection(&mut self, view_id: ViewId, lines_visible: usize) {
        self.buffer_mut(view_id)
            .page_up_and_modify_selection(view_id, lines_visible);
    }
    pub fn page_down_and_modify_selection(&mut self, view_id: ViewId, lines_visible: usize) {
        self.buffer_mut(view_id)
            .page_down_and_modify_selection(view_id, lines_visible);
    }
    pub fn select_all(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).select_all(view_id);
    }
    pub fn undo(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).undo(view_id);
    }
    pub fn redo(&mut self, view_id: ViewId) {
        self.buffer_mut(view_id).redo(view_id);
    }

    pub fn cut(&mut self, view_id: ViewId) -> Option<String> {
        self.buffer_mut(view_id).cut(view_id)
    }
    pub fn copy(&mut self, view_id: ViewId) -> Option<String> {
        self.buffer_mut(view_id).copy(view_id)
    }

    pub fn gesture_point_select(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        self.buffer_mut(view_id)
            .gesture_point_select(view_id, line_idx, line_byte_idx);
    }
    pub fn drag_update(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        self.buffer_mut(view_id)
            .drag_update(view_id, line_idx, line_byte_idx);
    }
}

// mod test {
//     use super::*;
//     #[test]
//     fn test_views() {
//         let mut ws = Workspace::new();
//         let v1 = ws.new_view(None).unwrap();
//         ws.close_view(v1);
//         ws.new_view(None).unwrap();
//         dbg!(ws);
//     }
// }
