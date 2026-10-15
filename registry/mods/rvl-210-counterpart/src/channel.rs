//! Section seven: the screen value channel and its seven readers.
//!
//! A screen and its server-side counterpart share a small keyed store of values
//! addressed by an integer the server allocates. Each value is boxed inside a
//! per-identifier compound under one fixed key, and every reader follows the
//! host's own absent-value defaults rather than raising.
//!
//! The distinction that matters is between "not sent yet" and "sent as empty".
//! Three readers keep it, three collapse it, and the numeric pair never had it.
//! A dependent screen distinguishes the two cases on exactly this, so the table
//! in [`Channel`] is the behaviour and not an implementation detail.

use std::collections::BTreeMap;

use ravel_sdk::JsonValue;

/// The key every value is boxed under inside its own compound.
pub const BOXED_KEY: &str = "value";

/// The most identifiers one channel will allocate.
///
/// The server allocates them and a screen reads them, so an unbounded allocator
/// is a screen that can be made to hold arbitrarily many compounds by whatever
/// drives it.
pub const MAX_IDENTIFIERS: u32 = 4096;

/// One shared store of values between a screen and its server-side counterpart.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Channel {
    values: BTreeMap<u32, JsonValue>,
    next: u32,
}

impl Channel {
    /// An empty channel that has allocated nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            next: 0,
        }
    }

    /// Allocates the next identifier.
    ///
    /// Answers nothing once [`MAX_IDENTIFIERS`] have been handed out, rather
    /// than wrapping onto an identifier a screen is already reading.
    pub fn allocate(&mut self) -> Option<u32> {
        if self.next >= MAX_IDENTIFIERS {
            return None;
        }
        let allocated = self.next;
        self.next += 1;
        Some(allocated)
    }

    /// How many identifiers hold a compound.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether nothing has been sent.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Boxes a value under an identifier, which is what the server pushes.
    ///
    /// Answers whether it was boxed. An identifier at or above
    /// [`MAX_IDENTIFIERS`] is refused: section seven's identifiers arrive from
    /// the server over the transport, so the allocator's own bound does not bind
    /// this path and a channel would otherwise grow past what it publishes.
    pub fn send(&mut self, identifier: u32, value: JsonValue) -> bool {
        if identifier >= MAX_IDENTIFIERS {
            return false;
        }
        self.values.insert(
            identifier,
            JsonValue::Object([(BOXED_KEY.to_owned(), value)].into_iter().collect()),
        );
        true
    }

    /// Whether an identifier has ever been sent.
    #[must_use]
    pub fn holds(&self, identifier: u32) -> bool {
        self.values.contains_key(&identifier)
    }

    /// The whole number under an identifier.
    ///
    /// Zero when the identifier is absent and zero when what is boxed is not a
    /// whole number. The two cases are not distinguishable through this reader
    /// and are not distinguishable through the original's either.
    #[must_use]
    pub fn number(&self, identifier: u32) -> i64 {
        self.boxed(identifier)
            .and_then(JsonValue::as_i64)
            .unwrap_or(0)
    }

    /// The fractional number under an identifier, on the same rule as
    /// [`Channel::number`].
    #[must_use]
    pub fn fraction(&self, identifier: u32) -> f64 {
        self.boxed(identifier)
            .and_then(JsonValue::as_f64)
            .unwrap_or(0.0)
    }

    /// The truth value under an identifier, on the same rule.
    #[must_use]
    pub fn truth(&self, identifier: u32) -> bool {
        self.boxed(identifier)
            .and_then(JsonValue::as_bool)
            .unwrap_or(false)
    }

    /// The text under an identifier.
    ///
    /// Nothing at all only when the identifier is **absent**. When the
    /// identifier is present and what is boxed is not text, the underlying read
    /// answers the empty string, so this answers an empty string rather than
    /// nothing. That is the distinction a dependent screen reads.
    #[must_use]
    pub fn text(&self, identifier: u32) -> Option<String> {
        let boxed = self.boxed(identifier)?;
        Some(boxed.as_str().unwrap_or_default().to_owned())
    }

    /// The list of display texts under an identifier, on the same rule as
    /// [`Channel::text`]: nothing when absent, an empty list when present and of
    /// the wrong kind.
    #[must_use]
    pub fn text_list(&self, identifier: u32) -> Option<Vec<String>> {
        let boxed = self.boxed(identifier)?;
        let Some(entries) = boxed.as_array() else {
            return Some(Vec::new());
        };
        Some(
            entries
                .iter()
                .map(|entry| entry.as_str().unwrap_or_default().to_owned())
                .collect(),
        )
    }

    /// Whatever is under the fixed key, without a type.
    ///
    /// Nothing when the identifier is absent and nothing when the compound holds
    /// no value under the fixed key, so this reader does not preserve the
    /// distinction the text reader does.
    #[must_use]
    pub fn raw(&self, identifier: u32) -> Option<&JsonValue> {
        self.boxed(identifier)
    }

    /// One display text, read as a string and handed to the deserialiser.
    ///
    /// Nothing when the identifier is absent and nothing when what is boxed is
    /// not text, so this reader also collapses the two cases.
    pub fn display_text<T>(
        &self,
        identifier: u32,
        deserialise: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        deserialise(self.boxed(identifier)?.as_str()?)
    }

    fn boxed(&self, identifier: u32) -> Option<&JsonValue> {
        self.values
            .get(&identifier)
            .and_then(|compound| compound.get(BOXED_KEY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::json;

    fn channel() -> Channel {
        let mut channel = Channel::new();
        channel.send(1, json!(7));
        channel.send(2, json!(true));
        channel.send(3, json!("written"));
        channel.send(4, json!(["one", "two"]));
        channel.send(5, json!({ "nested": 1 }));
        channel.send(6, json!(1.5));
        channel
    }

    #[test]
    fn every_reader_answers_what_was_sent() {
        let channel = channel();
        assert_eq!(channel.number(1), 7);
        assert!(channel.truth(2));
        assert_eq!(channel.text(3), Some("written".to_owned()));
        assert_eq!(
            channel.text_list(4),
            Some(vec!["one".to_owned(), "two".to_owned()])
        );
        assert_eq!(channel.raw(5), Some(&json!({ "nested": 1 })));
        assert!((channel.fraction(6) - 1.5).abs() < f64::EPSILON);
        assert_eq!(
            channel.display_text(3, |text| Some(text.to_uppercase())),
            Some("WRITTEN".to_owned())
        );
    }

    #[test]
    fn the_numeric_and_truth_readers_answer_the_same_for_absent_and_for_wrong_kind() {
        let channel = channel();
        assert_eq!(channel.number(99), 0, "absent");
        assert_eq!(channel.number(3), 0, "present as text");
        assert!(!channel.truth(99), "absent");
        assert!(!channel.truth(1), "present as a number");
        assert!(channel.fraction(99).abs() < f64::EPSILON);
        assert!(channel.fraction(3).abs() < f64::EPSILON);
    }

    #[test]
    fn the_text_reader_keeps_absent_apart_from_sent_as_the_wrong_kind() {
        let channel = channel();
        assert_eq!(channel.text(99), None, "not sent yet");
        assert_eq!(
            channel.text(1),
            Some(String::new()),
            "sent, and the underlying read answers an empty string"
        );
        assert_ne!(channel.text(99), channel.text(1));
    }

    #[test]
    fn the_display_text_list_reader_keeps_the_same_distinction() {
        let channel = channel();
        assert_eq!(channel.text_list(99), None, "not sent yet");
        assert_eq!(channel.text_list(1), Some(Vec::new()), "sent, wrong kind");
        assert_ne!(channel.text_list(99), channel.text_list(1));
    }

    #[test]
    fn the_raw_and_single_display_text_readers_collapse_the_two_cases() {
        let channel = channel();
        assert_eq!(channel.raw(99), None, "not sent yet");
        let deserialise = |text: &str| Some(text.to_owned());
        assert_eq!(channel.display_text(99, deserialise), None, "not sent yet");
        assert_eq!(
            channel.display_text(1, deserialise),
            None,
            "sent as a number, and this reader cannot say so"
        );
        assert_eq!(
            channel.display_text(99, deserialise),
            channel.display_text(1, deserialise)
        );
    }

    #[test]
    fn a_value_is_boxed_under_the_one_fixed_key() {
        let mut channel = Channel::new();
        channel.send(1, json!(7));
        assert_eq!(channel.raw(1), Some(&json!(7)));
        assert!(channel.holds(1));
        assert!(!channel.holds(2));
    }

    #[test]
    fn a_resent_identifier_answers_the_second_value() {
        let mut channel = Channel::new();
        channel.send(1, json!(7));
        channel.send(1, json!(9));
        assert_eq!(channel.number(1), 9);
        assert_eq!(channel.len(), 1);
    }

    #[test]
    fn identifiers_are_handed_out_in_order_and_stop_at_the_published_limit() {
        let mut channel = Channel::new();
        assert_eq!(channel.allocate(), Some(0));
        assert_eq!(channel.allocate(), Some(1));
        let mut channel = Channel::new();
        for expected in 0..MAX_IDENTIFIERS {
            assert_eq!(channel.allocate(), Some(expected));
        }
        assert_eq!(channel.allocate(), None, "the allocator does not wrap");
    }

    #[test]
    fn a_send_for_an_identifier_past_the_published_bound_is_refused() {
        let mut channel = Channel::new();
        assert!(channel.send(MAX_IDENTIFIERS - 1, json!(7)));
        assert!(
            !channel.send(MAX_IDENTIFIERS, json!(7)),
            "the identifiers arrive over the transport, so the allocator does \
             not bind this path"
        );
        assert!(!channel.send(u32::MAX, json!(7)));
        assert_eq!(channel.len(), 1);
        assert!(!channel.holds(MAX_IDENTIFIERS));
    }
}
