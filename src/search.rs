// Finding text in a value, and putting other text in its place.
//
// One definition serves both. What the filter hides a row for is what Replace
// All would change in it, so what you are looking at before you replace is what
// you are about to change.
//
// Case is folded a character at a time, on both sides, rather than by
// lowercasing whole values: a filter runs over every cell of the file each time
// a key is pressed, and a large file would be a great many strings built and
// thrown away. Folding as it reads costs nothing and means neither side has to
// arrive in any particular case.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

/// Whether `haystack` holds `needle` anywhere, ignoring case.
pub fn contains(haystack: &str, needle: &str) -> bool {
    find_from(haystack, needle, 0).is_some()
}

/// Whether two values are the same text, ignoring case.
///
/// Folded a character at a time on both sides, for the reason the module gives:
/// a filter asks this of every cell of the file and neither side should have to
/// arrive in any particular case.
pub fn equals(left: &str, right: &str) -> bool {
    let mut left = left.chars().flat_map(char::to_lowercase);
    let mut right = right.chars().flat_map(char::to_lowercase);

    loop {
        match (left.next(), right.next()) {
            (None, None) => return true,
            (left, right) if left == right => {}
            _ => return false,
        }
    }
}

/// `haystack` with every occurrence of `needle` replaced, or `None` when there
/// was nothing to replace. Saying nothing happened is the point: a cell nothing
/// matched in is left exactly as the file spelled it.
pub fn replace(haystack: &str, needle: &str, with: &str) -> Option<String> {
    let mut replaced: Option<String> = None;
    let mut taken = 0;

    while let Some((start, end)) = find_from(haystack, needle, taken) {
        let replaced = replaced.get_or_insert_with(String::new);
        replaced.push_str(&haystack[taken..start]);
        replaced.push_str(with);
        taken = end;

        if end == start {
            // An empty needle matches everywhere and would never move on.
            break;
        }
    }

    replaced.map(|mut replaced| {
        replaced.push_str(&haystack[taken..]);
        replaced
    })
}

/// Where the first match at or after `from` begins and ends, in bytes of
/// `haystack`. The end is where the matched text stops, which is not `start`
/// plus the needle's length: folding case can change how many bytes a character
/// takes.
fn find_from(haystack: &str, needle: &str, from: usize) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }

    haystack[from..]
        .char_indices()
        .map(|(offset, _)| from + offset)
        .find_map(|start| matches_at(haystack, needle, start).map(|end| (start, end)))
}

/// Where the match starting at `start` ends, if there is one.
fn matches_at(haystack: &str, needle: &str, start: usize) -> Option<usize> {
    let mut wanted = needle.chars().flat_map(char::to_lowercase);
    let mut end = start;

    for (offset, character) in haystack[start..].char_indices() {
        for folded in character.to_lowercase() {
            match wanted.next() {
                None => return Some(start + offset),
                Some(expected) if expected == folded => {}
                Some(_) => return None,
            }
        }
        end = start + offset + character.len_utf8();
    }

    wanted.next().is_none().then_some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_match_ignores_case_on_both_sides() {
        assert!(contains("Ada Lovelace", "lovelace"));
        assert!(contains("ADA", "ada"));
        assert!(contains("ada", "ADA"));
        assert!(!contains("Ada", "grace"));
    }

    #[test]
    fn the_whole_value_matches_in_whichever_case_it_is_written() {
        assert!(equals("Active", "active"));
        assert!(equals("STRASSE", "strasse"));
        assert!(!equals("active", "inactive"));
        assert!(!equals("act", "active"));
        assert!(equals("", ""));
    }

    #[test]
    fn folding_case_can_leave_two_values_the_same_length_or_not() {
        // Turkish İ lowercases to two characters, so equality cannot be decided
        // by comparing lengths first.
        assert!(equals("İ", "i\u{307}"));
    }

    #[test]
    fn nothing_matches_an_empty_needle() {
        // The caller treats an empty search as no search at all, and a match
        // that is everywhere is not a useful answer to give them.
        assert!(!contains("Ada", ""));
        assert_eq!(replace("Ada", "", "x"), None);
    }

    #[test]
    fn a_value_nothing_matched_is_left_alone() {
        assert_eq!(replace("Ada", "grace", "Grace"), None);
    }

    #[test]
    fn every_occurrence_goes() {
        assert_eq!(
            replace("a and A and a", "a", "x"),
            Some("x xnd x xnd x".to_string())
        );
    }

    #[test]
    fn replacing_keeps_what_it_did_not_match() {
        assert_eq!(
            replace("Lovelace, Ada", "ada", "Augusta"),
            Some("Lovelace, Augusta".to_string())
        );
    }

    #[test]
    fn folding_case_can_change_how_many_bytes_a_character_takes() {
        // Turkish İ lowercases to two characters, so the matched text is longer
        // than the needle and the rest of the value has to be found by where
        // the match actually ended.
        assert_eq!(replace("Xİ!", "i̇", "y"), Some("Xy!".to_string()));
    }

    #[test]
    fn german_text_matches_whichever_case_it_is_written_in() {
        assert!(contains("Ärger", "ärger"));
        assert_eq!(replace("Straße", "stra", "Gas"), Some("Gasße".to_string()));
    }
}
