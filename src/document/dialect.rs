// The delimiter and quote character a file is read and written with.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fmt;

/// How fields are separated and quoted.
///
/// Both characters must be ASCII. UTF-8 guarantees an ASCII byte never appears
/// inside a multi-byte sequence, so the parser can scan bytes and still be
/// correct on any UTF-8 file. A non-ASCII delimiter would break that and is
/// rejected rather than half-supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialect {
    delimiter: u8,
    quote: u8,
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
        })
    }

    pub fn comma() -> Self {
        Self {
            delimiter: b',',
            quote: b'"',
        }
    }

    pub fn tab() -> Self {
        Self {
            delimiter: b'\t',
            quote: b'"',
        }
    }

    pub fn semicolon() -> Self {
        Self {
            delimiter: b';',
            quote: b'"',
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
}

impl Default for Dialect {
    fn default() -> Self {
        Self::comma()
    }
}
