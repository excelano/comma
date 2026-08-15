// The last column's right edge, and dragging it.
//
// Every other column is resized by GTK. The drag it listens for is on the line
// between one column and the next, and it hands that line to the column on the
// left of it, so the rightmost column of a view is offered nothing: there is no
// column to its right for the line to be between. The last column of a file was
// the one column here that could not be made narrower or wider.
//
// So the grid listens for that one drag itself. Same eight pixels across the
// same line, changing the same width by the same amount. What it needs that GTK
// could not have is where it is listening from: the pixels past the end of the
// columns belong to the view rather than to the row of titles, and only
// something bound to the view is offered them. Bound to the titles, half the
// handle would be over nothing and the last column would be caught from the
// inside only.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::Cell;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

/// How far to either side of the line the pointer is still on it. GTK listens
/// across eight pixels of the lines it owns, so this is four of them, and the
/// last column is caught the same way as the rest rather than more fussily.
const GRIP: f64 = 4.0;

/// Lets the last column be dragged by its right edge.
///
/// Wired to the view rather than to a column, and once rather than at every
/// rebuild, because the columns are thrown away and built again each time a file
/// is opened and the view outlives all of them. Which column is last is asked at
/// the moment of the press instead.
pub fn setup(view: &gtk::ColumnView) {
    // The width the column had when the press landed, held until the button
    // comes back up. `None` means no resize is under way, which is also what
    // tells the pointer's own handler that the cursor is its business again.
    let started = Rc::new(Cell::new(None::<i32>));

    let drag = gtk::GestureDrag::new();
    // Ahead of the widgets inside the view, because the press lands on the last
    // column's title and a title takes a press for a click on itself. Left to
    // bubble, every resize of the last column would sort the grid by it on the
    // way back up.
    drag.set_propagation_phase(gtk::PropagationPhase::Capture);

    drag.connect_drag_begin(glib::clone!(
        #[weak]
        view,
        #[strong]
        started,
        move |drag, x, y| {
            let Some(column) = grip(&view, x, y) else {
                return;
            };
            // Claimed, so that the title under the pointer is told the press was
            // not for it.
            drag.set_state(gtk::EventSequenceState::Claimed);
            started.set(Some(column.fixed_width()));
            view.set_cursor_from_name(Some("col-resize"));
        }
    ));

    drag.connect_drag_update(glib::clone!(
        #[weak]
        view,
        #[strong]
        started,
        move |_, moved, _| {
            let (Some(width), Some(column)) = (started.get(), last(&view)) else {
                return;
            };
            // Not past its own left edge. Asking for less than nothing is how a
            // column ends up with a width GTK spends the next frame arguing
            // with; from zero it is the cells that decide how narrow it gets.
            let wanted = (width + moved as i32).max(0);
            // Setting a width lays the grid out again, and laying it out again
            // is what asks this question the next time. Writing only a width
            // that differs keeps a drag that has stopped moving from feeding
            // itself.
            if column.fixed_width() != wanted {
                column.set_fixed_width(wanted);
            }
        }
    ));

    drag.connect_drag_end(glib::clone!(
        #[weak]
        view,
        #[strong]
        started,
        move |_, _, _| {
            if started.take().is_some() {
                view.set_cursor(None);
            }
        }
    ));
    view.add_controller(drag);

    // What says the edge is there to be taken hold of, before anyone tries it.
    let pointer = gtk::EventControllerMotion::new();
    pointer.connect_motion(glib::clone!(
        #[weak]
        view,
        #[strong]
        started,
        move |_, x, y| {
            // A drag under way keeps its cursor wherever the pointer has got to,
            // which is what dragging anything does.
            if started.get().is_some() {
                return;
            }
            match grip(&view, x, y) {
                Some(_) => view.set_cursor_from_name(Some("col-resize")),
                None => view.set_cursor(None),
            }
        }
    ));
    pointer.connect_leave(glib::clone!(
        #[weak]
        view,
        #[strong]
        started,
        move |_| {
            if started.get().is_none() {
                view.set_cursor(None);
            }
        }
    ));
    view.add_controller(pointer);
}

/// The last column, if the pointer is on the edge of it and it is a column that
/// can be dragged at all.
fn grip(view: &gtk::ColumnView, x: f64, y: f64) -> Option<gtk::ColumnViewColumn> {
    let column = last(view)?;
    if !column.is_resizable() {
        return None;
    }

    let titles = titles(view)?.compute_bounds(view)?;
    // Where the last column ends is where the titles end: the row of them is as
    // wide as the columns beneath it and no wider, which is the same fact that
    // left these pixels outside it in the first place.
    let edge = f64::from(titles.x() + titles.width());
    if (x - edge).abs() > GRIP {
        return None;
    }

    // Among the titles rather than anywhere below them. Without this the whole
    // height of the grid would be a handle, and a click on the last column of
    // any row would be a resize instead of a cell.
    let (top, bottom) = (
        f64::from(titles.y()),
        f64::from(titles.y() + titles.height()),
    );
    (y >= top && y <= bottom).then_some(column)
}

/// The view's last column, which is the file's last column: the view has none of
/// its own.
fn last(view: &gtk::ColumnView) -> Option<gtk::ColumnViewColumn> {
    let columns = view.columns();
    columns
        .item(columns.n_items().checked_sub(1)?)
        .and_downcast()
}

/// The row of column titles. It is the view's own child rather than anything
/// belonging to a column, and it is asked for by what it is rather than by where
/// it sits, so that a view that grows another child keeps working.
fn titles(view: &gtk::ColumnView) -> Option<gtk::Widget> {
    let mut child = view.first_child();
    while let Some(current) = child {
        if current.css_name().as_str() == "header" {
            return Some(current);
        }
        child = current.next_sibling();
    }
    None
}

/// Whether the column titles are being pressed at this moment.
///
/// Worth asking because a press on them moves the keyboard as a side effect:
/// GTK takes the focus for the view when a column is about to be dragged, and
/// anything following the focus around is told the user has arrived in the table
/// when all they have done is take hold of a column edge.
///
/// Asked of the pointer rather than remembered from an earlier event, because a
/// gesture that claims a press cancels the other gestures watching it, and the
/// claim comes first: anything keeping a note here would have been told the
/// press was over before the focus had finished moving.
pub fn titles_pressed(view: &gtk::ColumnView) -> bool {
    let Some((x, y, buttons)) = pointer(view) else {
        return false;
    };
    if !buttons.contains(gdk::ModifierType::BUTTON1_MASK) {
        return false;
    }

    let Some(titles) = titles(view).and_then(|titles| titles.compute_bounds(view)) else {
        return false;
    };
    // Its full width rather than the edges alone. Pressing a title to sort by it
    // is no more a request to move the keyboard into the table than dragging one
    // is.
    x >= f64::from(titles.x())
        && x <= f64::from(titles.x() + titles.width())
        && y >= f64::from(titles.y())
        && y <= f64::from(titles.y() + titles.height())
}

/// Where the pointer is in the view's own coordinates, and what it is holding
/// down.
fn pointer(view: &gtk::ColumnView) -> Option<(f64, f64, gdk::ModifierType)> {
    let native = view.native()?;
    let device = view.display().default_seat()?.pointer()?;
    let (x, y, buttons) = native.surface()?.device_position(&device)?;

    // The surface and the window drawn in it do not share an origin: the shadow
    // a window casts is part of the surface and no part of the window.
    let (left, top) = native.surface_transform();
    let at = native.upcast_ref::<gtk::Widget>().compute_point(
        view,
        &gtk::graphene::Point::new((x - left) as f32, (y - top) as f32),
    )?;
    Some((f64::from(at.x()), f64::from(at.y()), buttons))
}
