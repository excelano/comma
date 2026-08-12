// What kind of thing a value is, and what order values go in.
//
// Sorting and filtering both read a value to decide where it belongs, and they
// have to agree about what they are reading. A column that sorts as numbers and
// filters as text would be wrong in a way nobody would think to check, so there
// is one answer to what a value is and both ask it here.
//
// This is not the type inference the app refuses. Nothing is rewritten, nothing
// is reformatted, and the file is untouched. A value's kind decides where it
// sits among the others and decides nothing else.
//
// This is the one part of the library that reaches for GLib, and it does so for
// collation: text order belongs to the reader's own language rather than to
// ASCII, and the alternative is either a wrong answer or a Unicode table of our
// own several times the size of the rest of this crate.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cmp::Ordering;

use gtk::glib;

/// What a value is, as far as anything here is concerned.
///
/// A value holding only whitespace is blank rather than text: a cell of spaces
/// looks empty and is treated as empty, which is what makes "is empty" find the
/// holes in a file instead of most of them.
///
/// A number here is always a finite one. Rust reads "NaN" and "inf" as numbers
/// and a data file does not: they are words that turned up in a column, and
/// sorting them among the figures would put them somewhere no reader expects.
/// So everything that holds a `Number` can compare it without wondering whether
/// the comparison has an answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind<'a> {
    Number(f64),
    Text(&'a str),
    Blank,
}

pub fn kind(value: &str) -> Kind<'_> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Kind::Blank;
    }
    match trimmed.parse::<f64>() {
        Ok(number) if number.is_finite() => Kind::Number(number),
        _ => Kind::Text(value),
    }
}

/// Which group a value sorts in. Blank cells go last, because a cell holding
/// nothing is the absence of a value rather than a small one. Pointing the
/// column the other way turns that around with everything else; keeping blanks
/// at the bottom either way would need the sort to know its own direction, and
/// a comparison does not.
fn rank(value: &str) -> u8 {
    match kind(value) {
        Kind::Number(_) => 0,
        Kind::Text(_) => 1,
        Kind::Blank => 2,
    }
}

/// The order a column goes in when it is sorted: numbers before text, text
/// before blanks, and like with like within each group.
pub fn compare(left: &str, right: &str) -> Ordering {
    rank(left)
        .cmp(&rank(right))
        .then_with(|| match (kind(left), kind(right)) {
            // Both are finite, so the comparison always has an answer.
            (Kind::Number(left), Kind::Number(right)) => {
                left.partial_cmp(&right).unwrap_or(Ordering::Equal)
            }
            _ => collate(left, right),
        })
}

/// Where a value sits relative to another, when the two are the same kind of
/// thing, and nothing when they are not.
///
/// This is what "greater than" is asked, and refusing to answer across kinds is
/// the point of it. Filtering a price column above 100 should not turn up "£5"
/// on the reasoning that text sorts after numbers, and a blank cell is neither
/// above nor below anything. A value that cannot be compared is not shown,
/// which is the same answer SQL gives a comparison against nothing.
pub fn compare_alike(value: &str, against: &str) -> Option<Ordering> {
    match (kind(value), kind(against)) {
        (Kind::Number(value), Kind::Number(against)) => value.partial_cmp(&against),
        (Kind::Text(value), Kind::Text(against)) => Some(collate(value, against)),
        _ => None,
    }
}

/// Text order as the reader's language has it, so that ä lands with a rather
/// than after z.
fn collate(left: &str, right: &str) -> Ordering {
    glib::CollationKey::from(left).cmp(&glib::CollationKey::from(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(values: &[&str]) -> Vec<String> {
        let mut values: Vec<String> = values.iter().map(|value| value.to_string()).collect();
        values.sort_by(|left, right| compare(left, right));
        values
    }

    #[test]
    fn numbers_go_in_number_order_rather_than_text_order() {
        assert_eq!(sorted(&["10", "9", "100"]), ["9", "10", "100"]);
    }

    #[test]
    fn a_number_written_with_a_leading_zero_is_still_a_number() {
        assert_eq!(sorted(&["007", "10", "2"]), ["2", "007", "10"]);
    }

    #[test]
    fn numbers_come_before_text_and_text_before_blanks() {
        assert_eq!(
            sorted(&["", "apple", "3", "  ", "zebra"]),
            ["3", "apple", "zebra", "", "  "]
        );
    }

    #[test]
    fn text_that_only_looks_numeric_is_text() {
        // A thousands separator, a currency, and a version are all text, and
        // sorting them as text is the only reading that does not invent one.
        assert_eq!(
            sorted(&["1.234,56", "£5", "1.2.3"]),
            ["1.2.3", "1.234,56", "£5"]
        );
    }

    #[test]
    fn a_word_that_rust_reads_as_a_number_is_still_a_word() {
        assert_eq!(kind("NaN"), Kind::Text("NaN"));
        assert_eq!(kind("inf"), Kind::Text("inf"));
        assert_eq!(sorted(&["inf", "10", "2"]), ["2", "10", "inf"]);
    }

    #[test]
    fn a_cell_of_spaces_is_blank_rather_than_text() {
        assert_eq!(kind("   "), Kind::Blank);
        assert_eq!(kind(""), Kind::Blank);
    }

    #[test]
    fn numbers_compare_as_numbers_and_text_as_text() {
        assert_eq!(compare_alike("9", "10"), Some(Ordering::Less));
        assert_eq!(compare_alike("apple", "banana"), Some(Ordering::Less));
    }

    #[test]
    fn a_value_of_another_kind_is_neither_above_nor_below() {
        // The reason "greater than 100" does not turn up a price of £5.
        assert_eq!(compare_alike("£5", "100"), None);
        assert_eq!(compare_alike("100", "£5"), None);
        assert_eq!(compare_alike("", "100"), None);
        assert_eq!(compare_alike("  ", "apple"), None);
    }
}
