//! UTF-16 length-prefixed fields shared by the Java crossings.
//!
//! Each field is `<length>:<UTF-16 units>`; counts and flags are ordinary fields.

use std::fmt::Write as _;

/// Shared payload version; readers refuse mismatched versions.
pub(crate) const VERSION: &str = "1";

/// Independent mod-list version, bumped when its row shape changes.
pub(crate) const MODS_VERSION: &str = "4";

/// Appends a human-facing label, never an identity used to select a runtime.
pub(crate) fn integrated_row(out: &mut String, id: &impl std::fmt::Display) {
    write!(out, "{id} [integrated-server]").expect("writing into a String cannot fail");
}

/// Appends a human-facing notice, never an identity used to select a runtime.
pub(crate) fn integrated_note(out: &mut String, text: &impl std::fmt::Display) {
    write!(out, "[integrated-server] {text}").expect("writing into a String cannot fail");
}

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

/// Decodes the same UTF-16 length-prefixed fields used by Java.
pub(crate) struct Reader {
    units: Vec<u16>,
    at: usize,
}

impl Reader {
    pub(crate) fn new(payload: &str) -> Self {
        Self {
            units: payload.encode_utf16().collect(),
            at: 0,
        }
    }

    pub(crate) fn field(&mut self) -> Option<String> {
        let separator = self.at
            + self
                .units
                .get(self.at..)?
                .iter()
                .position(|unit| *unit == u16::from(b':'))?;
        let length: usize = String::from_utf16(&self.units[self.at..separator])
            .ok()?
            .parse()
            .ok()?;
        let start = separator + 1;
        let end = start.checked_add(length)?;
        let field = String::from_utf16(self.units.get(start..end)?).ok()?;
        self.at = end;
        Some(field)
    }

    pub(crate) fn count(&mut self) -> Option<usize> {
        self.field()?.parse().ok()
    }

    pub(crate) const fn finished(&self) -> bool {
        self.at == self.units.len()
    }
}

/// Decodes fields for crossing tests using Java's UTF-16 indexing.
#[cfg(test)]
pub(crate) fn fields(payload: &str) -> Vec<String> {
    let mut reader = Reader::new(payload);
    let mut read = Vec::new();
    while !reader.finished() {
        read.push(reader.field().expect("a valid length-prefixed field"));
    }
    read
}

#[cfg(test)]
mod tests {
    use super::{Reader, count, field, fields, flag};

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

    #[test]
    fn malformed_fields_are_refused_without_advancing_the_reader() {
        for payload in [
            "missing",
            "x:value",
            "9:short",
            "1:🎈",
            "999999999999999999999999:x",
        ] {
            let mut reader = Reader::new(payload);
            assert_eq!(reader.field(), None, "{payload}");
            assert_eq!(reader.field(), None, "a refusal does not consume input");
            assert!(!reader.finished());
        }
    }

    #[test]
    fn counts_and_empty_fields_share_the_production_decoder() {
        let mut reader = Reader::new("1:20:3:a🎈");
        assert_eq!(reader.count(), Some(2));
        assert_eq!(reader.field().as_deref(), Some(""));
        assert_eq!(reader.field().as_deref(), Some("a🎈"));
        assert!(reader.finished());
        assert_eq!(reader.field(), None);
    }
}
