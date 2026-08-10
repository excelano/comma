// Comma — an editor for delimited text files.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

mod application;
mod config;
mod grid;
mod pdf;
mod shortcuts;
mod window;

use gtk::gio;
use gtk::glib;
use gtk::prelude::ApplicationExtManual;

use crate::application::CommaApplication;
use crate::config::{GETTEXT_PACKAGE, LOCALEDIR, PKGDATADIR};

/// Marks a string as one to translate without translating it here.
///
/// Some strings are written down long before there is a window to show them in,
/// in tables that are built once and read many times, and a translation has to
/// be looked up in the language in force at the moment it is read. This is what
/// lets the extractor see them all the same.
pub const fn translatable(text: &'static str) -> &'static str {
    text
}

fn main() -> glib::ExitCode {
    // SAFETY: setlocale mutates process-global state and is not thread-safe.
    // This is the first statement of main, before GTK or any thread starts.
    unsafe { gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "") };
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("could not bind text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("could not switch to text domain");

    let resources = gio::Resource::load(format!("{PKGDATADIR}/comma.gresource"))
        .expect("could not load the compiled resource bundle");
    gio::resources_register(&resources);

    CommaApplication::new().run()
}
