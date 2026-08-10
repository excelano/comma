// The menus, and the delimiters they offer.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::gdk;
use gtk::gio;

use comma::document::Dialect;

use super::CommaWindow;

/// The delimiters Comma offers, in the order the menu lists them. Everything
/// about a delimiter — its menu entry, its button label, the state the action
/// carries — comes from here, so there is one place to add another.
pub(super) const PRESETS: [(&str, Dialect); 5] = [
    ("comma", Dialect::comma()),
    ("tab", Dialect::tab()),
    ("semicolon", Dialect::semicolon()),
    ("pipe", Dialect::pipe()),
    ("unit-separator", Dialect::unit_separator()),
];

impl CommaWindow {
    /// Puts the row and column operations under the pointer, which is where a
    /// table is usually asked about them.
    ///
    /// The menu is the same one the main menu holds, built once and moved to
    /// wherever it was asked for.
    pub(super) fn show_cell_menu(&self, x: f64, y: f64) {
        let imp = self.imp();
        let Some(cell) = self.focused_cell() else {
            return;
        };

        let menu = imp.cell_menu.get_or_init(|| {
            let menu = gtk::PopoverMenu::from_model(Some(&structure_menu()));
            menu.set_has_arrow(false);
            menu.set_halign(gtk::Align::Start);
            menu.set_parent(&*imp.column_view);
            menu
        });

        // The click came in the cell's own coordinates, and the menu hangs off
        // the table.
        let Some(at) = cell.compute_point(
            &*imp.column_view,
            &gtk::graphene::Point::new(x as f32, y as f32),
        ) else {
            return;
        };
        menu.set_pointing_to(Some(&gdk::Rectangle::new(
            at.x() as i32,
            at.y() as i32,
            1,
            1,
        )));
        menu.popup();
    }

    /// Puts the delimiter in front of the user rather than leaving it guessed
    /// at silently: on the button, and as the item ticked in its menu.
    pub(super) fn show_dialect(&self) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };
        let dialect = document.borrow().dialect();
        let id = preset_id(dialect).expect("every dialect Comma reads with is one of its presets");

        self.imp().dialect_button.set_label(&preset_label(id));
        self.set_action_state("delimiter", &id.to_variant());
    }
}

pub(super) fn preset(id: &str) -> Option<Dialect> {
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

/// What a delimiter is called on the button and in its menu.
///
/// Said in the context of delimiters, because in English the comma delimiter and
/// the application share a word and in other languages they will not.
fn preset_label(id: &str) -> String {
    // Written out one by one rather than through a helper: what marks these for
    // translation is the extractor seeing the words next to the context, and it
    // cannot follow them through anything.
    match id {
        "comma" => pgettext("delimiter", "Comma"),
        "tab" => pgettext("delimiter", "Tab"),
        "semicolon" => pgettext("delimiter", "Semicolon"),
        "pipe" => pgettext("delimiter", "Pipe"),
        "unit-separator" => pgettext("delimiter", "ASCII Separators"),
        other => other.to_string(),
    }
}

/// The row and column operations, for the menu that appears where the pointer
/// is. The same items the main menu lists, because they are the same operations.
fn structure_menu() -> gio::Menu {
    let rows = gio::Menu::new();
    rows.append(
        Some(&gettext("Insert Row Above")),
        Some("win.insert-row-above"),
    );
    rows.append(
        Some(&gettext("Insert Row Below")),
        Some("win.insert-row-below"),
    );
    rows.append(Some(&gettext("Delete Row")), Some("win.delete-row"));

    let columns = gio::Menu::new();
    columns.append(
        Some(&gettext("Insert Column Before")),
        Some("win.insert-column-before"),
    );
    columns.append(
        Some(&gettext("Insert Column After")),
        Some("win.insert-column-after"),
    );
    columns.append(Some(&gettext("Delete Column")), Some("win.delete-column"));

    let menu = gio::Menu::new();
    menu.append_section(None, &rows);
    menu.append_section(None, &columns);
    menu
}

/// How the file is being read: which delimiter, and whether its first record is
/// data or column titles.
pub(super) fn reading_menu() -> gio::Menu {
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
