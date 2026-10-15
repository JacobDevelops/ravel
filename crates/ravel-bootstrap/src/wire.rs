//! UTF-16 length-prefixed fields shared by the Java crossings.
//!
//! Each field is `<length>:<UTF-16 units>`; counts and flags are ordinary fields.

use std::fmt::Write as _;

/// Shared payload version; readers refuse mismatched versions.
pub(crate) const VERSION: &str = "1";

/// Independent mod-list version, bumped when its row shape changes.
pub(crate) const MODS_VERSION: &str = "3";

/// Writes one field, prefixed by the length the other side reads it with.
pub(crate) fn field(out: &mut String, value: &str) {
    write!(out, "{}:{value}", value.encode_utf16().count())
        .expect("writing into a String cannot fail");
}

/// Writes a count, which is a field the other side reads back as a number.
pub(crate) fn count(out: &mut String, value: usize) {
    field(out, &value.to_string());
}

/// Writes a flag, which is a field carrying one of two fixed words.
pub(crate) fn flag(out: &mut String, value: bool) {
    field(out, if value { "true" } else { "false" });
}

/// Decodes fields for crossing tests using Java's UTF-16 indexing.
#[cfg(test)]
pub(crate) fn fields(payload: &str) -> Vec<String> {
    let units: Vec<u16> = payload.encode_utf16().collect();
    let mut read = Vec::new();
    let mut at = 0;
    while at < units.len() {
        let separator = at
            + units[at..]
                .iter()
                .position(|unit| *unit == u16::from(b':'))
                .expect("every field states a length");
        let length: usize = String::from_utf16(&units[at..separator])
            .expect("a length is text")
            .parse()
            .expect("a length is a number");
        let start = separator + 1;
        read.push(String::from_utf16(&units[start..start + length]).expect("a field is text"));
        at = start + length;
    }
    read
}

#[cfg(test)]
mod tests {
    use super::{count, field, fields, flag};

    #[test]
    fn a_field_states_the_length_the_other_side_reads() {
        let mut out = String::new();
        field(&mut out, "abc");

        assert_eq!(out, "3:abc");
    }

    #[test]
    fn a_length_counts_the_units_java_counts_rather_than_bytes() {
        let mut out = String::new();
        // A supplementary character occupies two UTF-16 units.
        field(&mut out, "a🎈");

        assert_eq!(out, "3:a🎈");
        assert_eq!(fields(&out), ["a🎈"]);
    }

    #[test]
    fn punctuation_inside_a_field_needs_no_escaping_at_all() {
        let mut out = String::new();
        field(&mut out, "a:b\n\"c\\");

        assert_eq!(out, "7:a:b\n\"c\\");
        assert_eq!(fields(&out), ["a:b\n\"c\\"]);
    }

    #[test]
    fn a_count_and_a_flag_are_ordinary_fields() {
        let mut out = String::new();
        count(&mut out, 12);
        flag(&mut out, true);
        flag(&mut out, false);

        assert_eq!(out, "2:124:true5:false");
        assert_eq!(fields(&out), ["12", "true", "false"]);
    }
}
