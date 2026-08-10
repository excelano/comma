// The main window: one document, one grid.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use comma::document::{Dialect, Document, sniff};

use crate::application::CommaApplication;
use crate::config::APP_ID;
use crate::grid::{self, RowModel};

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
        pub dialect_button: TemplateChild<gtk::MenuButton>,
        pub rows: RowModel,
        /// The file as it was read. Kept so that choosing a different delimiter
        /// re-reads the same bytes rather than the file as it is now.
        pub bytes: RefCell<Vec<u8>>,
        pub settings: gio::Settings,
    }

    impl Default for CommaWindow {
        fn default() -> Self {
            Self {
                window_title: TemplateChild::default(),
                stack: TemplateChild::default(),
                column_view: TemplateChild::default(),
                dialect_button: TemplateChild::default(),
                rows: RowModel::default(),
                bytes: RefCell::default(),
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
            self.column_view
                .set_model(Some(&gtk::NoSelection::new(Some(self.rows.clone()))));

            self.dialect_button.set_menu_model(Some(&reading_menu()));
        }
    }

    impl WidgetImpl for CommaWindow {}
    impl WindowImpl for CommaWindow {}
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

        self.add_action_entries([open, delimiter, header]);
    }

    fn set_action_state(&self, name: &str, state: &glib::Variant) {
        if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_state(state);
        }
    }

    fn choose_file(&self) {
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
                Err(error) => window.report_failure(&error.to_string()),
            },
        );
    }

    pub fn open_file(&self, file: &gio::File) {
        let bytes = match file.load_contents(gio::Cancellable::NONE) {
            Ok((bytes, _etag)) => bytes,
            Err(error) => return self.report_failure(&error.to_string()),
        };

        // Nothing on screen changes until the file has been read, so a file
        // that will not open leaves the one that did alone.
        let document = match Document::from_bytes(&bytes, sniff(&bytes)) {
            Ok(document) => document,
            Err(error) => return self.report_failure(&error.to_string()),
        };

        // A new file carries no opinions over from the last one. Its first row
        // is data until this file's own user says otherwise.
        self.imp().rows.set_header(false);
        self.set_action_state("header", &false.to_variant());

        self.imp().bytes.replace(bytes.to_vec());
        self.show(document);
        self.show_file_name(file);
    }

    /// Reads the file again under a different delimiter. The bytes have already
    /// been read once, so the only thing that can differ is where the fields
    /// are.
    fn read_again_as(&self, dialect: Dialect) {
        let imp = self.imp();
        if imp.rows.document().is_none_or(|document| {
            let current = document.borrow().dialect();
            current == dialect
        }) {
            return;
        }

        let document = Document::from_bytes(&imp.bytes.borrow(), dialect)
            .expect("these bytes were read once already");
        self.show(document);
    }

    fn show(&self, document: Document) {
        let imp = self.imp();

        imp.rows.set_document(document);
        self.rebuild_columns();
        self.show_dialect();

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

        let header = imp.rows.header() && document.row_count() > 0;
        let titles: Vec<String> = (0..document.column_count())
            .map(|column| {
                let title = if header {
                    document.value(0, column)
                } else {
                    ""
                };
                if title.is_empty() {
                    grid::column_letter(column)
                } else {
                    title.to_owned()
                }
            })
            .collect();

        grid::set_columns(&imp.column_view, &titles, document.row_count());
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

    fn show_file_name(&self, file: &gio::File) {
        let imp = self.imp();
        let name = display_name(file);

        imp.window_title.set_title(&name);
        imp.window_title.set_subtitle(&folder_of(file));
        self.set_title(Some(&name));
    }

    fn report_failure(&self, message: &str) {
        let dialog =
            adw::AlertDialog::new(Some(&gettext("Could Not Open the File")), Some(message));
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(self));
    }
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

fn preset_label(id: &str) -> String {
    match id {
        "comma" => gettext("Comma"),
        "tab" => gettext("Tab"),
        "semicolon" => gettext("Semicolon"),
        "pipe" => gettext("Pipe"),
        "unit-separator" => gettext("ASCII Separators"),
        other => other.to_string(),
    }
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
