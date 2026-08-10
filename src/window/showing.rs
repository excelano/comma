// Which rows the grid is showing, and in what order.
//
// A search and a sort are views of the file and change nothing about it. The
// two operations that turn a view into a change — Replace All and writing the
// order down — are here as well, next to the views they act on.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use comma::search;

use crate::grid::Row;

use super::CommaWindow;

impl CommaWindow {
    /// Searching hides the rows nothing matched in rather than walking a cursor
    /// from one match to the next. In a table those are the same question
    /// answered two ways, and the one that answers it all at once also shows
    /// you what Replace All is about to change.
    pub(super) fn setup_search(&self) {
        let imp = self.imp();

        // Without this the bar and its box are two unrelated widgets: typing
        // with the table focused would not reach the search, and Escape would
        // not close it.
        imp.search_bar.connect_entry(&*imp.search_entry);
        imp.search_bar.set_key_capture_widget(Some(self));
        imp.shown
            .set_filter(Some(&gtk::CustomFilter::new(glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or]
                true,
                move |object| window.row_matches(object)
            ))));

        imp.search_entry.connect_search_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |entry| window.search_for(&entry.text())
        ));

        // Closing the search puts every row back. A file quietly missing rows
        // because of a search nobody can see would be a lie about the file.
        imp.search_bar
            .connect_search_mode_enabled_notify(glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |bar| {
                    window.set_action_state("find", &bar.is_search_mode().to_variant());
                    if !bar.is_search_mode() {
                        window.imp().search_entry.set_text("");
                    }
                }
            ));
    }

    fn search_for(&self, needle: &str) {
        self.imp().needle.replace(needle.to_string());

        if let Some(filter) = self.imp().shown.filter() {
            filter.changed(gtk::FilterChange::Different);
        }
        self.show_state();
    }

    fn row_matches(&self, object: &glib::Object) -> bool {
        let imp = self.imp();
        let needle = imp.needle.borrow();
        if needle.is_empty() {
            return true;
        }

        let (Some(row), Some(document)) = (object.downcast_ref::<Row>(), imp.rows.document())
        else {
            return true;
        };
        let document = document.borrow();
        let row = row.index();

        (0..document.field_count(row))
            .any(|column| search::contains(document.value(row, column), &needle))
    }

    /// Replaces what is being searched for, in the rows the search is showing.
    /// What you are looking at is what changes.
    pub(super) fn replace_all(&self) {
        let imp = self.imp();
        let needle = imp.needle.borrow().clone();
        let Some(document) = imp.rows.document() else {
            return;
        };
        if needle.is_empty() {
            return;
        }

        let rows = self.shown_rows();

        let replacement = imp.replacement.text();
        let changed = document
            .borrow_mut()
            .replace_in(&rows, &needle, &replacement);
        if changed > 0 {
            self.reload();
        }
    }

    /// Writes the order the grid is showing into the file. Sorting is a view of
    /// the file until this is asked for, and this is the only thing that makes
    /// it anything else.
    pub(super) fn commit_order(&self) {
        let imp = self.imp();
        let Some(document) = imp.rows.document() else {
            return;
        };
        let rows = document.borrow().row_count();

        let mut order = Vec::with_capacity(rows);
        if imp.rows.header() {
            // The header record is not one of the rows and does not move.
            order.push(0);
        }
        order.extend(self.shown_rows());

        if order.len() != rows {
            // Some rows are not being shown, so this order does not account for
            // all of them, and a file is not rewritten from a part of itself.
            return;
        }

        document.borrow_mut().reorder_rows(order);
        // The file is in that order now, so the grid has nothing left to do
        // about it.
        imp.column_view
            .sort_by_column(None::<&gtk::ColumnViewColumn>, gtk::SortType::Ascending);
        self.reload();
    }

    /// The rows the grid is showing, in the order it is showing them. An export
    /// is a picture of the grid, so a search and a sort are part of it.
    pub(super) fn shown_rows(&self) -> Vec<usize> {
        // A sort model says its items are plain objects, because it cannot know
        // what it will be given until it is given it.
        self.imp()
            .sorted
            .iter::<glib::Object>()
            .flatten()
            .filter_map(|object| object.downcast::<Row>().ok())
            .map(|row| row.index())
            .collect()
    }

    /// Which data column the grid is sorted by, and which way, when it is
    /// sorted by one at all.
    pub(super) fn sorted_by(&self) -> Option<(usize, gtk::SortType)> {
        let sorter = self
            .imp()
            .column_view
            .sorter()?
            .downcast::<gtk::ColumnViewSorter>()
            .ok()?;
        let sorted = sorter.primary_sort_column()?;

        let columns = self.imp().column_view.columns();
        let position = (0..columns.n_items()).find(|&index| {
            columns
                .item(index)
                .and_downcast::<gtk::ColumnViewColumn>()
                .is_some_and(|column| column == sorted)
        })?;

        // The first column is the row-number gutter, which nothing sorts by.
        Some((
            position.checked_sub(1)? as usize,
            sorter.primary_sort_order(),
        ))
    }

    pub(super) fn sort_by(&self, column: usize, direction: gtk::SortType) {
        let columns = self.imp().column_view.columns();
        if let Some(column) = columns
            .item(column as u32 + 1)
            .and_downcast::<gtk::ColumnViewColumn>()
        {
            self.imp()
                .column_view
                .sort_by_column(Some(&column), direction);
        }
    }
}
