// A cell you can type in.
//
// The cell holds a label and an entry in a stack, and an edit is nothing more
// than which of the two the stack is showing. They live in a stack rather than
// side by side so that the widget the view binds a row to never changes and the
// one not in use asks for no room: a box asked to fit both in a cell's width
// gives one of them nothing and stops drawing it. Everything is connected once,
// when the cell is built.
//
// GtkEditableLabel is this same stack, and would be much less code. It is not
// used because its label cannot be told to ellipsize, so a single long value
// would stretch its column and the table would change shape as you scrolled.
//
// An edit ends when Enter is pressed or the cell loses focus, and both commit
// what was typed. Escape puts the original text back and then ends the edit the
// same way, so cancelling is not a second path: it commits the value the
// document already holds, and the document does nothing with it.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

use super::{Event, Row};

/// Where a cell's news goes: which row and column it came from, and what it is.
pub(super) type Report = dyn Fn(usize, usize, Event);

const DISPLAY: &str = "display";
const EDIT: &str = "edit";

/// Builds the contents of a fresh cell and connects every way an edit starts
/// and ends. Cells are recycled as the view scrolls, so this runs once per
/// widget rather than once per row.
pub(super) fn setup(cell: &gtk::ColumnViewCell, column: usize, report: Rc<Report>) {
    let label = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(pango::EllipsizeMode::End)
        // Without this the label's own idea of how wide it wants to be wins
        // and the column stops honouring its width.
        .max_width_chars(1)
        .build();

    let entry = gtk::Text::builder()
        .hexpand(true)
        // The same reason as the label's: an entry sized to what it holds would
        // drag its column wider the moment an edit started.
        .width_chars(1)
        .build();

    // The stack takes the size of whichever child it is showing rather than the
    // larger of the two, so a cell that is not being edited is as tall as its
    // text and no taller.
    let content = gtk::Stack::builder()
        .hhomogeneous(false)
        .vhomogeneous(false)
        .focusable(true)
        .css_classes(["cell-content"])
        .build();
    content.add_named(&label, Some(DISPLAY));
    content.add_named(&entry, Some(EDIT));

    // The cell would otherwise be a tab stop in front of the box that is the
    // thing actually worth stopping at.
    cell.set_focusable(false);
    cell.set_child(Some(&content));

    // One click says where you are, two say you want to change it. GTK gives a
    // plain container no click-to-focus of its own.
    let start = gtk::GestureClick::new();
    start.connect_pressed(glib::clone!(
        #[weak]
        content,
        move |gesture, presses, _, _| {
            content.grab_focus();
            if presses == 2 {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                begin(&content);
            }
        }
    ));
    content.add_controller(start);

    let arriving = gtk::EventControllerFocus::new();
    arriving.connect_enter(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        move |_| {
            if let Some(row) = cell.item().and_downcast::<Row>() {
                report(row.index(), column, Event::Focused);
            }
        }
    ));
    content.add_controller(arriving);

    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        content,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if !matches!(key, gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::F2) {
                return glib::Propagation::Proceed;
            }
            begin(&content);
            glib::Propagation::Stop
        }
    ));
    content.add_controller(keys);

    entry.connect_activate(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        move |_| finish(&cell, column, &report)
    ));

    let leaving = gtk::EventControllerFocus::new();
    leaving.connect_leave(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        move |_| finish(&cell, column, &report)
    ));
    entry.add_controller(leaving);

    let cancel = gtk::EventControllerKey::new();
    cancel.connect_key_pressed(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            let (label, entry) = parts_of(&content_of(&cell));
            entry.set_text(&label.text());
            finish(&cell, column, &report);
            glib::Propagation::Stop
        }
    ));
    entry.add_controller(cancel);
}

/// Shows the value this cell's row holds in this cell's column.
pub(super) fn bind(cell: &gtk::ColumnViewCell, column: usize) {
    let Some(row) = cell.item().and_downcast::<Row>() else {
        return;
    };
    let content = content_of(cell);
    let (label, _) = parts_of(&content);
    label.set_text(&row.value(column));
    // A cell handed a different row is not the cell that was being edited, and
    // whatever was half-typed into it belonged to a row that is no longer here.
    content.set_visible_child_name(DISPLAY);
}

/// Starts an edit. This is all an edit is: the entry showing instead of the
/// label, carrying what the label said.
fn begin(content: &gtk::Stack) {
    if editing(content) {
        return;
    }

    let (label, entry) = parts_of(content);
    entry.set_text(&label.text());
    content.set_visible_child_name(EDIT);
    entry.grab_focus();
}

fn finish(cell: &gtk::ColumnViewCell, column: usize, report: &Rc<Report>) {
    let content = content_of(cell);
    if !editing(&content) {
        // The edit has already ended. Committing on Enter takes the focus away
        // from the entry, and losing focus is the other way an edit ends.
        return;
    }

    let (label, entry) = parts_of(&content);
    let value = entry.text().to_string();
    label.set_text(&value);
    content.set_visible_child_name(DISPLAY);
    // The edit was started from here, so this is where the focus comes back to.
    content.grab_focus();

    if let Some(row) = cell.item().and_downcast::<Row>() {
        report(row.index(), column, Event::Edited(value));
    }
}

fn editing(content: &gtk::Stack) -> bool {
    content
        .visible_child_name()
        .is_some_and(|name| name == EDIT)
}

fn content_of(cell: &gtk::ColumnViewCell) -> gtk::Stack {
    cell.child()
        .and_downcast::<gtk::Stack>()
        .expect("setup gave every cell its contents")
}

fn parts_of(content: &gtk::Stack) -> (gtk::Label, gtk::Text) {
    let label = content
        .child_by_name(DISPLAY)
        .and_downcast::<gtk::Label>()
        .expect("setup named the label");
    let entry = content
        .child_by_name(EDIT)
        .and_downcast::<gtk::Text>()
        .expect("setup named the entry");
    (label, entry)
}
