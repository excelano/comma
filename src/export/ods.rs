// A table as an OpenDocument spreadsheet.
//
// An ODS file is a zip holding a few XML documents. Only three of them are
// needed for a spreadsheet a reader will open: the mimetype, which has to be the
// first entry and stored uncompressed for a reader to find it without unzipping;
// the manifest, which lists what is inside; and the content, which is the table.
// Styles and metadata are left out because Comma has nothing to say in them.
//
// Every cell is written as a string. That is the point of exporting from here
// rather than opening the CSV in a spreadsheet: `007` stays `007`, a sixteen
// digit account number stays itself instead of becoming 1.23E+15, and a date is
// whatever the file said it was.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::{Sheet, escape};

const MIMETYPE: &str = "application/vnd.oasis.opendocument.spreadsheet";

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">
 <manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>
 <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
</manifest:manifest>
"#;

/// Writes the sheet as an ODS file.
///
/// The bytes are built in memory, where writing cannot fail; the only errors zip
/// reports here would be ones that cannot happen without the machine already
/// being in trouble.
pub fn write(sheet: &Sheet) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut add = |name: &str, options: SimpleFileOptions, contents: &[u8]| {
        let written = "a zip file being built in memory cannot fail to be written to";
        zip.start_file(name, options).expect(written);
        zip.write_all(contents).expect(written);
    };

    add("mimetype", stored, MIMETYPE.as_bytes());
    add("META-INF/manifest.xml", deflated, MANIFEST.as_bytes());
    add("content.xml", deflated, content(sheet).as_bytes());

    zip.finish()
        .expect("a zip file being built in memory cannot fail to be finished")
        .into_inner()
}

fn content(sheet: &Sheet) -> String {
    let mut xml = String::new();

    let _ = write!(
        xml,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.3">
 <office:body>
  <office:spreadsheet>
   <table:table table:name="{}">
"#,
        escape(sheet.name)
    );

    // A spreadsheet heads its own columns with letters, so Comma's stand-in
    // letters are not written in as though they were data. A file's own titles
    // are data, and go in as the first row.
    if sheet.titled {
        write_row(&mut xml, sheet.columns.iter().map(String::as_str));
    }
    for row in 0..sheet.height() {
        write_row(
            &mut xml,
            (0..sheet.width()).map(|column| sheet.value(row, column)),
        );
    }

    xml.push_str(
        "   </table:table>\n  </office:spreadsheet>\n </office:body>\n</office:document-content>\n",
    );
    xml
}

fn write_row<'a>(xml: &mut String, values: impl Iterator<Item = &'a str>) {
    xml.push_str("    <table:table-row>\n");

    for value in values {
        xml.push_str(r#"     <table:table-cell office:value-type="string">"#);
        // A value holding a line break is several paragraphs, which is how
        // OpenDocument says a cell holds more than one line.
        for line in escape(value).split('\n') {
            let _ = write!(xml, "<text:p>{line}</text:p>");
        }
        xml.push_str("</table:table-cell>\n");
    }

    xml.push_str("    </table:table-row>\n");
}
