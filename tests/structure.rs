// Rows and columns coming and going.
//
// A structural change touches lines the user was not pointing at — that is what
// it is for — so what these tests measure is that it touches no more than it
// has to, and that undoing it puts the file back exactly.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod support;

use support::{dialect, load, read, visible};

use comma::document::{Document, Extent};

/// The file a document would write, as text.
fn written(document: &Document) -> String {
    String::from_utf8(document.to_bytes()).expect("the corpus is UTF-8")
}

#[test]
fn a_row_inserted_above_is_blank_and_as_wide_as_the_table() {
    let mut document = load("plain.csv");

    document.insert_row(1);

    assert_eq!(document.row_count(), 4);
    assert_eq!(document.field_count(1), 3);
    assert_eq!(
        written(&document),
        "id,name,city\n,,\n1,Ada,London\n2,Grace,Arlington\n"
    );
}

#[test]
fn an_inserted_row_ends_the_way_the_file_ends_its_records() {
    let mut document = load("crlf.csv");

    document.insert_row(1);

    let text = written(&document);
    assert_eq!(
        text.lines().count(),
        4,
        "a file of carriage-return line endings must not gain a bare line feed"
    );
    assert!(text.starts_with("id,name,note\r\n,,\r\n1,Ada,"), "{text}");
}

#[test]
fn a_row_added_to_a_file_with_no_trailing_newline_takes_over_ending_it() {
    let name = "no-trailing-newline.csv";
    let mut document = load(name);

    document.insert_row(2);

    assert_eq!(
        visible(&document.to_bytes()),
        visible(b"id,name\n1,Ada\n,"),
        "the record that stopped being last needs the terminator it did without"
    );
    assert_eq!(
        visible(&document.to_bytes()),
        visible(&{
            document.undo();
            document.redo();
            document.to_bytes()
        }),
        "and doing it again has to arrive at the same file"
    );
}

#[test]
fn deleting_the_last_row_leaves_the_file_ending_as_it_did() {
    let mut document = load("no-trailing-newline.csv");

    document.delete_row(1);

    assert_eq!(
        visible(&document.to_bytes()),
        visible(b"id,name"),
        "the file ended without a terminator and still should"
    );
}

#[test]
fn deleting_a_row_takes_that_line_and_no_other() {
    let name = "odd-quoting.csv";
    let original = String::from_utf8(read(name)).unwrap();
    let mut document = load(name);

    document.delete_row(1);

    let text = written(&document);
    assert_eq!(text.lines().count(), 2);
    assert_eq!(
        text.lines().next().unwrap(),
        original.lines().next().unwrap()
    );
    assert_eq!(
        text.lines().nth(1).unwrap(),
        original.lines().nth(2).unwrap()
    );
}

#[test]
fn a_line_ending_goes_with_the_line_it_belonged_to() {
    let mut document = load("mixed-endings.csv");

    document.delete_row(0);

    assert_eq!(
        visible(&document.to_bytes()),
        visible(b"c,d\ne,f\r\n"),
        "deleting the carriage-return line must not give its ending to another"
    );
}

#[test]
fn an_inserted_column_stops_at_the_records_that_do_not_reach_it() {
    let mut document = load("ragged.csv");

    document.insert_column(4);

    // Only the last record reaches column 4. The three-field record stops two
    // columns short of it, and is not padded out to reach.
    assert_eq!(written(&document), "a,b,c\nd\n\ne,f,g,h,,i\n");
}

#[test]
fn a_column_inserted_at_a_records_edge_reaches_it() {
    let mut document = load("ragged.csv");

    document.insert_column(1);

    // A record that ends exactly where the column goes gains a field, which is
    // the same rule that lets a column be added to the right of the last one.
    assert_eq!(written(&document), "a,,b,c\nd,\n,\ne,,f,g,h,i\n");
}

#[test]
fn a_column_can_be_added_at_the_end_of_a_table() {
    let mut document = load("plain.csv");

    document.insert_column(document.column_count());

    assert_eq!(document.column_count(), 4);
    assert_eq!(
        written(&document),
        "id,name,city,\n1,Ada,London,\n2,Grace,Arlington,\n"
    );
}

#[test]
fn a_deleted_column_leaves_the_records_that_never_had_it() {
    let mut document = load("ragged.csv");

    document.delete_column(1);

    assert_eq!(written(&document), "a,c\nd\n\ne,g,h,i\n");
}

#[test]
fn undoing_a_column_puts_back_the_spelling_of_the_fields_it_took() {
    // Column 1 of this file holds `b`, then `"x"y`, then `unquoted`. The middle
    // one is written in a way Comma would never produce, so a column restored
    // from its values rather than from its fields would be plain to see.
    let name = "odd-quoting.csv";
    let original = read(name);
    let mut document = load(name);

    document.delete_column(1);
    assert_ne!(visible(&document.to_bytes()), visible(&original));

    assert_eq!(document.undo(), Some(Extent::Shape));
    assert_eq!(visible(&document.to_bytes()), visible(&original));
}

#[test]
fn reordering_moves_the_fields_and_leaves_the_line_endings_where_they_are() {
    // The first and third lines of this file end with a carriage return and the
    // second does not. Reordering must not carry an ending along with the row
    // it belonged to, or the file changes shape as well as order.
    let mut document = load("mixed-endings.csv");

    document.reorder_rows(vec![2, 1, 0]);

    assert_eq!(
        visible(&document.to_bytes()),
        visible(b"e,f\r\nc,d\na,b\r\n")
    );
}

#[test]
fn reordering_a_file_that_ends_without_a_newline_still_ends_without_one() {
    let mut document = load("no-trailing-newline.csv");

    document.reorder_rows(vec![1, 0]);

    assert_eq!(visible(&document.to_bytes()), visible(b"1,Ada\nid,name"));
}

#[test]
fn undoing_a_reordering_puts_every_row_back() {
    let name = "odd-quoting.csv";
    let original = read(name);
    let mut document = load(name);

    document.reorder_rows(vec![2, 0, 1]);
    assert_eq!(document.value(0, 1), "unquoted");

    assert_eq!(document.undo(), Some(Extent::Shape));
    assert_eq!(
        visible(&document.to_bytes()),
        visible(&original),
        "an order taken back has to restore each field's original spelling too"
    );
}

#[test]
fn a_structural_change_is_one_thing_to_undo() {
    let mut document = load("plain.csv");

    document.insert_column(1);
    assert_eq!(document.column_count(), 4);

    assert_eq!(document.undo(), Some(Extent::Shape));
    assert_eq!(document.column_count(), 3);
    assert!(
        !document.can_undo(),
        "three records changed, but the user did one thing"
    );
}

#[test]
fn a_row_change_says_which_rows_it_spliced() {
    // What the grid redraws from. A row change that reported the whole file had
    // moved would take the view back to the top of it, which is what opening a
    // file does and not what inserting a row does.
    let mut document = load("plain.csv");

    assert_eq!(
        document.insert_row(1),
        Extent::Rows {
            at: 1,
            gone: 0,
            come: 1
        }
    );
    assert_eq!(
        document.undo(),
        Some(Extent::Rows {
            at: 1,
            gone: 1,
            come: 0
        }),
        "taking an insertion back is the same splice read the other way"
    );
    assert_eq!(
        document.delete_row(1),
        Extent::Rows {
            at: 1,
            gone: 1,
            come: 0
        }
    );
}

#[test]
fn appending_to_a_file_with_no_final_newline_says_the_last_row_moved_too() {
    // The record that was last gains a terminator, so two records read
    // differently and a splice that named only the new one would leave the old
    // last row drawn the way it was.
    let mut document = load("no-trailing-newline.csv");

    assert_eq!(
        document.insert_row(document.row_count()),
        Extent::Rows {
            at: 1,
            gone: 1,
            come: 2
        }
    );
}

#[test]
fn undo_says_a_cell_edit_moved_only_its_own_row() {
    let mut document = load("plain.csv");
    document.set_value(1, 1, "Lovelace");

    assert_eq!(document.undo(), Some(Extent::Record(1)));
}

#[test]
fn every_corpus_file_survives_each_structural_change_and_its_undo() {
    for name in [
        "plain.csv",
        "quoted.csv",
        "crlf.csv",
        "mixed-endings.csv",
        "no-trailing-newline.csv",
        "ragged.csv",
        "bom.csv",
        "text-preservation.csv",
        "odd-quoting.csv",
        "semicolon.csv",
        "european.csv",
        "tabs.tsv",
        "pipe.csv",
        "ascii-separated.dsv",
        "whitespace.csv",
        "german.csv",
    ] {
        let original = read(name);

        for (what, change) in changes() {
            let mut document = load(name);
            change(&mut document);

            let written = document.to_bytes();
            Document::from_bytes(&written, dialect(name))
                .unwrap_or_else(|error| panic!("{name} would not reload after {what}: {error}"));

            document.undo();
            assert_eq!(
                visible(&document.to_bytes()),
                visible(&original),
                "{name} did not come back from {what}"
            );
        }
    }
}

/// Something done to a document, named so a failure says which one it was. The
/// extent each one reports is what the window redraws from, and is checked on
/// its own; these tests are about what the change did to the file.
type Change = (&'static str, fn(&mut Document) -> Extent);

/// One of each kind of structural change, aimed at a position every corpus file
/// has.
fn changes() -> Vec<Change> {
    vec![
        ("an inserted first row", |document| document.insert_row(0)),
        ("an appended row", |document| {
            document.insert_row(document.row_count())
        }),
        ("a deleted first row", |document| document.delete_row(0)),
        ("a deleted last row", |document| {
            document.delete_row(document.row_count() - 1)
        }),
        ("an inserted first column", |document| {
            document.insert_column(0)
        }),
        ("an appended column", |document| {
            document.insert_column(document.column_count())
        }),
        ("a deleted first column", |document| {
            document.delete_column(0)
        }),
    ]
}
