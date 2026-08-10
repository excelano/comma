// The undo stack.
//
// A change stores what it replaced alongside what it put there, so one entry
// serves both directions and there is one description of what happened rather
// than a forward one and a backward one that have to agree. Doing a change and
// redoing it are the same code; undoing it is the same code read the other way.
//
// The kinds differ only in what they keep, and each keeps the least that will
// put the file back. A cell edit keeps the record it touched, because setting a
// cell can also widen a short record and always drops the original spelling of
// the field it replaces. A row change keeps whole records, because inserting one
// at the end of a file that has no trailing terminator also changes the record
// that used to be last. A column change keeps one field per record that had one,
// because a record too short to reach the column is not touched at all. And a
// reordering keeps no fields whatsoever, only where each record went.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use super::{Field, Record};

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
pub(super) enum Change {
    /// One record's fields replaced by another set.
    Fields {
        row: usize,
        before: Vec<Field>,
        after: Vec<Field>,
    },
    /// Records from `at` replaced by other records. Inserting is an empty
    /// `before` and deleting an empty `after`; a file whose last record ends
    /// without a terminator is neither, because gaining or losing a record
    /// after it changes how that record ends.
    Rows {
        at: usize,
        before: Vec<Record>,
        after: Vec<Record>,
    },
    /// A field in column `at` of each listed record, which the change either
    /// put there or took away.
    Column {
        at: usize,
        fields: Vec<(usize, Field)>,
        inserted: bool,
    },
    /// The records put in a new order, which names for each position the record
    /// that goes there. Taking it back is the same permutation read the other
    /// way about, so a whole file's worth of rearranging costs one number per
    /// record rather than a second copy of the file.
    Order { order: Vec<usize> },
}

/// How much of the file a change moved, which is as much as a view needs to
/// know to draw it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// One record reads differently and everything else is where it was.
    Record(usize),
    /// Rows or columns came or went, so nothing can be assumed to be where it
    /// was.
    Shape,
}

impl Change {
    /// Applies this change, or takes it back. `records` must be in the state
    /// this direction expects: the one before the change to apply it, the one
    /// after to take it back.
    fn apply(&self, records: &mut Vec<Record>, forward: bool) -> Extent {
        match self {
            Self::Fields { row, before, after } => {
                records[*row].fields = if forward {
                    after.clone()
                } else {
                    before.clone()
                };
                Extent::Record(*row)
            }
            Self::Rows { at, before, after } => {
                let (gone, come) = if forward {
                    (before, after)
                } else {
                    (after, before)
                };
                records.splice(*at..*at + gone.len(), come.iter().cloned());
                Extent::Shape
            }
            Self::Column {
                at,
                fields,
                inserted,
            } => {
                if *inserted == forward {
                    for (row, field) in fields {
                        records[*row].fields.insert(*at, field.clone());
                    }
                } else {
                    for (row, _) in fields {
                        records[*row].fields.remove(*at);
                    }
                }
                Extent::Shape
            }
            Self::Order { order } => {
                // Only the fields move. A record's terminator belongs to its
                // place in the file rather than to its contents, so the line
                // endings stay in the order the file had them and the record
                // that ends the file still ends it.
                let order = if forward {
                    order.clone()
                } else {
                    inverse(order)
                };
                let mut moving: Vec<Option<Vec<Field>>> = records
                    .iter_mut()
                    .map(|record| Some(std::mem::take(&mut record.fields)))
                    .collect();

                for (position, &from) in order.iter().enumerate() {
                    records[position].fields = moving[from]
                        .take()
                        .expect("an order names each record exactly once");
                }
                Extent::Shape
            }
        }
    }
}

fn inverse(order: &[usize]) -> Vec<usize> {
    let mut inverse = vec![0; order.len()];
    for (position, &from) in order.iter().enumerate() {
        inverse[from] = position;
    }
    inverse
}

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

    /// Applies a change and remembers it.
    pub(super) fn commit(&mut self, change: Change, records: &mut Vec<Record>) -> Extent {
        let extent = change.apply(records, true);

        // Anything undone and not redone is gone: this change happens instead
        // of it rather than after it.
        self.changes.truncate(self.position);
        if self.saved.is_some_and(|saved| saved > self.position) {
            // The state the file on disk holds was in the part just discarded,
            // so no amount of undoing will reach it again.
            self.saved = None;
        }

        self.changes.push(change);
        self.position += 1;
        extent
    }

    pub(super) fn can_undo(&self) -> bool {
        self.position > 0
    }

    pub(super) fn can_redo(&self) -> bool {
        self.position < self.changes.len()
    }

    pub(super) fn undo(&mut self, records: &mut Vec<Record>) -> Option<Extent> {
        if !self.can_undo() {
            return None;
        }

        self.position -= 1;
        Some(self.changes[self.position].apply(records, false))
    }

    pub(super) fn redo(&mut self, records: &mut Vec<Record>) -> Option<Extent> {
        let change = self.changes.get(self.position)?;
        let extent = change.apply(records, true);
        self.position += 1;
        Some(extent)
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
