// The menus, and the delimiters they offer.
//
// Every menu is built here rather than in the blueprint, because the ones that
// hang off a row number or a column heading have to say which row or column
// they belong to, and a menu written down in advance cannot. Building them all
// the same way is what keeps the four of them saying the same six things.
//
// A column offers two kinds of thing, in two sections, and the line between
// them is the one this app is built on: above it are the changes to the file,
// and below it the ways of looking at it, which write nothing.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::gdk;
use gtk::gio;
use gtk::glib;

use comma::document::Dialect;

use super::files::Format;
use super::place::{self, Tool};
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
            .get_or_init(|| self.popover(&whole_menu(), &*self.imp().column_view));
        point_at(menu, x, y);
    }

    /// The same, for a right button pressed on a row number. A row number is
    /// only on a row, so it offers only what a row can do — and the cursor has
    /// already been moved there, which is what the items act on.
    pub(super) fn show_row_menu(&self, x: f64, y: f64) {
        let imp = self.imp();
        let menu = imp
            .row_menu
            .get_or_init(|| self.popover(&section(Axis::Row, AT_CURSOR), &*imp.column_view));
        // Said in the gutter's coordinates, because that is what the number
        // pressed could measure itself against, and wanted in the table's,
        // because that is what the menu hangs off. The numbers are to the left
        // of the table, so the point lands outside it and the menu opens against
        // its edge, which is beside the number that was pressed.
        let at = imp.gutter.compute_point(
            &*imp.column_view,
            &gtk::graphene::Point::new(x as f32, y as f32),
        );
        if let Some(at) = at {
            point_at(menu, f64::from(at.x()).max(0.0), f64::from(at.y()));
        }
    }

    /// A menu that hangs off one of the two views and is moved to wherever it is
    /// next asked for, rather than one built for each press.
    ///
    /// Which widget matters: the point it is opened at is measured against the
    /// one it hangs off, and both of these hang off the table. A row number is
    /// no longer inside a view of its own, so what it presses on has to be said
    /// in the table's coordinates before the menu is pointed at it.
    fn popover(&self, model: &gio::Menu, at: &impl IsA<gtk::Widget>) -> gtk::PopoverMenu {
        let menu = gtk::PopoverMenu::from_model(Some(model));
        menu.set_has_arrow(false);
        menu.set_halign(gtk::Align::Start);
        menu.set_parent(at);
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
///
/// A cell is the one place that can offer the fastest filter there is, because
/// it is the only one holding a value: hold this column to what is written
/// here, in one press and without typing it out again.
fn whole_menu() -> gio::Menu {
    let here = gio::Menu::new();
    here.append(
        Some(&gettext("Show Only Rows Like This One")),
        Some("win.filter-to-value"),
    );

    let menu = gio::Menu::new();
    menu.append_section(None, &section(Axis::Row, AT_CURSOR));
    menu.append_section(None, &section(Axis::Column, AT_CURSOR));
    menu.append_section(None, &here);
    menu.append_section(None, &views(AT_CURSOR));
    menu
}

/// What a column heading offers. It names its own column, because GTK pops a
/// heading's menu without asking first and there is no moment in between to
/// move the cursor into it.
pub(super) fn column_menu(column: usize) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, &section(Axis::Column, column as i32));
    menu.append_section(None, &views(column as i32));
    menu
}

/// The ways of looking at a column, which change nothing about it.
///
/// One entry rather than two, because the dialog it opens already knows whether
/// the column has a condition on it and offers to take it off when it has. A
/// menu built when the columns were built could not: what is filtered changes
/// long after.
fn views(at: i32) -> gio::Menu {
    let menu = gio::Menu::new();
    let item = gio::MenuItem::new(Some(&gettext("_Filter This Column…")), None);
    item.set_action_and_target_value(Some("win.filter-column"), Some(&at.to_variant()));
    menu.append_item(&item);
    menu
}

/// The main menu, in the order it reads.
pub(super) fn primary_menu() -> gio::Menu {
    let history = gio::Menu::new();
    history.append(Some(&gettext("_Undo")), Some("win.undo"));
    history.append(Some(&gettext("_Redo")), Some("win.redo"));

    let view = gio::Menu::new();
    view.append(
        Some(&gettext("Apply This Order to the File")),
        Some("win.commit-order"),
    );
    view.append(
        Some(&gettext("Clear All Filters")),
        Some("win.clear-filters"),
    );

    // The three that act on the document this window holds. Writing it out as
    // something else is under the Save button, with the arrow that offers it.
    let file = gio::Menu::new();
    file.append(Some(&gettext("_Open…")), Some("win.open"));
    file.append(Some(&gettext("_Reload")), Some("win.reload"));
    file.append(Some(&gettext("_Save")), Some("win.save"));

    let about = gio::Menu::new();
    about.append(Some(&gettext("_Keyboard Shortcuts")), Some("win.shortcuts"));
    about.append(Some(&gettext("_About Comma")), Some("app.about"));

    let menu = gio::Menu::new();
    menu.append_section(None, &history);
    menu.append_section(None, &section(Axis::Row, AT_CURSOR));
    menu.append_section(None, &section(Axis::Column, AT_CURSOR));
    menu.append_section(None, &views(AT_CURSOR));
    menu.append_section(None, &view);
    menu.append_section(None, &file);
    menu.append_section(None, &places());
    if let Some(tools) = tools() {
        menu.append_section(None, &tools);
    }
    menu.append_section(None, &about);
    menu
}

/// What the arrow beside the Save button offers.
///
/// Two sections, and the line between them is the one the column menu draws:
/// above it the document, written under another name and carried on from
/// there; below it files that are output and never come back. Comma will not
/// reopen any of the three, and none of them is the document.
pub(super) fn save_menu() -> gio::Menu {
    let elsewhere = gio::Menu::new();
    elsewhere.append(Some(&gettext("Save _As…")), Some("win.save-as"));

    let exports = gio::Menu::new();
    for format in Format::ALL {
        exports.append(
            Some(&gettext(format.label())),
            Some(&format!("win.{}", format.action())),
        );
    }

    let menu = gio::Menu::new();
    menu.append_section(None, &elsewhere);
    // The section says Export so that the entries under it do not have to: what
    // each one is called finishes the sentence this heading starts.
    menu.append_section(Some(&gettext("Export")), &exports);
    menu
}

/// The ways out of the window that are about where the file is rather than
/// about the file: the folder it is in, and a shell standing in that folder.
fn places() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("Open Containing _Folder")),
        Some("win.open-containing-folder"),
    );
    if place::terminals_reachable() {
        menu.append(
            Some(&gettext("Open in _Terminal")),
            Some("win.open-terminal"),
        );
    }
    menu
}

/// The tools that are here to be opened in, and only those.
///
/// Looked for once, when the window is built, because what is installed does
/// not change while it is open. A tool that is not there is not named at all:
/// an item greyed out for a tool the user has never heard of would be Comma
/// advertising rather than offering.
fn tools() -> Option<gio::Menu> {
    let menu = gio::Menu::new();
    for tool in Tool::ALL {
        if glib::find_program_in_path(tool.program()).is_some() {
            menu.append(Some(&tool.label()), Some(&format!("win.{}", tool.action())));
        }
    }

    (menu.n_items() > 0).then_some(menu)
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
