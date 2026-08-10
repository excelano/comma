// A table as a web page.
//
// One file, no links out, no scripts: it opens in a browser with nothing else
// present and prints from there. The stylesheet is small enough to read and is
// there so the table is legible rather than to make it handsome.
//
// Cells keep their line breaks, because a value holding one is holding it on
// purpose, and a page that quietly ran the lines together would be showing
// something the file does not say.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fmt::Write;

use super::{Sheet, escape};

const STYLE: &str = "\
body { font-family: sans-serif; margin: 2rem; color: #222; background: #fff; }
h1 { font-size: 1.25rem; font-weight: 600; }
table { border-collapse: collapse; font-size: 0.9rem; }
th, td { border: 1px solid #ccc; padding: 0.25rem 0.5rem; text-align: left;
         vertical-align: top; white-space: pre-wrap; }
th { background: #f2f2f2; font-weight: 600; }
";

pub fn write(sheet: &Sheet) -> String {
    let mut page = String::new();
    let name = escape(sheet.name);

    let _ = write!(
        page,
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{name}</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n\
         <h1>{name}</h1>\n<table>\n<thead>\n<tr>"
    );

    for column in sheet.columns {
        let _ = write!(page, "<th>{}</th>", escape(column));
    }
    page.push_str("</tr>\n</thead>\n<tbody>\n");

    for row in 0..sheet.height() {
        page.push_str("<tr>");
        for column in 0..sheet.width() {
            let _ = write!(page, "<td>{}</td>", escape(sheet.value(row, column)));
        }
        page.push_str("</tr>\n");
    }

    page.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    page
}
