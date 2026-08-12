// The conditions a column is being held to, and the bar that says so.
//
// A filter hides rows, and a view that hides rows without saying it does is a
// lie about the file. That is the whole reason for the bar: every condition in
// force is on it, in words, with the button that takes it off, and the title
// says how much of the file is left. Comma numbers its rows from the top of the
// file for the same reason.
//
// The model behind it is `comma::filter`, which is where the vocabulary and the
// matching live. What is here is the asking and the showing.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;

use comma::filter::{Condition, Operator};

use super::CommaWindow;

impl CommaWindow {
    /// The title says how much of the file is showing whenever that is less
    /// than all of it. The count is read off the filtered model rather than
    /// counted here, and that model settles over several frames on a large
    /// file, so this follows it rather than asking once.
    pub(super) fn setup_filters(&self) {
        self.imp().shown.connect_items_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _, _, _| window.show_count()
        ));
    }

    /// Asks what this column should be held to. The column is the one the menu
    /// named, or the one the cursor is in when it named none.
    pub(super) fn filter_column(&self, at: i32) {
        let Some(column) = self.column_meant(at) else {
            return;
        };
        let Some(title) = self.column_title(column) else {
            return;
        };

        let existing = self.imp().filters.borrow().on(column).cloned();
        self.ask_for_filter(column, &title, existing);
    }

    /// Holds a column to the value already in front of you, which is the filter
    /// people reach for most and the one that should cost the least. A blank
    /// cell asks for the blanks, because that is what "this value" is there.
    pub(super) fn filter_to_value(&self) {
        let Some((row, column)) = self.current_cell() else {
            return;
        };
        let Some(document) = self.imp().rows.document() else {
            return;
        };

        let value = document.borrow().value(row, column).to_string();
        let condition = match value.trim().is_empty() {
            true => Condition::IsEmpty,
            false => Condition::Is(value),
        };
        self.set_filter(column, condition);
    }

    pub(super) fn clear_filter(&self, at: i32) {
        let Some(column) = self.column_meant(at) else {
            return;
        };
        if self.imp().filters.borrow_mut().clear_column(column) {
            self.filters_changed();
        }
    }

    pub(super) fn clear_filters(&self) {
        if self.imp().filters.borrow().is_empty() {
            return;
        }
        self.imp().filters.borrow_mut().clear();
        self.filters_changed();
    }

    /// Takes every condition off, without redrawing anything. For the moments
    /// when the file underneath them has become a different file: another
    /// document opened, or this one read again under a delimiter that gives it
    /// a different set of columns. The caller is already redrawing the grid.
    pub(super) fn forget_filters(&self) {
        self.imp().filters.borrow_mut().clear();
    }

    /// Follows a column being put in or taken out, so that a condition stays on
    /// the column it was about. A condition whose own column has gone goes with
    /// it: there is nothing left for it to be about.
    pub(super) fn columns_moved(&self, at: usize, inserted: bool) {
        let mut filters = self.imp().filters.borrow_mut();
        match inserted {
            true => filters.column_inserted(at),
            false => filters.column_deleted(at),
        }
    }

    fn set_filter(&self, column: usize, condition: Condition) {
        self.imp().filters.borrow_mut().set(column, condition);
        self.filters_changed();
    }

    /// Runs the conditions over the file again and says what is in force.
    pub(super) fn filters_changed(&self) {
        if let Some(filter) = self.imp().shown.filter() {
            filter.changed(gtk::FilterChange::Different);
        }
        self.show_filters();
        self.show_state();
    }

    /// Puts every condition in force on the bar, in the words it would be read
    /// out in, each with the button that takes it off. The bar is there while
    /// there is something on it and gone the rest of the time.
    pub(super) fn show_filters(&self) {
        let imp = self.imp();
        while let Some(child) = imp.chips.first_child() {
            imp.chips.remove(&child);
        }

        let titles = self.filtered_column_titles();
        let filters = imp.filters.borrow();
        for (column, condition) in filters.iter() {
            let title = titles.get(&column).cloned().unwrap_or_default();
            imp.chips
                .append(&chip(column, &describe(&title, condition)));
        }

        imp.filter_bar.set_visible(!filters.is_empty());
    }

    /// How much of the file the grid is showing, said only when that is less
    /// than all of it. A search narrows the view as surely as a filter does, so
    /// this counts what is on screen rather than what put it there.
    fn show_count(&self) {
        let imp = self.imp();
        let shown = imp.shown.n_items();
        let total = imp.rows.n_items();

        let subtitle = match shown == total {
            true => String::new(),
            // Translators: how much of the file the grid is showing, as in
            // "128 of 4312 rows". %s is the number showing and %t the number
            // the file holds.
            false => ngettext("%s of %t row", "%s of %t rows", total)
                .replace("%s", &shown.to_string())
                .replace("%t", &total.to_string()),
        };
        imp.window_title.set_subtitle(&subtitle);
    }

    /// Whether anything is keeping rows off the screen. Two things can, and
    /// everything that cares — whether an order can be written down, whether a
    /// row can be put next to another one — cares about both the same way.
    pub(super) fn hiding_rows(&self) -> bool {
        let imp = self.imp();
        !imp.needle.borrow().is_empty() || !imp.filters.borrow().is_empty()
    }

    /// Which column an action meant: the one it named, or the one the cursor is
    /// in when it named none.
    fn column_meant(&self, at: i32) -> Option<usize> {
        match usize::try_from(at) {
            Ok(column) => Some(column),
            Err(_) => self.current_cell().map(|(_, column)| column),
        }
    }

    /// The names of the columns a condition is on, which is all a chip needs
    /// and is a great deal less than every column of a wide file.
    fn filtered_column_titles(&self) -> std::collections::HashMap<usize, String> {
        self.imp()
            .filters
            .borrow()
            .iter()
            .filter_map(|(column, _)| Some((column, self.column_title(column)?)))
            .collect()
    }

    fn column_title(&self, column: usize) -> Option<String> {
        let document = self.imp().rows.document()?;
        let document = document.borrow();
        self.column_titles(&document).get(column).cloned()
    }

    /// The picker: an operator, and what to hold the column to.
    ///
    /// It is a dialog rather than a popover because a column heading's menu is
    /// GTK's own and there is nothing of ours under the pointer to hang a
    /// second popover off. A dialog also gets Escape, Enter and a tab order
    /// without being asked.
    fn ask_for_filter(&self, column: usize, title: &str, existing: Option<Condition>) {
        // Translators: the heading of the filter dialog. %c is a column name,
        // so this reads "Filter Status".
        let heading = gettext("Filter %c").replace("%c", title);
        let dialog = adw::AlertDialog::new(Some(&heading), None);

        let operators = gtk::StringList::new(&[]);
        for operator in Operator::ALL {
            operators.append(&operator_label(operator));
        }
        let picker = gtk::DropDown::builder()
            .model(&operators)
            .tooltip_text(gettext("What to Hold This Column To"))
            .build();

        let first = gtk::Entry::builder()
            .placeholder_text(gettext("Value"))
            .activates_default(true)
            .build();
        let second = gtk::Entry::builder()
            .placeholder_text(gettext("And"))
            .activates_default(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.append(&picker);
        content.append(&first);
        content.append(&second);
        dialog.set_extra_child(Some(&content));

        if let Some(existing) = &existing {
            let chosen = Operator::ALL
                .iter()
                .position(|operator| *operator == existing.operator())
                .unwrap_or(0);
            picker.set_selected(chosen as u32);
            let (from, to) = existing.values();
            first.set_text(from);
            second.set_text(to);
        }

        dialog.add_response(CANCEL, &gettext("_Cancel"));
        if existing.is_some() {
            dialog.add_response(REMOVE, &gettext("_Remove Filter"));
            dialog.set_response_appearance(REMOVE, adw::ResponseAppearance::Destructive);
        }
        dialog.add_response(APPLY, &gettext("_Apply"));
        dialog.set_response_appearance(APPLY, adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some(APPLY));
        dialog.set_close_response(CANCEL);

        // The picker decides how many values there are to give and whether
        // enough of them have been given, so both are settled in one place and
        // said again whenever either could have changed.
        let ready = glib::clone!(
            #[weak]
            dialog,
            #[weak]
            picker,
            #[weak]
            first,
            #[weak]
            second,
            move || {
                let operator = chosen_operator(&picker);
                first.set_visible(operator.values() >= 1);
                second.set_visible(operator.values() >= 2);
                dialog.set_response_enabled(
                    APPLY,
                    operator.build(&first.text(), &second.text()).is_some(),
                );
            }
        );
        ready();

        picker.connect_selected_notify(glib::clone!(
            #[strong]
            ready,
            move |_| ready()
        ));
        for entry in [&first, &second] {
            entry.connect_changed(glib::clone!(
                #[strong]
                ready,
                move |_| ready()
            ));
        }

        dialog.connect_response(
            None,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                move |_, response| match response {
                    REMOVE => window.clear_filter(column as i32),
                    APPLY => {
                        if let Some(condition) =
                            chosen_operator(&picker).build(&first.text(), &second.text())
                        {
                            window.set_filter(column, condition);
                        }
                    }
                    _ => {}
                }
            ),
        );

        dialog.present(Some(self));
    }
}

const CANCEL: &str = "cancel";
const REMOVE: &str = "remove";
const APPLY: &str = "apply";

fn chosen_operator(picker: &gtk::DropDown) -> Operator {
    Operator::ALL
        .get(picker.selected() as usize)
        .copied()
        .unwrap_or(Operator::Contains)
}

/// One condition on the bar: what it says, and the button that takes it off.
/// The words themselves are a button too, because the thing you want after
/// reading a filter you got slightly wrong is to change it.
fn chip(column: usize, label: &str) -> gtk::Box {
    let change = gtk::Button::builder()
        .label(label)
        .tooltip_text(gettext("Change This Filter"))
        .build();
    let remove = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text(gettext("Remove This Filter"))
        .build();

    for (button, action) in [
        (&change, "win.filter-column"),
        (&remove, "win.clear-filter"),
    ] {
        // A number rather than a name, because the action takes the column it
        // is to act on and a chip knows which one it is about.
        button.set_action_target_value(Some(&(column as i32).to_variant()));
        button.set_action_name(Some(action));
    }

    let chip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    chip.add_css_class("linked");
    chip.append(&change);
    chip.append(&remove);
    chip
}

/// What the picker lists an operator under. Lower case, because the dialog is
/// headed with the column's name and the list finishes the sentence it starts.
fn operator_label(operator: Operator) -> String {
    // Written out one by one rather than through a table, because what marks
    // these for translation is the extractor seeing the words, and it cannot
    // follow them through anything.
    match operator {
        Operator::Contains => gettext("contains"),
        Operator::DoesNotContain => gettext("does not contain"),
        Operator::Is => gettext("is"),
        Operator::IsNot => gettext("is not"),
        Operator::IsEmpty => gettext("is empty"),
        Operator::IsNotEmpty => gettext("is not empty"),
        Operator::GreaterThan => gettext("is greater than"),
        Operator::LessThan => gettext("is less than"),
        Operator::Between => gettext("is between"),
    }
}

/// What a condition reads as on its chip.
///
/// A whole sentence for each rather than the operator's own words with a column
/// and a value stuck either side of it, because where those parts go is not the
/// same in every language.
fn describe(title: &str, condition: &Condition) -> String {
    // Translators: each of these describes one filter in force and is shown on
    // the button that removes it. %c is the column's name, %v the value the
    // filter was given, and %w the far end of a range.
    let sentence = match condition.operator() {
        Operator::Contains => gettext("%c contains %v"),
        Operator::DoesNotContain => gettext("%c does not contain %v"),
        Operator::Is => gettext("%c is %v"),
        Operator::IsNot => gettext("%c is not %v"),
        Operator::IsEmpty => gettext("%c is empty"),
        Operator::IsNotEmpty => gettext("%c is not empty"),
        Operator::GreaterThan => gettext("%c is greater than %v"),
        Operator::LessThan => gettext("%c is less than %v"),
        Operator::Between => gettext("%c is between %v and %w"),
    };

    let (first, second) = condition.values();
    fill(&sentence, title, first, second)
}

/// Puts the parts of a description into their places, in one pass, so that a
/// column named `%v` goes in as a name rather than being read as somewhere to
/// put something else.
fn fill(sentence: &str, column: &str, first: &str, second: &str) -> String {
    let mut filled = String::with_capacity(sentence.len());
    let mut rest = sentence.chars();

    while let Some(character) = rest.next() {
        if character != '%' {
            filled.push(character);
            continue;
        }
        match rest.next() {
            Some('c') => filled.push_str(column),
            Some('v') => filled.push_str(first),
            Some('w') => filled.push_str(second),
            Some(other) => {
                filled.push('%');
                filled.push(other);
            }
            None => filled.push('%'),
        }
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_puts_each_part_where_the_sentence_asks_for_it() {
        assert_eq!(
            fill("%c is between %v and %w", "Price", "10", "100"),
            "Price is between 10 and 100"
        );
        // A sentence is translated whole, so a language that puts the parts in
        // another order simply says so.
        assert_eq!(
            fill("%v enthält %c", "Status", "aktiv", ""),
            "aktiv enthält Status"
        );
    }

    #[test]
    fn a_column_named_like_a_placeholder_goes_in_as_a_name() {
        // Filled in one pass, so the name is never read back over.
        assert_eq!(fill("%c is %v", "%v", "x", ""), "%v is x");
    }

    #[test]
    fn a_stray_percent_is_left_where_it_stands() {
        assert_eq!(fill("%c is 50%", "Share", "", ""), "Share is 50%");
        assert_eq!(fill("%c is %z", "Share", "", ""), "Share is %z");
    }
}
