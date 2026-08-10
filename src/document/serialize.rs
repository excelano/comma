// Turning a cell value back into the bytes a delimited file holds.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use super::dialect::Dialect;

/// A value has to be quoted when writing it plainly would create a field
/// boundary or a record boundary that is not there in the value itself.
pub(super) fn needs_quoting(value: &str, dialect: Dialect) -> bool {
    value
        .bytes()
        .any(|byte| byte == dialect.quote_byte() || dialect.is_boundary(byte))
}

/// Appends the canonical bytes for `value` to `out`.
pub(super) fn write_field(out: &mut String, value: &str, dialect: Dialect) {
    if !needs_quoting(value, dialect) {
        out.push_str(value);
        return;
    }

    let quote = dialect.quote();
    out.push(quote);
    for character in value.chars() {
        if character == quote {
            out.push(quote);
        }
        out.push(character);
    }
    out.push(quote);
}

/// Whether writing `value` out would reproduce `raw` byte for byte.
///
/// This is the check that makes the round-trip guarantee provable rather than
/// hopeful. Every field is measured against its own bytes as the file is read,
/// and only the ones that disagree pay to keep a copy.
pub(super) fn matches_canonical(value: &str, raw: &str, dialect: Dialect) -> bool {
    if !needs_quoting(value, dialect) {
        return raw == value;
    }

    let mut canonical = String::with_capacity(raw.len());
    write_field(&mut canonical, value, dialect);
    canonical == raw
}
