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

mod letters;
mod model;
mod row;

pub use model::RowModel;
pub use row::Row;

use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

use self::letters::column_letter;

/// The width a data column gets before anyone drags it. Columns that size
/// themselves to their contents would change width as you scroll and new rows
/// are realised, which reads as the grid shifting under the pointer.
const DEFAULT_COLUMN_WIDTH: i32 = 160;

/// Rebuilds the view's columns for a document of this shape: a row-number
/// gutter followed by `columns` lettered data columns.
pub fn set_columns(column_view: &gtk::ColumnView, columns: usize, rows: usize) {
    remove_all_columns(column_view);
    column_view.append_column(&gutter_column(rows));

    for index in 0..columns {
        column_view.append_column(&data_column(index));
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
    factory.connect_setup(move |_, cell| {
        let label = gtk::Label::builder()
            .xalign(1.0)
            .width_chars(digits)
            .css_classes(["dim-label", "numeric"])
            .build();
        cell.downcast_ref::<gtk::ColumnViewCell>()
            .expect("a column view factory is handed cells")
            .set_child(Some(&label));
    });
    factory.connect_bind(|_, cell| {
        let (cell, label) = cell_and_label(cell);
        let row = cell.item().and_downcast::<Row>().expect("rows hold Rows");
        label.set_text(&row.number().to_string());
    });

    gtk::ColumnViewColumn::builder()
        .factory(&factory)
        .resizable(false)
        .build()
}

fn data_column(index: usize) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, cell| {
        let label = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            // Without this the label's own idea of how wide it wants to be
            // wins and the column stops honouring its width.
            .max_width_chars(1)
            .build();
        cell.downcast_ref::<gtk::ColumnViewCell>()
            .expect("a column view factory is handed cells")
            .set_child(Some(&label));
    });
    factory.connect_bind(move |_, cell| {
        let (cell, label) = cell_and_label(cell);
        let row = cell.item().and_downcast::<Row>().expect("rows hold Rows");
        label.set_text(&row.value(index));
    });

    gtk::ColumnViewColumn::builder()
        .title(column_letter(index))
        .factory(&factory)
        .resizable(true)
        .fixed_width(DEFAULT_COLUMN_WIDTH)
        .build()
}

fn cell_and_label(cell: &glib::Object) -> (gtk::ColumnViewCell, gtk::Label) {
    let cell = cell
        .downcast_ref::<gtk::ColumnViewCell>()
        .expect("a column view factory is handed cells")
        .clone();
    let label = cell
        .child()
        .and_downcast::<gtk::Label>()
        .expect("setup put a label here");
    (cell, label)
}
