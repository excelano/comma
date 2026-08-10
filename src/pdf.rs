// A table as pages.
//
// Drawn through GtkPrintOperation, which is GTK's own way of turning content
// into a document. Asked to export rather than to print, it writes a PDF and
// never opens a printer dialog — and it is the same code printing would use, if
// Comma ever grows a Print.
//
// The sheet is copied into owned text first, because a print operation runs
// through callbacks that outlive the call and cannot borrow the document. That
// is a second copy of everything being exported, which is the price of the only
// route to a PDF that lays text out properly.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::Cell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

use comma::export::Sheet;

/// Points of white space between one column's text and the next.
const GAP: f64 = 8.0;

/// Points of white space around the page. Exporting has no printer to ask about
/// margins, and a table run right up to the edge of the paper reads badly and
/// cannot be printed by most of them anyway.
const MARGIN: f64 = 36.0;

/// How many rows are looked at to decide how wide a column should be. Every row
/// would mean reading the whole file to draw its first page, and a column that
/// fits its first few hundred values fits it well enough.
const SAMPLE: usize = 200;

/// The sheet as the print callbacks need it: owned, and flat.
struct Table {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// How a page is laid out, worked out once the paper size is known.
#[derive(Default)]
struct Layout {
    widths: Vec<f64>,
    line: f64,
    rows_per_page: usize,
}

pub fn write(sheet: &Sheet, path: &str, window: &impl IsA<gtk::Window>) -> Result<(), glib::Error> {
    let table = Rc::new(Table {
        name: sheet.name.to_string(),
        columns: sheet.columns.to_vec(),
        rows: (0..sheet.height())
            .map(|row| {
                (0..sheet.width())
                    .map(|column| sheet.value(row, column).to_string())
                    .collect()
            })
            .collect(),
    });
    let layout = Rc::new(std::cell::RefCell::new(Layout::default()));

    let page = gtk::PageSetup::new();
    for set in [
        gtk::PageSetup::set_top_margin,
        gtk::PageSetup::set_bottom_margin,
        gtk::PageSetup::set_left_margin,
        gtk::PageSetup::set_right_margin,
    ] {
        set(&page, MARGIN, gtk::Unit::Points);
    }

    let print = gtk::PrintOperation::new();
    print.set_export_filename(path);
    print.set_job_name(&table.name);
    print.set_unit(gtk::Unit::Points);
    print.set_default_page_setup(Some(&page));

    print.connect_begin_print(glib::clone!(
        #[strong]
        table,
        #[strong]
        layout,
        move |print, context| {
            let measured = measure(&table, context);
            let pages = table.rows.len().div_ceil(measured.rows_per_page.max(1));
            print.set_n_pages(pages.max(1) as i32);
            layout.replace(measured);
        }
    ));

    print.connect_draw_page(glib::clone!(
        #[strong]
        table,
        #[strong]
        layout,
        move |print, context, page| {
            draw(&table, &layout.borrow(), context, page as usize);
            foot(&table, context, page as usize, print.n_pages() as usize);
        }
    ));

    print
        .run(gtk::PrintOperationAction::Export, Some(window.as_ref()))
        .map(|_| ())
}

/// Works out how wide each column should be and how many rows fit on a page.
///
/// Columns are measured by their widest sampled value and then scaled to the
/// page: wide columns give up more than narrow ones, which keeps a column of
/// short codes narrow instead of sharing the page out equally.
fn measure(table: &Table, context: &gtk::PrintContext) -> Layout {
    let text = |value: &str, bold: bool| {
        let layout = context.create_pango_layout();
        layout.set_text(value);
        if bold {
            let mut description = layout.context().font_description().unwrap_or_default();
            description.set_weight(pango::Weight::Bold);
            layout.set_font_description(Some(&description));
        }
        let (width, height) = layout.pixel_size();
        (width as f64, height as f64)
    };

    let mut widths: Vec<f64> = table
        .columns
        .iter()
        .map(|title| text(title, true).0)
        .collect();
    for row in table.rows.iter().take(SAMPLE) {
        for (column, value) in row.iter().enumerate().take(widths.len()) {
            widths[column] = widths[column].max(text(value, false).0);
        }
    }

    let gaps = GAP * widths.len().saturating_sub(1) as f64;
    let available = (context.width() - gaps).max(1.0);
    let wanted: f64 = widths.iter().sum();
    if wanted > available {
        let scale = available / wanted;
        for width in &mut widths {
            *width *= scale;
        }
    }

    let line = text("Ag", false).1 + 2.0;
    Layout {
        widths,
        line,
        // Two lines go to the column heads and their rule, and two more to the
        // footer and the gap above it.
        rows_per_page: (((context.height() - line * 4.0) / line) as usize).max(1),
    }
}

fn draw(table: &Table, layout: &Layout, context: &gtk::PrintContext, page: usize) {
    let cairo = context.cairo_context();
    let first = page * layout.rows_per_page;
    let last = (first + layout.rows_per_page).min(table.rows.len());

    let at = Cell::new(0.0);
    let line = |values: &[String], bold: bool| {
        let mut x = 0.0;
        for (column, width) in layout.widths.iter().enumerate() {
            let text = values.get(column).map(String::as_str).unwrap_or_default();
            // A value with a line break in it is drawn on one line here: a page
            // of a table wants rows of the same height, and the whole value is
            // still in the file it was exported from.
            let cell = context.create_pango_layout();
            cell.set_text(&text.replace('\n', " "));
            cell.set_width((*width * pango::SCALE as f64) as i32);
            cell.set_ellipsize(pango::EllipsizeMode::End);
            if bold {
                let mut description = cell.context().font_description().unwrap_or_default();
                description.set_weight(pango::Weight::Bold);
                cell.set_font_description(Some(&description));
            }

            cairo.move_to(x, at.get());
            pangocairo::functions::show_layout(&cairo, &cell);
            x += width + GAP;
        }
        at.set(at.get() + layout.line);
    };

    line(&table.columns, true);
    let rule = at.get() - layout.line * 0.15;
    for row in &table.rows[first..last] {
        line(row, false);
    }

    // One rule under the column heads. Lines around every cell would be a lot of
    // ink for something a reader can already see the shape of.
    cairo.set_line_width(0.5);
    cairo.move_to(0.0, rule);
    cairo.line_to(context.width(), rule);
    let _ = cairo.stroke();
}

/// What file this was and how far through it you are. A table spread over pages
/// is not much use without both.
fn foot(table: &Table, context: &gtk::PrintContext, page: usize, pages: usize) {
    let cairo = context.cairo_context();
    let line = context.create_pango_layout();
    line.set_width((context.width() * pango::SCALE as f64) as i32);
    line.set_ellipsize(pango::EllipsizeMode::Middle);
    line.set_text(&table.name);

    let baseline = context.height() - line.pixel_size().1 as f64;
    cairo.move_to(0.0, baseline);
    pangocairo::functions::show_layout(&cairo, &line);

    line.set_alignment(pango::Alignment::Right);
    line.set_text(&gettext("Page {of}").replace("{of}", &format!("{} of {pages}", page + 1)));
    cairo.move_to(0.0, baseline);
    pangocairo::functions::show_layout(&cairo, &line);
}
