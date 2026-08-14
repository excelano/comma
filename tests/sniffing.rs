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

use comma::document::{Dialect, Document, sniff, sniff_header};

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

// Guessing whether the first record names the columns.
//
// Same bar as the delimiter, and the same reason for accepting a guess at all:
// what was chosen is shown in the reading menu and one click undoes it. Where
// the file gives no evidence either way the answer is titles, because a title
// mistaken for data is a row that sorts, filters and exports as data, while
// data mistaken for a title sits at the top of the table where it can be seen.

fn headed(name: &str) -> bool {
    let bytes = read(name);
    sniff_header(&bytes, sniff(&bytes))
}

#[test]
fn every_corpus_file_is_read_as_having_the_titles_it_has() {
    for name in [
        "plain.csv",
        "quoted.csv",
        "crlf.csv",
        "mixed-endings.csv",
        "no-trailing-newline.csv",
        "bom.csv",
        "text-preservation.csv",
        "odd-quoting.csv",
        "whitespace.csv",
        "german.csv",
        "ragged.csv",
        "semicolon.csv",
        "european.csv",
        "tabs.tsv",
        "pipe.csv",
        "ascii-separated.dsv",
    ] {
        assert!(headed(name), "{name} lost the titles it has");
    }
}

#[test]
fn a_column_of_numbers_under_a_word_is_a_column_with_a_title() {
    let bytes = b"id,name\n1,Ada\n2,Grace\n3,Katherine\n";
    assert!(sniff_header(bytes, Dialect::comma()));
}

#[test]
fn a_column_of_numbers_all_the_way_up_has_no_title_on_it() {
    let bytes = b"1,Ada\n2,Grace\n3,Katherine\n";
    assert!(!sniff_header(bytes, Dialect::comma()));
}

#[test]
fn the_columns_that_speak_outvote_the_one_that_is_named_for_a_year() {
    // "2024" is a column name that looks like data. Two columns say titles and
    // that one says otherwise, so the file has titles.
    let bytes = b"record,2024,price,count\nr1,1200,19.99,7\nr2,1400,24.50,9\n";
    assert!(sniff_header(bytes, Dialect::comma()));
}

#[test]
fn dates_and_money_and_padded_codes_count_as_figures() {
    // Not one of these is a number, and every one of them fills a column of
    // figures. Holding out for numbers would silence the columns with the most
    // to say about the record above them.
    let bytes = b"when;paid;code\n2026-01-05;1.234,56;0007\n2026-02-05;987,00;0008\n";
    assert!(sniff_header(bytes, Dialect::semicolon()));
}

#[test]
fn an_index_column_with_no_name_on_it_is_still_a_column_with_a_title() {
    // What a spreadsheet or a dataframe writes out: a row number down the left
    // with nothing over it. Nothing is not a figure, so the record it sits in
    // is not another row of that column.
    let bytes = b",task,owner\n1,Inventory apps,Alice\n2,Score portfolio,Bob\n";
    assert!(sniff_header(bytes, Dialect::comma()));
}

#[test]
fn a_file_of_nothing_but_words_falls_back_to_titles() {
    let bytes = b"name,role,city\nAlice,Engineer,Portland\nBob,Designer,Austin\n";
    assert!(sniff_header(bytes, Dialect::comma()));
}

#[test]
fn a_first_record_with_a_hole_in_it_is_not_a_set_of_titles() {
    // A report heading sitting above the table. It names one column and leaves
    // the rest unnamed, which no set of titles does.
    let bytes = b"Application Risk Log,,,\nProject,Owner,Opened,Status\nAtlas,Ada,Monday,open\n";
    assert!(!sniff_header(bytes, Dialect::comma()));
}

#[test]
fn the_same_word_twice_is_not_a_set_of_titles() {
    let bytes = b"London,London,Oxford\nAda,Grace,Katherine\n";
    assert!(!sniff_header(bytes, Dialect::comma()));
}

#[test]
fn a_record_with_nothing_under_it_is_not_a_title() {
    // Titles name the rows below them. With none there, taking the only record
    // out of the body would leave an empty table.
    assert!(!sniff_header(b"id,name,city\n", Dialect::comma()));
    assert!(!sniff_header(b"", Dialect::comma()));
    assert!(!sniff_header(b"\n", Dialect::comma()));
}

#[test]
fn gaps_in_a_column_of_figures_do_not_silence_it() {
    let bytes = b"id,name\n1,Ada\n,Grace\n3,Katherine\n";
    assert!(sniff_header(bytes, Dialect::comma()));
}
