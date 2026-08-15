// Where the keyboard is in the grid, and the keys the grid answers to:
// the ones that move the keyboard, and the ones that act where it already is.
//
// The cells are recycled underneath the view, so none of them can be asked
// where the user is working. The window is the only thing that outlives them,
// which is why the cursor is kept here.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::grid::{self, Row};

use super::CommaWindow;

/// Where the keyboard is in the grid.
///
/// A row is held twice over because the two are asked different questions.
/// `position` is where the row sits in the view and is what moving up or down
/// counts in; `row` is where it sits in the file and is what a change to the
/// document is addressed by. A sort or a search makes them differ.
#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    pub position: u32,
    pub row: usize,
    pub column: usize,
}

/// How the keyboard moves around the grid, and the keys that ask for it.
///
/// These are bound to the grid rather than to the window, so that the arrow keys
/// belong to the search box while the search box has the focus, and to the table
/// the rest of the time. The same goes for `ACTS`.
const MOVES: [(&str, &str); 10] = [
    ("up", "Up"),
    ("down", "Down"),
    ("left", "Left"),
    ("right", "Right"),
    // Tab crosses the table and then wraps, as it does in a spreadsheet, and at
    // either end of the file it gives up and leaves — a table nothing can tab
    // out of is a trap for anyone working without a mouse.
    ("next", "Tab"),
    ("previous", "<shift>Tab"),
    ("row-start", "Home"),
    ("row-end", "End"),
    ("start", "<primary>Home"),
    ("end", "<primary>End"),
];

/// One of the things the grid does where the keyboard already is: what it is
/// called, every key that asks for it, and what it does. Carrying the last of
/// those is what keeps asking for one by name and doing it a single list.
type Act = (&'static str, &'static [&'static str], fn(&CommaWindow));

/// What the grid does instead of moving the keyboard.
///
/// Opening a cell that is already open is what commits it, which is why Enter
/// belongs here and not among the moves: it acts on where you are.
const ACTS: [Act; 1] = [(
    "edit",
    &["Return", "KP_Enter", "F2"],
    CommaWindow::edit_cell,
)];

/// How many frames to keep looking for a cell the keyboard was sent to before
/// giving up on it. A handful: the view draws the row it was scrolled to within
/// a frame or two, and a cell that has not appeared by then is not going to.
const TRIES: u8 = 8;

/// Which key asks the grid for one of the things it does, for anything that
/// wants to say so without spelling it out a second time.
///
/// The first key of an act is the one that answers. Three keys open a cell, and
/// the shortcuts window has one line to name them in.
pub fn key_for(how: &str) -> Option<&'static str> {
    let moves = MOVES.iter().map(|(name, key)| (*name, *key));
    let acts = ACTS
        .iter()
        .filter_map(|(name, keys, _)| Some((*name, *keys.first()?)));
    moves
        .chain(acts)
        .find(|(name, _)| *name == how)
        .map(|(_, key)| key)
}

impl CommaWindow {
    /// The keys the grid answers to, bound to the table rather than to the
    /// window so that they belong to whatever else has the focus when something
    /// else does.
    pub(super) fn setup_navigation(&self) {
        let keys = gtk::ShortcutController::new();
        // Before the widgets inside the table have their say, because the column
        // view binds Home, End and Tab itself and means different things by them
        // than a table of cells does.
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);

        for (how, key) in MOVES {
            self.bind_key(&keys, key, move |window| window.move_cursor(how));
        }
        for (_, pressed, act) in ACTS {
            for key in pressed {
                self.bind_key(&keys, key, act);
            }
        }

        self.imp().column_view.add_controller(keys);
    }

    /// Binds one key to the grid, to do one thing.
    fn bind_key(&self, keys: &gtk::ShortcutController, key: &str, run: impl Fn(&Self) + 'static) {
        let Some(trigger) = gtk::ShortcutTrigger::parse_string(key) else {
            return;
        };

        // Declining a key rather than always taking it is what lets a cell that
        // is open for typing keep the keys that belong to typing.
        let action = gtk::CallbackAction::new(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _| {
                if window.typing() {
                    return glib::Propagation::Proceed;
                }
                run(&window);
                glib::Propagation::Stop
            }
        ));

        keys.add_shortcut(
            gtk::Shortcut::builder()
                .trigger(&trigger)
                .action(&action)
                .build(),
        );
    }

    /// Follows the keyboard around the grid. Where it is decides what a row or
    /// column operation happens to and what moving does next, and the only place
    /// that knows is the window: the cells are recycled underneath it.
    pub(super) fn watch_focus(&self) {
        self.connect_focus_widget_notify(|window| {
            let focus = gtk::prelude::RootExt::focus(window);
            // Where to put the keyboard back if something takes it without
            // being asked. Kept before the table is asked about, because the
            // answer is as often somewhere else in the window.
            if !grid::titles_pressed(&window.imp().column_view) {
                window
                    .imp()
                    .was_focused
                    .replace(focus.as_ref().map(|w| w.downgrade()).unwrap_or_default());
            }

            let (Some(focused), Some(cell)) = (focus, window.focused_cell()) else {
                // Nothing in the table has the keyboard, so the row it was in
                // stops being lit. The table's own row gives the tint up on its
                // own, from `:focus-within`; the number beside it is in another
                // view and has to be told.
                window.imp().gutter.set_current(None);
                return;
            };

            // Focus arriving from the toolbar lands on a whole row rather than
            // on any one cell of it, and a row lit up on its own says the row
            // is what the next thing will happen to, which is not true here.
            // So the row is passed straight through to a cell: the one the
            // cursor already names, rather than the first one to hand, so that
            // leaving the table and coming back brings you back where you were.
            //
            // Only from above, though. Focus goes *into* a cell as well — the
            // entry it holds is where it is for as long as the cell is open for
            // typing — and taking it back from there ends the edit at the
            // moment it begins.
            let on_the_cell = focused == *cell.upcast_ref::<gtk::Widget>();
            if !on_the_cell && !focused.is_ancestor(&cell) {
                // Unless the keyboard was only handed here because a column is
                // being taken hold of. GTK moves the focus to the view when a
                // column is about to be dragged, which is not an arrival in the
                // table and not a reason to light a cell up in it: the cursor
                // belongs wherever it was before anyone touched the titles.
                if grid::titles_pressed(&window.imp().column_view) {
                    // Back where it was, rather than merely not moved on: GTK
                    // has already taken it, and a row of the grid holding the
                    // keyboard lights up whether or not a cell of it does.
                    if let Some(before) = window.imp().was_focused.borrow().upgrade() {
                        before.grab_focus();
                    }
                    return;
                }
                match window.imp().current.get() {
                    Some(cursor) => window.go_to(cursor.position, cursor.column),
                    None => {
                        cell.grab_focus();
                    }
                }
                return;
            }
            // A cell the view has taken back is showing nothing and names no
            // record, so the cursor stays where it was rather than following the
            // focus onto a widget on its way somewhere else.
            let (Some(position), Some(row)) = (cell.position(), cell.row()) else {
                return;
            };
            window.set_cursor(Cursor {
                position,
                row,
                column: cell.column(),
            });
            window.show_reach();
        });
    }

    /// Moves the keyboard to another cell, or opens the one it is on.
    ///
    /// Moving is done by asking the view to scroll somewhere and take the focus
    /// with it, which is the one way that works whether or not the cell being
    /// moved to has been drawn yet.
    pub(super) fn move_cursor(&self, how: &str) {
        let imp = self.imp();
        let Some(cursor) = imp.current.get() else {
            // Nothing is current yet — the table has the keyboard but no cell
            // of it does — so the first thing asked for is only to be
            // somewhere, and the beginning is where that is.
            self.go_to(0, 0);
            return;
        };

        let rows = imp.sorted.n_items();
        let columns = imp.column_view.columns().n_items();
        if rows == 0 || columns == 0 {
            return;
        }
        let (last_row, last_column) = (rows - 1, columns as usize - 1);

        let (position, column) = match how {
            "up" => (cursor.position.saturating_sub(1), cursor.column),
            "down" => ((cursor.position + 1).min(last_row), cursor.column),
            "left" => (cursor.position, cursor.column.saturating_sub(1)),
            "right" => (cursor.position, (cursor.column + 1).min(last_column)),
            "row-start" => (cursor.position, 0),
            "row-end" => (cursor.position, last_column),
            "start" => (0, 0),
            "end" => (last_row, last_column),
            "next" if cursor.column < last_column => (cursor.position, cursor.column + 1),
            "next" if cursor.position < last_row => (cursor.position + 1, 0),
            "previous" if cursor.column > 0 => (cursor.position, cursor.column - 1),
            "previous" if cursor.position > 0 => (cursor.position - 1, last_column),
            // Off the end of the table in either direction, which is the one
            // way out of it.
            "next" | "previous" => return self.leave_table(),
            _ => return,
        };

        self.go_to(position, column);
    }

    /// Puts the keyboard back where it was after the table has been rebuilt
    /// underneath it.
    ///
    /// The cursor has to be handed in from before the change rather than read
    /// here. Throwing away the widget the focus was on makes GTK put the focus
    /// somewhere else of its own accord, and following the focus is how the
    /// cursor is kept, so by now it has already been moved to wherever the
    /// wreckage left it.
    ///
    /// Where it goes back to is the same place in the table, not the same row
    /// of the file: after a delete that is whatever took the old row's place,
    /// which is where the next thing is likely to happen.
    pub(super) fn follow_change(&self, cursor: Option<Cursor>) {
        let imp = self.imp();
        let Some(cursor) = cursor else {
            return;
        };

        let rows = imp.sorted.n_items();
        let columns = imp.column_view.columns().n_items() as usize;
        if rows == 0 || columns == 0 {
            return;
        }
        self.go_to(
            cursor.position.min(rows - 1),
            cursor.column.min(columns - 1),
        );
    }

    /// Takes the keyboard out of the table, to the first thing in the window
    /// that is not part of it.
    fn leave_table(&self) {
        self.imp().open_button.grab_focus();
    }

    /// Whether a cell is open for typing, in which case the keys that move
    /// around the table are the editor's: Home and End move the caret through
    /// what is being typed, and on a value with a line break in it the arrows
    /// move between its lines, rather than any of them moving to another cell.
    pub(super) fn typing(&self) -> bool {
        gtk::prelude::RootExt::focus(self).is_some_and(|widget| widget.is::<gtk::TextView>())
    }

    /// Puts the cursor, and the keyboard with it, on one cell of the table.
    pub(super) fn go_to(&self, position: u32, column: usize) {
        let imp = self.imp();
        let Some(target) = imp
            .column_view
            .columns()
            .item(column as u32)
            .and_downcast::<gtk::ColumnViewColumn>()
        else {
            return;
        };

        // The same question asked of the other axis, and it has to be asked:
        // scrolling to a row the view does not have is an assertion inside GTK,
        // which drops the scroll and complains. Two ways to arrive at one. A
        // position can be past the end of the file, because this is reached
        // through an action and an action takes what it is given. And it can be
        // a position the view has not caught up to: the filtered model is built
        // over several frames, so a row number clicked moments after a filter
        // or a search changed can name a row that is on its way.
        //
        // Declining is the right answer to both. Landing somewhere else instead
        // would mean a click on row 500 putting the keyboard on row 7.
        let Some(row) = imp
            .sorted
            .item(position)
            .and_downcast::<Row>()
            .map(|row| row.index())
        else {
            return;
        };

        // The cursor moves whether or not the keyboard can follow it there yet,
        // so that moving twice in a row lands where two moves should.
        self.set_cursor(Cursor {
            position,
            row,
            column,
        });

        imp.column_view
            .scroll_to(position, Some(&target), gtk::ListScrollFlags::empty(), None);
        if !grid::focus_cell(&imp.column_view, position, column) {
            // That cell has not been drawn yet, because the view was just
            // asked to scroll or because every widget in it was just thrown
            // away and built again. Either way it appears at a frame, not at a
            // turn of the loop, so this waits for frames rather than for idle.
            let left = std::cell::Cell::new(TRIES);
            imp.column_view.add_tick_callback(move |view, _| {
                if grid::focus_cell(view, position, column) || left.get() == 0 {
                    return glib::ControlFlow::Break;
                }
                left.set(left.get() - 1);
                glib::ControlFlow::Continue
            });
        }
        self.show_reach();
    }

    /// Puts the cursor back on the row it was on, after the file has been read
    /// again.
    ///
    /// The cursor is kept twice over, and reading the file again can move one
    /// of them without moving the other. A value somebody else edits can be the
    /// one the grid is sorted by, and a file that comes back narrower drops the
    /// filters that were holding columns it no longer has; either way the rows
    /// keep their numbers and change their order. Held by its place in the
    /// view, the cursor is left on whichever row moved into that place. So the
    /// row is what is kept, and the place is looked up again.
    ///
    /// This holds a row of the file, which is not the same as holding a record.
    /// Nothing here can be: a row number is a position too, so a write that
    /// inserts rows above the cursor renumbers everything below it, and telling
    /// that apart from a write that edited those rows in place would mean
    /// matching them by content — a guess, on a file that may well have
    /// duplicate rows. What the cursor holds across an outside write is the
    /// line of the file, and that is all it claims to hold.
    ///
    /// The view is filtered a few rows at a time, so the row may not have been
    /// reached yet. Looking again over the next few turns is what the rest of
    /// this file does about the same thing; a row still missing by then is one
    /// the filters are holding back, and there is no place in the view to put
    /// it.
    pub(super) fn find_cursor_again(&self) {
        let Some(cursor) = self.imp().current.get() else {
            return;
        };

        let left = std::cell::Cell::new(TRIES);
        glib::idle_add_local(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                let sorted = window.imp().sorted.clone();
                let found = (0..sorted.n_items()).find(|at| {
                    sorted
                        .item(*at)
                        .and_downcast::<Row>()
                        .is_some_and(|row| row.index() == cursor.row)
                });

                match found {
                    // Already where it should be, which is the ordinary case:
                    // most writes do not move the row anybody is sitting on.
                    Some(position) if position == cursor.position => glib::ControlFlow::Break,
                    Some(position) => {
                        window.set_cursor(Cursor { position, ..cursor });
                        // The keyboard follows only if it is in the table to
                        // begin with. Reading the file again is not a reason to
                        // take it out of the search box.
                        if window.focused_cell().is_some() {
                            window.go_to(position, cursor.column);
                        }
                        glib::ControlFlow::Break
                    }
                    None if left.get() == 0 => glib::ControlFlow::Break,
                    None => {
                        left.set(left.get() - 1);
                        glib::ControlFlow::Continue
                    }
                }
            }
        ));
    }

    /// Puts the cursor somewhere and lights the row it lands on, in the numbers
    /// beside the table as well as in the table.
    fn set_cursor(&self, cursor: Cursor) {
        let imp = self.imp();
        imp.current.set(Some(cursor));
        imp.gutter.set_current(Some(cursor.position));
    }

    /// Opens the cell the keyboard is on for typing.
    pub(super) fn edit_cell(&self) {
        if let Some(cell) = self.focused_cell() {
            cell.begin();
        }
    }

    /// The cell the keyboard is in, wherever inside it the focus has landed.
    ///
    /// Focus sits on the entry inside a cell while one is being typed in, and on
    /// a whole row of the table on the way in from the toolbar, so the answer is
    /// looked for above and below whatever has it.
    pub(super) fn focused_cell(&self) -> Option<grid::Cell> {
        let focused = gtk::prelude::RootExt::focus(self)?;

        if let Some(cell) = focused.downcast_ref::<grid::Cell>() {
            return Some(cell.clone());
        }
        if let Some(cell) = focused.ancestor(grid::Cell::static_type()) {
            return cell.downcast().ok();
        }
        // Tabbing into the table lands on a whole row rather than on any one of
        // its cells, and a row the keyboard is on is somewhere: the first cell
        // in it, rather than nowhere at all.
        grid::cell_within(&focused)
    }

    /// Where the next row or column goes, kept inside the document it refers
    /// to: rows and columns the current cell outlived are no longer there.
    pub(super) fn current_cell(&self) -> Option<(usize, usize)> {
        let document = self.imp().rows.document()?;
        let cursor = self.imp().current.get()?;

        let document = document.borrow();
        let (rows, columns) = (document.row_count(), document.column_count());
        if rows == 0 || columns == 0 {
            return None;
        }
        Some((cursor.row.min(rows - 1), cursor.column.min(columns - 1)))
    }
}
