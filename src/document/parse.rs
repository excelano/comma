// Reading bytes into records and fields.
//
// One scanner produces both, in a single pass. Splitting records and fields
// into separate passes would mean two pieces of code with their own opinion
// about where quoting starts, and a file where they disagreed would be read
// one way and written back another.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use super::dialect::Dialect;
use super::serialize;
use super::{Field, Record, RecordTerminator};

pub(super) fn parse(text: &str, dialect: Dialect) -> Vec<Record> {
    let mut records = Vec::new();
    let mut position = 0;

    // A file that ends with a terminator has no empty record after it, and a
    // file of no bytes has no records at all.
    while position < text.len() {
        let (record, next) = parse_record(text, position, dialect);
        records.push(record);
        position = next;
    }

    records
}

/// Where a field stopped, which is also how the record it belongs to ended.
enum FieldEnd {
    Delimiter,
    Lf,
    CrLf,
    EndOfText,
}

fn parse_record(text: &str, start: usize, dialect: Dialect) -> (Record, usize) {
    let mut fields = Vec::new();
    let mut position = start;

    loop {
        let (field, next, end) = parse_field(text, position, dialect);
        fields.push(field);
        position = next;

        let terminator = match end {
            FieldEnd::Delimiter => continue,
            FieldEnd::Lf => RecordTerminator::Lf,
            FieldEnd::CrLf => RecordTerminator::CrLf,
            FieldEnd::EndOfText => RecordTerminator::Absent,
        };

        return (Record { fields, terminator }, position);
    }
}

fn parse_field(text: &str, start: usize, dialect: Dialect) -> (Field, usize, FieldEnd) {
    let bytes = text.as_bytes();
    let mut value = String::new();
    let mut position = start;

    if bytes.get(position) == Some(&dialect.quote_byte()) {
        position = read_quoted(text, position + 1, dialect, &mut value);
    }

    // Everything from here to the delimiter or the end of the record is
    // literal. Starting this loop unconditionally is what lets a field like
    // `"abc"def` survive: some exporters emit it, and the characters after the
    // closing quote are data like any other.
    loop {
        let run_start = position;
        while position < bytes.len() && !is_boundary(bytes[position], dialect) {
            position += 1;
        }
        value.push_str(&text[run_start..position]);

        match bytes.get(position) {
            None => {
                let field = finish(text, start, position, value, dialect);
                return (field, position, FieldEnd::EndOfText);
            }
            Some(b'\n') => {
                let field = finish(text, start, position, value, dialect);
                return (field, position + 1, FieldEnd::Lf);
            }
            Some(b'\r') if bytes.get(position + 1) == Some(&b'\n') => {
                let field = finish(text, start, position, value, dialect);
                return (field, position + 2, FieldEnd::CrLf);
            }
            Some(b'\r') => {
                // A carriage return on its own is not a line ending. It is data.
                value.push('\r');
                position += 1;
            }
            // is_boundary stops on the delimiter, a line feed, or a carriage
            // return, and the three cases above have taken the latter two.
            Some(_) => {
                let field = finish(text, start, position, value, dialect);
                return (field, position + 1, FieldEnd::Delimiter);
            }
        }
    }
}

/// Reads the inside of a quoted field, starting just past the opening quote,
/// and returns the position just past the closing one.
fn read_quoted(text: &str, start: usize, dialect: Dialect, value: &mut String) -> usize {
    let bytes = text.as_bytes();
    let quote = dialect.quote_byte();
    let mut position = start;

    loop {
        let run_start = position;
        while position < bytes.len() && bytes[position] != quote {
            position += 1;
        }
        value.push_str(&text[run_start..position]);

        if position >= bytes.len() {
            // The quote was never closed. Take what is there and let the field
            // keep its original bytes, so a damaged file still round-trips.
            return position;
        }

        if bytes.get(position + 1) == Some(&quote) {
            value.push(quote as char);
            position += 2;
        } else {
            return position + 1;
        }
    }
}

fn is_boundary(byte: u8, dialect: Dialect) -> bool {
    byte == dialect.delimiter_byte() || byte == b'\n' || byte == b'\r'
}

fn finish(text: &str, start: usize, end: usize, value: String, dialect: Dialect) -> Field {
    let raw = &text[start..end];
    let verbatim = if serialize::matches_canonical(&value, raw, dialect) {
        None
    } else {
        Some(raw.to_owned())
    };

    Field { value, verbatim }
}
