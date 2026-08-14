// Guessing the two things a delimited file does not say about itself: which
// delimiter separates its fields, and whether its first record names the
// columns or is one of them.
//
// The question "which delimiter is this?" is answered by asking a different
// one: which delimiter makes the file look like a table? Each candidate is run
// through the real parser and scored on how many records come out the same
// width. That way the guess agrees with the reader by construction — a
// candidate cannot score well on quoting the parser would read differently,
// because it is the parser doing the reading.
//
// The header is guessed the same way, from what the records hold rather than
// from what the fields are called, and for the same reason: nothing here knows
// anything about the file that the parser did not tell it.
//
// A guess is never silent. The window shows what was chosen and changing it is
// one click, which is the whole reason a guess is acceptable here at all.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use super::Record;
use super::dialect::{Dialect, RECORD_SEPARATOR};
use super::parse;

/// How much of the file to look at. Enough to see hundreds of records in any
/// real file, and small enough that scoring five candidates is not felt.
const SAMPLE_BYTES: usize = 64 * 1024;

/// The dialects Comma guesses between, in the order it prefers them when two
/// fit equally well.
const CANDIDATES: [Dialect; 5] = [
    Dialect::comma(),
    Dialect::tab(),
    Dialect::semicolon(),
    Dialect::pipe(),
    Dialect::unit_separator(),
];

/// The dialect that best explains these bytes, or a comma when none of them do.
///
/// A single-column file has no delimiter to find, and neither does an empty
/// one. Both come back as comma, which is what an unadorned line of text is
/// under any dialect: one field.
pub fn sniff(bytes: &[u8]) -> Dialect {
    let sample = sample(bytes);

    let mut best = Fit::NONE;
    let mut chosen = Dialect::comma();

    for candidate in CANDIDATES {
        let fit = measure(sample, candidate);
        if fit.fits() && fit.better_than(&best) {
            best = fit;
            chosen = candidate;
        }
    }

    chosen
}

/// The leading part of the file to judge by, as text and without a partial
/// record on the end.
fn sample(bytes: &[u8]) -> &str {
    let bytes = bytes.strip_prefix(super::BYTE_ORDER_MARK).unwrap_or(bytes);
    let truncated = bytes.len() > SAMPLE_BYTES;
    let bytes = &bytes[..bytes.len().min(SAMPLE_BYTES)];

    // A file Comma will refuse to open still gets sniffed, because the caller
    // has not tried to read it yet. Whatever is valid text is enough to judge.
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => std::str::from_utf8(&bytes[..error.valid_up_to()])
            .expect("the bytes before the first invalid one are valid"),
    };

    if !truncated {
        return text;
    }

    match text.rfind(['\n', RECORD_SEPARATOR as char]) {
        Some(end) => &text[..end + 1],
        None => text,
    }
}

/// How well one dialect explains the sample.
struct Fit {
    /// Records whose width is the most common one.
    agreeing: usize,
    /// Records in total. Never zero, so the two can be compared as a ratio.
    records: usize,
    /// That most common width.
    width: usize,
}

impl Fit {
    const NONE: Self = Self {
        agreeing: 0,
        records: 1,
        width: 0,
    };

    /// A dialect that finds no delimiter reads every record as a single field,
    /// which explains nothing: every dialect does that.
    fn fits(&self) -> bool {
        self.width >= 2
    }

    /// Compared as ratios, cross-multiplied rather than divided, since the two
    /// dialects may not have found the same number of records. A wider table
    /// wins a tie: if two delimiters both split the file evenly, the one
    /// finding more columns has found more structure.
    fn better_than(&self, other: &Self) -> bool {
        let mine = self.agreeing * other.records;
        let theirs = other.agreeing * self.records;
        mine > theirs || (mine == theirs && self.width > other.width)
    }
}

fn measure(sample: &str, dialect: Dialect) -> Fit {
    let records = parse::parse(sample, dialect);
    if records.is_empty() {
        return Fit::NONE;
    }

    let mut widths: Vec<usize> = records.iter().map(|record| record.fields.len()).collect();
    widths.sort_unstable();

    let mut agreeing = 0;
    let mut width = 0;
    let mut run = 1;
    for position in 1..=widths.len() {
        if position == widths.len() || widths[position] != widths[position - 1] {
            if run > agreeing {
                agreeing = run;
                width = widths[position - 1];
            }
            run = 1;
        } else {
            run += 1;
        }
    }

    Fit {
        agreeing,
        records: widths.len(),
        width,
    }
}

/// Whether the first record of these bytes is column titles rather than data.
///
/// The file cannot say, any more than it can say which delimiter it uses, so
/// the question is settled the same way: by looking at what the records hold.
/// A column of figures with a word over it has a title on it, and so does one
/// with nothing over it, since neither is another figure. A column of figures
/// running all the way to the top has no title. A column of text says nothing
/// either way, because a word above words is what both answers look like.
///
/// When no column has anything to say, the answer is yes. Delimited files
/// usually carry titles, and the two ways of being wrong do not cost the same.
/// A header mistaken for data is an ordinary row: it sorts into the body, hides
/// behind a filter, and goes out to the other tools as a record. Data mistaken
/// for a header sits at the top of the table in plain sight, changes nothing
/// about the file, and is one click from being put back.
pub fn sniff_header(bytes: &[u8], dialect: Dialect) -> bool {
    let records = parse::parse(sample(bytes), dialect);

    // Titles name the columns of something. With no record under them there is
    // nothing named, and taking the only record there is would leave an empty
    // table on screen looking like a file that would not open.
    let Some((first, body)) = records.split_first() else {
        return false;
    };
    if body.is_empty() {
        return false;
    }

    let mut titles = 0;
    let mut data = 0;
    for column in 0..first.fields.len() {
        match reading(first, body, column) {
            Some(Reading::Titles) => titles += 1,
            Some(Reading::Data) => data += 1,
            None => {}
        }
    }

    if titles != data {
        return titles > data;
    }

    names_its_columns(first)
}

/// What one column says about the record above it.
enum Reading {
    Titles,
    Data,
}

/// A column only speaks when its body is figures throughout. Then the record on
/// top is one more figure, or it is a title: a word is a title, and so is a
/// blank, which is what an unnamed index column looks like and what no row of
/// figures looks like.
///
/// Blanks in the body are passed over rather than counted against. A column of
/// figures with gaps in it is still a column of figures; one that is blank the
/// whole way down holds no figures and says nothing.
fn reading(first: &Record, body: &[Record], column: usize) -> Option<Reading> {
    let mut figures = 0;
    for record in body {
        match kind(field(record, column)) {
            Kind::Number => figures += 1,
            Kind::Blank => {}
            Kind::Text => return None,
        }
    }
    if figures == 0 {
        return None;
    }

    match kind(field(first, column)) {
        Kind::Number => Some(Reading::Data),
        Kind::Text | Kind::Blank => Some(Reading::Titles),
    }
}

/// Whether a record would make a decent set of titles, which is what decides it
/// when the columns are silent.
///
/// Titles name every column, and name them apart. A record with a hole in it or
/// with the same word twice names neither, and is more likely a line of data or
/// the heading of a report sitting above the table.
fn names_its_columns(record: &Record) -> bool {
    let mut named: Vec<&str> = Vec::with_capacity(record.fields.len());
    for field in &record.fields {
        let value = field.value.trim();
        if value.is_empty() || named.contains(&value) {
            return false;
        }
        named.push(value);
    }
    true
}

/// What a field looks like, coarsely enough to be right about files rather than
/// about values.
enum Kind {
    Blank,
    Number,
    Text,
}

/// Anything holding digits and no letters counts as a number, which takes in
/// the things that fill numeric columns without being numbers: dates, money,
/// part numbers, a zip code with a leading zero. Getting those wrong the other
/// way would silence the columns that have the most to say.
fn kind(value: &str) -> Kind {
    let value = value.trim();
    if value.is_empty() {
        return Kind::Blank;
    }
    if value.chars().any(char::is_alphabetic) || !value.chars().any(|c| c.is_ascii_digit()) {
        return Kind::Text;
    }
    Kind::Number
}

fn field(record: &Record, column: usize) -> &str {
    record
        .fields
        .get(column)
        .map_or("", |field| field.value.as_str())
}
