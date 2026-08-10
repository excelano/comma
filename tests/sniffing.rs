// Guessing the dialect.
//
// The bar is not that the guess is always right — no guess is. It is that the
// guess is right on files people actually have, and that it is never confidently
// wrong on a file where the evidence is thin.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod support;

use support::read;

use comma::document::{Dialect, Document, sniff};

fn sniffed(name: &str) -> Dialect {
    sniff(&read(name))
}

#[test]
fn every_corpus_file_is_recognised_as_what_it_is() {
    for (name, expected) in [
        ("plain.csv", Dialect::comma()),
        ("quoted.csv", Dialect::comma()),
        ("crlf.csv", Dialect::comma()),
        ("mixed-endings.csv", Dialect::comma()),
        ("no-trailing-newline.csv", Dialect::comma()),
        ("bom.csv", Dialect::comma()),
        ("text-preservation.csv", Dialect::comma()),
        ("odd-quoting.csv", Dialect::comma()),
        ("whitespace.csv", Dialect::comma()),
        ("german.csv", Dialect::comma()),
        ("semicolon.csv", Dialect::semicolon()),
        ("european.csv", Dialect::semicolon()),
        ("tabs.tsv", Dialect::tab()),
        ("pipe.csv", Dialect::pipe()),
        ("ascii-separated.dsv", Dialect::unit_separator()),
    ] {
        assert_eq!(sniffed(name), expected, "{name} was read as the wrong kind");
    }
}

#[test]
fn a_delimiter_inside_quotes_does_not_count() {
    // Every line holds more commas than semicolons, and all of the commas are
    // inside quoted fields. Counting characters would pick the wrong one;
    // parsing does not.
    let bytes = b"a;b\n\"x,y,z\";\"p,q,r\"\n\"1,2,3\";\"4,5,6\"\n";
    assert_eq!(sniff(bytes), Dialect::semicolon());
}

#[test]
fn a_file_with_no_delimiter_at_all_is_read_as_one_column() {
    let bytes = b"first line\nsecond line\nthird line\n";
    let dialect = sniff(bytes);

    assert_eq!(dialect, Dialect::comma());
    let document = Document::from_bytes(bytes, dialect).unwrap();
    assert_eq!(document.column_count(), 1);
    assert_eq!(document.row_count(), 3);
}

#[test]
fn an_empty_file_does_not_produce_a_wild_guess() {
    assert_eq!(sniff(b""), Dialect::comma());
    assert_eq!(sniff(b"\n"), Dialect::comma());
}

#[test]
fn a_file_too_ragged_to_judge_falls_back_to_comma() {
    assert_eq!(sniffed("ragged.csv"), Dialect::comma());
}

#[test]
fn the_wider_reading_wins_when_two_delimiters_both_fit() {
    // Both split every line evenly. The comma finds three columns where the
    // semicolon finds two, so it has found more of the structure.
    let bytes = b"a,b;c,d\ne,f;g,h\n";
    assert_eq!(sniff(bytes), Dialect::comma());
}

#[test]
fn only_the_first_part_of_a_large_file_is_read() {
    // A megabyte of tab-delimited text with a comma-delimited tail. The tail is
    // far past the sample, so it cannot change the answer.
    let mut bytes = Vec::new();
    while bytes.len() < 1_000_000 {
        bytes.extend_from_slice(b"one\ttwo\tthree\n");
    }
    let tail_starts = bytes.len();
    for _ in 0..1000 {
        bytes.extend_from_slice(b"one,two,three\n");
    }

    assert!(tail_starts > 64 * 1024, "the tail has to be out of sight");
    assert_eq!(sniff(&bytes), Dialect::tab());
}

#[test]
fn a_file_that_is_not_utf8_is_still_sniffed() {
    // Comma refuses to open this, but the refusal comes later. Sniffing reads
    // what it can rather than failing first.
    let bytes = b"name;city\nAda;K\xf6ln\nGrace;M\xfcnchen\n";
    assert_eq!(sniff(bytes), Dialect::semicolon());
    assert!(Document::from_bytes(bytes, Dialect::semicolon()).is_err());
}
