// What the grid shows: the values behind the bytes.
//
// Round-tripping a file proves nothing on its own — a parser that read every
// file as a single opaque field would pass all of those tests. These check that
// the values are the ones a person would expect to see in the cells.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod support;

use support::load;

use comma::document::{Dialect, Document};

fn parse(text: &str) -> Document {
    Document::from_bytes(text.as_bytes(), Dialect::comma()).expect("valid UTF-8")
}

#[test]
fn quotes_are_not_part_of_the_value() {
    let document = load("quoted.csv");
    assert_eq!(document.value(1, 1), "Hello, world");
}

#[test]
fn a_quoted_field_can_hold_a_newline() {
    let document = load("quoted.csv");
    assert_eq!(document.value(2, 1), "line one\nline two");
    assert_eq!(document.row_count(), 5);
}

#[test]
fn doubled_quotes_become_one() {
    let document = load("quoted.csv");
    assert_eq!(document.value(3, 1), "She said \"yes\"");
}

#[test]
fn characters_after_a_closing_quote_are_kept() {
    let document = load("odd-quoting.csv");
    assert_eq!(document.value(1, 1), "xy");
}

#[test]
fn an_empty_quoted_field_is_an_empty_value() {
    let document = load("odd-quoting.csv");
    assert_eq!(document.value(1, 2), "");
    assert_eq!(document.value(2, 0), "");
}

#[test]
fn nothing_is_interpreted_as_a_number_or_a_date() {
    let document = load("text-preservation.csv");
    assert_eq!(document.value(1, 0), "0001234567890123");
    assert_eq!(document.value(1, 1), "007");
    assert_eq!(document.value(1, 2), "000.50");
    assert_eq!(document.value(2, 0), "9007199254740993");
    assert_eq!(document.value(2, 2), "1.23E+15");
    assert_eq!(document.value(2, 3), "01/02/2026");
}

#[test]
fn whitespace_around_a_value_belongs_to_it() {
    let document = load("whitespace.csv");
    assert_eq!(document.value(0, 0), " a ");
    assert_eq!(document.value(0, 1), " b ");
    assert_eq!(document.value(0, 2), "c ");
    assert_eq!(document.value(1, 1), "   ");
}

#[test]
fn ragged_records_keep_their_own_widths() {
    let document = load("ragged.csv");
    assert_eq!(document.row_count(), 4);
    assert_eq!(document.column_count(), 5);
    assert_eq!(document.field_count(0), 3);
    assert_eq!(document.field_count(1), 1);
    assert_eq!(document.field_count(2), 1);
    assert_eq!(document.field_count(3), 5);
}

#[test]
fn a_column_a_record_does_not_reach_reads_as_empty() {
    let document = load("ragged.csv");
    assert_eq!(document.value(1, 4), "");
}

#[test]
fn the_byte_order_mark_is_not_part_of_the_first_value() {
    let document = load("bom.csv");
    assert_eq!(document.value(0, 0), "id");
}

#[test]
fn a_file_that_ends_without_a_newline_has_no_extra_record() {
    let document = load("no-trailing-newline.csv");
    assert_eq!(document.row_count(), 2);
}

#[test]
fn a_file_that_ends_with_a_newline_has_no_extra_record() {
    let document = parse("a,b\n");
    assert_eq!(document.row_count(), 1);
}

#[test]
fn a_blank_line_is_a_record_of_one_empty_field() {
    let document = parse("a\n\nb\n");
    assert_eq!(document.row_count(), 3);
    assert_eq!(document.value(1, 0), "");
    assert_eq!(document.field_count(1), 1);
}

#[test]
fn an_empty_file_has_no_records() {
    let document = parse("");
    assert_eq!(document.row_count(), 0);
    assert_eq!(document.column_count(), 0);
}

#[test]
fn a_lone_carriage_return_is_data_rather_than_a_line_ending() {
    let document = parse("a\rb,c\n");
    assert_eq!(document.row_count(), 1);
    assert_eq!(document.value(0, 0), "a\rb");
}

#[test]
fn the_delimiter_decides_where_fields_split() {
    let document = load("semicolon.csv");
    assert_eq!(document.value(1, 2), "has;semicolon");
    assert_eq!(document.field_count(1), 3);
}

#[test]
fn a_comma_is_ordinary_text_in_a_tab_file() {
    let document = load("tabs.tsv");
    assert_eq!(document.value(1, 2), "commas, need no quoting here");
}

#[test]
fn multi_byte_characters_survive() {
    let document = load("german.csv");
    assert_eq!(document.value(0, 1), "Straße");
    assert_eq!(document.value(1, 0), "München");
    assert_eq!(document.value(2, 1), "Weißenburgstraße");
}

#[test]
fn a_dialect_needs_two_different_ascii_characters() {
    use comma::document::DialectError;
    assert_eq!(Dialect::new('¦', '"'), Err(DialectError::NotAscii));
    assert_eq!(Dialect::new('\n', '"'), Err(DialectError::LineBreak));
    assert_eq!(Dialect::new(',', ','), Err(DialectError::SameCharacter));
    assert!(Dialect::new('|', '\'').is_ok());
}

#[test]
fn a_file_that_is_not_utf8_is_refused_rather_than_guessed_at() {
    use comma::document::LoadError;
    // 0xFF cannot appear anywhere in UTF-8.
    let error = Document::from_bytes(b"a,b\n1,\xff\n", Dialect::comma()).unwrap_err();
    assert_eq!(error, LoadError::NotUtf8 { valid_up_to: 6 });
}
