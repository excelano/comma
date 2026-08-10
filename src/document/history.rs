// The undo stack.
//
// A change is stored as the whole record before and after it, rather than the
// one field that differed. A record is a handful of short strings and a change
// is a person pressing Enter, so the wider snapshot costs nothing worth
// counting, and it is what makes undo exact: setting a cell can also widen a
// short record, and can drop the original spelling of a field that was written
// in a way Comma would not choose for itself. Restoring the record restores all
// of that together.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use super::Field;

#[derive(Debug, Clone)]
pub(super) struct History {
    changes: Vec<Change>,
    /// How many changes have been applied. Everything before this point can be
    /// undone and everything from it onwards can be redone.
    position: usize,
    /// The position at which the file on disk was written, when it was written
    /// at one. `None` means no point in this history matches what is on disk.
    saved: Option<usize>,
}

#[derive(Debug, Clone)]
struct Change {
    row: usize,
    before: Vec<Field>,
    after: Vec<Field>,
}

/// What undoing or redoing produced: which record changed, and what it is now.
pub(super) type Restored = (usize, Vec<Field>);

impl History {
    /// The history of a document that has just been read, and so already agrees
    /// with the file it came from.
    pub(super) fn new() -> Self {
        Self {
            changes: Vec::new(),
            position: 0,
            saved: Some(0),
        }
    }

    pub(super) fn record(&mut self, row: usize, before: Vec<Field>, after: Vec<Field>) {
        // Anything undone and not redone is gone: this change happens instead
        // of it rather than after it.
        self.changes.truncate(self.position);
        if self.saved.is_some_and(|saved| saved > self.position) {
            // The state the file on disk holds was in the part just discarded,
            // so no amount of undoing will reach it again.
            self.saved = None;
        }

        self.changes.push(Change { row, before, after });
        self.position += 1;
    }

    pub(super) fn can_undo(&self) -> bool {
        self.position > 0
    }

    pub(super) fn can_redo(&self) -> bool {
        self.position < self.changes.len()
    }

    pub(super) fn undo(&mut self) -> Option<Restored> {
        if !self.can_undo() {
            return None;
        }

        self.position -= 1;
        let change = &self.changes[self.position];
        Some((change.row, change.before.clone()))
    }

    pub(super) fn redo(&mut self) -> Option<Restored> {
        let change = self.changes.get(self.position)?;
        let restored = (change.row, change.after.clone());
        self.position += 1;
        Some(restored)
    }

    pub(super) fn is_modified(&self) -> bool {
        self.saved != Some(self.position)
    }

    pub(super) fn mark_saved(&mut self) {
        self.saved = Some(self.position);
    }

    /// Says that the file on disk no longer matches any point in this history,
    /// without recording a change that could be undone.
    pub(super) fn mark_unsaved(&mut self) {
        self.saved = None;
    }
}
