// The files that have been open here before, and the way back to one.
//
// Two halves that meet at the desktop's own list of recent documents. Comma
// writes to it, so a file opened here turns up in the Files sidebar and the
// dash the way one opened in any other application does; and Comma reads it
// back, so the way to a file worked on yesterday is the arrow beside Open
// rather than a walk through the file chooser to find it by name again.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use super::CommaWindow;
use super::files::folder_of;

/// What Comma calls itself in the recent documents, and the command the desktop
/// relaunches it by. The name is also what tells Comma's own entries from
/// everybody else's when the list is read back.
const APP_NAME: &str = "Comma";
const APP_EXEC: &str = "comma %u";

/// The kinds of file Comma opens, as the desktop names them.
///
/// A file of one of these is worth offering back whichever application put it
/// in the list, since Comma can read it. Anything else is offered back only
/// when Comma is what opened it — which is what makes a `.dsv` registered under
/// its true type still findable here without every text file on the machine
/// coming with it.
const MIME_TYPES: [&str; 2] = ["text/csv", "text/tab-separated-values"];

/// How many to offer.
///
/// This is the way back to what you have been working on, not a second file
/// chooser. Past twenty it stops being the first and is not good at the second.
const MOST: usize = 20;

/// One entry as the popover shows it: what it is called, where it is, and the
/// URI that is opened when it is clicked.
///
/// The words are kept here rather than read back off the row they were put in,
/// because a row's title is read as markup and what goes into it is escaped. A
/// file called "Q1 & Q2.csv" is searched for by its name and not by its
/// spelling in markup.
#[derive(Debug)]
pub struct Recent {
    uri: String,
    name: String,
    folder: String,
}

impl CommaWindow {
    /// Puts the file in the desktop's recent documents, where the files every
    /// other application opens go.
    ///
    /// Called for a file opened and for a file written, so that Save As puts
    /// the new name in the list and working in a file keeps it at the top of
    /// it. Exports are not files Comma will ever read back, so they are not
    /// here.
    pub(super) fn remember_file(&self, file: &gio::File) {
        // From the name alone, so that the same file is always filed under the
        // same type. The names the desktop knows are the two it knows; the rest
        // come back as a binary blob, which is the one thing a file Comma can
        // open is not, so those are filed as what they are instead.
        let (mime, uncertain) = gio::content_type_guess(file.basename(), None);
        let mime = if uncertain { "text/plain" } else { &mime };

        let data = gtk::RecentData::new(None, None, mime, APP_NAME, APP_EXEC, &[], false);
        // Nothing depends on this having worked. A desktop with no recent
        // documents store is one where the list is empty, not one where opening
        // a file failed.
        gtk::RecentManager::default().add_full(&file.uri(), &data);
    }

    /// Wires up the popover under the arrow beside Open.
    pub(super) fn setup_recents(&self) {
        let imp = self.imp();

        // Built when the popover opens rather than kept up to date. What is in
        // the list changes in other windows and in other applications, and this
        // is the only moment it is looked at.
        imp.recents.connect_show(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.show_recents()
        ));

        imp.recent_search.connect_search_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_| window.imp().recent_list.invalidate_filter()
        ));

        imp.recent_list.set_filter_func(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            true,
            move |row| window.recent_matches(row)
        ));

        imp.recent_list.connect_row_activated(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, row| window.open_recent(row.index())
        ));
    }

    /// Fills the popover with what there is to go back to.
    ///
    /// The search is cleared and the list replaced before a row is built,
    /// because every row appended is put through the filter as it goes and the
    /// filter reads both. Left to the end, the first list a popover showed
    /// would be filtered by the last search typed into it.
    fn show_recents(&self) {
        let imp = self.imp();
        imp.recent_search.set_text("");
        imp.recent_list.remove_all();
        imp.recents_shown.replace(self.recent_files());

        let recents = imp.recents_shown.borrow();
        for recent in recents.iter() {
            // A title is read as markup, and a file is free to be called
            // "Q1 & Q2.csv".
            imp.recent_list.append(
                &adw::ActionRow::builder()
                    .title(glib::markup_escape_text(&recent.name))
                    .subtitle(glib::markup_escape_text(&recent.folder))
                    .activatable(true)
                    .build(),
            );
        }

        // Two different nothings, and the difference is worth saying: one is a
        // list with nothing in it, the other a search that found none of what
        // is there.
        imp.recents_message.set_label(&if recents.is_empty() {
            gettext("No Recent Files")
        } else {
            gettext("No Results Found")
        });
        // Nothing to search through is nothing to search.
        imp.recent_search.set_visible(!recents.is_empty());
    }

    /// The files worth offering back, most recently used first.
    fn recent_files(&self) -> Vec<Recent> {
        let mut items: Vec<_> = gtk::RecentManager::default()
            .items()
            .into_iter()
            .filter(|info| {
                MIME_TYPES.contains(&info.mime_type().as_str()) || info.has_application(APP_NAME)
            })
            // A file that has been moved or deleted since is a row that would
            // fail when it was clicked. Only a local file can be looked for:
            // anything else is offered and found out about when it is opened.
            .filter(|info| !info.is_local() || info.exists())
            .collect();
        // What the store bumps when a file is registered again, by Comma or by
        // anything else. The visited time is written once when the entry is
        // made and left alone after that, so it says when a file was first seen
        // rather than how lately it was worked on.
        items.sort_by_key(|info| std::cmp::Reverse(info.modified().to_unix()));

        items
            .into_iter()
            .take(MOST)
            .map(|info| {
                let uri = info.uri().to_string();
                Recent {
                    name: info.display_name().to_string(),
                    folder: folder_of(&gio::File::for_uri(&uri)),
                    uri,
                }
            })
            .collect()
    }

    /// Whether a row is one of the ones being searched for. The name and the
    /// folder both count, because half of what makes a file the one you want is
    /// where it is.
    fn recent_matches(&self, row: &gtk::ListBoxRow) -> bool {
        let imp = self.imp();
        let needle = imp.recent_search.text().to_lowercase();
        if needle.is_empty() {
            return true;
        }

        let recents = imp.recents_shown.borrow();
        let Some(recent) = recents.get(row.index() as usize) else {
            return true;
        };
        recent.name.to_lowercase().contains(&needle)
            || recent.folder.to_lowercase().contains(&needle)
    }

    /// Opens the file a row stands for, in this window, asking about anything
    /// unsaved first — which is what the Open button itself does, since this is
    /// the same act with the walk through the chooser taken out.
    fn open_recent(&self, index: i32) {
        let uri = self
            .imp()
            .recents_shown
            .borrow()
            .get(index as usize)
            .map(|recent| recent.uri.clone());
        let Some(uri) = uri else {
            return;
        };

        // Down before the question is asked, or the dialog comes up behind it.
        self.imp().recents.popdown();

        let file = gio::File::for_uri(&uri);
        self.confirm_discard(move |window| window.open_file(&file));
    }
}
