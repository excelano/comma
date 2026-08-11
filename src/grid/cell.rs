// A cell you can type in.
//
// The cell shows a label and swaps in an editor for as long as it is being
// edited. Exactly one of the two is in the cell at a time, so the one not in use
// asks for no room; a box asked to fit both in a cell's width would give one of
// them nothing and stop drawing it.
//
// The editor is built the first time a cell is opened rather than when the cell
// is made. The grid realises a cell for every column of every row it might draw,
// which is a few thousand on an ordinary file, and at most one of them is being
// edited. Giving each of them its own text view and scrolled window up front was
// three quarters of the cost of opening a file.
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
use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::{Edited, Records};

/// Where a cell's news goes: which row and column it came from, and what it is.
pub(super) type Report = dyn Fn(Edited);

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

    /// The half of a cell that only a cell being edited needs.
    #[derive(Debug, Clone)]
    pub struct Editor {
        pub entry: gtk::TextView,
        /// What lets the editor be scrolled sideways to the end of a long value
        /// without being allowed to drag its column wider. A text view does not
        /// scroll itself.
        pub scroller: gtk::ScrolledWindow,
    }

    // No `Debug`: what a cell reports its edits to is a closure, and there is
    // nothing to print about one.
    #[derive(Default)]
    pub struct Cell {
        pub label: gtk::Label,
        /// Made the first time this cell is opened, and kept afterwards: a cell
        /// that has been edited once is likely to be edited again, and the
        /// saving is in the thousands that never are.
        pub editor: RefCell<Option<Editor>>,
        /// Where this cell's news goes. Held here because the editor is wired up
        /// long after the factory that knew it has returned.
        pub report: RefCell<Option<Rc<Report>>>,
        /// Which field of a record this cell shows. Columns do not move under a
        /// cell: one coming or going builds them all again.
        pub column: Value<usize>,
        /// The list item this cell was put in, which is what GTK moves when rows
        /// move, and so the one thing here that still knows where the row is.
        pub item: RefCell<glib::WeakRef<gtk::ColumnViewCell>>,
        /// What to ask which record a position is showing.
        pub records: RefCell<Option<Records>>,
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
            self.label.set_parent(&*self.obj());

            self.obj().add_css_class("cell-content");
        }

        fn dispose(&self) {
            // Whichever of the label and the editor is in the cell at the
            // moment, and only ever one of them.
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
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
    /// Which record of the file this cell is showing, now.
    ///
    /// Asked rather than remembered, because a row put in above this one moves
    /// it without binding it again: the value on screen is still this record's
    /// value, and the record it belongs to is one further down the file than it
    /// was. A cell that has been handed back to the view is showing nothing and
    /// says so.
    pub fn row(&self) -> Option<usize> {
        let position = self.position()?;
        let records = self.imp().records.borrow().clone()?;
        records.at(position).map(|row| row.index())
    }

    pub fn column(&self) -> usize {
        self.imp().column.get()
    }

    /// Where this cell's row sits in the view, now, for the same reason.
    pub fn position(&self) -> Option<u32> {
        super::position_of(&self.imp().item.borrow())
    }

    /// Opens the cell for typing. This is all an edit is: the editor showing
    /// instead of the label, carrying what the label said. A cell already open
    /// stays open.
    pub fn begin(&self) {
        if self.editing() {
            return;
        }

        let imp = self.imp();
        let editor = self.editor();

        let buffer = editor.entry.buffer();
        buffer.set_text(&imp.label.text());
        // At the end of what is there, which is where an entry puts it and
        // where you would carry on typing from.
        buffer.place_cursor(&buffer.end_iter());

        imp.label.unparent();
        editor.scroller.set_parent(self);

        editor.entry.grab_focus();
        self.fit_to_lines();

        // Putting the caret somewhere does not go and look at it, and a view
        // that has not been given its size yet cannot look anywhere. So this
        // waits a frame: a long value would otherwise open showing its
        // beginning with the caret off the side, which is not where an entry
        // leaves you.
        editor.entry.add_tick_callback(|entry, _| {
            let insert = entry.buffer().get_insert();
            entry.scroll_to_mark(&insert, 0.0, false, 0.0, 0.0);
            glib::ControlFlow::Break
        });
    }

    /// This cell's editor, built and wired up if this is the first time it has
    /// been asked for.
    fn editor(&self) -> imp::Editor {
        if let Some(editor) = self.imp().editor.borrow().as_ref() {
            return editor.clone();
        }

        let entry = gtk::TextView::new();
        // Lines are the value's own, never the width's: a cell being edited is
        // exactly as tall as the same cell was to look at, so opening one does
        // not move the rest of the table.
        entry.set_wrap_mode(gtk::WrapMode::None);
        // Tab belongs to the table, as it does everywhere else in it.
        entry.set_accepts_tab(false);

        let scroller = gtk::ScrolledWindow::new();
        // No scrollbars: the view follows the caret to the end of a long value,
        // which is what an entry does, and a bar inside a table cell would be a
        // thing to look at in every row.
        scroller.set_policy(gtk::PolicyType::External, gtk::PolicyType::External);
        // As tall as what it holds, so the row grows a line at a time with the
        // value rather than being some height of its own choosing.
        scroller.set_propagate_natural_height(true);
        scroller.set_hexpand(true);
        // The same reason as the label's: an editor sized to what it holds would
        // drag its column wider the moment an edit started.
        scroller.set_size_request(1, -1);
        scroller.set_child(Some(&entry));

        let editor = imp::Editor { entry, scroller };
        self.connect_editor(&editor);
        self.imp().editor.replace(Some(editor.clone()));
        editor
    }

    /// Every way an edit ends, and the one thing that happens while it runs.
    fn connect_editor(&self, editor: &imp::Editor) {
        // Adding a line makes the cell taller as it is typed, rather than at the
        // moment the edit ends.
        editor.entry.buffer().connect_changed(glib::clone!(
            #[weak(rename_to = cell)]
            self,
            move |_| cell.fit_to_lines()
        ));

        let leaving = gtk::EventControllerFocus::new();
        leaving.connect_leave(glib::clone!(
            #[weak(rename_to = cell)]
            self,
            move |_| cell.finish(false)
        ));
        editor.entry.add_controller(leaving);

        // The keys an open cell answers to itself. Before the text view's own,
        // which would otherwise take Enter for a line break — the thing it is
        // for everywhere else and the one thing it cannot mean here, because
        // Enter is how an edit is finished and how a column is filled downwards.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = cell)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, state| {
                match key {
                    gdk::Key::Escape => {
                        // Put back what the document holds and end the edit on
                        // it, so that cancelling is not a second way out: it
                        // commits a value that is not a change.
                        let text = cell.imp().label.text();
                        cell.editor().entry.buffer().set_text(&text);
                        cell.finish(false);
                    }
                    gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                        match breaks_a_line(key, state) {
                            true => cell.editor().entry.buffer().insert_at_cursor("\n"),
                            false => cell.finish(true),
                        }
                    }
                    _ => return glib::Propagation::Proceed,
                }
                glib::Propagation::Stop
            }
        ));
        editor.entry.add_controller(keys);
    }

    /// Takes the editor back out and puts the label in, without saying anything
    /// happened. What the two ways of ending an edit have in common.
    fn close_editor(&self) -> bool {
        if !self.editing() {
            return false;
        }

        // Taken out of the cell before it is unparented, because unparenting a
        // widget is a thing other code watches for.
        let editor = self.imp().editor.borrow().clone();
        if let Some(editor) = editor {
            editor.scroller.unparent();
        }
        self.imp().label.set_parent(self);
        true
    }

    /// How tall this cell is for a value: one line's height for each line of it,
    /// counted the way `height_for_lines` explains.
    ///
    /// Both the label and the editor are given this same answer, so the two are
    /// the same height by construction rather than by luck, and a cell does not
    /// change size at the moment it opens. The number beside the row asks the
    /// same question of the row's tallest value.
    pub(super) fn height_for(&self, value: &str) -> i32 {
        super::height_for_lines(&self.imp().label, value.split('\n').count())
    }

    /// Makes the editor as tall as the lines it holds, and the row with it.
    ///
    /// A text view is a thing meant to be scrolled, so it asks for one line's
    /// worth however much it holds — that is what a scrolled window around it is
    /// normally for. Here the cell is the scrolling, sideways only, so the
    /// height has to be handed to it.
    fn fit_to_lines(&self) {
        let editor = self.editor();
        let margins = editor.entry.top_margin() + editor.entry.bottom_margin();
        editor
            .scroller
            .set_min_content_height(self.height_for(&self.typed()) + margins);
    }

    /// What is in the editor at this moment.
    fn typed(&self) -> String {
        let buffer = self.editor().entry.buffer();
        let (start, end) = buffer.bounds();
        buffer.text(&start, &end, false).to_string()
    }

    /// Whether this cell is open for typing, which is whether its editor is the
    /// one of the two that is in it.
    fn editing(&self) -> bool {
        self.imp()
            .editor
            .borrow()
            .as_ref()
            .is_some_and(|editor| editor.scroller.parent().is_some())
    }

    fn finish(&self, moving_on: bool) {
        if !self.editing() {
            // The edit has already ended. Committing on Enter takes the focus
            // away from the entry, and losing focus is the other way one ends.
            return;
        }

        let value = self.typed();
        // The focus comes back to the cell of the table rather than to the entry
        // inside it — but only when it is still in here at all. An edit ended by
        // clicking somewhere else has already put the focus where it belongs,
        // and taking it back would drag it off the cell just clicked on.
        let held_focus = self.editor().entry.has_focus();

        self.imp().label.set_text(&value);
        self.close_editor();

        if held_focus && let Some(item) = self.parent() {
            item.grab_focus();
        }

        // Cloned out rather than reported from inside the borrow: what this says
        // comes back around through the document and into `bind`.
        let (Some(report), Some(row)) = (self.imp().report.borrow().clone(), self.row()) else {
            // A cell the view has taken back has no record to write to, and
            // guessing at one would write into whatever moved into its place.
            return;
        };
        report(Edited {
            row,
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
pub(super) fn setup(
    item: &gtk::ColumnViewCell,
    column: usize,
    records: Records,
    report: Rc<Report>,
) {
    let cell = Cell::default();
    cell.imp().column.set(column);
    cell.imp().report.replace(Some(report));
    cell.imp().records.replace(Some(records));
    cell.imp().item.replace(item.downgrade());
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
        // Nothing is cut off in a cell that is open for typing, and the value
        // being shown there is the editor's rather than the label's.
        if cell.editing() {
            return false;
        }
        let imp = cell.imp();
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

    // A cell handed a different row is not the cell that was being edited, and
    // whatever was half-typed into it belonged to a row that is no longer here.
    cell.close_editor();

    let imp = cell.imp();
    let value = row.value(column);
    imp.label.set_text(&value);
    // The same height the editor will ask for, so that opening this cell does
    // not move the table around it.
    imp.label.set_size_request(-1, cell.height_for(&value));
    imp.column.set(column);

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
