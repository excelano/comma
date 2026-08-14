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

use comma::document::{Dialect, Document, sniff, sniff_header};
use comma::export::{self, Sheet};

use crate::pdf;
use crate::translatable;

use super::CommaWindow;

/// What Comma can write that it will not read back. Each is output: never
/// reopened, never offered as Save, and never the document.
///
/// Everything about an export is here: the action that asks for it, what the
/// Export submenu calls it, the extension it is offered under, and what the
/// file chooser says it is. A format left half-declared used to be a menu entry
/// that was never enabled and said nothing about why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Format {
    Pdf,
    Html,
    Ods,
}

impl Format {
    /// The exports, in the order the menu offers them.
    pub(super) const ALL: [Format; 3] = [Self::Pdf, Self::Html, Self::Ods];

    pub(super) fn action(self) -> &'static str {
        match self {
            Self::Pdf => "export-pdf",
            Self::Html => "export-html",
            Self::Ods => "export-ods",
        }
    }

    /// What the Export submenu calls it. Said as what the file will be rather
    /// than as its extension, because the submenu has already said Export.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pdf => translatable("As PDF…"),
            Self::Html => translatable("As Web Page…"),
            Self::Ods => translatable("As Spreadsheet…"),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Ods => "ods",
        }
    }

    /// What the file chooser calls it, which is the name of the format rather
    /// than of the operation.
    fn description(self) -> String {
        match self {
            Self::Pdf => gettext("PDF Document"),
            Self::Html => gettext("Web Page"),
            Self::Ods => gettext("OpenDocument Spreadsheet"),
        }
    }
}

/// What Comma was in the middle of when something went wrong, which is what the
/// dialog about it is titled by.
///
/// The title is here rather than at each place that reports one, because the
/// same failure is reported from more than one place — opening from three —
/// and a title reworded in one of them and not the others is a window that
/// says two things about one failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Task {
    Open,
    Save,
    Export,
}

impl Task {
    pub(super) fn failed(self) -> String {
        match self {
            Self::Open => gettext("Could Not Open the File"),
            Self::Save => gettext("Could Not Save the File"),
            Self::Export => gettext("Could Not Export the File"),
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
                Err(error) => window.report(Task::Open, &error.to_string()),
            },
        );
    }

    pub fn open_file(&self, file: &gio::File) {
        let bytes = match file.load_contents(gio::Cancellable::NONE) {
            Ok((bytes, _etag)) => bytes,
            Err(error) => {
                return self.report(Task::Open, &error.to_string());
            }
        };

        // Nothing on screen changes until the file has been read, so a file
        // that will not open leaves the one that did alone.
        let dialect = sniff(&bytes);
        let document = match Document::from_bytes(&bytes, dialect) {
            Ok(document) => document,
            Err(error) => {
                return self.report(Task::Open, &error.to_string());
            }
        };

        // A new file carries no opinions over from the last one. What its first
        // record is gets guessed from the file itself, the same as the
        // delimiter was.
        self.guess_header(&bytes, dialect);

        self.imp().file.replace(Some(file.clone()));
        self.note_the_file();
        self.remember_file(file);
        self.watch_file();
        self.show(document);
        self.show_folder(file);
    }

    /// Puts a guess about the first record in front of the user, and leaves it
    /// standing as a guess.
    ///
    /// Marking it matters, because the delimiter can still change. A guess made
    /// under one delimiter was made about columns another delimiter does not
    /// find, so it is made again; an answer the user gave was about this file
    /// and is not second-guessed.
    pub(super) fn guess_header(&self, bytes: &[u8], dialect: Dialect) {
        let header = sniff_header(bytes, dialect);
        self.imp().rows.set_header(header);
        self.set_action_state("header", &header.to_variant());
        self.imp().header_is_a_guess.set(true);
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

    /// Saves, unless somebody else has written to the file since Comma read
    /// it, in which case it asks first. Comma writes a file whole, so there is
    /// no version of this where both sets of changes survive, and the choice is
    /// the user's to make rather than ours to make quietly.
    fn save_to(&self, file: &gio::File) -> bool {
        if self.changed_underneath(file) {
            self.ask_before_overwriting(file);
            return false;
        }
        self.write_to(file)
    }

    fn write_to(&self, file: &gio::File) -> bool {
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
            self.report(Task::Save, &error.to_string());
            return false;
        }

        document.borrow_mut().mark_saved();
        self.imp().file.replace(Some(file.clone()));
        // What is on disk is Comma's own writing now, and it is this that the
        // next write by anybody else will be told apart from. Save As leaves
        // the old file behind as well, so what is watched moves with it.
        self.note_the_file();
        // Under this name now, whether that is the name it came in under or a
        // new one Save As gave it.
        self.remember_file(file);
        self.watch_file();
        self.show_folder(file);
        self.show_state();
        true
    }

    /// Asks before writing over somebody else's work, and offers the way out
    /// that keeps both: write this one somewhere else.
    fn ask_before_overwriting(&self, file: &gio::File) {
        let message = gettext(
            "“{}” has been written to since Comma read it. Saving replaces what was written there.",
        )
        .replace("{}", &display_name(file));
        let dialog = adw::AlertDialog::new(
            Some(&gettext("This File Has Changed on Disk")),
            Some(&message),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("save-as", &gettext("Save _As…"));
        dialog.add_response("overwrite", &gettext("_Overwrite"));
        dialog.set_response_appearance("overwrite", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let window = self.clone();
        let file = file.clone();
        dialog.choose(
            Some(self),
            gio::Cancellable::NONE,
            move |response| match response.as_str() {
                "overwrite" => {
                    window.write_to(&file);
                }
                "save-as" => window.save_as(),
                _ => {}
            },
        );
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
                Err(error) => window.report(Task::Save, &error.to_string()),
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
                Err(error) => window.report(Task::Export, &error.to_string()),
            },
        );
    }

    fn export_to(&self, file: &gio::File, format: Format) {
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
            self.report(Task::Export, &error);
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
///
/// A file reached over the network has no path on this computer, so what is
/// said about it is where GIO would say it is. Saying nothing was the other
/// answer, and it left a file opened from a share looking like one with no
/// folder at all.
pub(super) fn folder_of(file: &gio::File) -> String {
    let Some(parent) = file.parent() else {
        return String::new();
    };
    let Some(folder) = parent.path() else {
        return parent.parse_name().to_string();
    };

    match folder.strip_prefix(glib::home_dir()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => folder.display().to_string(),
    }
}
