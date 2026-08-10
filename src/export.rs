// What Comma writes and will not read back.
//
// An export is output. It is never reopened, never offered as Save, and never
// becomes the document: the file that was opened stays the document, and
// exporting leaves it exactly as it was. That is also why an export is a picture
// of the *grid* rather than of the file — what you exported is what you were
// looking at, in the order you put it in and without the rows a search was
// hiding.
//
// Values are written as text in every format, which is the same promise the rest
// of Comma makes. A spreadsheet that decided `007` was the number seven would
// have thrown away the thing the file was keeping.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod html;
mod ods;

pub use html::write as html;
pub use ods::write as ods;

use crate::document::Document;

/// A picture of the grid, as an export needs it.
pub struct Sheet<'a> {
    pub document: &'a Document,
    /// What to head each column with: the file's own titles, or the spreadsheet
    /// letters when it has none.
    pub columns: &'a [String],
    /// Whether `columns` are the file's own titles. A format that supplies its
    /// own column letters does not want Comma's written in as data.
    pub titled: bool,
    /// The records to write, in the order they are to appear.
    pub rows: &'a [usize],
    /// What to call the table.
    pub name: &'a str,
}

impl Sheet<'_> {
    pub fn width(&self) -> usize {
        self.columns.len()
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// One value, addressed by where it is in this sheet rather than where it is
    /// in the file: a sheet can be in any order and can leave rows out.
    ///
    /// An export writes a rectangle. A record too short to reach a column reads
    /// as empty there, which is what a table wants and what the grid shows.
    pub fn value(&self, row: usize, column: usize) -> &str {
        self.document.value(self.rows[row], column)
    }
}

/// Text as a markup document can carry it.
///
/// The characters that go missing are the ones neither XML nor HTML can hold at
/// all — the control codes below a space, apart from tab and the line breaks.
/// That is a limit of the formats being written rather than a decision about the
/// file, which still holds every byte it always did.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            character if (character as u32) < 0x20 => {}
            character => escaped.push(character),
        }
    }

    escaped
}
