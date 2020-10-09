use crate::style::AttrSpan;
use crate::style::Theme;
use crate::Buffer;
use ropey::RopeSlice;
use std::collections::HashMap;
use std::io;
use std::path::Path;

pub type BufferId = usize;
pub type ViewId = usize;

pub struct Workspace {
    next_view_id: ViewId,
    next_buf_id: BufferId,
    views: HashMap<ViewId, BufferId>,
    buffers: HashMap<BufferId, Buffer>,
    pub theme: Theme,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            next_view_id: 0,
            next_buf_id: 0,
            views: HashMap::new(),
            buffers: HashMap::new(),
            theme: Theme::new(),
        }
    }

    pub fn new_view(&mut self, path: Option<&Path>) -> Result<ViewId, io::Error> {
        let view_id = self.next_view_id;
        self.next_view_id += 1;
        let buf_id = self.next_buf_id;
        self.next_buf_id += 1;
        self.views.insert(view_id, buf_id);
        let mut buffer = if let Some(path) = path {
            Buffer::from_file(path)?
        } else {
            Buffer::new()
        };
        buffer.init_view(view_id);
        self.buffers.insert(buf_id, buffer);

        Ok(view_id)
    }

    pub fn buffer(&mut self, view_id: usize) -> &mut Buffer {
        self.buffers.get_mut(&view_id).unwrap()
    }

    pub fn buffer_and_theme(&mut self, view_id: usize) -> (&mut Buffer, &Theme) {
        (self.buffers.get_mut(&view_id).unwrap(), &self.theme)
    }

    pub fn save(&mut self, view_id: usize, path: &Path) {}

    pub fn insert(&mut self, view_id: ViewId, text: &str) {
        self.buffer(view_id).insert(view_id, text);
    }

    pub fn insert_newline(&mut self, view_id: ViewId) {
        self.buffer(view_id).insert_newline(view_id);
    }

    pub fn insert_tab(&mut self, view_id: ViewId) {
        self.buffer(view_id).insert_tab(view_id);
    }

    pub fn delete_forward(&mut self, view_id: ViewId) {
        self.buffer(view_id).delete_forward(view_id);
    }

    pub fn delete_backward(&mut self, view_id: ViewId) {
        self.buffer(view_id).delete_backward(view_id);
    }

    pub fn move_left(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_left(view_id);
    }

    pub fn move_right(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_right(view_id);
    }

    pub fn move_up(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_up(view_id);
    }
    pub fn move_up_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_up_and_modify_selection(view_id);
    }

    pub fn move_down(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_down(view_id);
    }

    pub fn move_down_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_down_and_modify_selection(view_id);
    }

    pub fn move_word_left(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_word_left(view_id);
    }
    pub fn move_word_right(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_word_right(view_id);
    }

    pub fn move_left_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_left_and_modify_selection(view_id);
    }

    pub fn move_right_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_right_and_modify_selection(view_id);
    }

    pub fn move_word_left_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_word_left_and_modify_selection(view_id);
    }
    pub fn move_word_right_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_word_right_and_modify_selection(view_id);
    }
    pub fn move_to_left_end_of_line(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_to_left_end_of_line(view_id);
    }
    pub fn move_to_right_end_of_line(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_to_right_end_of_line(view_id);
    }
    pub fn move_to_left_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_to_left_end_of_line_and_modify_selection(view_id);
    }
    pub fn move_to_right_end_of_line_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_to_right_end_of_line_and_modify_selection(view_id);
    }
    pub fn move_to_beginning_of_document(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_to_beginning_of_document(view_id);
    }

    pub fn move_to_end_of_document(&mut self, view_id: ViewId) {
        self.buffer(view_id).move_to_end_of_document(view_id);
    }
    pub fn move_to_beginning_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_to_beginning_of_document_and_modify_selection(view_id);
    }
    pub fn move_to_end_of_document_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id)
            .move_to_end_of_document_and_modify_selection(view_id);
    }
    pub fn page_down(&mut self, view_id: ViewId) {
        self.buffer(view_id).page_down(view_id);
    }
    pub fn page_up(&mut self, view_id: ViewId) {
        self.buffer(view_id).page_up(view_id);
    }
    pub fn page_up_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id).page_up_and_modify_selection(view_id);
    }
    pub fn page_down_and_modify_selection(&mut self, view_id: ViewId) {
        self.buffer(view_id).page_down_and_modify_selection(view_id);
    }
    pub fn select_all(&mut self, view_id: ViewId) {
        self.buffer(view_id).select_all(view_id);
    }
    pub fn undo(&mut self, view_id: ViewId) {
        self.buffer(view_id).undo(view_id);
    }
    pub fn redo(&mut self, view_id: ViewId) {
        self.buffer(view_id).redo(view_id);
    }

    pub fn cut(&mut self, view_id: ViewId) -> Option<String> {
        self.buffer(view_id).cut(view_id)
    }
    pub fn copy(&mut self, view_id: ViewId) -> Option<String> {
        self.buffer(view_id).copy(view_id)
    }
    pub fn paste(&mut self, view_id: ViewId) {
        self.buffer(view_id).paste(view_id);
    }

    pub fn gesture_point_select(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        self.buffer(view_id)
            .gesture_point_select(view_id, line_idx, line_byte_idx);
    }
    pub fn drag(&mut self, view_id: ViewId, line_idx: usize, line_byte_idx: usize) {
        self.buffer(view_id).drag(view_id, line_idx, line_byte_idx);
    }

    pub fn get_line_with_attributes(
        &mut self,
        view_id: ViewId,
        line_idx: usize,
    ) -> Option<(RopeSlice, Vec<AttrSpan>)> {
        let (buffer, theme) = self.buffer_and_theme(view_id);
        buffer.get_line_with_attributes(line_idx, view_id, &theme)
    }
}
