// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::grid::Row;

use super::CommaWindow;
use super::menus::primary_menu;

impl CommaWindow {
    pub(super) fn setup_selection(&self) {
        let rows = gtk::SignalListItemFactory::new();
        rows.connect_bind(|_, item| {
            if let Some(row) = item.downcast_ref::<gtk::ColumnViewRow>() {
                for on in [true, false] {
                    row.set_selectable(on);
                    row.set_activatable(on);
                }
            }
        });
        self.imp().column_view.set_row_factory(Some(&rows));

        self.imp().selection.connect_selection_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |selection, _, _| {
                if selection.selection().is_empty() {
                    window.imp().anchor.set(None);
                }
                window
                    .imp()
                    .menu_button
                    .set_menu_model(Some(&primary_menu(window.selection_size())));
                window.imp().gutter.queue_allocate();
            }
        ));
    }

    pub(super) fn select_row(&self, position: u32) {
        let imp = self.imp();
        if position >= imp.sorted.n_items() {
            return;
        }
        imp.selection.select_item(position, true);
        imp.anchor.set(Some(position));
        self.go_to(position, 0);
    }

    pub(super) fn select_to(&self, position: u32, column: usize) {
        let imp = self.imp();
        if position >= imp.sorted.n_items() {
            return;
        }
        let Some(anchor) = imp
            .anchor
            .get()
            .or_else(|| imp.current.get().map(|cursor| cursor.position))
        else {
            return self.select_row(position);
        };

        let (from, to) = (anchor.min(position), anchor.max(position));
        imp.selection.select_range(from, to - from + 1, true);
        imp.anchor.set(Some(anchor));
        self.go_to(position, column);
    }

    pub(super) fn toggle_row(&self, position: u32) {
        let imp = self.imp();
        if position >= imp.sorted.n_items() {
            return;
        }
        imp.anchor.set(Some(position));
        if imp.selection.is_selected(position) {
            imp.selection.unselect_item(position);
        } else {
            imp.selection.select_item(position, false);
        }
        self.go_to(position, 0);
    }

    pub(super) fn select_all(&self) -> bool {
        self.imp().selection.select_all()
    }

    pub(super) fn clear_selection(&self) -> bool {
        let selection = &self.imp().selection;
        !selection.selection().is_empty() && selection.unselect_all()
    }

    pub(super) fn selection_size(&self) -> u32 {
        self.imp().selection.selection().size() as u32
    }

    pub(super) fn selected_rows(&self) -> Option<(u32, Vec<usize>)> {
        let imp = self.imp();
        let selected = imp.selection.selection();
        let (iter, first) = gtk::BitsetIter::init_first(&selected)?;
        let rows = std::iter::once(first)
            .chain(iter)
            .filter_map(|position| imp.sorted.item(position).and_downcast::<Row>())
            .map(|row| row.index())
            .collect();
        Some((first, rows))
    }
}
