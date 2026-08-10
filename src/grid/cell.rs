// A cell you can type in.
//
// The cell shows a label and swaps in an editor for as long as it is being
// edited. The two live in a stack so that the widget the view binds a row to
// never changes and the one not in use asks for no room; a box asked to fit both
// in a cell's width gives one of them nothing and stops drawing it.
//
// The editor is a text view rather than an entry because a field of a delimited
// file is allowed to hold a line break — quoted, and Comma reads and writes them
// — and an entry is one line by construction. So Enter is not what an entry does
// with it: Enter finishes the edit, and Alt or Ctrl with it puts a line break in,
// which is what the two spreadsheets people arrive from do between them.
//
// GtkEditableLabel is this same stack and would be much less code. It is not
// used because its label cannot be told to ellipsize, so a single long value
// would stretch its column and the table would change shape as you scrolled —
// and it, too, is one line only.
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

/// The keys that put a line break into a value instead of finishing the edit.
///
/// Excel does it with Alt and LibreOffice with Ctrl, so Comma answers to both
/// and whichever one is in your hands is the right one. The first is the one
/// the list of shortcuts shows.
pub const LINE_BREAK: [&str; 2] = ["<alt>Return", "<primary>Return"];

/// Whether a press of Enter was asking for a line break rather than for the end
/// of the edit. The keypad's Enter is the same key as the other one here, as it
/// is everywhere else in Comma.
fn breaks_a_line(pressed: gdk::Key, state: gdk::ModifierType) -> bool {
    let pressed = match pressed {
        gdk::Key::KP_Enter | gdk::Key::ISO_Enter => gdk::Key::Return,
        other => other,
    };

    LINE_BREAK
        .iter()
        .filter_map(|accelerator| gtk::accelerator_parse(*accelerator))
        .any(|(key, modifiers)| key == pressed && state.contains(modifiers))
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Cell {
        pub stack: gtk::Stack,
        pub label: gtk::Label,
        pub entry: gtk::TextView,
        /// What lets the editor be scrolled sideways to the end of a long value
        /// without being allowed to drag its column wider. A text view does not
        /// scroll itself.
        pub scroller: gtk::ScrolledWindow,
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

            // Lines are the value's own, never the width's: a cell being edited
            // is exactly as tall as the same cell was to look at, so opening one
            // does not move the rest of the table.
            self.entry.set_wrap_mode(gtk::WrapMode::None);
            // Tab belongs to the table, as it does everywhere else in it.
            self.entry.set_accepts_tab(false);

            // No scrollbars: the view follows the caret to the end of a long
            // value, which is what an entry does, and a bar inside a table cell
            // would be a thing to look at in every row.
            self.scroller
                .set_policy(gtk::PolicyType::External, gtk::PolicyType::External);
            // As tall as what it holds, so the row grows a line at a time with
            // the value rather than being some height of its own choosing.
            self.scroller.set_propagate_natural_height(true);
            self.scroller.set_hexpand(true);
            // The same reason as the label's: an editor sized to what it holds
            // would drag its column wider the moment an edit started.
            self.scroller.set_size_request(1, -1);
            self.scroller.set_child(Some(&self.entry));

            // The stack takes the size of whichever child it is showing rather
            // than the larger of the two, so a cell that is not being edited is
            // as tall as its text and no taller.
            self.stack.set_hhomogeneous(false);
            self.stack.set_vhomogeneous(false);
            self.stack.add_named(&self.label, Some(DISPLAY));
            self.stack.add_named(&self.scroller, Some(EDIT));
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

    /// Opens the cell for typing. This is all an edit is: the editor showing
    /// instead of the label, carrying what the label said. A cell already open
    /// stays open.
    pub fn begin(&self) {
        if self.editing() {
            return;
        }

        let imp = self.imp();
        let buffer = imp.entry.buffer();
        buffer.set_text(&imp.label.text());
        // At the end of what is there, which is where an entry puts it and
        // where you would carry on typing from.
        buffer.place_cursor(&buffer.end_iter());

        imp.stack.set_visible_child_name(EDIT);
        imp.entry.grab_focus();
        self.fit_to_lines();

        // Putting the caret somewhere does not go and look at it, and a view
        // that has not been given its size yet cannot look anywhere. So this
        // waits a frame: a long value would otherwise open showing its
        // beginning with the caret off the side, which is not where an entry
        // leaves you.
        imp.entry.add_tick_callback(|entry, _| {
            let insert = entry.buffer().get_insert();
            entry.scroll_to_mark(&insert, 0.0, false, 0.0, 0.0);
            glib::ControlFlow::Break
        });
    }

    /// Makes the editor as tall as the lines it holds, and the row with it.
    ///
    /// A text view is a thing meant to be scrolled, so it asks for one line's
    /// worth however much it holds — that is what a scrolled window around it is
    /// normally for. Here the cell is the scrolling, sideways only, and the
    /// height has to come from somewhere else.
    ///
    /// It comes from laying the text out and asking how tall that came to, which
    /// is what the label does with the same text and the same font. Counting
    /// lines and multiplying by the font's ascent and descent is two pixels
    /// short of it over three lines, and two pixels is a whole cell's worth of
    /// shift at the moment an edit begins.
    fn fit_to_lines(&self) {
        let imp = self.imp();
        let (_, height) = imp
            .entry
            .create_pango_layout(Some(&self.typed()))
            .pixel_size();
        let margins = imp.entry.top_margin() + imp.entry.bottom_margin();

        imp.scroller.set_min_content_height(height + margins);
    }

    /// What is in the editor at this moment.
    fn typed(&self) -> String {
        let buffer = self.imp().entry.buffer();
        let (start, end) = buffer.bounds();
        buffer.text(&start, &end, false).to_string()
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
        let value = self.typed();
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

    // A value too long for its column is cut off with an ellipsis, and this is
    // how to read the rest of it without opening the cell. Asked at the moment
    // of hovering rather than settled when the cell is filled, because whether
    // a value fits is not known until the column has been given its width — and
    // it changes again every time that width is dragged.
    cell.set_has_tooltip(true);
    cell.connect_query_tooltip(|cell, _, _, _, tooltip| {
        let imp = cell.imp();
        // Nothing is cut off in a cell that is open for typing, and the value
        // being shown there is the entry's rather than the label's.
        if imp.stack.visible_child_name().as_deref() != Some(DISPLAY) {
            return false;
        }
        if !imp.label.layout().is_ellipsized() {
            return false;
        }

        tooltip.set_text(Some(&imp.label.text()));
        true
    });

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

    // Adding a line makes the cell taller as it is typed, rather than at the
    // moment the edit ends.
    entry.buffer().connect_changed(glib::clone!(
        #[weak]
        cell,
        move |_| cell.fit_to_lines()
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

    // The keys an open cell answers to itself. Before the text view's own, which
    // would otherwise take Enter for a line break — the thing it is for
    // everywhere else and the one thing it cannot mean here, because Enter is
    // how an edit is finished and how a column is filled downwards.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        cell,
        #[strong]
        report,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, state| {
            match key {
                gdk::Key::Escape => {
                    // Put back what the document holds and end the edit on it,
                    // so that cancelling is not a second way out: it commits a
                    // value that is not a change.
                    let imp = cell.imp();
                    imp.entry.buffer().set_text(&imp.label.text());
                    cell.finish(&report, false);
                }
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                    match breaks_a_line(key, state) {
                        true => cell.imp().entry.buffer().insert_at_cursor("\n"),
                        false => cell.finish(&report, true),
                    }
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        }
    ));
    entry.add_controller(keys);
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
