//! The length-prefixed framing both screen crossings are written against.
//!
//! Every field is `<length>:<that many UTF-16 code units>`, and a payload is a
//! sequence of fields in a fixed order. Counts are fields that read back as
//! numbers.
//!
//! This is length-prefixed rather than delimited or JSON, and the reason is the
//! reader on the other side. A setting's title, a mod's failure detail and a
//! player's typed value are all free-form text somebody else wrote, so any
//! delimiter can appear inside one and a delimited format would need escaping.
//! JSON would too, and the screen cannot borrow the game's parser for it: the
//! screen compiles against stubs generated from the pinned client jar, that jar
//! carries the game's own classes rather than the libraries they compile
//! against, and it has no `com.google.gson` entry — so JSON would mean shipping
//! a hand-written parser inside the screen jar and being right about escapes and
//! surrogate pairs forever. A length says where a field ends, so nothing inside
//! it can be misread and there is nothing to escape at all.
//!
//! UTF-16 is the unit because that is what a Java `String` counts in and what
//! crosses the JNI boundary.

use std::fmt::Write as _;

/// The wire contract the screen is written against.
///
/// A screen built against another one is reading fields under rules this side
/// never agreed to, so a payload that does not open with this is refused whole.
pub(crate) const VERSION: &str = "1";

/// Writes one field, prefixed by the length the other side reads it with.
pub(crate) fn field(out: &mut String, value: &str) {
    let _ = write!(out, "{}:{value}", value.encode_utf16().count());
}

/// Writes a count, which is a field the other side reads back as a number.
pub(crate) fn count(out: &mut String, value: usize) {
    field(out, &value.to_string());
}

/// Writes a flag, which is a field carrying one of two fixed words.
pub(crate) fn flag(out: &mut String, value: bool) {
    field(out, if value { "true" } else { "false" });
}

#[cfg(test)]
mod tests {
    use super::{count, field, flag};

    #[test]
    fn a_field_states_the_length_the_other_side_reads() {
        let mut out = String::new();
        field(&mut out, "abc");

        assert_eq!(out, "3:abc");
    }

    #[test]
    fn a_length_counts_the_units_java_counts_rather_than_bytes() {
        let mut out = String::new();
        // Outside the basic plane, so one character is two UTF-16 units and
        // four UTF-8 bytes. Java would read three.
        field(&mut out, "a🎈");

        assert_eq!(out, "3:a🎈");
    }

    #[test]
    fn punctuation_inside_a_field_needs_no_escaping_at_all() {
        let mut out = String::new();
        field(&mut out, "a:b\n\"c\\");

        assert_eq!(out, "7:a:b\n\"c\\");
    }

    #[test]
    fn a_count_and_a_flag_are_ordinary_fields() {
        let mut out = String::new();
        count(&mut out, 12);
        flag(&mut out, true);
        flag(&mut out, false);

        assert_eq!(out, "2:124:true5:false");
    }
}
