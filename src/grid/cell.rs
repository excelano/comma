// A cell you can type in.
//
// The cell shows a label and swaps in an entry for as long as it is being
// edited. The two live in a stack so that the widget the view binds a row to
// never changes and the one not in use asks for no room; a box asked to fit both
// in a cell's width gives one of them nothing and stops drawing it.
//
// GtkEditableLabel is this same stack and would be much less code. It is not
// used because its label cannot be told to ellipsize, so a single long value
// would stretch its column and the table would change shape as you scrolled.
//
// It is a widget of its own rather than a bare stack because it has to know
// where it is. Which row and column a cell holds is the answer to what the
// keyboard is on, what a row or column operation happens to, and what a screen
// reader should say — and the only thing there is to ask is the widget the focus
// landed on.
//
// An edit ends when Enter is pressed or the cell loses focus, and both commit
// what was typed. Escape puts the original text back and then ends the edit the
// same way, so cancelling is not a second path: it commits the value the
// document already holds, and the document does nothing with it.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::Cell as Value;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::Edited;

/// Where a cell's news goes: which row and column it came from, and what it is.
pub(super) type Report = dyn Fn(Edited);

const DISPLAY: &str = "display";
const EDIT: &str = "edit";

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Cell {
        pub stack: gtk::Stack,
        pub label: gtk::Label,
        pub entry: gtk::Text,
        /// Which record of the file this cell is showing, and which field of it.
        pub row: Value<usize>,
        pub column: Value<usize>,
        /// Where that row sits in the view, which a sort or a search makes
        /// different from where it sits in the file.
        pub position: Value<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Cell {
        const NAME: &'static str = "CommaCell";
        type Type = super::Cell;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Cell {
        fn constructed(&self) {
            self.parent_constructed();

            self.label.set_xalign(0.0);
            self.label.set_hexpand(true);
            self.label.set_ellipsize(pango::EllipsizeMode::End);
            // Without this the label's own idea of how wide it wants to be wins
            // and the column stops honouring its width.
            self.label.set_max_width_chars(1);

            self.entry.set_hexpand(true);
            // The same reason as the label's: an entry sized to what it holds
            // would drag its column wider the moment an edit started.
            self.entry.set_width_chars(1);

            // The stack takes the size of whichever child it is showing rather
            // than the larger of the two, so a cell that is not being edited is
            // as tall as its text and no taller.
            self.stack.set_hhomogeneous(false);
            self.stack.set_vhomogeneous(false);
            self.stack.add_named(&self.label, Some(DISPLAY));
            self.stack.add_named(&self.entry, Some(EDIT));
            self.stack.set_parent(&*self.obj());
            self.obj().add_css_class("cell-content");
        }

        fn dispose(&self) {
            self.stack.unparent();
        }
    }

    impl WidgetImpl for Cell {}
}

glib::wrapper! {
    pub struct Cell(ObjectSubclass<imp::Cell>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Cell {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Cell {
    pub fn row(&self) -> usize {
        self.imp().row.get()
    }

    pub fn column(&self) -> usize {
        self.imp().column.get()
    }

    pub fn position(&self) -> u32 {
        self.imp().position.get()
    }

    /// Opens the cell for typing. This is all an edit is: the entry showing
    /// instead of the label, carrying what the label said. A cell already open
    /// stays open.
    pub fn begin(&self) {
        if self.editing() {
            return;
        }

        let imp = self.imp();
        imp.entry.set_text(&imp.label.text());
        imp.stack.set_visible_child_name(EDIT);
        imp.entry.grab_focus();
    }

    fn editing(&self) -> bool {
        self.imp()
            .stack
            .visible_child_name()
            .is_some_and(|name| name == EDIT)
    }

    fn finish(&self, report: &Rc<Report>, moving_on: bool) {
        if !self.editing() {
            // The edit has already ended. Committing on Enter takes the focus
            // away from the entry, and losing focus is the other way one ends.
            return;
        }

        let imp = self.imp();
        let value = imp.entry.text().to_string();
        imp.label.set_text(&value);
        imp.stack.set_visible_child_name(DISPLAY);
        // The focus comes back to the cell of the table rather than to the entry
        // inside it — but only when it is still in here at all. An edit ended by
        // clicking somewhere else has already put the focus where it belongs,
        // and taking it back would drag it off the cell just clicked on.
        if imp.entry.has_focus()
            && let Some(item) = self.parent()
        {
            item.grab_focus();
        }

        report(Edited {
            row: self.row(),
            column: self.column(),
            value,
            moving_on,
        });
    }
}

/// Builds the contents of a fresh cell and connects every way an edit ends.
/// Cells are recycled as the view scrolls, so this runs once per widget rather
/// than once per row.
///
/// Starting one is not connected here: Enter is bound to an action on the table,
/// so that every key Comma answers to is declared in one place.
pub(super) fn setup(item: &gtk::ColumnViewCell, column: usize, report: Rc<Report>) {
    let cell = Cell::default();
    cell.imp().column.set(column);
    // The keyboard goes to the cell rather than to the row around it, because a
    // table is read a cell at a time. The table's own cell would otherwise be a
    // stop in front of ours.
    cell.set_focusable(true);
    item.set_focusable(false);
    item.set_child(Some(&cell));

    // A double click opens a cell. A single one only says where you are, which
    // is what focusing the table's cell does.
    let clicks = gtk::GestureClick::new();
    clicks.connect_pressed(glib::clone!(
        #[weak]
        cell,
        move |gesture, presses, _, _| {
            if let Some(item) = cell.parent() {
                item.grab_focus();
            }
            if presses == 2 {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                cell.begin();
            }
        }
    ));
    cell.add_controller(clicks);

    // The other button asks what can be done here, which first means saying
    // where here is.
    let menu = gtk::GestureClick::new();
    menu.set_button(gdk::BUTTON_SECONDARY);
    menu.connect_pressed(glib::clone!(
        #[weak]
        cell,
        move |gesture, _, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            if let Some(item) = cell.parent() {
                item.grab_focus();
            }
            if let Some((x, y)) = super::point_in_view(cell.upcast_ref(), x, y) {
                cell.activate_action("win.cell-menu", Some(&(x, y).to_variant()))
                    .unwrap_or_default();
            }
        }
    ));
    cell.add_controller(menu);

    let entry = cell.imp().entry.clone();
    entry.connect_activate(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        move |_| cell.finish(&report, true)
    ));

    let leaving = gtk::EventControllerFocus::new();
    leaving.connect_leave(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        move |_| cell.finish(&report, false)
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
            let imp = cell.imp();
            imp.entry.set_text(&imp.label.text());
            cell.finish(&report, false);
            glib::Propagation::Stop
        }
    ));
    entry.add_controller(cancel);
}

/// Shows the value this cell's row holds in this cell's column, and says what it
/// is for anything reading the screen aloud.
pub(super) fn bind(item: &gtk::ColumnViewCell, column: usize, title: &str) {
    let Some(cell) = item.child().and_downcast::<Cell>() else {
        return;
    };
    let Some(row) = item.item().and_downcast::<super::Row>() else {
        return;
    };

    let imp = cell.imp();
    let value = row.value(column);
    imp.label.set_text(&value);
    imp.row.set(row.index());
    imp.column.set(column);
    imp.position.set(item.position());

    // A cell handed a different row is not the cell that was being edited, and
    // whatever was half-typed into it belonged to a row that is no longer here.
    imp.stack.set_visible_child_name(DISPLAY);

    // What a screen reader says on reaching this cell. The column view supplies
    // the table around it; this is the cell's own part of the answer, and a
    // blank cell has to say so rather than say nothing.
    cell.update_property(&[
        gtk::accessible::Property::Label(&if value.is_empty() {
            gettext("Empty")
        } else {
            value
        }),
        gtk::accessible::Property::Description(&format!("{title} {}", row.number())),
    ]);
}
