// The main window: one document, one grid.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::gdk;
use gtk::gio;
use gtk::glib;

use comma::document::{Dialect, Document, Extent, sniff};
use comma::export::{self, Sheet};
use comma::search;

use crate::application::CommaApplication;
use crate::config::APP_ID;
use crate::grid::{self, Edited, Row, RowModel};
use crate::pdf;
use crate::shortcuts;

/// The delimiters Comma offers, in the order the menu lists them. Everything
/// about a delimiter — its menu entry, its button label, the state the action
/// carries — comes from here, so there is one place to add another.
const PRESETS: [(&str, Dialect); 5] = [
    ("comma", Dialect::comma()),
    ("tab", Dialect::tab()),
    ("semicolon", Dialect::semicolon()),
    ("pipe", Dialect::pipe()),
    ("unit-separator", Dialect::unit_separator()),
];

/// Something done to the document at a row and a column.
type Operation = fn(&mut Document, usize, usize);

/// The exports, so there is one list of what needs a document open to be worth
/// offering.
const EXPORTS: [&str; 3] = ["export-pdf", "export-html", "export-ods"];

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
/// the rest of the time.
const MOVES: [(&str, &str); 13] = [
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
    // The one key that both moves and does something: Enter opens the cell it
    // is on, and opening a cell that is already open is what commits it.
    ("edit", "Return"),
    ("edit", "KP_Enter"),
    ("edit", "F2"),
];

/// Which key asks the table for a move, for anything that wants to say so
/// without spelling it out a second time.
pub fn key_for_move(how: &str) -> Option<&'static str> {
    MOVES
        .iter()
        .find(|(name, _)| *name == how)
        .map(|(_, key)| *key)
}

/// What Comma can write that it will not read back. Each is output: never
/// reopened, never offered as Save, and never the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Pdf,
    Html,
    Ods,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Ods => "ods",
        }
    }

    fn description(self) -> String {
        match self {
            Self::Pdf => gettext("PDF Document"),
            Self::Html => gettext("Web Page"),
            Self::Ods => gettext("OpenDocument Spreadsheet"),
        }
    }
}

/// The operations that add or take away a row or a column, and whether each one
/// needs a cell to already exist. They all happen at the cell the user last
/// pointed at, which is why they all take the same two numbers.
///
/// Inserting a row is the one that does not need a cell. A file every row has
/// been taken out of has none to point at, and would otherwise be a file no row
/// could ever be put back into.
const STRUCTURE: [(&str, Operation, bool); 6] = [
    (
        "insert-row-above",
        |document, row, _| document.insert_row(row),
        false,
    ),
    (
        "insert-row-below",
        // Below the last row is the end of the file, and below the row of a
        // file with no rows is the same place.
        |document, row, _| document.insert_row((row + 1).min(document.row_count())),
        false,
    ),
    (
        "delete-row",
        |document, row, _| document.delete_row(row),
        true,
    ),
    (
        "insert-column-before",
        |document, _, column| document.insert_column(column),
        true,
    ),
    (
        "insert-column-after",
        |document, _, column| document.insert_column(column + 1),
        true,
    ),
    (
        "delete-column",
        |document, _, column| document.delete_column(column),
        true,
    ),
];

mod imp {
    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/com/excelano/Comma/gtk/window.ui")]
    pub struct CommaWindow {
        #[template_child]
        pub window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub column_view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub dialect_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub replacement: TemplateChild<gtk::Entry>,
        pub rows: RowModel,
        /// The rows as the view has them, which is the document's rows put
        /// through whatever the user has asked to see. Hiding rows and putting
        /// them in another order are views of the file and change nothing about
        /// it until they are asked to.
        pub shown: gtk::FilterListModel,
        pub sorted: gtk::SortListModel,
        /// What is being searched for. Empty means nothing is.
        pub needle: RefCell<String>,
        /// The file the document was read from, and the one Save writes back
        /// to.
        pub file: RefCell<Option<gio::File>>,
        /// The cell the user last put the keyboard on, as a row and a column of
        /// the document. Rows and columns are added and removed here. It
        /// outlives the focus that set it, so that reaching for a menu does not
        /// count as pointing somewhere else.
        pub current: Cell<Option<Cursor>>,
        /// The menu the right button opens, made the first time it is asked for
        /// and then moved to wherever it is asked for next.
        pub cell_menu: OnceCell<gtk::PopoverMenu>,
        pub settings: gio::Settings,
    }

    impl Default for CommaWindow {
        fn default() -> Self {
            Self {
                window_title: TemplateChild::default(),
                stack: TemplateChild::default(),
                column_view: TemplateChild::default(),
                open_button: TemplateChild::default(),
                dialect_button: TemplateChild::default(),
                search_bar: TemplateChild::default(),
                search_entry: TemplateChild::default(),
                replacement: TemplateChild::default(),
                rows: RowModel::default(),
                shown: gtk::FilterListModel::default(),
                sorted: gtk::SortListModel::default(),
                needle: RefCell::default(),
                file: RefCell::default(),
                current: Cell::default(),
                cell_menu: OnceCell::default(),
                settings: gio::Settings::new(APP_ID),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CommaWindow {
        const NAME: &'static str = "CommaWindow";
        type Type = super::CommaWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CommaWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let window = self.obj();
            window.bind_window_state();
            window.setup_actions();

            // The grid shows every row and selects none: there is nothing yet
            // that acts on a selected row, and a highlight would promise one.
            self.shown.set_model(Some(&self.rows));
            // A search runs over every cell of the file, so it is spread across
            // frames rather than done between one keystroke and the next.
            self.shown.set_incremental(true);
            self.sorted.set_model(Some(&self.shown));
            self.sorted.set_sorter(self.column_view.sorter().as_ref());
            self.column_view
                .set_model(Some(&gtk::NoSelection::new(Some(self.sorted.clone()))));

            // Which column the grid is sorted by decides whether there is an
            // order worth writing to the file.
            if let Some(sorter) = self.column_view.sorter() {
                sorter.connect_changed(glib::clone!(
                    #[weak]
                    window,
                    move |_, _| window.show_state()
                ));
            }

            self.dialect_button.set_menu_model(Some(&reading_menu()));
            window.setup_search();
            window.setup_navigation();
            window.watch_focus();
        }
    }

    impl WidgetImpl for CommaWindow {}

    impl WindowImpl for CommaWindow {
        /// Nothing unsaved leaves without being asked about first.
        fn close_request(&self) -> glib::Propagation {
            if !self.obj().is_modified() {
                return self.parent_close_request();
            }

            self.obj().confirm_discard(|window| window.destroy());
            glib::Propagation::Stop
        }
    }

    impl ApplicationWindowImpl for CommaWindow {}
    impl AdwApplicationWindowImpl for CommaWindow {}
}

glib::wrapper! {
    pub struct CommaWindow(ObjectSubclass<imp::CommaWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl CommaWindow {
    pub fn new(application: &CommaApplication) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    /// Window geometry survives a restart, as GNOME apps are expected to do.
    fn bind_window_state(&self) {
        let settings = &self.imp().settings;
        settings.bind("window-width", self, "default-width").build();
        settings
            .bind("window-height", self, "default-height")
            .build();
        settings.bind("window-maximized", self, "maximized").build();
    }

    fn setup_actions(&self) {
        let open = gio::ActionEntry::builder("open")
            .activate(|window: &Self, _, _| window.choose_file())
            .build();

        let save = gio::ActionEntry::builder("save")
            .activate(|window: &Self, _, _| {
                window.save();
            })
            .build();

        let save_as = gio::ActionEntry::builder("save-as")
            .activate(|window: &Self, _, _| window.save_as())
            .build();

        let undo = gio::ActionEntry::builder("undo")
            .activate(|window: &Self, _, _| window.step_history(Document::undo))
            .build();

        let redo = gio::ActionEntry::builder("redo")
            .activate(|window: &Self, _, _| window.step_history(Document::redo))
            .build();

        let find = gio::ActionEntry::builder("find")
            .state(false.to_variant())
            .change_state(|window: &Self, action, state| {
                let Some(state) = state else { return };
                action.set_state(state);

                if let Some(searching) = state.get::<bool>() {
                    window.imp().search_bar.set_search_mode(searching);
                    if searching {
                        window.imp().search_entry.grab_focus();
                    }
                }
            })
            .build();

        let replace_all = gio::ActionEntry::builder("replace-all")
            .activate(|window: &Self, _, _| window.replace_all())
            .build();

        let export_pdf = gio::ActionEntry::builder("export-pdf")
            .activate(|window: &Self, _, _| window.export(Format::Pdf))
            .build();
        let export_html = gio::ActionEntry::builder("export-html")
            .activate(|window: &Self, _, _| window.export(Format::Html))
            .build();
        let export_ods = gio::ActionEntry::builder("export-ods")
            .activate(|window: &Self, _, _| window.export(Format::Ods))
            .build();

        let move_cursor = gio::ActionEntry::builder("move-cursor")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|window: &Self, _, param| {
                if let Some(how) = param.and_then(|p| p.get::<String>()) {
                    window.move_cursor(&how);
                }
            })
            .build();

        let cell_menu = gio::ActionEntry::builder("cell-menu")
            .parameter_type(Some(
                glib::VariantTy::new("(dd)").expect("a pair of numbers"),
            ))
            .activate(|window: &Self, _, param| {
                if let Some((x, y)) = param.and_then(|at| at.get::<(f64, f64)>()) {
                    window.show_cell_menu(x, y);
                }
            })
            .build();

        let keyboard = gio::ActionEntry::builder("shortcuts")
            .activate(|window: &Self, _, _| shortcuts::present(window))
            .build();

        let commit_order = gio::ActionEntry::builder("commit-order")
            .activate(|window: &Self, _, _| window.commit_order())
            .build();

        let delimiter = gio::ActionEntry::builder("delimiter")
            .parameter_type(Some(glib::VariantTy::STRING))
            .state(PRESETS[0].0.to_variant())
            .change_state(|window: &Self, action, state| {
                let Some(state) = state else { return };
                action.set_state(state);

                if let Some(dialect) = state.str().and_then(preset) {
                    window.read_again_as(dialect);
                }
            })
            .build();

        let header = gio::ActionEntry::builder("header")
            .state(false.to_variant())
            .change_state(|window: &Self, action, state| {
                let Some(state) = state else { return };
                action.set_state(state);

                if let Some(header) = state.get::<bool>() {
                    window.imp().rows.set_header(header);
                    window.rebuild_columns();
                }
            })
            .build();

        let mut entries = vec![
            open,
            save,
            save_as,
            undo,
            redo,
            find,
            replace_all,
            move_cursor,
            cell_menu,
            keyboard,
            export_pdf,
            export_html,
            export_ods,
            commit_order,
            delimiter,
            header,
        ];
        for (name, change, needs_cell) in STRUCTURE {
            entries.push(
                gio::ActionEntry::builder(name)
                    .activate(move |window: &Self, _, _| window.change_shape(change, needs_cell))
                    .build(),
            );
        }
        self.add_action_entries(entries);

        // Everything but Open needs a document to work on.
        for (name, _, _) in STRUCTURE {
            self.set_action_enabled(name, false);
        }
        for name in [
            "save",
            "save-as",
            "undo",
            "redo",
            "commit-order",
            "replace-all",
        ] {
            self.set_action_enabled(name, false);
        }
    }

    /// The keys that move around the table, bound to the table rather than to
    /// the window so that they belong to whatever else has the focus when
    /// something else does.
    fn setup_navigation(&self) {
        let keys = gtk::ShortcutController::new();
        // Before the widgets inside the table have their say, because the column
        // view binds Home, End and Tab itself and means different things by them
        // than a table of cells does.
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);

        for (how, key) in MOVES {
            let Some(trigger) = gtk::ShortcutTrigger::parse_string(key) else {
                continue;
            };

            // Declining a key rather than always taking it is what lets a cell
            // that is open for typing keep the keys that belong to typing.
            let action = gtk::CallbackAction::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, _| {
                    if window.typing() {
                        return glib::Propagation::Proceed;
                    }
                    window.move_cursor(how);
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

        self.imp().column_view.add_controller(keys);
    }

    /// Searching hides the rows nothing matched in rather than walking a cursor
    /// from one match to the next. In a table those are the same question
    /// answered two ways, and the one that answers it all at once also shows
    /// you what Replace All is about to change.
    fn setup_search(&self) {
        let imp = self.imp();

        // Without this the bar and its box are two unrelated widgets: typing
        // with the table focused would not reach the search, and Escape would
        // not close it.
        imp.search_bar.connect_entry(&*imp.search_entry);
        imp.search_bar.set_key_capture_widget(Some(self));
        imp.shown
            .set_filter(Some(&gtk::CustomFilter::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                true,
                move |object| window.row_matches(object)
            ))));

        imp.search_entry.connect_search_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |entry| window.search_for(&entry.text())
        ));

        // Closing the search puts every row back. A file quietly missing rows
        // because of a search nobody can see would be a lie about the file.
        imp.search_bar
            .connect_search_mode_enabled_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |bar| {
                    window.set_action_state("find", &bar.is_search_mode().to_variant());
                    if !bar.is_search_mode() {
                        window.imp().search_entry.set_text("");
                    }
                }
            ));
    }

    fn search_for(&self, needle: &str) {
        self.imp().needle.replace(needle.to_string());

        if let Some(filter) = self.imp().shown.filter() {
            filter.changed(gtk::FilterChange::Different);
        }
        self.show_state();
    }

    fn row_matches(&self, object: &glib::Object) -> bool {
        let imp = self.imp();
        let needle = imp.needle.borrow();
        if needle.is_empty() {
            return true;
        }

        let (Some(row), Some(document)) = (object.downcast_ref::<Row>(), imp.rows.document())
        else {
            return true;
        };
        let document = document.borrow();
        let row = row.index();

        (0..document.field_count(row))
            .any(|column| search::contains(document.value(row, column), &needle))
    }

    /// Replaces what is being searched for, in the rows the search is showing.
    /// What you are looking at is what changes.
    fn replace_all(&self) {
        let imp = self.imp();
        let needle = imp.needle.borrow().clone();
        let Some(document) = imp.rows.document() else {
            return;
        };
        if needle.is_empty() {
            return;
        }

        let rows = self.shown_rows();

        let replacement = imp.replacement.text();
        let changed = document
            .borrow_mut()
            .replace_in(&rows, &needle, &replacement);
        if changed > 0 {
            self.reload();
        }
    }

    fn set_action_state(&self, name: &str, state: &glib::Variant) {
        if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_state(state);
        }
    }

    fn set_action_enabled(&self, name: &str, enabled: bool) {
        if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_enabled(enabled);
        }
    }

    fn choose_file(&self) {
        self.confirm_discard(|window| window.show_open_dialog());
    }

    fn show_open_dialog(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Open File"))
            .filters(&file_filters())
            .modal(true)
            .build();

        // A strong reference, deliberately: the window is what the answer is
        // for, and the dialog resolves once.
        let window = self.clone();
        dialog.open(
            Some(self),
            gio::Cancellable::NONE,
            move |result| match result {
                Ok(file) => window.open_file(&file),
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => {
                    window.report(&gettext("Could Not Open the File"), &error.to_string())
                }
            },
        );
    }

    pub fn open_file(&self, file: &gio::File) {
        let bytes = match file.load_contents(gio::Cancellable::NONE) {
            Ok((bytes, _etag)) => bytes,
            Err(error) => {
                return self.report(&gettext("Could Not Open the File"), &error.to_string());
            }
        };

        // Nothing on screen changes until the file has been read, so a file
        // that will not open leaves the one that did alone.
        let document = match Document::from_bytes(&bytes, sniff(&bytes)) {
            Ok(document) => document,
            Err(error) => {
                return self.report(&gettext("Could Not Open the File"), &error.to_string());
            }
        };

        // A new file carries no opinions over from the last one. Its first row
        // is data until this file's own user says otherwise.
        self.imp().rows.set_header(false);
        self.set_action_state("header", &false.to_variant());
        // Nor does it carry over where the last file was being worked on.
        self.imp().current.set(None);

        self.imp().file.replace(Some(file.clone()));
        self.show(document);
        self.show_folder(file);
    }

    /// Reads the document again under a different delimiter.
    ///
    /// It is read from itself rather than from the file. An untouched document
    /// writes back the bytes it was opened with, so for one that has not been
    /// edited this is the same as reading the file again; for one that has, the
    /// edits come across into whatever shape the new delimiter gives them.
    fn read_again_as(&self, dialect: Dialect) {
        let Some(current) = self.imp().rows.document() else {
            return;
        };
        let (bytes, modified) = {
            let current = current.borrow();
            if current.dialect() == dialect {
                return;
            }
            (current.to_bytes(), current.is_modified())
        };

        let mut document = Document::from_bytes(&bytes, dialect)
            .expect("these bytes came from a document that was read once already");
        if modified {
            // The edits came across; the history did not. What was undoable was
            // undoable in a shape the file no longer has.
            document.mark_modified();
        }

        self.show(document);
    }

    fn show(&self, document: Document) {
        let imp = self.imp();

        imp.rows.set_document(document);
        self.rebuild_columns();
        self.show_dialect();
        self.show_state();

        imp.dialect_button.set_visible(true);
        imp.stack.set_visible_child_name("grid");
    }

    /// Names the columns: the header record if there is one, otherwise the
    /// spreadsheet letters. A header cell that is blank names nothing, so its
    /// column keeps its letter.
    fn rebuild_columns(&self) {
        let imp = self.imp();
        let Some(document) = imp.rows.document() else {
            return;
        };
        let document = document.borrow();
        let titles = self.column_titles(&document);

        // Rebuilding the columns throws away the sorters with them, and with
        // those the arrow saying which column the grid is sorted by.
        let sorted_by = self.sorted_by();

        grid::set_columns(
            &imp.column_view,
            &titles,
            document.row_count(),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |edit| window.cell_edited(edit)
            ),
        );

        if let Some((column, direction)) = sorted_by
            && column < titles.len()
        {
            self.sort_by(column, direction);
        }
    }

    /// What to head each column with: the header record if the grid is showing
    /// one, otherwise the spreadsheet letters. A header cell that is blank names
    /// nothing, so its column keeps its letter.
    fn column_titles(&self, document: &Document) -> Vec<String> {
        let header = self.imp().rows.header() && document.row_count() > 0;

        (0..document.column_count())
            .map(|column| {
                let title = if header {
                    document.value(0, column)
                } else {
                    ""
                };
                match title.is_empty() {
                    true => grid::column_letter(column),
                    false => title.to_owned(),
                }
            })
            .collect()
    }

    /// Takes what was typed into a cell. The document decides whether that is a
    /// change at all: retyping a value leaves the file exactly as it was.
    fn cell_edited(&self, edit: Edited) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };
        document
            .borrow_mut()
            .set_value(edit.row, edit.column, edit.value);
        self.show_state();

        if edit.moving_on {
            // Enter finished the edit, so it also asks for the row below:
            // filling a column downwards is then one key per cell.
            self.move_cursor("down");
        }
    }

    /// Follows the keyboard around the grid. Where it is decides what a row or
    /// column operation happens to and what moving does next, and the only place
    /// that knows is the window: the cells are recycled underneath it.
    fn watch_focus(&self) {
        self.connect_focus_widget_notify(|window| {
            let Some(cell) = window.focused_cell() else {
                return;
            };
            window.imp().current.set(Some(Cursor {
                position: cell.position(),
                row: cell.row(),
                column: cell.column(),
            }));
            window.show_reach();
        });
    }

    /// Writes the order the grid is showing into the file. Sorting is a view of
    /// the file until this is asked for, and this is the only thing that makes
    /// it anything else.
    fn commit_order(&self) {
        let imp = self.imp();
        let Some(document) = imp.rows.document() else {
            return;
        };
        let rows = document.borrow().row_count();

        let mut order = Vec::with_capacity(rows);
        if imp.rows.header() {
            // The header record is not one of the rows and does not move.
            order.push(0);
        }
        order.extend(self.shown_rows());

        if order.len() != rows {
            // Some rows are not being shown, so this order does not account for
            // all of them, and a file is not rewritten from a part of itself.
            return;
        }

        document.borrow_mut().reorder_rows(order);
        // The file is in that order now, so the grid has nothing left to do
        // about it.
        imp.column_view
            .sort_by_column(None::<&gtk::ColumnViewColumn>, gtk::SortType::Ascending);
        self.reload();
    }

    /// Writes what the grid is showing somewhere else, in a format Comma cannot
    /// read back.
    ///
    /// Deliberately not Save, and deliberately somewhere the user has to name:
    /// there is never a moment where the file being edited has quietly become a
    /// PDF.
    fn export(&self, format: Format) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Export"))
            .filters(&format_filter(format))
            .modal(true)
            .build();
        dialog.set_initial_name(Some(&export_name(&self.document_name(), format)));
        if let Some(folder) = self
            .imp()
            .file
            .borrow()
            .as_ref()
            .and_then(gio::File::parent)
        {
            dialog.set_initial_folder(Some(&folder));
        }

        let window = self.clone();
        dialog.save(
            Some(self),
            gio::Cancellable::NONE,
            move |result| match result {
                Ok(file) => window.export_to(&file, format),
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => {
                    window.report(&gettext("Could Not Export the File"), &error.to_string())
                }
            },
        );
    }

    fn export_to(&self, file: &gio::File, format: Format) {
        let failed = gettext("Could Not Export the File");
        let Some(document) = self.imp().rows.document() else {
            return;
        };

        let document = document.borrow();
        let columns = self.column_titles(&document);
        let rows = self.shown_rows();
        let sheet = Sheet {
            document: &document,
            columns: &columns,
            titled: self.imp().rows.header() && document.row_count() > 0,
            rows: &rows,
            name: &self.document_name(),
        };

        let written = match format {
            // A print operation writes to a path of its own accord, so this is
            // the one export that cannot be sent somewhere GIO can reach but the
            // filesystem cannot.
            Format::Pdf => match file.path() {
                Some(path) => pdf::write(&sheet, &path.to_string_lossy(), self)
                    .map_err(|error| error.to_string()),
                None => Err(gettext(
                    "A PDF can only be written to a folder on this computer.",
                )),
            },
            Format::Html => self.put(file, export::html(&sheet).as_bytes()),
            Format::Ods => self.put(file, &export::ods(&sheet)),
        };

        if let Err(error) = written {
            self.report(&failed, &error);
        }
    }

    fn put(&self, file: &gio::File, bytes: &[u8]) -> Result<(), String> {
        file.replace_contents(
            bytes,
            None,
            false,
            gio::FileCreateFlags::NONE,
            gio::Cancellable::NONE,
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    /// The rows the grid is showing, in the order it is showing them. An export
    /// is a picture of the grid, so a search and a sort are part of it.
    fn shown_rows(&self) -> Vec<usize> {
        // A sort model says its items are plain objects, because it cannot know
        // what it will be given until it is given it.
        self.imp()
            .sorted
            .iter::<glib::Object>()
            .flatten()
            .filter_map(|object| object.downcast::<Row>().ok())
            .map(|row| row.index())
            .collect()
    }

    /// Which data column the grid is sorted by, and which way, when it is
    /// sorted by one at all.
    fn sorted_by(&self) -> Option<(usize, gtk::SortType)> {
        let sorter = self
            .imp()
            .column_view
            .sorter()?
            .downcast::<gtk::ColumnViewSorter>()
            .ok()?;
        let sorted = sorter.primary_sort_column()?;

        let columns = self.imp().column_view.columns();
        let position = (0..columns.n_items()).find(|&index| {
            columns
                .item(index)
                .and_downcast::<gtk::ColumnViewColumn>()
                .is_some_and(|column| column == sorted)
        })?;

        // The first column is the row-number gutter, which nothing sorts by.
        Some((
            position.checked_sub(1)? as usize,
            sorter.primary_sort_order(),
        ))
    }

    fn sort_by(&self, column: usize, direction: gtk::SortType) {
        let columns = self.imp().column_view.columns();
        if let Some(column) = columns
            .item(column as u32 + 1)
            .and_downcast::<gtk::ColumnViewColumn>()
        {
            self.imp()
                .column_view
                .sort_by_column(Some(&column), direction);
        }
    }

    /// Adds or removes a row or a column at the current cell.
    fn change_shape(&self, change: Operation, needs_cell: bool) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };
        let (row, column) = match (self.current_cell(), needs_cell) {
            (Some(cell), _) => cell,
            (None, true) => return,
            // Nothing to point at, so the operation happens where the file
            // begins.
            (None, false) => (0, 0),
        };

        change(&mut document.borrow_mut(), row, column);
        self.reload();
    }

    /// Where the next row or column goes, kept inside the document it refers
    /// to: rows and columns the current cell outlived are no longer there.
    fn current_cell(&self) -> Option<(usize, usize)> {
        let document = self.imp().rows.document()?;
        let cursor = self.imp().current.get()?;

        let document = document.borrow();
        let (rows, columns) = (document.row_count(), document.column_count());
        if rows == 0 || columns == 0 {
            return None;
        }
        Some((cursor.row.min(rows - 1), cursor.column.min(columns - 1)))
    }

    /// Moves the keyboard to another cell, or opens the one it is on.
    ///
    /// Moving is done by asking the view to scroll somewhere and take the focus
    /// with it, which is the one way that works whether or not the cell being
    /// moved to has been drawn yet.
    fn move_cursor(&self, how: &str) {
        let imp = self.imp();
        let Some(cursor) = imp.current.get() else {
            // Nothing is current yet — the table has the keyboard but no cell
            // of it does — so the first thing asked for is only to be
            // somewhere, and the beginning is where that is.
            self.go_to(0, 0);
            return;
        };
        if how == "edit" {
            self.edit_cell();
            return;
        }

        let rows = imp.sorted.n_items();
        let columns = imp.column_view.columns().n_items().saturating_sub(1);
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

    /// Takes the keyboard out of the table, to the first thing in the window
    /// that is not part of it.
    fn leave_table(&self) {
        self.imp().open_button.grab_focus();
    }

    /// Whether a cell is open for typing, in which case the keys that move
    /// around the table are the entry's: Home and End move the caret through
    /// what is being typed rather than moving to another cell.
    fn typing(&self) -> bool {
        gtk::prelude::RootExt::focus(self).is_some_and(|widget| widget.is::<gtk::Text>())
    }

    /// Puts the cursor, and the keyboard with it, on one cell of the table.
    fn go_to(&self, position: u32, column: usize) {
        let imp = self.imp();
        let Some(target) = imp
            .column_view
            .columns()
            .item(column as u32 + 1)
            .and_downcast::<gtk::ColumnViewColumn>()
        else {
            return;
        };

        // The cursor moves whether or not the keyboard can follow it there yet,
        // so that moving twice in a row lands where two moves should.
        if let Some(row) = imp
            .sorted
            .item(position)
            .and_downcast::<Row>()
            .map(|row| row.index())
        {
            imp.current.set(Some(Cursor {
                position,
                row,
                column,
            }));
        }

        imp.column_view
            .scroll_to(position, Some(&target), gtk::ListScrollFlags::empty(), None);
        if !grid::focus_cell(&imp.column_view, position, column) {
            // That row has not been drawn yet. It will have been once the
            // scrolling above has happened, which is the next turn of the loop.
            glib::idle_add_local_once(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || {
                    grid::focus_cell(&window.imp().column_view, position, column);
                }
            ));
        }
        self.show_reach();
    }

    /// Puts the row and column operations under the pointer, which is where a
    /// table is usually asked about them.
    ///
    /// The menu is the same one the main menu holds, built once and moved to
    /// wherever it was asked for.
    fn show_cell_menu(&self, x: f64, y: f64) {
        let imp = self.imp();
        let Some(cell) = self.focused_cell() else {
            return;
        };

        let menu = imp.cell_menu.get_or_init(|| {
            let menu = gtk::PopoverMenu::from_model(Some(&structure_menu()));
            menu.set_has_arrow(false);
            menu.set_halign(gtk::Align::Start);
            menu.set_parent(&*imp.column_view);
            menu
        });

        // The click came in the cell's own coordinates, and the menu hangs off
        // the table.
        let Some(at) = cell.compute_point(
            &*imp.column_view,
            &gtk::graphene::Point::new(x as f32, y as f32),
        ) else {
            return;
        };
        menu.set_pointing_to(Some(&gdk::Rectangle::new(
            at.x() as i32,
            at.y() as i32,
            1,
            1,
        )));
        menu.popup();
    }

    /// Opens the cell the keyboard is on for typing.
    fn edit_cell(&self) {
        if let Some(cell) = self.focused_cell() {
            cell.begin();
        }
    }

    /// The cell the keyboard is in, wherever inside it the focus has landed.
    ///
    /// Focus sits on the entry inside a cell while one is being typed in, and on
    /// a whole row of the table on the way in from the toolbar, so the answer is
    /// looked for above and below whatever has it.
    fn focused_cell(&self) -> Option<grid::Cell> {
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

    fn step_history(&self, step: fn(&mut Document) -> Option<Extent>) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };
        let Some(extent) = step(&mut document.borrow_mut()) else {
            return;
        };

        match extent {
            Extent::Record(row) => {
                let imp = self.imp();
                imp.rows.row_changed(row);
                if imp.rows.header() && row == 0 {
                    // That record is a set of column titles at the moment.
                    self.rebuild_columns();
                }
                self.show_state();
            }
            Extent::Shape => self.reload(),
        }
    }

    /// Draws the grid again from the document, for changes that moved rows or
    /// columns rather than only what one of them says.
    fn reload(&self) {
        self.imp().rows.reload();
        self.rebuild_columns();
        self.show_state();
    }

    /// Writes the document back to the file it came from, and says whether it
    /// managed to. A document with no file of its own has to be asked where to
    /// go, and that answer arrives too late to report here.
    fn save(&self) -> bool {
        let file = self.imp().file.borrow().clone();
        match file {
            Some(file) => self.save_to(&file),
            None => {
                self.save_as();
                false
            }
        }
    }

    fn save_to(&self, file: &gio::File) -> bool {
        let Some(document) = self.imp().rows.document() else {
            return false;
        };

        let bytes = document.borrow().to_bytes();
        if let Err(error) = file.replace_contents(
            &bytes,
            None,
            false,
            gio::FileCreateFlags::NONE,
            gio::Cancellable::NONE,
        ) {
            self.report(&gettext("Could Not Save the File"), &error.to_string());
            return false;
        }

        document.borrow_mut().mark_saved();
        self.imp().file.replace(Some(file.clone()));
        self.show_folder(file);
        self.show_state();
        true
    }

    fn save_as(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Save File"))
            .filters(&file_filters())
            .modal(true)
            .build();

        if let Some(file) = self.imp().file.borrow().as_ref() {
            dialog.set_initial_name(Some(&display_name(file)));
            if let Some(folder) = file.parent() {
                dialog.set_initial_folder(Some(&folder));
            }
        }

        let window = self.clone();
        dialog.save(
            Some(self),
            gio::Cancellable::NONE,
            move |result| match result {
                Ok(file) => {
                    window.save_to(&file);
                }
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => {
                    window.report(&gettext("Could Not Save the File"), &error.to_string())
                }
            },
        );
    }

    fn is_modified(&self) -> bool {
        self.imp()
            .rows
            .document()
            .is_some_and(|document| document.borrow().is_modified())
    }

    /// Asks before anything unsaved is thrown away, then does `next`. With
    /// nothing to lose there is nothing to ask, and `next` happens straight
    /// away.
    fn confirm_discard(&self, next: impl Fn(&Self) + 'static) {
        if !self.is_modified() {
            return next(self);
        }

        let message = gettext("“{}” has unsaved changes. Changes that are not saved will be lost.")
            .replace("{}", &self.document_name());
        let dialog = adw::AlertDialog::new(Some(&gettext("Save Changes?")), Some(&message));
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("discard", &gettext("_Discard"));
        dialog.add_response("save", &gettext("_Save"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        let window = self.clone();
        dialog.choose(Some(self), gio::Cancellable::NONE, move |response| {
            match response.as_str() {
                "discard" => next(&window),
                // Nothing goes ahead on the strength of a save that did not
                // happen.
                "save" if window.save() => next(&window),
                _ => {}
            }
        });
    }

    /// Puts the delimiter in front of the user rather than leaving it guessed
    /// at silently: on the button, and as the item ticked in its menu.
    fn show_dialect(&self) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };
        let dialect = document.borrow().dialect();
        let id = preset_id(dialect).expect("every dialect Comma reads with is one of its presets");

        self.imp().dialect_button.set_label(&preset_label(id));
        self.set_action_state("delimiter", &id.to_variant());
    }

    /// What the window says about the document rather than shows of it: whether
    /// it has unsaved changes, and which of Save, Undo and Redo have anything
    /// to do.
    fn show_state(&self) {
        let (modified, undo, redo) = match self.imp().rows.document() {
            Some(document) => {
                let document = document.borrow();
                (
                    document.is_modified(),
                    document.can_undo(),
                    document.can_redo(),
                )
            }
            None => (false, false, false),
        };

        self.show_title(modified);
        self.set_action_enabled("save", modified);
        let open = self.imp().rows.document().is_some();
        self.set_action_enabled("save-as", open);
        for name in EXPORTS {
            self.set_action_enabled(name, open);
        }
        self.set_action_enabled("undo", undo);
        self.set_action_enabled("redo", redo);
        let searching = !self.imp().needle.borrow().is_empty();
        self.set_action_enabled("replace-all", searching);
        // An order cannot be written down from a view that is not showing every
        // row it would put in order.
        self.set_action_enabled("commit-order", self.sorted_by().is_some() && !searching);

        self.show_reach();
    }

    /// Which row and column operations have somewhere to happen.
    fn show_reach(&self) {
        let cell = self.current_cell().is_some();
        let document = self.imp().rows.document().is_some();
        for (name, _, needs_cell) in STRUCTURE {
            self.set_action_enabled(name, if needs_cell { cell } else { document });
        }
    }

    /// The bullet in front of the name is what Apostrophe does, and what Comma
    /// does for the same reason: it is the one part of the window that is
    /// always on screen.
    fn show_title(&self, modified: bool) {
        let name = self.document_name();
        let title = if modified {
            format!("• {name}")
        } else {
            name
        };

        self.imp().window_title.set_title(&title);
        self.set_title(Some(&title));
    }

    fn show_folder(&self, file: &gio::File) {
        self.imp().window_title.set_subtitle(&folder_of(file));
    }

    fn document_name(&self) -> String {
        match self.imp().file.borrow().as_ref() {
            Some(file) => display_name(file),
            None => gettext("Untitled"),
        }
    }

    fn report(&self, title: &str, message: &str) {
        let dialog = adw::AlertDialog::new(Some(title), Some(message));
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(self));
    }
}

/// A name to offer for an export: the file's own, wearing the new extension.
fn export_name(name: &str, format: Format) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    format!("{stem}.{}", format.extension())
}

fn format_filter(format: Format) -> gio::ListStore {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(&format.description()));
    filter.add_pattern(&format!("*.{}", format.extension()));

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

fn file_filters() -> gio::ListStore {
    let delimited = gtk::FileFilter::new();
    delimited.set_name(Some(&gettext("Delimited Text")));
    for pattern in ["*.csv", "*.tsv", "*.tab", "*.dsv", "*.txt"] {
        delimited.add_pattern(pattern);
    }

    let all = gtk::FileFilter::new();
    all.set_name(Some(&gettext("All Files")));
    all.add_pattern("*");

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&delimited);
    filters.append(&all);
    filters
}

fn preset(id: &str) -> Option<Dialect> {
    PRESETS
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, dialect)| *dialect)
}

fn preset_id(dialect: Dialect) -> Option<&'static str> {
    PRESETS
        .iter()
        .find(|(_, candidate)| *candidate == dialect)
        .map(|(id, _)| *id)
}

/// What a delimiter is called on the button and in its menu.
///
/// Said in the context of delimiters, because in English the comma delimiter and
/// the application share a word and in other languages they will not.
fn preset_label(id: &str) -> String {
    // Written out one by one rather than through a helper: what marks these for
    // translation is the extractor seeing the words next to the context, and it
    // cannot follow them through anything.
    match id {
        "comma" => pgettext("delimiter", "Comma"),
        "tab" => pgettext("delimiter", "Tab"),
        "semicolon" => pgettext("delimiter", "Semicolon"),
        "pipe" => pgettext("delimiter", "Pipe"),
        "unit-separator" => pgettext("delimiter", "ASCII Separators"),
        other => other.to_string(),
    }
}

/// The row and column operations, for the menu that appears where the pointer
/// is. The same items the main menu lists, because they are the same operations.
fn structure_menu() -> gio::Menu {
    let rows = gio::Menu::new();
    rows.append(
        Some(&gettext("Insert Row Above")),
        Some("win.insert-row-above"),
    );
    rows.append(
        Some(&gettext("Insert Row Below")),
        Some("win.insert-row-below"),
    );
    rows.append(Some(&gettext("Delete Row")), Some("win.delete-row"));

    let columns = gio::Menu::new();
    columns.append(
        Some(&gettext("Insert Column Before")),
        Some("win.insert-column-before"),
    );
    columns.append(
        Some(&gettext("Insert Column After")),
        Some("win.insert-column-after"),
    );
    columns.append(Some(&gettext("Delete Column")), Some("win.delete-column"));

    let menu = gio::Menu::new();
    menu.append_section(None, &rows);
    menu.append_section(None, &columns);
    menu
}

/// How the file is being read: which delimiter, and whether its first record is
/// data or column titles.
fn reading_menu() -> gio::Menu {
    let delimiters = gio::Menu::new();
    for (id, _) in PRESETS {
        let item = gio::MenuItem::new(Some(&preset_label(id)), None);
        item.set_action_and_target_value(Some("win.delimiter"), Some(&id.to_variant()));
        delimiters.append_item(&item);
    }

    let records = gio::Menu::new();
    records.append(Some(&gettext("First Row Is a Header")), Some("win.header"));

    let menu = gio::Menu::new();
    menu.append_section(Some(&gettext("Delimiter")), &delimiters);
    menu.append_section(None, &records);
    menu
}

fn display_name(file: &gio::File) -> String {
    file.basename()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| gettext("Untitled"))
}

/// The folder the file is in, with the home directory written the way people
/// write it.
fn folder_of(file: &gio::File) -> String {
    let Some(folder) = file.parent().and_then(|parent| parent.path()) else {
        return String::new();
    };

    match folder.strip_prefix(glib::home_dir()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => folder.display().to_string(),
    }
}
