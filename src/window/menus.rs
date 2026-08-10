// The menus, and the delimiters they offer.
//
// Every menu is built here rather than in the blueprint, because the ones that
// hang off a row number or a column heading have to say which row or column
// they belong to, and a menu written down in advance cannot. Building them all
// the same way is what keeps the four of them saying the same six things.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::gdk;
use gtk::gio;

use comma::document::Dialect;

use super::{AT_CURSOR, Axis, CommaWindow, STRUCTURE};

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
    /// Puts both halves of the operations under the pointer, which is where a
    /// table is usually asked about them. A cell is on a row and in a column, so
    /// it offers what both can do.
    pub(super) fn show_cell_menu(&self, x: f64, y: f64) {
        let menu = self
            .imp()
            .cell_menu
            .get_or_init(|| self.popover(&whole_menu()));
        point_at(menu, x, y);
    }

    /// The same, for a right button pressed on a row number. A row number is
    /// only on a row, so it offers only what a row can do — and the cursor has
    /// already been moved there, which is what the items act on.
    pub(super) fn show_row_menu(&self, x: f64, y: f64) {
        let menu = self
            .imp()
            .row_menu
            .get_or_init(|| self.popover(&section(Axis::Row, AT_CURSOR)));
        point_at(menu, x, y);
    }

    /// A menu that hangs off the table and is moved to wherever it is next
    /// asked for, rather than one built for each press.
    fn popover(&self, model: &gio::Menu) -> gtk::PopoverMenu {
        let menu = gtk::PopoverMenu::from_model(Some(model));
        menu.set_has_arrow(false);
        menu.set_halign(gtk::Align::Start);
        menu.set_parent(&*self.imp().column_view);
        menu
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

/// Points a menu at somewhere in the table and opens it.
fn point_at(menu: &gtk::PopoverMenu, x: f64, y: f64) {
    menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    menu.popup();
}

/// The operations for one axis, each carrying what it is to act on: a row or a
/// column, or `AT_CURSOR`, which means wherever the cursor is.
fn section(axis: Axis, at: i32) -> gio::Menu {
    let menu = gio::Menu::new();
    for operation in STRUCTURE.iter().filter(|operation| operation.axis == axis) {
        let item = gio::MenuItem::new(Some(&gettext(operation.label)), None);
        item.set_action_and_target_value(
            Some(&format!("win.{}", operation.name)),
            Some(&at.to_variant()),
        );
        menu.append_item(&item);
    }
    menu
}

/// Both halves, for the places that are on a row and in a column at once.
fn whole_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, &section(Axis::Row, AT_CURSOR));
    menu.append_section(None, &section(Axis::Column, AT_CURSOR));
    menu
}

/// What a column heading offers. It names its own column, because GTK pops a
/// heading's menu without asking first and there is no moment in between to
/// move the cursor into it.
pub(super) fn column_menu(column: usize) -> gio::Menu {
    section(Axis::Column, column as i32)
}

/// The main menu, in the order it reads.
pub(super) fn primary_menu() -> gio::Menu {
    let history = gio::Menu::new();
    history.append(Some(&gettext("_Undo")), Some("win.undo"));
    history.append(Some(&gettext("_Redo")), Some("win.redo"));

    let order = gio::Menu::new();
    order.append(
        Some(&gettext("Apply This Order to the File")),
        Some("win.commit-order"),
    );

    let exports = gio::Menu::new();
    exports.append(Some(&gettext("As PDF…")), Some("win.export-pdf"));
    exports.append(Some(&gettext("As Web Page…")), Some("win.export-html"));
    exports.append(Some(&gettext("As Spreadsheet…")), Some("win.export-ods"));
    let export = gio::Menu::new();
    export.append_submenu(Some(&gettext("_Export")), &exports);

    let file = gio::Menu::new();
    file.append(Some(&gettext("_Open…")), Some("win.open"));
    file.append(Some(&gettext("_Save")), Some("win.save"));
    file.append(Some(&gettext("Save _As…")), Some("win.save-as"));

    let about = gio::Menu::new();
    about.append(Some(&gettext("_Keyboard Shortcuts")), Some("win.shortcuts"));
    about.append(Some(&gettext("_About Comma")), Some("app.about"));

    let menu = gio::Menu::new();
    menu.append_section(None, &history);
    menu.append_section(None, &section(Axis::Row, AT_CURSOR));
    menu.append_section(None, &section(Axis::Column, AT_CURSOR));
    menu.append_section(None, &order);
    menu.append_section(None, &export);
    menu.append_section(None, &file);
    menu.append_section(None, &about);
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
