// Guessing which dialect a file is written in.
//
// The question "which delimiter is this?" is answered by asking a different
// one: which delimiter makes the file look like a table? Each candidate is run
// through the real parser and scored on how many records come out the same
// width. That way the guess agrees with the reader by construction — a
// candidate cannot score well on quoting the parser would read differently,
// because it is the parser doing the reading.
//
// A guess is never silent. The window shows what was chosen and changing it is
// one click, which is the whole reason a guess is acceptable here at all.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

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
