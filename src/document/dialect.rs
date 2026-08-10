// The characters a file is read and written with.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fmt;

/// The ASCII record separator. Files that use it put their record boundary in
/// a character that cannot occur in text, which is the whole reason the ASCII
/// separators exist: a field can then hold line breaks with no quoting at all.
pub(super) const RECORD_SEPARATOR: u8 = 0x1E;

/// The ASCII unit separator, the field delimiter that goes with it.
const UNIT_SEPARATOR: u8 = 0x1F;

/// How fields are separated, quoted, and gathered into records.
///
/// The delimiter and quote character must be ASCII. UTF-8 guarantees an ASCII
/// byte never appears inside a multi-byte sequence, so the parser can scan
/// bytes and still be correct on any UTF-8 file. A non-ASCII delimiter would
/// break that and is rejected rather than half-supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialect {
    delimiter: u8,
    quote: u8,
    records: RecordStyle,
}

/// What ends a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordStyle {
    /// A line break, as nearly every delimited file does it.
    LineBreak,
    /// The ASCII record separator. Line breaks are then ordinary text.
    RecordSeparator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectError {
    NotAscii,
    LineBreak,
    SameCharacter,
}

impl fmt::Display for DialectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotAscii => "the delimiter and quote character must be ASCII",
            Self::LineBreak => "a line break cannot be used as a delimiter or quote character",
            Self::SameCharacter => "the delimiter and quote character must differ",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for DialectError {}

impl Dialect {
    pub fn new(delimiter: char, quote: char) -> Result<Self, DialectError> {
        for character in [delimiter, quote] {
            if !character.is_ascii() {
                return Err(DialectError::NotAscii);
            }
            if character == '\n' || character == '\r' {
                return Err(DialectError::LineBreak);
            }
        }
        if delimiter == quote {
            return Err(DialectError::SameCharacter);
        }
        Ok(Self {
            delimiter: delimiter as u8,
            quote: quote as u8,
            records: RecordStyle::LineBreak,
        })
    }

    pub const fn comma() -> Self {
        Self::line_break(b',')
    }

    pub const fn tab() -> Self {
        Self::line_break(b'\t')
    }

    pub const fn semicolon() -> Self {
        Self::line_break(b';')
    }

    pub const fn pipe() -> Self {
        Self::line_break(b'|')
    }

    /// The ASCII separators: unit separator between fields, record separator
    /// between records.
    pub const fn unit_separator() -> Self {
        Self {
            delimiter: UNIT_SEPARATOR,
            quote: b'"',
            records: RecordStyle::RecordSeparator,
        }
    }

    const fn line_break(delimiter: u8) -> Self {
        Self {
            delimiter,
            quote: b'"',
            records: RecordStyle::LineBreak,
        }
    }

    pub fn delimiter(&self) -> char {
        self.delimiter as char
    }

    pub fn quote(&self) -> char {
        self.quote as char
    }

    pub(super) fn delimiter_byte(&self) -> u8 {
        self.delimiter
    }

    pub(super) fn quote_byte(&self) -> u8 {
        self.quote
    }

    /// Whether this byte ends a field or a record.
    ///
    /// One definition serves both directions: these are the characters the
    /// parser stops at, and so exactly the characters a value has to be quoted
    /// to contain.
    pub(super) fn is_boundary(&self, byte: u8) -> bool {
        if byte == self.delimiter {
            return true;
        }
        match self.records {
            RecordStyle::LineBreak => byte == b'\n' || byte == b'\r',
            RecordStyle::RecordSeparator => byte == RECORD_SEPARATOR,
        }
    }
}

impl Default for Dialect {
    fn default() -> Self {
        Self::comma()
    }
}
