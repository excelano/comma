// What Comma writes that it will not read back.
//
// An export is output, so what these tests measure is that the output says what
// the file said: every value as text, nothing reinterpreted, nothing dropped
// that the format could have carried, and the document left exactly as it was.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod support;

use support::{load, read, visible};

use comma::document::Document;
use comma::export::{self, Sheet};

/// A sheet showing the whole document in file order, headed by its first record.
fn titled(document: &Document) -> (Vec<String>, Vec<usize>) {
    let columns = (0..document.column_count())
        .map(|column| document.value(0, column).to_string())
        .collect();
    let rows = (1..document.row_count()).collect();
    (columns, rows)
}

/// A sheet showing the whole document in file order, headed by column letters.
fn lettered(document: &Document) -> (Vec<String>, Vec<usize>) {
    let columns = (0..document.column_count())
        .map(|column| format!("{}", (b'A' + column as u8) as char))
        .collect();
    let rows = (0..document.row_count()).collect();
    (columns, rows)
}

/// The single XML document inside an ODS file that holds the table.
fn content_of(ods: &[u8]) -> String {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(ods)).expect("an export has to be a zip file");
    let mut content = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name("content.xml")
            .expect("an export has to hold its table"),
        &mut content,
    )
    .unwrap();
    content
}

#[test]
fn html_writes_every_value_the_grid_shows() {
    let document = load("plain.csv");
    let (columns, rows) = titled(&document);
    let page = export::html(&Sheet {
        document: &document,
        columns: &columns,
        titled: true,
        rows: &rows,
        name: "plain.csv",
    });

    assert!(page.contains("<th>name</th>"), "{page}");
    assert!(page.contains("<td>Ada</td>"), "{page}");
    assert!(page.contains("<td>Arlington</td>"), "{page}");
    assert!(
        !page.contains("<td>id</td>"),
        "the record the titles came from is not also a row"
    );
}

#[test]
fn html_escapes_what_would_otherwise_be_markup() {
    let mut document = load("plain.csv");
    document.set_value(1, 1, "<b>Ada</b> & \"Grace\"");

    let (columns, rows) = lettered(&document);
    let page = export::html(&Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        rows: &rows,
        name: "plain.csv",
    });

    assert!(
        page.contains("&lt;b&gt;Ada&lt;/b&gt; &amp; &quot;Grace&quot;"),
        "{page}"
    );
    assert!(!page.contains("<b>Ada</b>"));
}

#[test]
fn a_cell_holding_a_line_break_keeps_it() {
    let mut document = load("plain.csv");
    document.set_value(1, 2, "London\nEngland");

    let (columns, rows) = lettered(&document);
    let sheet = Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        rows: &rows,
        name: "plain.csv",
    };

    // The page keeps the break in the text and asks the browser not to collapse
    // it; the spreadsheet makes it two paragraphs, which is how OpenDocument
    // says a cell holds more than one line.
    let page = export::html(&sheet);
    assert!(page.contains("<td>London\nEngland</td>"), "{page}");
    assert!(page.contains("white-space: pre-wrap"));

    let content = content_of(&export::ods(&sheet));
    assert!(
        content.contains("<text:p>London</text:p><text:p>England</text:p>"),
        "{content}"
    );
}

#[test]
fn every_cell_of_a_spreadsheet_is_written_as_text() {
    // This is the whole reason to export from Comma rather than open the CSV in
    // a spreadsheet: a leading zero and a long identifier survive because
    // nothing is asked to decide what they are.
    let document = load("text-preservation.csv");
    let (columns, rows) = lettered(&document);
    let content = content_of(&export::ods(&Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        rows: &rows,
        name: "text-preservation.csv",
    }));

    assert!(
        !content.contains("office:value-type=\"float\""),
        "nothing may be written as a number"
    );
    assert_eq!(
        content.matches("office:value-type=\"string\"").count(),
        document.row_count() * document.column_count()
    );
}

#[test]
fn a_spreadsheet_starts_with_an_uncompressed_mimetype() {
    // A reader identifies an OpenDocument file by reading the first entry
    // straight out of the zip, which only works if it was stored rather than
    // compressed.
    let document = load("plain.csv");
    let (columns, rows) = lettered(&document);
    let ods = export::ods(&Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        rows: &rows,
        name: "plain.csv",
    });

    let mimetype = "application/vnd.oasis.opendocument.spreadsheet";
    let head = String::from_utf8_lossy(&ods[..120]);
    assert!(head.contains("mimetype"), "{head}");
    assert!(head.contains(mimetype), "{head}");
}

#[test]
fn column_letters_are_not_written_into_a_spreadsheet_as_data() {
    let document = load("plain.csv");
    let (columns, rows) = lettered(&document);
    let content = content_of(&export::ods(&Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        rows: &rows,
        name: "plain.csv",
    }));

    assert_eq!(
        content.matches("<table:table-row>").count(),
        document.row_count(),
        "a spreadsheet heads its own columns, so Comma's stand-in letters stay out"
    );
}

#[test]
fn a_sheet_is_written_in_the_order_it_is_given() {
    let document = load("plain.csv");
    let columns = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let page = export::html(&Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        // Backwards, and missing the first record: an export is a picture of
        // the grid, which can be sorted and can be hiding rows.
        rows: &[2, 1],
        name: "plain.csv",
    });

    let grace = page.find("Grace").expect("the row it was given first");
    let ada = page.find("Ada").expect("the row it was given second");
    assert!(grace < ada);
    assert!(!page.contains("<td>id</td>"));
}

#[test]
fn exporting_leaves_the_document_exactly_as_it_was() {
    let name = "odd-quoting.csv";
    let original = read(name);
    let document = load(name);
    let (columns, rows) = lettered(&document);
    let sheet = Sheet {
        document: &document,
        columns: &columns,
        titled: false,
        rows: &rows,
        name,
    };

    export::html(&sheet);
    export::ods(&sheet);

    assert_eq!(visible(&document.to_bytes()), visible(&original));
    assert!(
        !document.is_modified(),
        "an export is not a change to the file"
    );
}
