// What an edit costs the file.
//
// The claim Comma makes is not only that an untouched file survives, but that a
// touched one changes in one place. These tests measure the size of the change.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod support;

use support::{dialect, load, read, visible};

use comma::document::{Dialect, Document};

/// The line numbers on which two files differ, counting a line as everything up
/// to and including its terminator.
fn differing_lines(before: &[u8], after: &[u8]) -> Vec<usize> {
    let before = String::from_utf8_lossy(before);
    let after = String::from_utf8_lossy(after);
    let before: Vec<&str> = before.split_inclusive('\n').collect();
    let after: Vec<&str> = after.split_inclusive('\n').collect();

    assert_eq!(
        before.len(),
        after.len(),
        "the number of lines changed, so a line-by-line comparison would mislead"
    );

    (0..before.len())
        .filter(|&line| before[line] != after[line])
        .collect()
}

#[test]
fn editing_one_cell_changes_one_line() {
    let name = "crlf.csv";
    let original = read(name);
    let mut document = load(name);

    document.set_value(1, 1, "Ada Lovelace");

    let written = document.to_bytes();
    assert_eq!(differing_lines(&original, &written), vec![1]);
}

#[test]
fn the_untouched_lines_are_untouched_byte_for_byte() {
    let name = "odd-quoting.csv";
    let original = read(name);
    let mut document = load(name);

    document.set_value(1, 0, "changed");

    let written = document.to_bytes();
    let before = String::from_utf8_lossy(&original);
    let after = String::from_utf8_lossy(&written);
    let before: Vec<&str> = before.split_inclusive('\n').collect();
    let after: Vec<&str> = after.split_inclusive('\n').collect();

    assert_eq!(before[0], after[0]);
    assert_eq!(before[2], after[2]);
}

#[test]
fn an_edit_does_not_disturb_odd_quoting_elsewhere_in_the_same_line() {
    // The second line is `"plain","x"y,""`. Every field on it is written in a
    // way Comma would not choose for itself, so rewriting the line rather than
    // the cell would be plainly visible.
    let mut document = load("odd-quoting.csv");
    document.set_value(1, 0, "changed");

    let written = String::from_utf8(document.to_bytes()).unwrap();
    let line = written.lines().nth(1).unwrap();
    assert_eq!(line, r#"changed,"x"y,"""#);
}

#[test]
fn an_edited_value_is_quoted_only_when_it_has_to_be() {
    let mut document = load("plain.csv");

    document.set_value(1, 1, "Lovelace, Ada");
    document.set_value(2, 1, "Hopper");

    let written = String::from_utf8(document.to_bytes()).unwrap();
    assert_eq!(
        written.lines().nth(1).unwrap(),
        r#"1,"Lovelace, Ada",London"#
    );
    assert_eq!(written.lines().nth(2).unwrap(), "2,Hopper,Arlington");
}

#[test]
fn a_value_containing_a_quote_is_escaped_on_the_way_out() {
    let mut document = load("plain.csv");
    document.set_value(1, 1, r#"Ada "the Countess""#);

    let written = String::from_utf8(document.to_bytes()).unwrap();
    assert_eq!(
        written.lines().nth(1).unwrap(),
        r#"1,"Ada ""the Countess""",London"#
    );
}

#[test]
fn a_value_containing_a_newline_survives_a_save_and_a_reload() {
    let mut document = load("plain.csv");
    document.set_value(1, 2, "London\nEngland");

    let written = document.to_bytes();
    let reloaded = Document::from_bytes(&written, Dialect::comma()).unwrap();

    assert_eq!(reloaded.value(1, 2), "London\nEngland");
    assert_eq!(reloaded.row_count(), 3);
}

#[test]
fn editing_past_the_end_of_a_short_record_widens_it() {
    let mut document = load("ragged.csv");
    assert_eq!(document.field_count(1), 1);

    document.set_value(1, 2, "z");

    assert_eq!(document.field_count(1), 3);
    let written = String::from_utf8(document.to_bytes()).unwrap();
    assert_eq!(written.lines().nth(1).unwrap(), "d,,z");
}

#[test]
fn a_line_ending_belongs_to_its_own_line() {
    let name = "mixed-endings.csv";
    let original = read(name);
    let mut document = load(name);

    document.set_value(1, 0, "C");

    let written = document.to_bytes();
    assert_eq!(
        visible(&written),
        visible(b"a,b\r\nC,d\ne,f\r\n"),
        "editing a line-feed line must not give it a carriage return"
    );
    assert_eq!(differing_lines(&original, &written), vec![1]);
}

#[test]
fn saving_a_document_with_a_byte_order_mark_keeps_it() {
    let mut document = load("bom.csv");
    document.set_value(1, 1, "Lovelace");

    let written = document.to_bytes();
    assert_eq!(&written[..3], &[0xEF, 0xBB, 0xBF]);
}

#[test]
fn a_document_reports_whether_it_has_been_changed() {
    let mut document = load("plain.csv");
    assert!(!document.is_modified());

    document.set_value(1, 1, "Lovelace");
    assert!(document.is_modified());

    document.mark_saved();
    assert!(!document.is_modified());
}

#[test]
fn setting_a_cell_to_what_it_already_holds_changes_nothing() {
    // Every field on this file's second line is spelled in a way Comma would
    // not choose for itself, so a field rewritten rather than left alone shows
    // up immediately.
    let name = "odd-quoting.csv";
    let original = read(name);
    let mut document = load(name);

    let unchanged = document.value(1, 1).to_owned();
    document.set_value(1, 1, unchanged);

    assert_eq!(visible(&document.to_bytes()), visible(&original));
    assert!(!document.is_modified());
    assert!(!document.can_undo());
}

#[test]
fn undo_puts_the_original_bytes_back() {
    let name = "odd-quoting.csv";
    let original = read(name);
    let mut document = load(name);

    document.set_value(1, 1, "changed");
    assert_ne!(visible(&document.to_bytes()), visible(&original));

    assert_eq!(document.undo(), Some(1), "undo reports the row it restored");
    assert_eq!(
        visible(&document.to_bytes()),
        visible(&original),
        "undo has to restore the field's original spelling, not just its value"
    );
}

#[test]
fn redo_puts_the_edit_back() {
    let mut document = load("plain.csv");

    document.set_value(1, 1, "Lovelace");
    document.undo();
    assert_eq!(document.value(1, 1), "Ada");

    assert_eq!(document.redo(), Some(1));
    assert_eq!(document.value(1, 1), "Lovelace");
    assert!(!document.can_redo());
}

#[test]
fn undo_runs_out_at_the_file_it_started_from() {
    let mut document = load("plain.csv");
    assert!(!document.can_undo());
    assert!(!document.can_redo());

    document.set_value(1, 1, "one");
    document.set_value(1, 1, "two");

    assert_eq!(document.undo(), Some(1));
    assert_eq!(document.undo(), Some(1));
    assert_eq!(document.undo(), None);
    assert_eq!(document.value(1, 1), "Ada");
}

#[test]
fn undoing_a_widening_edit_narrows_the_record_again() {
    let name = "ragged.csv";
    let original = read(name);
    let mut document = load(name);

    document.set_value(1, 2, "z");
    assert_eq!(document.field_count(1), 3);

    document.undo();
    assert_eq!(document.field_count(1), 1);
    assert_eq!(visible(&document.to_bytes()), visible(&original));
}

#[test]
fn a_new_edit_discards_what_was_undone() {
    let mut document = load("plain.csv");

    document.set_value(1, 1, "one");
    document.undo();
    assert!(document.can_redo());

    document.set_value(1, 1, "two");
    assert!(
        !document.can_redo(),
        "the new edit happens instead of the old"
    );

    document.undo();
    assert_eq!(document.value(1, 1), "Ada");
}

#[test]
fn undoing_back_to_the_saved_state_is_not_modified() {
    let mut document = load("plain.csv");
    document.set_value(1, 1, "Lovelace");
    document.mark_saved();

    document.set_value(2, 1, "Hopper");
    assert!(document.is_modified());

    document.undo();
    assert!(
        !document.is_modified(),
        "this is the file that was written, so there is nothing to write"
    );

    document.redo();
    assert!(document.is_modified());
}

#[test]
fn a_saved_state_that_can_no_longer_be_reached_stays_modified() {
    let mut document = load("plain.csv");

    document.set_value(1, 1, "Lovelace");
    document.mark_saved();

    // Undoing past the save and then editing throws away the only route back
    // to what is on disk, so the document differs from it from here on.
    document.undo();
    document.set_value(2, 1, "Hopper");
    assert!(document.is_modified());

    document.undo();
    assert!(document.is_modified());
}

#[test]
fn a_document_can_be_told_it_no_longer_matches_its_file() {
    let mut document = load("plain.csv");
    assert!(!document.is_modified());

    document.mark_modified();
    assert!(document.is_modified());
    assert!(!document.can_undo(), "that was not an edit");
}

#[test]
fn every_corpus_file_survives_an_edit_and_a_reload() {
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
        let mut document = load(name);
        document.set_value(0, 0, "edited");

        let written = document.to_bytes();
        let reloaded = Document::from_bytes(&written, dialect(name))
            .unwrap_or_else(|error| panic!("{name} would not reload after an edit: {error}"));

        assert_eq!(reloaded.value(0, 0), "edited", "{name}");
        assert_eq!(
            visible(&reloaded.to_bytes()),
            visible(&written),
            "{name} did not settle after one save"
        );
    }
}
