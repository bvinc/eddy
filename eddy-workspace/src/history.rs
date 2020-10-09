use crate::Selection;
use eddy_ts::InputEdit;
use ropey::Rope;

#[derive(Debug)]
pub struct History {
    history: Vec<ChangeGroup>,
    history_ix: usize,
}

#[derive(Debug)]
struct ChangeGroup {
    edits: Vec<InputEdit>,
    /// The selections by the editing view immediately before the first edit
    selections_before: Vec<Selection>,
    /// The selections by the editing view immediately after the last edit
    selections_after: Vec<Selection>,
    /// The state of the rope immediately after the last edit
    rope: Rope,
}

impl History {
    pub fn new(rope: &Rope) -> Self {
        let cg = ChangeGroup {
            edits: Vec::new(),
            selections_before: Vec::new(),
            selections_after: Vec::new(),
            rope: rope.clone(),
        };
        History {
            history: vec![cg],
            history_ix: 0,
        }
    }
    // pub fn rope_mut(&mut self) -> &mut Rope {
    //     &mut self.history[self.history_ix].rope
    // }

    pub fn new_change(
        &mut self,
        rope: &Rope,
        selections_before: Vec<Selection>,
        selections_after: Vec<Selection>,
    ) {
        let cg = ChangeGroup {
            edits: Vec::new(),
            selections_before,
            selections_after,
            rope: rope.clone(),
        };

        self.history.truncate(self.history_ix + 1);
        self.history.push(cg);
        self.history_ix = self.history.len() - 1;
    }

    /// Performs an undo.  If an undo action was successfully performed,
    /// returns where the selection regions should be changed to.
    pub fn undo(&mut self) -> Option<(Rope, &[Selection])> {
        if self.history_ix <= 0 {
            return None;
        }

        let selections_before = &self.history[self.history_ix].selections_before;
        self.history_ix -= 1;
        let rope = self.history[self.history_ix].rope.clone();

        Some((rope, selections_before))
    }

    /// Performs a redo.  If a redo action was successfully performed, returns
    /// where the selection regions should be changed to.
    pub fn redo(&mut self) -> Option<(Rope, &[Selection])> {
        if self.history_ix >= self.history.len() - 1 {
            return None;
        }

        self.history_ix += 1;
        let rope = self.history[self.history_ix].rope.clone();
        let selections_after = &self.history[self.history_ix].selections_after;

        Some((rope, selections_after))
    }
}
