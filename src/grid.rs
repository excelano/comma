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
// The row numbers are not among them. They are a view of their own, next door in
// `gutter`, so that they stay put while the table scrolls sideways.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod cell;
mod gutter;
mod model;
mod number;
mod order;
mod row;

pub use cell::{Cell, LINE_BREAK};
pub use gutter::Gutter;
pub use model::RowModel;
pub use row::Row;

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// The width a data column gets before anyone drags it. Columns that size
/// themselves to their contents would change width as you scroll and new rows
/// are realised, which reads as the grid shifting under the pointer.
const DEFAULT_COLUMN_WIDTH: i32 = 160;

/// Which record of the file a row of the view is showing, asked at the moment it
/// matters rather than remembered from when the row was last bound.
///
/// A splice above a row moves it without binding it again. The values it holds
/// are still that record's values, so the view is right to leave them where they
/// are, but which record they belong to has moved underneath. So anything that
/// addresses the document — the number a row shows, the record an edit is written
/// to — asks where the row is now rather than where it was.
#[derive(Clone, Debug)]
pub struct Records(gtk::SelectionModel);

impl Records {
    pub fn new(model: &impl IsA<gtk::SelectionModel>) -> Self {
        Self(model.clone().upcast())
    }

    /// The row a position is showing, or nothing if the view has since moved on
    /// from that position.
    pub(crate) fn at(&self, position: u32) -> Option<Row> {
        self.0.item(position).and_downcast::<Row>()
    }
}

/// Where a widget's row sits in the view at this moment.
///
/// GTK keeps this current as rows move, which is what makes it worth asking. A
/// widget that has been handed back and not yet handed out again has no position
/// at all, and says so.
pub(crate) fn position_of(item: &glib::WeakRef<gtk::ColumnViewCell>) -> Option<u32> {
    let position = item.upgrade()?.position();
    (position != gtk::INVALID_LIST_POSITION).then_some(position)
}

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

/// Rebuilds the view's columns, one per title. The row numbers are not among
/// them: they are a view of their own, beside this one.
///
/// `report` is called each time an edit finishes.
pub fn set_columns(
    column_view: &gtk::ColumnView,
    titles: &[String],
    records: &Records,
    report: impl Fn(Edited) + 'static,
) {
    remove_all_columns(column_view);

    let report: Rc<cell::Report> = Rc::new(report);
    for (index, title) in titles.iter().enumerate() {
        column_view.append_column(&data_column(index, title, records.clone(), report.clone()));
    }
}

/// Puts the keyboard on one cell of the table, if that cell has been drawn.
///
/// Only the cells on screen exist; the rest are made as they scroll into view.
/// So this says whether it found one, and a caller that has just asked the view
/// to scroll somewhere can ask again once it has.
pub fn focus_cell(column_view: &gtk::ColumnView, position: u32, column: usize) -> bool {
    let wanted = |cell: &Cell| cell.position() == Some(position) && cell.column() == column;
    match find::<Cell>(column_view.upcast_ref(), &wanted) {
        Some(cell) => cell.grab_focus(),
        None => false,
    }
}

/// The first cell anywhere inside a widget, for when the keyboard has landed on
/// something that holds cells rather than on one of them.
pub fn cell_within(widget: &gtk::Widget) -> Option<Cell> {
    find::<Cell>(widget, &|_| true)
}

/// A point inside one of the grid's widgets, in the table's own coordinates,
/// which is where a menu that hangs off the table has to be told to point.
fn point_in_view(widget: &gtk::Widget, x: f64, y: f64) -> Option<(f64, f64)> {
    let view = widget.ancestor(gtk::ColumnView::static_type())?;
    let at = widget.compute_point(&view, &gtk::graphene::Point::new(x as f32, y as f32))?;
    Some((at.x() as f64, at.y() as f64))
}

/// The first widget of a kind inside another that answers to `wanted`.
fn find<T: IsA<gtk::Widget>>(widget: &gtk::Widget, wanted: &dyn Fn(&T) -> bool) -> Option<T> {
    let mut found = None;
    visit(widget, &mut |candidate: &T| {
        if !wanted(candidate) {
            return glib::ControlFlow::Continue;
        }
        found = Some(candidate.clone());
        glib::ControlFlow::Break
    });
    found
}

/// Hands `act` every widget of a kind inside another, until it says to stop.
///
/// Only the widgets on screen are here to be walked: the rest of the file has
/// none yet, and the ones it had have been handed to another row by now.
pub(crate) fn visit<T: IsA<gtk::Widget>>(
    widget: &gtk::Widget,
    act: &mut dyn FnMut(&T) -> glib::ControlFlow,
) -> glib::ControlFlow {
    if let Some(found) = widget.downcast_ref::<T>() {
        return act(found);
    }

    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        if visit(&current, act) == glib::ControlFlow::Break {
            return glib::ControlFlow::Break;
        }
    }
    glib::ControlFlow::Continue
}

/// How tall something showing this many lines of text has to be: one line's
/// height for each of them, measured on the widget that will show them.
///
/// Counted a line at a time rather than laid out all at once, because that is
/// how the text view does it. Three lines laid out together come to one pixel
/// less than three lines measured one by one, and that pixel is room for the
/// editor to scroll in, which is a shift every time the caret crosses between
/// lines.
///
/// A cell asks this for its own value and a row number asks it for the tallest
/// value in the row, so a row and the number naming it come out the same height
/// by construction rather than by luck. They are in two views now, and nothing
/// else lines them up.
pub(crate) fn height_for_lines(widget: &impl IsA<gtk::Widget>, lines: usize) -> i32 {
    let (_, line) = widget.create_pango_layout(Some("X")).pixel_size();
    line * lines as i32
}

fn remove_all_columns(column_view: &gtk::ColumnView) {
    let columns = column_view.columns();
    while let Some(column) = columns.item(0).and_downcast::<gtk::ColumnViewColumn>() {
        column_view.remove_column(&column);
    }
}

/// How many digits wide the row-number gutter has to be for a file of this many
/// rows. Sized to the widest number the file can show rather than to the ones on
/// screen, so the gutter does not change width as you scroll into four-digit
/// territory.
pub fn gutter_digits(rows: usize) -> i32 {
    rows.to_string().len() as i32
}

fn data_column(
    index: usize,
    title: &str,
    records: Records,
    report: Rc<cell::Report>,
) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        cell::setup(as_cell(item), index, records.clone(), report.clone());
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
