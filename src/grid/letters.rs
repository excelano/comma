// Spreadsheet column names.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

/// The name of a column by its position: A, B, … Z, AA, AB, … The same scheme
/// every spreadsheet uses, so a column named in one is the column meant here.
///
/// It is base 26 without a zero, which is why the carry subtracts one.
pub fn column_letter(mut index: usize) -> String {
    let mut letters = Vec::new();

    loop {
        letters.push(b'A' + (index % 26) as u8);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }

    letters.reverse();
    String::from_utf8(letters).expect("every byte pushed is an ASCII letter")
}

#[cfg(test)]
mod tests {
    use super::column_letter;

    #[test]
    fn the_first_twenty_six_columns_are_single_letters() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(1), "B");
        assert_eq!(column_letter(25), "Z");
    }

    #[test]
    fn the_twenty_seventh_column_carries() {
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(27), "AB");
        assert_eq!(column_letter(51), "AZ");
        assert_eq!(column_letter(52), "BA");
    }

    #[test]
    fn the_names_match_a_spreadsheet_at_the_awkward_places() {
        assert_eq!(column_letter(701), "ZZ");
        assert_eq!(column_letter(702), "AAA");
        assert_eq!(column_letter(16383), "XFD");
    }
}
