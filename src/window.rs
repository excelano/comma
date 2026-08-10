// The main window: one document, one grid.
//
// What is here is the window itself — the widget, the state it holds, the
// actions everything else is reached through, and what the window says about
// the document. The rest is next door: `cursor` for where the keyboard is,
// `files` for where the document comes from and goes, `showing` for which rows
// the grid is showing and in what order, and `menus` for the menus.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod cursor;
mod files;
mod menus;
mod showing;

use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use comma::document::{Dialect, Document, Extent};

use crate::application::CommaApplication;
use crate::config::APP_ID;
use crate::grid::{self, Edited, RowModel};
use crate::shortcuts;

use cursor::Cursor;
use files::{EXPORTS, Format};
use menus::{PRESETS, preset, reading_menu};

pub use cursor::key_for_move;

/// Something done to the document at a row and a column.
type Operation = fn(&mut Document, usize, usize);

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

    /// Draws the grid again from the document, for changes that moved rows or
    /// columns rather than only what one of them says.
    fn reload(&self) {
        self.imp().rows.reload();
        self.rebuild_columns();
        self.show_state();
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

    fn report(&self, title: &str, message: &str) {
        let dialog = adw::AlertDialog::new(Some(title), Some(message));
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(self));
    }
}
