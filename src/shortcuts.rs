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

use crate::grid::LINE_BREAK;
use crate::translatable;
use crate::window::{CommaWindow, key_for_move};

/// Where a row's key comes from.
enum Key {
    /// The accelerator bound to an action.
    Accel(&'static str),
    /// The key that asks the table for one of its moves.
    Move(&'static str),
    /// An accelerator a widget answers to itself, rather than one bound to an
    /// action or to a move.
    Handled(&'static str),
}

/// What the list says, in the order it says it. The descriptions are written
/// here; the keys are not.
const GROUPS: [(&str, &[(&str, Key)]); 4] = [
    (
        translatable("File"),
        &[
            (translatable("Open a File"), Key::Accel("win.open")),
            (translatable("Save"), Key::Accel("win.save")),
            (translatable("Save As"), Key::Accel("win.save-as")),
            (translatable("Quit"), Key::Accel("app.quit")),
        ],
    ),
    (
        translatable("Editing"),
        &[
            (translatable("Edit the Cell"), Key::Move("edit")),
            (
                translatable("Insert a Line Break"),
                Key::Handled(LINE_BREAK[0]),
            ),
            (translatable("Undo"), Key::Accel("win.undo")),
            (translatable("Redo"), Key::Accel("win.redo")),
            (translatable("Find and Replace"), Key::Accel("win.find")),
        ],
    ),
    (
        translatable("What the Grid Is Showing"),
        &[(
            translatable("Filter This Column"),
            Key::Accel("win.filter-column(int32 -1)"),
        )],
    ),
    (
        translatable("Moving Around the Table"),
        &[
            (translatable("One Cell Up"), Key::Move("up")),
            (translatable("One Cell Down"), Key::Move("down")),
            (translatable("One Cell Left"), Key::Move("left")),
            (translatable("One Cell Right"), Key::Move("right")),
            (translatable("Start of the Row"), Key::Move("row-start")),
            (translatable("End of the Row"), Key::Move("row-end")),
            (translatable("Start of the File"), Key::Move("start")),
            (translatable("End of the File"), Key::Move("end")),
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
        Key::Handled(accelerator) => accelerator.to_string(),
    };

    let (key, modifiers) = gtk::accelerator_parse(&bound)?;
    Some(gtk::accelerator_get_label(key, modifiers).to_string())
}
