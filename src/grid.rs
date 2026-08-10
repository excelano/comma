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
mod order;
mod row;

pub use cell::Cell;
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

/// An edit that finished. The grid knows how to take a value from someone; what
/// to do with it is not its business.
pub struct Edited {
    pub row: usize,
    pub column: usize,
    pub value: String,
    /// True when Enter finished the edit rather than the focus simply going
    /// somewhere else.
    pub moving_on: bool,
}

/// Rebuilds the view's columns: a row-number gutter wide enough for `rows`,
/// then one column per title.
///
/// `report` is called each time an edit finishes.
pub fn set_columns(
    column_view: &gtk::ColumnView,
    titles: &[String],
    rows: usize,
    report: impl Fn(Edited) + 'static,
) {
    remove_all_columns(column_view);
    column_view.append_column(&gutter_column(rows));

    let report: Rc<cell::Report> = Rc::new(report);
    for (index, title) in titles.iter().enumerate() {
        column_view.append_column(&data_column(index, title, report.clone()));
    }
}

/// Puts the keyboard on one cell of the table, if that cell has been drawn.
///
/// Only the cells on screen exist; the rest are made as they scroll into view.
/// So this says whether it found one, and a caller that has just asked the view
/// to scroll somewhere can ask again once it has.
pub fn focus_cell(column_view: &gtk::ColumnView, position: u32, column: usize) -> bool {
    match find_cell(column_view.clone().upcast(), position, column) {
        Some(cell) => cell.grab_focus(),
        None => false,
    }
}

fn find_cell(widget: gtk::Widget, position: u32, column: usize) -> Option<Cell> {
    if let Some(cell) = widget.downcast_ref::<Cell>()
        && cell.position() == position
        && cell.column() == column
    {
        return Some(cell.clone());
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if let Some(cell) = find_cell(current, position, column) {
            return Some(cell);
        }
    }
    None
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
    let heading = title.to_string();
    factory.connect_bind(move |_, item| {
        cell::bind(as_cell(item), index, &heading);
    });

    gtk::ColumnViewColumn::builder()
        .title(title)
        .factory(&factory)
        .sorter(&value_sorter(index))
        .resizable(true)
        .fixed_width(DEFAULT_COLUMN_WIDTH)
        .build()
}

/// Giving a column a sorter is what makes its header a thing you can click, and
/// what puts the arrow there once you have.
fn value_sorter(index: usize) -> gtk::CustomSorter {
    gtk::CustomSorter::new(move |left, right| {
        let value = |object: &glib::Object| {
            object
                .downcast_ref::<Row>()
                .expect("the model holds Rows")
                .value(index)
        };
        order::compare(&value(left), &value(right)).into()
    })
}

fn as_cell(item: &glib::Object) -> &gtk::ColumnViewCell {
    item.downcast_ref::<gtk::ColumnViewCell>()
        .expect("a column view factory is handed cells")
}
