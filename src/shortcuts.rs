// The list of what the keyboard does.
//
// Nothing here spells a key out. Each row asks either the application what
// accelerator it bound to an action, or the table what key asks for one of its
// moves — so the list cannot come to disagree with the keys themselves, which is
// the usual fate of a page like this.
//
// Built by hand rather than with GtkShortcutsWindow, which is deprecated, and
// rather than with AdwShortcutsDialog, which arrived after the libadwaita this
// builds against.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use gettextrs::gettext;

use crate::window::{CommaWindow, key_for_move};

/// Where a row's key comes from.
enum Key {
    /// The accelerator bound to an action.
    Accel(&'static str),
    /// The key that asks the table for one of its moves.
    Move(&'static str),
}

/// What the list says, in the order it says it. The descriptions are written
/// here; the keys are not.
const GROUPS: [(&str, &[(&str, Key)]); 3] = [
    (
        "File",
        &[
            ("Open a File", Key::Accel("win.open")),
            ("Save", Key::Accel("win.save")),
            ("Save As", Key::Accel("win.save-as")),
            ("Quit", Key::Accel("app.quit")),
        ],
    ),
    (
        "Editing",
        &[
            ("Edit the Cell", Key::Move("edit")),
            ("Undo", Key::Accel("win.undo")),
            ("Redo", Key::Accel("win.redo")),
            ("Find and Replace", Key::Accel("win.find")),
        ],
    ),
    (
        "Moving Around the Table",
        &[
            ("One Cell Up", Key::Move("up")),
            ("One Cell Down", Key::Move("down")),
            ("One Cell Left", Key::Move("left")),
            ("One Cell Right", Key::Move("right")),
            ("Start of the Row", Key::Move("row-start")),
            ("End of the Row", Key::Move("row-end")),
            ("Start of the File", Key::Move("start")),
            ("End of the File", Key::Move("end")),
        ],
    ),
];

pub fn present(window: &CommaWindow) {
    let page = adw::PreferencesPage::new();

    for (title, rows) in GROUPS {
        let group = adw::PreferencesGroup::builder()
            .title(gettext(title))
            .build();

        for (description, key) in rows {
            let Some(accelerator) = accelerator(window, key) else {
                continue;
            };

            let row = adw::ActionRow::builder()
                .title(gettext(*description))
                .build();
            row.add_suffix(
                &gtk::Label::builder()
                    .label(&accelerator)
                    .css_classes(["dim-label"])
                    .build(),
            );
            group.add(&row);
        }

        page.add(&group);
    }

    let dialog = adw::PreferencesDialog::builder()
        .title(gettext("Keyboard Shortcuts"))
        .build();
    dialog.add(&page);
    dialog.present(Some(window));
}

/// The key a row shows, written the way this desktop writes keys: Ctrl rather
/// than `<primary>`, and whatever the keyboard's own layout calls the rest.
fn accelerator(window: &CommaWindow, key: &Key) -> Option<String> {
    let bound = match key {
        Key::Accel(action) => {
            let application = window.application()?.downcast::<gtk::Application>().ok()?;
            application.accels_for_action(action).first()?.to_string()
        }
        Key::Move(how) => key_for_move(how)?.to_string(),
    };

    let (key, modifiers) = gtk::accelerator_parse(&bound)?;
    Some(gtk::accelerator_get_label(key, modifiers).to_string())
}
