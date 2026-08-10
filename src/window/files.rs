// Where the document comes from and where it goes: opening, saving, and the
// exports, which are written and never read back.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use comma::document::{Document, sniff};
use comma::export::{self, Sheet};

use crate::pdf;

use super::CommaWindow;

/// The exports, so there is one list of what needs a document open to be worth
/// offering.
pub(super) const EXPORTS: [&str; 3] = ["export-pdf", "export-html", "export-ods"];

/// What Comma can write that it will not read back. Each is output: never
/// reopened, never offered as Save, and never the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Format {
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

impl CommaWindow {
    pub(super) fn choose_file(&self) {
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

    /// Writes the document back to the file it came from, and says whether it
    /// managed to. A document with no file of its own has to be asked where to
    /// go, and that answer arrives too late to report here.
    pub(super) fn save(&self) -> bool {
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

    pub(super) fn save_as(&self) {
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

    pub(super) fn is_modified(&self) -> bool {
        self.imp()
            .rows
            .document()
            .is_some_and(|document| document.borrow().is_modified())
    }

    /// Asks before anything unsaved is thrown away, then does `next`. With
    /// nothing to lose there is nothing to ask, and `next` happens straight
    /// away.
    pub(super) fn confirm_discard(&self, next: impl Fn(&Self) + 'static) {
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

    /// Writes what the grid is showing somewhere else, in a format Comma cannot
    /// read back.
    ///
    /// Deliberately not Save, and deliberately somewhere the user has to name:
    /// there is never a moment where the file being edited has quietly become a
    /// PDF.
    pub(super) fn export(&self, format: Format) {
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

    fn show_folder(&self, file: &gio::File) {
        self.imp().window_title.set_subtitle(&folder_of(file));
    }

    pub(super) fn document_name(&self) -> String {
        match self.imp().file.borrow().as_ref() {
            Some(file) => display_name(file),
            None => gettext("Untitled"),
        }
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
