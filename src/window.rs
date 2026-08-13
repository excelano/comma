// The main window: one document, one grid.
//
// What is here is the window itself — the widget, the state it holds, the
// actions everything else is reached through, and what the window says about
// the document. The rest is next door: `cursor` for where the keyboard is,
// `files` for where the document comes from and goes, `showing` for which rows
// the grid is showing and in what order, `menus` for the menus, and `place` for
// handing the file to something outside the window.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod cursor;
mod files;
mod filters;
mod menus;
mod place;
mod showing;
mod watch;

use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use comma::document::{Dialect, Document, Extent};
use comma::filter::Filters;

use crate::application::CommaApplication;
use crate::config::APP_ID;
use crate::grid::{self, Edited, Gutter, RowModel};
use crate::shortcuts;
use crate::translatable;

use cursor::Cursor;
use files::{Format, Task};
use menus::{PRESETS, column_menu, preset, primary_menu, reading_menu};
use place::Tool;
use watch::Adrift;

pub use cursor::key_for;

/// Which way into the file an operation reaches. It is what the operation is
/// addressed by, and it is what decides which handle offers it: the row numbers
/// down the side offer the rows, the headings across the top offer the columns,
/// and a cell offers both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Row,
    Column,
}

/// One thing that can be done to the shape of the file.
///
/// Everything about an operation is here: the action it is reached through, the
/// words every menu offers it in, the axis it is addressed by, and what it does.
/// Four menus are built from this table, so a seventh operation is one line
/// rather than an errand around the source.
#[derive(Debug, Clone, Copy)]
struct Operation {
    name: &'static str,
    label: &'static str,
    axis: Axis,
    /// Takes the row or the column, whichever the axis says, and says how much
    /// of the file it moved so the grid can redraw that much and no more.
    change: fn(&mut Document, usize) -> Extent,
    /// Whether the row or column has to already exist for this to mean
    /// anything. Inserting a row is the one that does not: a file every row has
    /// been taken out of has none to point at, and would otherwise be a file no
    /// row could ever be put back into.
    needs_cell: bool,
    /// Whether it names a place next to another row rather than the row itself,
    /// which only means something while the grid is showing the file in the
    /// file's own order.
    ///
    /// Above and below are the two words a sort or a search takes the meaning
    /// out of. Sorted, the row a new one is put next to is somewhere else on
    /// screen, and a blank row sorts to whichever end blanks go to rather than
    /// to where it was asked for. Searched, it does not match and is not shown
    /// at all. Deleting a row survives both, because "this row" means the same
    /// thing in any order, and so do all the columns, which nothing here sorts
    /// or hides.
    needs_file_order: bool,
}

const STRUCTURE: [Operation; 6] = [
    Operation {
        name: "insert-row-above",
        label: translatable("Insert Row Above"),
        axis: Axis::Row,
        change: |document, row| document.insert_row(row),
        needs_cell: false,
        needs_file_order: true,
    },
    Operation {
        name: "insert-row-below",
        label: translatable("Insert Row Below"),
        axis: Axis::Row,
        // Below the last row is the end of the file, and below the row of a
        // file with no rows is the same place.
        change: |document, row| document.insert_row((row + 1).min(document.row_count())),
        needs_cell: false,
        needs_file_order: true,
    },
    Operation {
        name: "delete-row",
        label: translatable("Delete Row"),
        axis: Axis::Row,
        change: |document, row| document.delete_row(row),
        needs_cell: true,
        needs_file_order: false,
    },
    Operation {
        name: "insert-column-before",
        label: translatable("Insert Column Before"),
        axis: Axis::Column,
        change: |document, column| document.insert_column(column),
        needs_cell: true,
        needs_file_order: false,
    },
    Operation {
        name: "insert-column-after",
        label: translatable("Insert Column After"),
        axis: Axis::Column,
        change: |document, column| document.insert_column(column + 1),
        needs_cell: true,
        needs_file_order: false,
    },
    Operation {
        name: "delete-column",
        label: translatable("Delete Column"),
        axis: Axis::Column,
        change: |document, column| document.delete_column(column),
        needs_cell: true,
        needs_file_order: false,
    },
];

/// What an operation's action carries: which row or column to act on, or
/// `AT_CURSOR` for wherever the cursor is.
///
/// The headings are why a target exists at all. A menu on a heading is GTK's
/// own, popped without asking us first, so it cannot move the cursor into the
/// column it belongs to on the way up — it has to say which column it means.
/// Everywhere else the cursor has already been moved and there is nothing to
/// say.
///
/// A number with a spare value in it rather than the maybe type this wants,
/// because GTK puts a window's actions on the session bus and D-Bus has no
/// maybe: an action typed `mu` brings the window down as it is being exported.
const TARGET: &str = "i";
const AT_CURSOR: i32 = -1;

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
        /// The table's scrolled window, which holds the adjustments both views
        /// are moved by.
        #[template_child]
        pub table: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub gutter_scroller: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub gutter_view: TemplateChild<gtk::ColumnView>,
        /// The table's horizontal scrollbar, which is outside it so that it
        /// takes its strip of height off the gutter as well.
        #[template_child]
        pub across: TemplateChild<gtk::Scrollbar>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub dialect_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub replacement: TemplateChild<gtk::Entry>,
        /// The bar that says which conditions the columns are being held to,
        /// and the box the chips saying so are put in.
        /// Where something that has just happened is said, over the grid.
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
        /// Where something the file is doing is said, until it stops.
        #[template_child]
        pub banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub filter_bar: TemplateChild<gtk::Box>,
        #[template_child]
        pub chips: TemplateChild<gtk::Box>,
        pub rows: RowModel,
        /// The row numbers, in a view of their own beside the table.
        pub gutter: Gutter,
        /// The rows as the view has them, which is the document's rows put
        /// through whatever the user has asked to see. Hiding rows and putting
        /// them in another order are views of the file and change nothing about
        /// it until they are asked to.
        pub shown: gtk::FilterListModel,
        pub sorted: gtk::SortListModel,
        /// What is being searched for. Empty means nothing is.
        pub needle: RefCell<String>,
        /// What each column is being held to, at most one condition apiece.
        /// Empty means every row the search leaves is shown.
        pub filters: RefCell<Filters>,
        /// The file the document was read from, and the one Save writes back
        /// to.
        pub file: RefCell<Option<gio::File>>,
        /// What that file looked like when Comma last read or wrote it. What
        /// it looks like now is compared against this to tell somebody else's
        /// write from our own.
        pub stamp: RefCell<Option<(glib::GString, u64)>>,
        /// Watches the file for writes made anywhere else. None when there is
        /// no file, or when the filesystem will not be watched.
        pub monitor: RefCell<Option<gio::FileMonitor>>,
        /// Whether a look at the file is already coming. One write arrives as
        /// several events and is worth one look.
        pub settling: Cell<bool>,
        /// What the banner is saying, so that it is not set to what it already
        /// says.
        pub adrift: Cell<Option<Adrift>>,
        /// The cell the user last put the keyboard on, as a row and a column of
        /// the document. Rows and columns are added and removed here. It
        /// outlives the focus that set it, so that reaching for a menu does not
        /// count as pointing somewhere else.
        pub current: Cell<Option<Cursor>>,
        /// Which column the grid was sorted by before the sorter last changed,
        /// which is the only way to tell a heading clicked a third time from
        /// one clicked for the first.
        pub previous_sort: Cell<Option<(usize, gtk::SortType)>>,
        /// The menus the right button opens, each made the first time it is
        /// asked for and then moved to wherever it is asked for next. A cell
        /// offers both halves; a row number offers only the rows.
        pub cell_menu: OnceCell<gtk::PopoverMenu>,
        pub row_menu: OnceCell<gtk::PopoverMenu>,
        pub settings: gio::Settings,
    }

    impl Default for CommaWindow {
        fn default() -> Self {
            Self {
                window_title: TemplateChild::default(),
                stack: TemplateChild::default(),
                column_view: TemplateChild::default(),
                table: TemplateChild::default(),
                gutter_scroller: TemplateChild::default(),
                gutter_view: TemplateChild::default(),
                across: TemplateChild::default(),
                open_button: TemplateChild::default(),
                menu_button: TemplateChild::default(),
                dialect_button: TemplateChild::default(),
                search_bar: TemplateChild::default(),
                search_entry: TemplateChild::default(),
                replacement: TemplateChild::default(),
                toasts: TemplateChild::default(),
                banner: TemplateChild::default(),
                filter_bar: TemplateChild::default(),
                chips: TemplateChild::default(),
                rows: RowModel::default(),
                gutter: Gutter::default(),
                shown: gtk::FilterListModel::default(),
                sorted: gtk::SortListModel::default(),
                needle: RefCell::default(),
                filters: RefCell::default(),
                file: RefCell::default(),
                stamp: RefCell::default(),
                monitor: RefCell::default(),
                settling: Cell::default(),
                adrift: Cell::default(),
                current: Cell::default(),
                previous_sort: Cell::default(),
                cell_menu: OnceCell::default(),
                row_menu: OnceCell::default(),
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
            // One model behind both views, so the numbers are in the order the
            // table is in without being told, and row n of one is row n of the
            // other by identity rather than by arithmetic.
            let model = gtk::NoSelection::new(Some(self.sorted.clone()));
            self.column_view.set_model(Some(&model));
            self.gutter.attach(&self.gutter_view, &model);
            window.share_scrolling();

            // Which column the grid is sorted by decides whether there is an
            // order worth writing to the file, and whether a row can be put
            // above another one at all.
            if let Some(sorter) = self.column_view.sorter() {
                sorter.connect_changed(glib::clone!(
                    #[weak]
                    window,
                    move |_, _| window.sorting_changed()
                ));
            }

            self.menu_button.set_menu_model(Some(&primary_menu()));
            self.dialect_button.set_menu_model(Some(&reading_menu()));
            // The banner offers one thing, and only when there is something to
            // offer: reading the file again.
            self.banner.connect_button_clicked(glib::clone!(
                #[weak]
                window,
                move |_| {
                    ActionGroupExt::activate_action(&window, "reload", None);
                }
            ));

            window.setup_search();
            window.setup_filters();
            window.setup_navigation();
            window.watch_focus();
        }

        /// A popover is parented to the widget it opens over rather than held
        /// as its child, so nothing takes these two down with the window. Left
        /// alone they outlive the view they point at, which GTK says so on the
        /// way out.
        fn dispose(&self) {
            for menu in [self.cell_menu.get(), self.row_menu.get()]
                .into_iter()
                .flatten()
            {
                menu.unparent();
            }
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

        let filter_column = gio::ActionEntry::builder("filter-column")
            .parameter_type(Some(
                glib::VariantTy::new(TARGET).expect("a column, or the one the cursor is in"),
            ))
            .activate(|window: &Self, _, param| {
                let at = param.and_then(|at| at.get::<i32>()).unwrap_or(AT_CURSOR);
                window.filter_column(at);
            })
            .build();

        let filter_to_value = gio::ActionEntry::builder("filter-to-value")
            .activate(|window: &Self, _, _| window.filter_to_value())
            .build();

        let clear_filter = gio::ActionEntry::builder("clear-filter")
            .parameter_type(Some(
                glib::VariantTy::new(TARGET).expect("a column, or the one the cursor is in"),
            ))
            .activate(|window: &Self, _, param| {
                let at = param.and_then(|at| at.get::<i32>()).unwrap_or(AT_CURSOR);
                window.clear_filter(at);
            })
            .build();

        let clear_filters = gio::ActionEntry::builder("clear-filters")
            .activate(|window: &Self, _, _| window.clear_filters())
            .build();

        // Opening the cell the cursor is on, as something that can be asked for
        // rather than only pressed. The keys that do it are the grid's own and
        // are bound to the table; this is the same act named, which is what
        // anything driving Comma from outside has to have.
        let edit = gio::ActionEntry::builder("edit")
            .activate(|window: &Self, _, _| window.edit_cell())
            .build();

        let reload = gio::ActionEntry::builder("reload")
            .activate(|window: &Self, _, _| {
                // The same question Open asks, for the same reason: reading the
                // file again is how edits that were never saved stop existing.
                window.confirm_discard(|window| {
                    if window.read_the_file_again().is_some() {
                        window.show_adrift(None);
                    }
                })
            })
            .build();

        let move_cursor = gio::ActionEntry::builder("move-cursor")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|window: &Self, _, param| {
                if let Some(how) = param.and_then(|p| p.get::<String>()) {
                    window.move_cursor(&how);
                }
            })
            .build();

        let go_to = gio::ActionEntry::builder("go-to")
            .parameter_type(Some(
                glib::VariantTy::new("(uu)").expect("a position and a column"),
            ))
            .activate(|window: &Self, _, param| {
                if let Some((position, column)) = param.and_then(|at| at.get::<(u32, u32)>()) {
                    window.go_to(position, column as usize);
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

        let row_menu = gio::ActionEntry::builder("row-menu")
            .parameter_type(Some(
                glib::VariantTy::new("(dd)").expect("a pair of numbers"),
            ))
            .activate(|window: &Self, _, param| {
                if let Some((x, y)) = param.and_then(|at| at.get::<(f64, f64)>()) {
                    window.show_row_menu(x, y);
                }
            })
            .build();

        let open_containing_folder = gio::ActionEntry::builder("open-containing-folder")
            .activate(|window: &Self, _, _| window.open_containing_folder())
            .build();

        let open_terminal = gio::ActionEntry::builder("open-terminal")
            .activate(|window: &Self, _, _| window.open_terminal())
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
            reload,
            edit,
            undo,
            redo,
            find,
            replace_all,
            filter_column,
            filter_to_value,
            clear_filter,
            clear_filters,
            move_cursor,
            go_to,
            cell_menu,
            row_menu,
            open_containing_folder,
            open_terminal,
            keyboard,
            commit_order,
            delimiter,
            header,
        ];
        for format in Format::ALL {
            entries.push(
                gio::ActionEntry::builder(format.action())
                    .activate(move |window: &Self, _, _| window.export(format))
                    .build(),
            );
        }
        for tool in Tool::ALL {
            entries.push(
                gio::ActionEntry::builder(tool.action())
                    .activate(move |window: &Self, _, _| window.open_with(tool))
                    .build(),
            );
        }
        for operation in STRUCTURE {
            entries.push(
                gio::ActionEntry::builder(operation.name)
                    .parameter_type(Some(
                        glib::VariantTy::new(TARGET).expect("a row or a column, or neither"),
                    ))
                    .activate(move |window: &Self, _, param| {
                        let at = param.and_then(|at| at.get::<i32>()).unwrap_or(AT_CURSOR);
                        window.change_shape(&operation, at);
                    })
                    .build(),
            );
        }
        self.add_action_entries(entries);

        // Everything but Open needs a document to work on.
        for operation in STRUCTURE {
            self.set_action_enabled(operation.name, false);
        }
        for name in [
            "save",
            "save-as",
            "reload",
            "undo",
            "redo",
            "commit-order",
            "replace-all",
            "filter-column",
            "filter-to-value",
            "clear-filter",
            "edit",
            "clear-filters",
            "open-containing-folder",
            "open-terminal",
        ] {
            self.set_action_enabled(name, false);
        }
        for tool in Tool::ALL {
            self.set_action_enabled(tool.action(), false);
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

        // The conditions were about the columns of another file, or of this one
        // read under a delimiter that gave it different ones. Either way there
        // is nothing left for them to be about.
        self.forget_filters();

        // A file starts at its beginning. The cursor is where an operation
        // lands, so leaving it undecided until the keyboard arrives would mean
        // a menu full of things greyed out on a file that is plainly there.
        imp.current.set(
            match document.row_count() > 0 && document.column_count() > 0 {
                true => Some(Cursor {
                    position: 0,
                    row: 0,
                    column: 0,
                }),
                false => None,
            },
        );

        imp.rows.set_document(document);
        self.rebuild_columns();
        self.show_filters();
        self.show_dialect();
        self.show_state();

        imp.dialect_button.set_visible(true);
        imp.stack.set_visible_child_name("grid");
    }

    /// Puts the same file, read again, in front of the user — keeping what
    /// they had asked to see of it.
    ///
    /// The difference from `show` is what is kept rather than what is done. A
    /// different file arrives knowing nothing about the last one, but this is
    /// the file already on screen and the conditions, the sorting and the place
    /// the keyboard is were asked for about *these* columns. Throwing them away
    /// on every write would make watching a file somebody else is editing
    /// useless, which is what this is for.
    /// Says whether it had anything to tell the user, so that a reload which
    /// has already spoken for itself is not announced twice.
    fn show_again(&self, document: Document) -> bool {
        let imp = self.imp();

        // Unless the file came back narrower, in which case some of them are
        // about columns that are not there any more.
        let dropped = imp.filters.borrow_mut().clamp(document.column_count());
        if dropped {
            self.say(&gettext(
                "Reloaded. Some filters were on columns the file no longer has.",
            ));
        }
        self.hold_cursor(&document);

        // Where along the file the user is looking. The columns are built again
        // below, and a view rebuilt while the keyboard is inside it scrolls to
        // wherever the keyboard is — which on a wide file means somebody else's
        // write moving what you were reading.
        let across = imp.table.hadjustment().value();

        imp.rows.set_document(document);
        self.rebuild_columns();
        self.show_filters();
        self.show_dialect();
        self.show_state();
        self.find_cursor_again();

        // Once the columns have been measured. Before that the view is as wide
        // as it is going to be told to be, and putting the number back means
        // nothing.
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move || window.imp().table.hadjustment().set_value(across)
        ));

        dropped
    }

    /// Keeps the cursor on the record it was on across a file being read
    /// again, as far as the file still goes. A shorter file takes the last row
    /// with it.
    ///
    /// The row is what is held; where that row now sits in the view is looked
    /// up afterwards by `find_cursor_again`, once there is a view to look it up
    /// in. A row of the file rather than a record of it — see there for why
    /// that is as far as this goes.
    fn hold_cursor(&self, document: &Document) {
        let imp = self.imp();
        let (rows, columns) = (document.row_count(), document.column_count());
        let Some(cursor) = imp.current.get().filter(|_| rows > 0 && columns > 0) else {
            imp.current.set(None);
            return;
        };

        // The position is where the row sits in the view, and the view is
        // rebuilt after this rather than before it. Going to a position that
        // has gone is already declined, so it is left as it is.
        imp.current.set(Some(Cursor {
            row: cursor.row.min(rows - 1),
            column: cursor.column.min(columns - 1),
            ..cursor
        }));
    }

    /// Ties the row numbers to the table: one vertical adjustment moves both,
    /// rather than one of them following the other a frame later.
    ///
    /// That holds only while the two are the same height, so the table's
    /// horizontal scrollbar is outside it and under both. Left inside, it takes
    /// a strip off the bottom of the table that the gutter does not lose, and
    /// the two disagree about how much of the file a screen holds. A scrollbar
    /// of our own does not hide itself when there is nothing to scroll, which is
    /// the one thing it costs.
    fn share_scrolling(&self) {
        let imp = self.imp();
        imp.gutter_scroller
            .set_vadjustment(Some(&imp.table.vadjustment()));

        let across = imp.table.hadjustment();
        imp.across.set_adjustment(Some(&across));
        across.connect_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.show_across()
        ));
        self.show_across();
    }

    /// Shows the horizontal scrollbar while the table is wider than the window.
    fn show_across(&self) {
        let across = self.imp().table.hadjustment();
        self.imp()
            .across
            .set_visible(across.upper() > across.page_size());
    }

    /// Draws as much of the grid again as a change moved, and no more.
    fn apply(&self, extent: Extent) {
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
            Extent::Columns { at, inserted } => {
                // The conditions are held by column number and a column has
                // just moved, so they follow it before anything reads them
                // again. Redrawing the grid is what reads them again.
                self.columns_moved(at, inserted);
                self.reload();
            }
            Extent::Rows { at, gone, come } => self.reload_rows(at, gone, come),
            Extent::Shape => self.reload(),
        }
    }

    /// Redraws a splice of rows where it is, which keeps every row above it
    /// drawn and the grid where it was scrolled to.
    ///
    /// One thing a row change can do reaches further than the splice and falls
    /// back to drawing the grid again: while the header is on, the first record
    /// is the column titles. Growing the row numbers used to be the other, and
    /// is not any more, because they are a view of their own to be widened
    /// where they stand.
    fn reload_rows(&self, at: usize, gone: usize, come: usize) {
        let imp = self.imp();
        let Some(document) = imp.rows.document() else {
            return;
        };

        if imp.rows.header() && at == 0 {
            return self.reload();
        }

        imp.gutter
            .set_digits(grid::gutter_digits(document.borrow().row_count()));
        imp.rows.rows_changed(at, gone, come);
        // The rows below the splice keep their widgets, which is what keeps the
        // grid where it was scrolled to. What they cannot keep is the number
        // beside them: that counts from the top of the file, and the file has
        // changed above them.
        imp.gutter.renumber();

        self.show_state();
    }

    /// Draws the whole grid again from the document, for changes that moved
    /// columns or that moved rows too far to describe.
    fn reload(&self) {
        self.imp().rows.reload();
        self.rebuild_columns();
        // A chip names its column, and a reload is the one moment that name can
        // have changed underneath it.
        self.show_filters();
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
        imp.gutter
            .set_digits(grid::gutter_digits(document.row_count()));

        // Rebuilding the columns throws away the sorters with them, and with
        // those the arrow saying which column the grid is sorted by.
        let sorted_by = self.sorted_by();

        // The view holds the model both it and the gutter read, which is what a
        // cell asks which record it is showing.
        let Some(model) = imp.column_view.model() else {
            return;
        };
        grid::set_columns(
            &imp.column_view,
            &titles,
            &grid::Records::new(&model),
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |edit| window.cell_edited(edit)
            ),
        );

        // Each heading offers what can be done to its own column. Left button
        // sorts, as it did; the other one asks.
        let columns = imp.column_view.columns();
        for index in 0..titles.len() {
            if let Some(column) = columns
                .item(index as u32)
                .and_downcast::<gtk::ColumnViewColumn>()
            {
                column.set_header_menu(Some(&column_menu(index)));
            }
        }

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
                    true => xaddr::col_to_letter(column),
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

        self.apply(extent);
    }

    /// Adds or removes a row or a column, at the one the menu named or at the
    /// cursor when it named none.
    fn change_shape(&self, operation: &Operation, at: i32) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };

        // The action stays reachable from the session bus whatever the menus
        // are showing, so what the menus refuse is refused here too.
        if operation.needs_file_order && !self.showing_file_order() {
            return;
        }

        let index = match (usize::try_from(at), self.current_cell()) {
            (Ok(index), _) => index,
            (Err(_), Some((row, column))) => match operation.axis {
                Axis::Row => row,
                Axis::Column => column,
            },
            // Nothing to point at, so the operation happens where the file
            // begins.
            (Err(_), None) if !operation.needs_cell => 0,
            (Err(_), None) => return,
        };

        // A menu names something that was there when it opened. Taking away
        // what is no longer there is nothing at all.
        let reach = match operation.axis {
            Axis::Row => document.borrow().row_count(),
            Axis::Column => document.borrow().column_count(),
        };
        if operation.needs_cell && index >= reach {
            return;
        }

        // Read before the change, because redrawing the grid can move the focus
        // and the cursor follows the focus.
        let was = self.imp().current.get();
        let extent = (operation.change)(&mut document.borrow_mut(), index);
        self.apply(extent);
        self.follow_change(was);
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
        // Reading it again needs a file to read, which a document typed into a
        // window that never opened one does not have.
        self.set_action_enabled("reload", self.imp().file.borrow().is_some());
        for format in Format::ALL {
            self.set_action_enabled(format.action(), open);
        }
        self.set_action_enabled("undo", undo);
        self.set_action_enabled("redo", redo);
        let searching = !self.imp().needle.borrow().is_empty();
        self.set_action_enabled("replace-all", searching);
        // An order cannot be written down from a view that is not showing every
        // row it would put in order, and either a search or a filter can be why
        // it is not.
        let hiding = self.hiding_rows();
        self.set_action_enabled("commit-order", self.sorted_by().is_some() && !hiding);

        for name in ["filter-column", "clear-filter"] {
            self.set_action_enabled(name, open);
        }
        self.show_the_way_out();
        self.set_action_enabled("clear-filters", !self.imp().filters.borrow().is_empty());

        self.show_reach();
    }

    /// Which of the ways out of the window have somewhere to go.
    ///
    /// Each asks for what it actually needs rather than for a file in general.
    /// Showing the folder goes through the portal and works wherever the file
    /// came from; a shell has to stand somewhere on this computer; and a tool
    /// has to be able to read the file it is handed, which is not true of every
    /// tool for every way of writing one.
    fn show_the_way_out(&self) {
        let file = self.imp().file.borrow().clone();
        self.set_action_enabled("open-containing-folder", file.is_some());
        self.set_action_enabled("open-terminal", self.folder_path().is_some());

        let readable = file.as_ref().and_then(gio::File::path).is_some();
        let dialect = self
            .imp()
            .rows
            .document()
            .map(|document| document.borrow().dialect());
        for tool in Tool::ALL {
            let reads = dialect.is_some_and(|dialect| tool.reads(dialect));
            self.set_action_enabled(tool.action(), readable && reads);
        }
    }

    /// Which row and column operations have somewhere to happen.
    ///
    /// Read again every time the cursor moves rather than only when the file
    /// changes, because what is reachable follows where you are.
    fn show_reach(&self) {
        let cell = self.current_cell().is_some();
        // Holding a column to the value in front of you needs a value in front
        // of you, and so does opening one to type in.
        self.set_action_enabled("filter-to-value", cell);
        self.set_action_enabled("edit", cell);
        let document = self.imp().rows.document().is_some();
        let file_order = self.showing_file_order();
        for operation in STRUCTURE {
            let somewhere = if operation.needs_cell { cell } else { document };
            let meaningful = file_order || !operation.needs_file_order;
            self.set_action_enabled(operation.name, somewhere && meaningful);
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

    /// Says that something did not work, in a dialog titled by what Comma was
    /// trying to do at the time.
    fn report(&self, task: Task, message: &str) {
        let dialog = adw::AlertDialog::new(Some(&task.failed()), Some(message));
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(self));
    }
}
