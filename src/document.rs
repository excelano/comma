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
mod parse;
mod serialize;

pub use dialect::{Dialect, DialectError};

use std::fmt;

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
    modified: bool,
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

/// How a record ended. Kept per record rather than per file, so a file with
/// mixed line endings comes back out with the same mixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordTerminator {
    Lf,
    CrLf,
    /// The last record of a file that does not end with a line break. Comma
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
            modified: false,
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
    pub fn set_value(&mut self, row: usize, column: usize, value: impl Into<String>) {
        let record = &mut self.records[row];

        while record.fields.len() <= column {
            record.fields.push(Field {
                value: String::new(),
                verbatim: None,
            });
        }

        let field = &mut record.fields[column];
        field.value = value.into();
        // The original bytes described the old value and say nothing about this
        // one, so they go.
        field.verbatim = None;

        self.modified = true;
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
    }
}
