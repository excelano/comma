// What order values go in.
//
// Sorting reads a value to decide where it goes, which is not the type
// inference this app refuses: nothing is rewritten, nothing is reformatted, and
// a column of numbers put in text order would simply be wrong. A value's kind
// decides its group and only then its place — numbers before text, text before
// nothing at all — so a column of mixed contents still has one settled order
// rather than an order that depends on which pair is being compared.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cmp::Ordering;

use gtk::glib;

/// Which group a value sorts in. Blank cells go last, because a cell holding
/// nothing is the absence of a value rather than a small one. Pointing the
/// column the other way turns that around with everything else; keeping blanks
/// at the bottom either way would need the sort to know its own direction, and
/// a comparison does not.
fn rank(value: &str) -> u8 {
    if value.trim().is_empty() {
        2
    } else if number(value).is_some() {
        0
    } else {
        1
    }
}

fn number(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

pub fn compare(left: &str, right: &str) -> Ordering {
    rank(left)
        .cmp(&rank(right))
        .then_with(|| match (number(left), number(right)) {
            (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
            _ => collate(left, right),
        })
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
}
