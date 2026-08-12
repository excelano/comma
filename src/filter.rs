// The conditions a column can be held to, and which rows they leave showing.
//
// A filter is a view of the file and never a change to it. Nothing here writes,
// and nothing here is ever written down: a sorted order can be expressed in a
// delimited file and so may be committed on request, and a filter cannot, so it
// never is. The rows a filter hides are still in the file, still saved, and
// still numbered where they always were.
//
// The vocabulary is xql's, without its grammar. A predicate there is a column,
// an operator, and a value, joined to other predicates by AND; that is exactly
// what this is, and it is picked from a list rather than typed, because the
// person who reached for it right-clicked a column heading. Anyone who wants to
// write the query out has xql to write it in.
//
// At most one condition per column. `Between` is here precisely because a range
// is the one case that genuinely needs two, and having it means the rest of the
// model does not have to carry a list where a single answer will do.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::cmp::Ordering;

use crate::search;
use crate::value;

/// What a condition asks, apart from what it asks it against. This is the list
/// the picker offers, in the order it offers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Contains,
    DoesNotContain,
    Is,
    IsNot,
    IsEmpty,
    IsNotEmpty,
    GreaterThan,
    LessThan,
    Between,
}

impl Operator {
    /// Every operator, in the order the picker lists them: the two that look
    /// inside a value, the two that match it whole, the two that ask whether
    /// there is one at all, and the three that put it in order.
    pub const ALL: [Operator; 9] = [
        Operator::Contains,
        Operator::DoesNotContain,
        Operator::Is,
        Operator::IsNot,
        Operator::IsEmpty,
        Operator::IsNotEmpty,
        Operator::GreaterThan,
        Operator::LessThan,
        Operator::Between,
    ];

    /// What the operator is called where a name has to survive being written
    /// down — in an action's state, and in a menu item's target.
    pub fn id(self) -> &'static str {
        match self {
            Operator::Contains => "contains",
            Operator::DoesNotContain => "does-not-contain",
            Operator::Is => "is",
            Operator::IsNot => "is-not",
            Operator::IsEmpty => "is-empty",
            Operator::IsNotEmpty => "is-not-empty",
            Operator::GreaterThan => "greater-than",
            Operator::LessThan => "less-than",
            Operator::Between => "between",
        }
    }

    pub fn from_id(id: &str) -> Option<Operator> {
        Operator::ALL
            .into_iter()
            .find(|operator| operator.id() == id)
    }

    /// How many values the operator needs before it means anything: none for
    /// the two that ask whether a cell holds anything, two for a range, one for
    /// the rest. The picker shows this many entries.
    pub fn values(self) -> usize {
        match self {
            Operator::IsEmpty | Operator::IsNotEmpty => 0,
            Operator::Between => 2,
            _ => 1,
        }
    }

    /// The condition this operator makes of these values, or nothing when a
    /// value it needs was not given.
    ///
    /// A blank where a value belongs is refused rather than taken literally.
    /// "Contains nothing" would hide the whole file, and the person who wanted
    /// the empty cells has an operator that says so.
    pub fn build(self, first: &str, second: &str) -> Option<Condition> {
        let given = |value: &str| (!value.trim().is_empty()).then(|| value.to_string());

        Some(match self {
            Operator::IsEmpty => Condition::IsEmpty,
            Operator::IsNotEmpty => Condition::IsNotEmpty,
            Operator::Contains => Condition::Contains(given(first)?),
            Operator::DoesNotContain => Condition::DoesNotContain(given(first)?),
            Operator::Is => Condition::Is(given(first)?),
            Operator::IsNot => Condition::IsNot(given(first)?),
            Operator::GreaterThan => Condition::GreaterThan(given(first)?),
            Operator::LessThan => Condition::LessThan(given(first)?),
            Operator::Between => Condition::Between(given(first)?, given(second)?),
        })
    }
}

/// One operator and what it asks against.
///
/// Text is matched with case folded on both sides, as the search bar does, so
/// that a filter finds `Active` whichever case it was typed in. Nothing is
/// trimmed: a cell holding `" active "` is not the value `active`, and Comma
/// does not quietly decide otherwise about the contents of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Contains(String),
    DoesNotContain(String),
    Is(String),
    IsNot(String),
    IsEmpty,
    IsNotEmpty,
    GreaterThan(String),
    LessThan(String),
    Between(String, String),
}

impl Condition {
    pub fn operator(&self) -> Operator {
        match self {
            Condition::Contains(_) => Operator::Contains,
            Condition::DoesNotContain(_) => Operator::DoesNotContain,
            Condition::Is(_) => Operator::Is,
            Condition::IsNot(_) => Operator::IsNot,
            Condition::IsEmpty => Operator::IsEmpty,
            Condition::IsNotEmpty => Operator::IsNotEmpty,
            Condition::GreaterThan(_) => Operator::GreaterThan,
            Condition::LessThan(_) => Operator::LessThan,
            Condition::Between(_, _) => Operator::Between,
        }
    }

    /// What it was asked against, as the two entries the picker holds. An
    /// operator that takes fewer values leaves the rest empty, so reopening the
    /// picker on a condition already in force fills in what is there.
    pub fn values(&self) -> (&str, &str) {
        match self {
            Condition::IsEmpty | Condition::IsNotEmpty => ("", ""),
            Condition::Contains(value)
            | Condition::DoesNotContain(value)
            | Condition::Is(value)
            | Condition::IsNot(value)
            | Condition::GreaterThan(value)
            | Condition::LessThan(value) => (value, ""),
            Condition::Between(from, to) => (from, to),
        }
    }

    /// Whether one cell meets the condition.
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Condition::Contains(text) => search::contains(value, text),
            Condition::DoesNotContain(text) => !search::contains(value, text),
            Condition::Is(text) => search::equals(value, text),
            Condition::IsNot(text) => !search::equals(value, text),
            Condition::IsEmpty => value.trim().is_empty(),
            Condition::IsNotEmpty => !value.trim().is_empty(),
            Condition::GreaterThan(bound) => places(value, bound, &[Ordering::Greater]),
            Condition::LessThan(bound) => places(value, bound, &[Ordering::Less]),
            Condition::Between(from, to) => {
                places(value, from, &[Ordering::Greater, Ordering::Equal])
                    && places(value, to, &[Ordering::Less, Ordering::Equal])
            }
        }
    }
}

/// Whether a value falls one of these ways against a bound.
///
/// A value of another kind falls no way at all and is not shown. Filtering a
/// price column above 100 turns up neither `£5` nor a blank cell, on the
/// reasoning that neither of them is a price above 100 — which is the answer
/// the reader expected, and the one a comparison across kinds cannot give.
fn places(value: &str, bound: &str, ways: &[Ordering]) -> bool {
    value::compare_alike(value, bound).is_some_and(|order| ways.contains(&order))
}

/// The conditions in force, at most one per column, held in column order so
/// that they read across the way the table does.
///
/// Conditions are joined by AND. Several of them narrow one another, which is
/// what a person adding a second filter means by it. Asking for a row that
/// matches in any one of several columns is what the search bar is for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    conditions: Vec<(usize, Condition)>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.conditions.len()
    }

    /// Puts a condition on a column, replacing whatever that column had.
    pub fn set(&mut self, column: usize, condition: Condition) {
        match self.position(column) {
            Ok(at) => self.conditions[at].1 = condition,
            Err(at) => self.conditions.insert(at, (column, condition)),
        }
    }

    /// Takes the condition off one column, and says whether there was one.
    pub fn clear_column(&mut self, column: usize) -> bool {
        match self.position(column) {
            Ok(at) => {
                self.conditions.remove(at);
                true
            }
            Err(_) => false,
        }
    }

    pub fn clear(&mut self) {
        self.conditions.clear();
    }

    pub fn on(&self, column: usize) -> Option<&Condition> {
        self.position(column).ok().map(|at| &self.conditions[at].1)
    }

    /// Every condition in force, in column order.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Condition)> {
        self.conditions
            .iter()
            .map(|(column, condition)| (*column, condition))
    }

    /// Whether a row meets every condition. `value` is asked for each filtered
    /// column and for no others, so a wide file costs one lookup per filter
    /// rather than one per column.
    pub fn admits<'a>(&self, value: impl Fn(usize) -> &'a str) -> bool {
        self.conditions
            .iter()
            .all(|(column, condition)| condition.matches(value(*column)))
    }

    /// Follows a column being inserted. Everything from that column onwards is
    /// one place further along, and the new column has no condition on it.
    pub fn column_inserted(&mut self, at: usize) {
        for (column, _) in &mut self.conditions {
            if *column >= at {
                *column += 1;
            }
        }
    }

    /// Follows a column being taken out. Its own condition goes with it — the
    /// column it was about is not there any more — and everything after it
    /// moves back one.
    pub fn column_deleted(&mut self, at: usize) {
        self.conditions.retain(|(column, _)| *column != at);
        for (column, _) in &mut self.conditions {
            if *column > at {
                *column -= 1;
            }
        }
    }

    fn position(&self, column: usize) -> Result<usize, usize> {
        self.conditions
            .binary_search_by_key(&column, |(column, _)| *column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shows(condition: &Condition, values: &[&str]) -> Vec<String> {
        values
            .iter()
            .filter(|value| condition.matches(value))
            .map(|value| value.to_string())
            .collect()
    }

    fn condition(operator: Operator, first: &str, second: &str) -> Condition {
        operator
            .build(first, second)
            .expect("the test gave every value this operator takes")
    }

    #[test]
    fn contains_looks_inside_a_value_and_is_looks_at_the_whole_of_it() {
        let values = ["active", "inactive", "Active"];

        // The trap this pair exists to get out of: filtering a status column to
        // active with contains also turns up inactive.
        assert_eq!(
            shows(&condition(Operator::Contains, "active", ""), &values),
            ["active", "inactive", "Active"]
        );
        assert_eq!(
            shows(&condition(Operator::Is, "active", ""), &values),
            ["active", "Active"]
        );
    }

    #[test]
    fn case_is_folded_on_both_sides_as_the_search_bar_folds_it() {
        assert!(condition(Operator::Is, "ACTIVE", "").matches("active"));
        assert!(condition(Operator::Contains, "ÄRGER", "").matches("kein ärger"));
    }

    #[test]
    fn a_value_is_not_trimmed_before_it_is_matched() {
        // Comma does not decide that a file meant something other than what it
        // says. A cell holding spaces around a word is not that word.
        assert!(!condition(Operator::Is, "active", "").matches(" active "));
    }

    #[test]
    fn empty_finds_the_cells_that_look_empty() {
        let values = ["", "   ", "0", "x"];

        assert_eq!(shows(&Condition::IsEmpty, &values), ["", "   "]);
        assert_eq!(shows(&Condition::IsNotEmpty, &values), ["0", "x"]);
    }

    #[test]
    fn a_number_filter_reads_numbers_and_passes_over_everything_else() {
        let values = ["50", "150", "£5", "1.234,56", "", "abc"];

        assert_eq!(
            shows(&condition(Operator::GreaterThan, "100", ""), &values),
            ["150"]
        );
        assert_eq!(
            shows(&condition(Operator::LessThan, "100", ""), &values),
            ["50"]
        );
    }

    #[test]
    fn between_takes_both_ends_with_it() {
        let values = ["9", "10", "50", "100", "101"];

        assert_eq!(
            shows(&condition(Operator::Between, "10", "100"), &values),
            ["10", "50", "100"]
        );
    }

    #[test]
    fn text_can_be_put_in_order_too() {
        let values = ["apple", "mango", "zebra"];

        assert_eq!(
            shows(&condition(Operator::GreaterThan, "banana", ""), &values),
            ["mango", "zebra"]
        );
    }

    #[test]
    fn an_operator_that_needs_a_value_refuses_to_be_built_without_one() {
        assert_eq!(Operator::Contains.build("", ""), None);
        assert_eq!(Operator::Contains.build("   ", ""), None);
        assert_eq!(Operator::Between.build("10", ""), None);
        // The two that ask whether there is a value take none themselves.
        assert_eq!(Operator::IsEmpty.build("", ""), Some(Condition::IsEmpty));
    }

    #[test]
    fn every_operator_answers_to_its_own_name() {
        for operator in Operator::ALL {
            assert_eq!(Operator::from_id(operator.id()), Some(operator));
        }
        assert_eq!(Operator::from_id("sideways"), None);
    }

    #[test]
    fn conditions_are_joined_by_and() {
        let mut filters = Filters::default();
        filters.set(0, condition(Operator::Is, "active", ""));
        filters.set(2, condition(Operator::GreaterThan, "100", ""));

        let row = |values: [&'static str; 3]| filters.admits(|column| values[column]);

        assert!(row(["active", "anything", "150"]));
        assert!(!row(["closed", "anything", "150"]));
        assert!(!row(["active", "anything", "50"]));
    }

    #[test]
    fn one_condition_per_column_and_the_last_one_wins() {
        let mut filters = Filters::default();
        filters.set(1, condition(Operator::Is, "a", ""));
        filters.set(1, condition(Operator::Is, "b", ""));

        assert_eq!(filters.len(), 1);
        assert_eq!(filters.on(1), Some(&Condition::Is("b".to_string())));
    }

    #[test]
    fn conditions_read_across_in_column_order_however_they_were_added() {
        let mut filters = Filters::default();
        filters.set(4, Condition::IsEmpty);
        filters.set(1, Condition::IsNotEmpty);
        filters.set(3, Condition::IsEmpty);

        let columns: Vec<usize> = filters.iter().map(|(column, _)| column).collect();
        assert_eq!(columns, [1, 3, 4]);
    }

    #[test]
    fn a_condition_follows_its_column_when_another_is_inserted() {
        let mut filters = Filters::default();
        filters.set(2, Condition::IsEmpty);

        filters.column_inserted(0);
        assert_eq!(filters.on(3), Some(&Condition::IsEmpty));

        // A column inserted after it leaves it where it is.
        filters.column_inserted(9);
        assert_eq!(filters.on(3), Some(&Condition::IsEmpty));
    }

    #[test]
    fn a_condition_goes_when_its_own_column_does() {
        let mut filters = Filters::default();
        filters.set(1, Condition::IsEmpty);
        filters.set(3, Condition::IsNotEmpty);

        filters.column_deleted(1);

        assert_eq!(filters.len(), 1, "the condition on column 1 went with it");
        assert_eq!(filters.on(2), Some(&Condition::IsNotEmpty));
    }
}
