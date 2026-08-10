// The document model.
//
// The file on disk is the document. Reading it produces a grid of text and
// nothing else: no type inference, no reformatting, no normalising. Writing it
// reproduces every field the user did not edit byte for byte.
//
// The mechanism for that is small enough to state in a sentence. As each field
// is read, it is measured against what writing its value back out would
// produce. Almost always the two agree, and the field costs nothing extra. When
// they disagree — an unnecessarily quoted field, a stray character after a
// closing quote, a quote that was never closed — the field keeps its original
// bytes and writes those instead. Editing a cell discards its original bytes,
// because they no longer describe it. So a saved file differs from the one that
// was opened only where a cell was actually changed, and that holds by
// construction rather than by care.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod dialect;
mod history;
mod parse;
mod serialize;
mod sniff;

pub use dialect::{Dialect, DialectError};
pub use history::Extent;
pub use sniff::sniff;

use std::fmt;

use history::{Change, Edit, History};

use crate::search;

const BYTE_ORDER_MARK: &[u8] = &[0xEF, 0xBB, 0xBF];

/// A delimited file held in memory.
///
/// The grid vocabulary and the file vocabulary are both used here on purpose.
/// A *record* is one entry in the file and a *field* is one value within it;
/// those are the names the format uses. A *row* and a *column* are what the
/// same things are called once they are on screen. The accessors speak in rows
/// and columns because that is what callers are working with.
#[derive(Debug, Clone)]
pub struct Document {
    dialect: Dialect,
    byte_order_mark: bool,
    records: Vec<Record>,
    history: History,
}

#[derive(Debug, Clone)]
struct Record {
    fields: Vec<Field>,
    terminator: RecordTerminator,
}

#[derive(Debug, Clone)]
struct Field {
    value: String,
    /// The bytes this field occupied in the file, kept only when they differ
    /// from what writing `value` would produce. `None` is the common case and
    /// costs no memory beyond the value itself.
    verbatim: Option<String>,
}

impl Field {
    /// A field holding nothing, which is what a new one holds.
    fn blank() -> Self {
        Self {
            value: String::new(),
            verbatim: None,
        }
    }
}

/// How a record ended. Kept per record rather than per file, so a file with
/// mixed line endings comes back out with the same mixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordTerminator {
    Lf,
    CrLf,
    /// The ASCII record separator, in files that use it instead of line breaks.
    RecordSeparator,
    /// The last record of a file that does not end with a terminator. Comma
    /// does not add one.
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The file is not valid UTF-8. Comma does not transcode other encodings
    /// yet, and guessing at one would break the promise that what is written
    /// back matches what was read.
    NotUtf8 { valid_up_to: usize },
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUtf8 { valid_up_to } => {
                // Sentence case, because this message is shown to a person
                // rather than composed into a chain of other errors.
                write!(
                    formatter,
                    "This file is not valid UTF-8 text. The first byte Comma could not read is at offset {valid_up_to}."
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

impl Document {
    pub fn from_bytes(bytes: &[u8], dialect: Dialect) -> Result<Self, LoadError> {
        let (byte_order_mark, rest) = match bytes.strip_prefix(BYTE_ORDER_MARK) {
            Some(rest) => (true, rest),
            None => (false, bytes),
        };

        let text = std::str::from_utf8(rest).map_err(|error| LoadError::NotUtf8 {
            valid_up_to: error.valid_up_to(),
        })?;

        Ok(Self {
            dialect,
            byte_order_mark,
            records: parse::parse(text, dialect),
            history: History::new(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();

        if self.byte_order_mark {
            out.push('\u{FEFF}');
        }

        for record in &self.records {
            for (column, field) in record.fields.iter().enumerate() {
                if column > 0 {
                    out.push(self.dialect.delimiter());
                }
                match &field.verbatim {
                    Some(raw) => out.push_str(raw),
                    None => serialize::write_field(&mut out, &field.value, self.dialect),
                }
            }

            match record.terminator {
                RecordTerminator::Lf => out.push('\n'),
                RecordTerminator::CrLf => out.push_str("\r\n"),
                RecordTerminator::RecordSeparator => out.push(dialect::RECORD_SEPARATOR as char),
                RecordTerminator::Absent => {}
            }
        }

        out.into_bytes()
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    pub fn row_count(&self) -> usize {
        self.records.len()
    }

    /// The widest record in the file. Ragged files are common and are left
    /// ragged; this is how many columns the grid needs, not a claim that every
    /// row has that many.
    pub fn column_count(&self) -> usize {
        self.records
            .iter()
            .map(|record| record.fields.len())
            .max()
            .unwrap_or(0)
    }

    /// How many fields this record actually has, which may be fewer than
    /// [`Document::column_count`].
    pub fn field_count(&self, row: usize) -> usize {
        self.records[row].fields.len()
    }

    /// The value of one cell, or the empty string for a column this record does
    /// not reach.
    ///
    /// An out-of-range column is ordinary: files are ragged. An out-of-range
    /// row is a caller bug and panics, because silently returning nothing would
    /// hide it.
    pub fn value(&self, row: usize, column: usize) -> &str {
        match self.records[row].fields.get(column) {
            Some(field) => &field.value,
            None => "",
        }
    }

    /// Sets one cell, widening the record with empty fields if the column is
    /// past its end.
    ///
    /// Setting a cell to what it already holds does nothing at all: no change
    /// to the file, and nothing to undo. Retyping a value is not changing it,
    /// and a field left alone keeps whatever spelling the file gave it.
    pub fn set_value(&mut self, row: usize, column: usize, value: impl Into<String>) {
        let value = value.into();
        if self.value(row, column) == value {
            return;
        }

        let edit = self.edit(row, column, value);
        self.commit(Change::Fields { edits: vec![edit] });
    }

    /// Replaces every occurrence of `find` with `with`, in the given rows only,
    /// and says how many cells changed. One thing to undo, because it was one
    /// thing to ask for.
    ///
    /// A cell nothing matched in is not written to at all, so it keeps whatever
    /// spelling the file gave it.
    pub fn replace_in(&mut self, rows: &[usize], find: &str, with: &str) -> usize {
        let mut edits = Vec::new();
        let mut changed = 0;

        // One edit per record rather than per cell: an edit carries the whole
        // record, so two of them for the same record would each undo the other.
        for &row in rows {
            let before = self.records[row].fields.clone();
            let mut after = before.clone();
            let touched = after.iter_mut().fold(0, |touched, field| {
                match search::replace(&field.value, find, with) {
                    Some(replaced) => {
                        *field = Field {
                            value: replaced,
                            verbatim: None,
                        };
                        touched + 1
                    }
                    None => touched,
                }
            });

            if touched > 0 {
                changed += touched;
                edits.push(Edit { row, before, after });
            }
        }

        if changed > 0 {
            self.commit(Change::Fields { edits });
        }
        changed
    }

    /// One record's fields as they are, and as they would be with one cell set
    /// to `value`. Nothing is changed here; the change is what is returned.
    fn edit(&self, row: usize, column: usize, value: String) -> Edit {
        let before = self.records[row].fields.clone();
        let mut after = before.clone();

        while after.len() <= column {
            after.push(Field::blank());
        }
        // The original bytes described the old value and say nothing about this
        // one, so they go.
        after[column] = Field {
            value,
            verbatim: None,
        };

        Edit { row, before, after }
    }

    /// Puts an empty record at `at`, which may be the end of the file.
    pub fn insert_row(&mut self, at: usize) -> Extent {
        // A record always holds at least one field: an empty line is a record
        // of one empty field rather than of none, which is what the parser
        // would make of the row this writes.
        let width = self.column_count().max(1);
        let blank = |terminator| Record {
            fields: vec![Field::blank(); width],
            terminator,
        };

        // Only the last record of a file may end without a terminator, so
        // appending to a file that ends without one moves that over: the record
        // that was last gains a terminator, and the new one ends the file.
        let displaces = at == self.records.len() && self.ends_without_terminator();
        let (at, before, after) = if displaces {
            let last = self.records[at - 1].clone();
            let terminated = Record {
                terminator: self.terminator_style(),
                ..last.clone()
            };
            (
                at - 1,
                vec![last],
                vec![terminated, blank(RecordTerminator::Absent)],
            )
        } else {
            (at, Vec::new(), vec![blank(self.terminator_at(at))])
        };

        self.commit(Change::Rows { at, before, after })
    }

    /// Takes one record out of the file.
    pub fn delete_row(&mut self, at: usize) -> Extent {
        // A file that ended without a terminator still should, so when the last
        // record goes, the one that becomes last takes that over.
        let displaces = at + 1 == self.records.len() && self.ends_without_terminator() && at > 0;
        let (at, before, after) = if displaces {
            let previous = self.records[at - 1].clone();
            let unterminated = Record {
                terminator: RecordTerminator::Absent,
                ..previous.clone()
            };
            (
                at - 1,
                vec![previous, self.records[at].clone()],
                vec![unterminated],
            )
        } else {
            (at, vec![self.records[at].clone()], Vec::new())
        };

        self.commit(Change::Rows { at, before, after })
    }

    /// Puts an empty field at `at` in every record that reaches that far.
    ///
    /// Reaching that far means having a field there or ending exactly at it, so
    /// that a column can be added to the right of the last one. A record that
    /// stops short of the column is left alone: it has no field there to push
    /// aside, and padding it out to reach would rewrite a line the user was not
    /// pointing at. Ragged files stay ragged.
    pub fn insert_column(&mut self, at: usize) -> Extent {
        let fields = self
            .rows_reaching(at, |length, at| length >= at)
            .map(|row| (row, Field::blank()))
            .collect();

        self.commit(Change::Column {
            at,
            fields,
            inserted: true,
        })
    }

    /// Takes the field at `at` out of every record that has one.
    pub fn delete_column(&mut self, at: usize) -> Extent {
        let fields = self
            .rows_reaching(at, |length, at| length > at)
            .map(|row| (row, self.records[row].fields[at].clone()))
            .collect();

        self.commit(Change::Column {
            at,
            fields,
            inserted: false,
        })
    }

    /// Puts the records in a new order, which names for each position the
    /// record that goes there.
    ///
    /// Only the fields move. A record's terminator belongs to its place in the
    /// file rather than to its contents, so a file of mixed line endings keeps
    /// the shape it had and the record that ends the file still ends it.
    pub fn reorder_rows(&mut self, order: Vec<usize>) {
        assert_eq!(
            order.len(),
            self.records.len(),
            "a new order has to say where every record goes"
        );

        self.commit(Change::Order { order });
    }

    fn rows_reaching(
        &self,
        at: usize,
        reaches: fn(usize, usize) -> bool,
    ) -> impl Iterator<Item = usize> {
        self.records
            .iter()
            .enumerate()
            .filter(move |(_, record)| reaches(record.fields.len(), at))
            .map(|(row, _)| row)
    }

    /// What a record inserted at `at` should end with: the same as the record
    /// it displaces, or the way the rest of the file ends if there is none.
    fn terminator_at(&self, at: usize) -> RecordTerminator {
        match self.records.get(at) {
            Some(record) => record.terminator,
            None => self.terminator_style(),
        }
    }

    /// How this file ends its records, taken from the records it has. An empty
    /// file has only its dialect to go on.
    fn terminator_style(&self) -> RecordTerminator {
        self.records
            .iter()
            .map(|record| record.terminator)
            .find(|terminator| *terminator != RecordTerminator::Absent)
            .unwrap_or(if self.dialect.uses_record_separator() {
                RecordTerminator::RecordSeparator
            } else {
                RecordTerminator::Lf
            })
    }

    fn ends_without_terminator(&self) -> bool {
        self.records
            .last()
            .is_some_and(|last| last.terminator == RecordTerminator::Absent)
    }

    fn commit(&mut self, change: Change) -> Extent {
        self.history.commit(change, &mut self.records)
    }

    /// Puts the last change back the way it was, and says how much of the file
    /// moved so a view can redraw only that. `None` when there is nothing to
    /// undo.
    pub fn undo(&mut self) -> Option<Extent> {
        self.history.undo(&mut self.records)
    }

    pub fn redo(&mut self) -> Option<Extent> {
        self.history.redo(&mut self.records)
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Whether the file on disk still matches this document. Undoing back to
    /// the state that was last written answers no again: what makes a document
    /// modified is differing from the file, not having been touched.
    pub fn is_modified(&self) -> bool {
        self.history.is_modified()
    }

    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Says that this document no longer matches the file it came from, for
    /// changes that are not edits and so cannot be undone.
    pub fn mark_modified(&mut self) {
        self.history.mark_unsaved();
    }
}
