// The file as it is on disk, and what to do when that stops being what is on
// screen.
//
// Comma is not the only thing that writes these files. A command-line tool, a
// script, or another window can rewrite one while it is open here, and until
// this existed the grid went on showing what the file used to say and Save
// wrote that back over the top. Nothing said so, and nothing could be undone.
//
// What is shown depends on whether there is anything to lose. With no unsaved
// edits the file is simply read again and a toast says it happened; with edits
// there is a decision to make and only the user can make it, so the banner
// stays up until they do. Comma writes a file whole, so there is no third
// answer where both sets of changes survive.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use comma::document::Document;

use super::CommaWindow;
use super::files::Task;

/// How long to wait for a file to stop being written before looking at it.
///
/// One save arrives as several events, and a program that writes to a temporary
/// file and renames it over the top arrives as a deletion followed by a
/// creation. Looking at any one of those says something that is about to stop
/// being true.
const SETTLE: Duration = Duration::from_millis(250);

/// What the file on disk is doing that the document is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adrift {
    /// Somebody else has written to it since Comma read it.
    Changed,
    /// It is not there any more.
    Gone,
}

impl Adrift {
    fn banner(self) -> String {
        match self {
            Self::Changed => gettext("This file has changed on disk."),
            Self::Gone => gettext("This file is no longer on disk."),
        }
    }

    /// The button beside it, where there is something one press can do about
    /// it. A file that is gone cannot be read again.
    fn offer(self) -> Option<String> {
        match self {
            Self::Changed => Some(gettext("_Reload")),
            Self::Gone => None,
        }
    }
}

impl CommaWindow {
    /// Watches the file the document was read from, so that a change made
    /// somewhere else does not go unnoticed.
    ///
    /// One monitor at a time, replaced whenever the window changes which file
    /// it is about. Dropping the old one is what cancels it.
    pub(super) fn watch_file(&self) {
        let imp = self.imp();
        imp.monitor.replace(None);
        self.show_adrift(None);

        let Some(file) = imp.file.borrow().clone() else {
            return;
        };
        // Moves as well as writes: a file replaced by a rename is the ordinary
        // way for a careful program to write one.
        let Ok(monitor) =
            file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        else {
            // Watching is what this does for the user, not what it does for the
            // document. A file that cannot be watched is still editable, and the
            // guard on Save asks the same question again at the moment it counts.
            return;
        };

        monitor.connect_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _, _, _| window.file_stirred()
        ));
        imp.monitor.replace(Some(monitor));
    }

    /// Something happened to the file. Waits for whatever it is to finish
    /// before asking what it was, and asks once however many events arrive.
    fn file_stirred(&self) {
        if self.imp().settling.replace(true) {
            return;
        }

        glib::timeout_add_local_once(
            SETTLE,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move || window.look_at_the_file()
            ),
        );
    }

    /// Whether what is on disk is still what Comma read, and what to do if it
    /// is not.
    fn look_at_the_file(&self) {
        let imp = self.imp();
        imp.settling.set(false);

        let Some(file) = imp.file.borrow().clone() else {
            return;
        };

        match stamp(&file) {
            // Gone, or no longer readable, which from here is the same thing:
            // what is on screen is the only copy left.
            None => {
                if let Some(document) = imp.rows.document() {
                    // Modified means differing from the file, and a document
                    // whose file has gone differs from it as far as anything
                    // can. Saying so is also what puts Save back within reach,
                    // which is how the file gets written again.
                    document.borrow_mut().mark_modified();
                    self.show_state();
                }
                self.show_adrift(Some(Adrift::Gone));
            }
            // Comma's own last write, or a touch that changed nothing.
            Some(stamp) if Some(&stamp) == imp.stamp.borrow().as_ref() => self.show_adrift(None),
            Some(_) => self.file_changed(),
        }
    }

    /// The file says something else now. Whether that can simply be shown
    /// depends on whether anything would be lost by showing it.
    fn file_changed(&self) {
        // Reading it again under someone's hands would take the cell they are
        // typing in out from under them. The banner waits, and they can ask for
        // it when they have finished.
        if self.is_modified() || self.typing() {
            return self.show_adrift(Some(Adrift::Changed));
        }

        let Some(spoken_for) = self.read_the_file_again() else {
            return;
        };
        self.show_adrift(None);
        if !spoken_for {
            self.say(&gettext("Reloaded. The file had changed on disk."));
        }
    }

    /// Reads the file again and puts it in front of the user, keeping what they
    /// had asked to see of it.
    ///
    /// Nothing if it could not be read. Otherwise whether showing it had
    /// something to say for itself, which is how one write avoids being
    /// announced twice.
    ///
    /// Under the delimiter in force rather than by guessing again: the guess
    /// was made once and may since have been corrected, and a file does not
    /// change what it is separated by between one write and the next.
    pub(super) fn read_the_file_again(&self) -> Option<bool> {
        let imp = self.imp();
        let file = imp.file.borrow().clone()?;
        let dialect = imp
            .rows
            .document()
            .map(|document| document.borrow().dialect())?;

        let (bytes, stamp) = match file.load_contents(gio::Cancellable::NONE) {
            Ok((bytes, _etag)) => (bytes, stamp(&file)),
            Err(error) => {
                self.report(Task::Open, &error.to_string());
                return None;
            }
        };
        let document = match Document::from_bytes(&bytes, dialect) {
            Ok(document) => document,
            Err(error) => {
                self.report(Task::Open, &error.to_string());
                return None;
            }
        };

        imp.stamp.replace(stamp);
        Some(self.show_again(document))
    }

    /// What the file on disk is doing, said once and left up until it stops
    /// being true. Nothing said at all is the ordinary case.
    pub(super) fn show_adrift(&self, adrift: Option<Adrift>) {
        let imp = self.imp();
        if imp.adrift.replace(adrift) == adrift {
            return;
        }

        let Some(adrift) = adrift else {
            imp.banner.set_revealed(false);
            return;
        };

        imp.banner.set_title(&adrift.banner());
        imp.banner.set_button_label(adrift.offer().as_deref());
        imp.banner.set_revealed(true);
    }

    /// Says something that has already happened, which is a different kind of
    /// thing from the banner: it wants no answer and goes away on its own.
    pub(super) fn say(&self, message: &str) {
        self.imp().toasts.add_toast(adw::Toast::new(message));
    }

    /// Whether the file has been written to since Comma last read or wrote it,
    /// which is the question Save has to ask before it writes over the answer.
    pub(super) fn changed_underneath(&self, file: &gio::File) -> bool {
        let imp = self.imp();
        // Only about the file this document came from. Saving somewhere else is
        // a different question, and the file dialog has already asked it.
        if imp.file.borrow().as_ref() != Some(file) {
            return false;
        }

        match (stamp(file), imp.stamp.borrow().as_ref()) {
            // Never read from a file, so there is nothing of anyone else's here
            // to write over.
            (_, None) => false,
            (Some(now), Some(ours)) => now != *ours,
            // It is gone. Writing it puts it back rather than replacing
            // somebody's work.
            (None, Some(_)) => false,
        }
    }

    /// Remembers the file as it stands, as the thing every later look at it is
    /// compared against.
    pub(super) fn note_the_file(&self) {
        let stamp = self.imp().file.borrow().as_ref().and_then(stamp);
        self.imp().stamp.replace(stamp);
    }
}

/// What a file looks like from outside, closely enough to tell one write from
/// another: what the filesystem last recorded about it, and how big it is.
///
/// The etag is the first of those and the one that matters, because it is what
/// changes when contents do. Size comes along because a file rewritten within
/// the same clock tick can carry the etag it had.
fn stamp(file: &gio::File) -> Option<(glib::GString, u64)> {
    let info = file
        .query_info(
            "etag::value,standard::size",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .ok()?;
    Some((info.etag()?, info.size() as u64))
}
