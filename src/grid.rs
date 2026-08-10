// The grid.
//
// GtkColumnView wants a GListModel whose items are GObjects, one per row, and a
// Document is neither. `RowModel` is the bridge: it holds the document and
// builds a `Row` for a position only when the view asks for one. The view asks
// only about rows it is about to draw, so opening a large file costs one object
// per visible row rather than one per row in the file.
//
// Columns are built to match the document rather than declared in the template,
// because how many there are is not known until a file is open.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod cell;
mod letters;
mod model;
mod row;

pub use letters::column_letter;
pub use model::RowModel;
pub use row::Row;

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// The width a data column gets before anyone drags it. Columns that size
/// themselves to their contents would change width as you scroll and new rows
/// are realised, which reads as the grid shifting under the pointer.
const DEFAULT_COLUMN_WIDTH: i32 = 160;

/// What a cell says about itself. The grid knows how to show a value and how to
/// take a new one; what any of that means is not its business.
pub enum Event {
    /// The cell has the keyboard focus, and so is where the next thing the user
    /// asks for is to happen.
    Focused,
    /// An edit finished, leaving this value behind.
    Edited(String),
}

/// Rebuilds the view's columns: a row-number gutter wide enough for `rows`,
/// then one column per title.
///
/// `report` is called with a row, a column, and whatever the cell there has to
/// say.
pub fn set_columns(
    column_view: &gtk::ColumnView,
    titles: &[String],
    rows: usize,
    report: impl Fn(usize, usize, Event) + 'static,
) {
    remove_all_columns(column_view);
    column_view.append_column(&gutter_column(rows));

    let report: Rc<cell::Report> = Rc::new(report);
    for (index, title) in titles.iter().enumerate() {
        column_view.append_column(&data_column(index, title, report.clone()));
    }
}

fn remove_all_columns(column_view: &gtk::ColumnView) {
    let columns = column_view.columns();
    while let Some(column) = columns.item(0).and_downcast::<gtk::ColumnViewColumn>() {
        column_view.remove_column(&column);
    }
}

/// The row numbers. Sized to the widest number the file can show, so it does
/// not grow as you scroll into four-digit territory.
fn gutter_column(rows: usize) -> gtk::ColumnViewColumn {
    let digits = rows.to_string().len() as i32;

    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let label = gtk::Label::builder()
            .xalign(1.0)
            .width_chars(digits)
            .css_classes(["dim-label", "numeric"])
            .build();
        as_cell(item).set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let cell = as_cell(item);
        let label = cell
            .child()
            .and_downcast::<gtk::Label>()
            .expect("setup put a label here");
        let row = cell.item().and_downcast::<Row>().expect("rows hold Rows");
        label.set_text(&row.number().to_string());
    });

    gtk::ColumnViewColumn::builder()
        .factory(&factory)
        .resizable(false)
        .build()
}

fn data_column(index: usize, title: &str, report: Rc<cell::Report>) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        cell::setup(as_cell(item), index, report.clone());
    });
    factory.connect_bind(move |_, item| {
        cell::bind(as_cell(item), index);
    });

    gtk::ColumnViewColumn::builder()
        .title(title)
        .factory(&factory)
        .resizable(true)
        .fixed_width(DEFAULT_COLUMN_WIDTH)
        .build()
}

fn as_cell(item: &glib::Object) -> &gtk::ColumnViewCell {
    item.downcast_ref::<gtk::ColumnViewCell>()
        .expect("a column view factory is handed cells")
}
