// The round-trip guarantee: a file read and written back with no edits is the
// same file, byte for byte.
//
// Every case here is one Comma is expected to meet in the wild, and most of
// them are cases some other tool gets wrong.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod support;

use support::{dialect, read, visible};

use comma::document::Document;

fn assert_round_trips(name: &str) {
    let original = read(name);
    let document = Document::from_bytes(&original, dialect(name))
        .unwrap_or_else(|error| panic!("could not load {name}: {error}"));
    let written = document.to_bytes();

    assert_eq!(
        visible(&written),
        visible(&original),
        "{name} did not come back out the way it went in"
    );
}

#[test]
fn plain_file() {
    assert_round_trips("plain.csv");
}

#[test]
fn quoted_fields_with_delimiters_and_newlines() {
    assert_round_trips("quoted.csv");
}

#[test]
fn crlf_line_endings_with_a_quoted_final_field() {
    assert_round_trips("crlf.csv");
}

#[test]
fn mixed_line_endings_in_one_file() {
    assert_round_trips("mixed-endings.csv");
}

#[test]
fn no_trailing_newline_stays_missing() {
    assert_round_trips("no-trailing-newline.csv");
}

#[test]
fn ragged_records_stay_ragged() {
    assert_round_trips("ragged.csv");
}

#[test]
fn byte_order_mark_is_preserved() {
    assert_round_trips("bom.csv");
}

#[test]
fn values_that_look_like_numbers_and_dates() {
    assert_round_trips("text-preservation.csv");
}

#[test]
fn quoting_the_file_did_not_need() {
    assert_round_trips("odd-quoting.csv");
}

#[test]
fn semicolon_delimited() {
    assert_round_trips("semicolon.csv");
}

#[test]
fn tab_delimited() {
    assert_round_trips("tabs.tsv");
}

#[test]
fn pipe_delimited() {
    assert_round_trips("pipe.csv");
}

#[test]
fn ascii_separators_with_a_line_break_inside_a_field() {
    assert_round_trips("ascii-separated.dsv");
}

#[test]
fn decimal_commas_inside_a_semicolon_file() {
    assert_round_trips("european.csv");
}

#[test]
fn surrounding_whitespace_is_data() {
    assert_round_trips("whitespace.csv");
}

#[test]
fn multi_byte_characters() {
    assert_round_trips("german.csv");
}

#[test]
fn empty_file() {
    assert_round_trips("empty.csv");
}

#[test]
fn a_file_of_one_newline() {
    assert_round_trips("newline-only.csv");
}

#[test]
fn an_unterminated_quote_still_round_trips() {
    let original = br#"a,b
1,"never closed
"#;
    let document = Document::from_bytes(original, comma::document::Dialect::comma()).unwrap();
    assert_eq!(visible(&document.to_bytes()), visible(original));
}
