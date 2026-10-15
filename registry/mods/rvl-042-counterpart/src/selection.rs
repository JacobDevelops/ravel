//! Choosing one concrete entry to stand for a tag.
//!
//! A recipe may declare its result as a tag rather than an entry. This decides
//! which entry that tag means in one installation, and records the answer so it
//! does not move under a pack once it has been given.
//!
//! The selection rule here is a declared behaviour change, not parity. The
//! original ranks nothing: it folds its comparator over a sequence of one and
//! takes the tag's first member, so its configured preference order has no
//! effect at all. This honours the preference order, which is what the setting
//! was written for, and orders the remainder by full identifier so the answer
//! does not depend on member iteration order. Member order is the host's and is
//! not reproducible across runs, so reproducing it would make the recorded
//! answer arbitrary.

use std::collections::BTreeMap;
use std::fmt;

use ravel_sdk::JsonValue;
use ravel_sdk::content::RegistryKey;
use ravel_sdk::tags::TagId;

/// The key one entry of the record is written under: a documentation pointer,
/// skipped when reading, and the only key a fresh record carries.
pub const POINTER_KEY: &str = "_pointer";

/// What another mod that has already unified a tag says the tag means.
///
/// An installation that unified its tags must not be contradicted, so this is
/// consulted first and its answer is taken whole.
pub trait Unification {
    /// Returns the entry this tag was unified onto, if it was.
    fn preferred(&self, tag: &TagId) -> Option<RegistryKey>;
}

/// One recorded answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Recorded {
    /// The tag means this entry.
    Entry(RegistryKey),
    /// The record says the tag has no representative, so it is chosen again.
    Absent,
}

/// The preference order a pack author states, most preferred first.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Preference {
    namespaces: Vec<String>,
}

impl Preference {
    /// Builds a preference order from namespaces, most preferred first.
    ///
    /// A repeated namespace keeps its first position; a blank entry is dropped.
    pub fn new<I, S>(namespaces: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut ordered: Vec<String> = Vec::new();
        for namespace in namespaces {
            let namespace = namespace.into().trim().to_owned();
            if !namespace.is_empty() && !ordered.iter().any(|held| held == &namespace) {
                ordered.push(namespace);
            }
        }
        Self {
            namespaces: ordered,
        }
    }

    /// Reads the comma-separated form a setting carries.
    #[must_use]
    pub fn parse(setting: &str) -> Self {
        Self::new(setting.split(','))
    }

    /// Returns the namespaces, most preferred first.
    #[must_use]
    pub fn namespaces(&self) -> &[String] {
        &self.namespaces
    }

    /// Returns how far down the order a namespace sits, listed before unlisted.
    fn rank(&self, namespace: &str) -> usize {
        self.namespaces
            .iter()
            .position(|held| held == namespace)
            .unwrap_or(self.namespaces.len())
    }
}

/// Why a record was not read.
#[derive(Debug, Eq, PartialEq)]
pub enum RecordError {
    /// The document is not an object of tag to entry.
    Shape,
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape => write!(
                formatter,
                "the record is a flat object of tag identifier to entry identifier"
            ),
        }
    }
}

impl std::error::Error for RecordError {}

/// The recorded answers and the rule that fills them in.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    preference: Preference,
    self_heal: bool,
    recorded: BTreeMap<String, Recorded>,
    rewrite_pending: bool,
}

impl Selection {
    /// Builds a selection holding no recorded answer.
    ///
    /// `self_heal` is the setting of the same name: with it off, a recorded
    /// entry the registry no longer holds is kept and nothing is rewritten.
    #[must_use]
    pub fn new(preference: Preference, self_heal: bool) -> Self {
        Self {
            preference,
            self_heal,
            recorded: BTreeMap::new(),
            rewrite_pending: false,
        }
    }

    /// Reads a record document into a selection.
    ///
    /// The pointer key is skipped. A value that is empty, or spells the word
    /// `null` in any mixture of cases, records that the tag has no
    /// representative and is chosen again rather than answered with.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError::Shape`] when the document is not a flat object.
    pub fn read(
        preference: Preference,
        self_heal: bool,
        document: &JsonValue,
    ) -> Result<Self, RecordError> {
        let Some(entries) = document.as_object() else {
            return Err(RecordError::Shape);
        };
        let mut selection = Self::new(preference, self_heal);
        for (tag, value) in entries {
            if tag == POINTER_KEY {
                continue;
            }
            if value.is_null() {
                selection.recorded.insert(tag.clone(), Recorded::Absent);
                continue;
            }
            let Some(text) = value.as_str() else {
                return Err(RecordError::Shape);
            };
            let text = text.trim();
            if text.is_empty() || text.eq_ignore_ascii_case("null") {
                selection.recorded.insert(tag.clone(), Recorded::Absent);
                continue;
            }
            if let Ok(key) = text.parse::<RegistryKey>() {
                selection.recorded.insert(tag.clone(), Recorded::Entry(key));
            } else {
                selection.recorded.insert(tag.clone(), Recorded::Absent);
                selection.rewrite_pending |= self_heal;
            }
        }
        Ok(selection)
    }

    /// Reads a record, falling back to an empty one when it cannot be read.
    ///
    /// The original leaves the table empty *and* stops selecting, so every
    /// tag-resulted recipe in the installation silently has no result until the
    /// record is fixed. This selects instead, which is a recorded divergence.
    #[must_use]
    pub fn read_or_select(preference: Preference, self_heal: bool, document: &JsonValue) -> Self {
        Self::read(preference.clone(), self_heal, document)
            .unwrap_or_else(|_| Self::new(preference, self_heal))
    }

    /// Returns whether the self-heal setting is on for this selection.
    #[must_use]
    pub const fn self_heal(&self) -> bool {
        self.self_heal
    }

    /// Returns the preference order in force.
    #[must_use]
    pub const fn preference(&self) -> &Preference {
        &self.preference
    }

    /// Returns whether the record should be written back.
    #[must_use]
    pub const fn rewrite_pending(&self) -> bool {
        self.rewrite_pending
    }

    /// Returns what is recorded for a tag, without selecting.
    #[must_use]
    pub fn recorded(&self, tag: &TagId) -> Option<&Recorded> {
        self.recorded.get(&tag.to_string())
    }

    /// Drops every recorded entry naming something the registry no longer holds.
    ///
    /// This is the self-heal setting's behaviour, so with the setting off it
    /// drops nothing and asks for no rewrite. It answers whether anything
    /// changed, which is what decides the one rewrite afterwards.
    pub fn heal(&mut self, present: &dyn Fn(&RegistryKey) -> bool) -> bool {
        if !self.self_heal {
            return false;
        }
        let mut healed = false;
        for value in self.recorded.values_mut() {
            if let Recorded::Entry(key) = value
                && !present(key)
            {
                *value = Recorded::Absent;
                healed = true;
            }
        }
        self.rewrite_pending |= healed;
        healed
    }

    /// Returns the entry this tag stands for, recording a fresh choice.
    ///
    /// A tag the unification path claims is answered from there and nothing is
    /// recorded for it, so a later unification change is still honoured. A tag
    /// already recorded keeps its recorded answer whatever rule produced it.
    /// Only a tag asked for the first time reaches the rule below.
    pub fn representative(
        &mut self,
        tag: &TagId,
        members: &[RegistryKey],
        unification: Option<&dyn Unification>,
    ) -> Option<RegistryKey> {
        if let Some(unified) = unification.and_then(|source| source.preferred(tag)) {
            return Some(unified);
        }
        let key = tag.to_string();
        if let Some(Recorded::Entry(recorded)) = self.recorded.get(&key) {
            return Some(recorded.clone());
        }
        let chosen = self.choose(members)?;
        self.recorded.insert(key, Recorded::Entry(chosen.clone()));
        self.rewrite_pending = true;
        Some(chosen)
    }

    /// Applies the selection rule to a tag's members without recording anything.
    #[must_use]
    pub fn choose(&self, members: &[RegistryKey]) -> Option<RegistryKey> {
        members
            .iter()
            .min_by(|left, right| {
                self.preference
                    .rank(left.id().namespace().as_str())
                    .cmp(&self.preference.rank(right.id().namespace().as_str()))
                    .then_with(|| left.to_string().cmp(&right.to_string()))
            })
            .cloned()
    }

    /// Writes the record back out, pointer key included.
    #[must_use]
    pub fn write(&self, pointer: &str) -> JsonValue {
        let mut document = serde_map(pointer);
        for (tag, value) in &self.recorded {
            let text = match value {
                Recorded::Entry(key) => key.to_string(),
                Recorded::Absent => String::new(),
            };
            document.insert(tag.clone(), JsonValue::String(text));
        }
        JsonValue::Object(document.into_iter().collect())
    }
}

fn serde_map(pointer: &str) -> BTreeMap<String, JsonValue> {
    let mut document = BTreeMap::new();
    document.insert(
        POINTER_KEY.to_owned(),
        JsonValue::String(pointer.to_owned()),
    );
    document
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravel_sdk::content::{ContentId, ContentKind};
    use ravel_sdk::json;

    fn key(namespace: &str, path: &str) -> RegistryKey {
        let namespace = namespace.parse().expect("a legal namespace");
        RegistryKey::new(
            ContentKind::Item,
            ContentId::new(namespace, path).expect("a legal path"),
        )
    }

    fn tag(namespace: &str, path: &str) -> TagId {
        let namespace = namespace.parse().expect("a legal namespace");
        TagId::new(
            ContentKind::Item,
            ContentId::new(namespace, path).expect("a legal path"),
        )
        .expect("a legal tag")
    }

    struct Unified(RegistryKey);

    impl Unification for Unified {
        fn preferred(&self, _tag: &TagId) -> Option<RegistryKey> {
            Some(self.0.clone())
        }
    }

    #[test]
    fn the_preference_order_decides_before_the_identifier_does() {
        let selection = Selection::new(Preference::parse("second,first"), true);
        let members = [key("first", "ingot"), key("second", "ingot")];
        assert_eq!(selection.choose(&members), Some(key("second", "ingot")));
    }

    #[test]
    fn an_unlisted_namespace_comes_after_every_listed_one() {
        let selection = Selection::new(Preference::parse("listed"), true);
        let members = [key("aaa", "ingot"), key("listed", "zzz")];
        assert_eq!(selection.choose(&members), Some(key("listed", "zzz")));
    }

    #[test]
    fn a_tie_is_broken_by_full_identifier_and_not_by_member_order() {
        let selection = Selection::new(Preference::default(), true);
        let forwards = [key("beta", "one"), key("alpha", "two")];
        let backwards = [key("alpha", "two"), key("beta", "one")];
        assert_eq!(selection.choose(&forwards), Some(key("alpha", "two")));
        assert_eq!(selection.choose(&backwards), Some(key("alpha", "two")));
    }

    #[test]
    fn a_tag_with_no_member_has_no_representative() {
        let mut selection = Selection::new(Preference::default(), true);
        assert_eq!(
            selection.representative(&tag("owner", "empty"), &[], None),
            None
        );
    }

    #[test]
    fn a_unified_tag_is_answered_from_there_and_nothing_is_recorded() {
        let mut selection = Selection::new(Preference::default(), true);
        let unification = Unified(key("unifier", "chosen"));
        let subject = tag("owner", "ingot");

        assert_eq!(
            selection.representative(&subject, &[key("alpha", "one")], Some(&unification)),
            Some(key("unifier", "chosen"))
        );
        assert!(selection.recorded(&subject).is_none());
        assert!(!selection.rewrite_pending());
    }

    #[test]
    fn a_recorded_answer_is_kept_whatever_rule_produced_it() {
        let subject = tag("owner", "ingot");
        let document =
            json!({ POINTER_KEY: "docs", subject.to_string(): key("odd", "legacy").to_string() });
        let mut selection = Selection::read(Preference::parse("alpha"), true, &document)
            .expect("the record is a flat object");

        assert_eq!(
            selection.representative(&subject, &[key("alpha", "one")], None),
            Some(key("odd", "legacy"))
        );
    }

    #[test]
    fn the_two_absent_spellings_make_the_tag_be_chosen_again() {
        let one = tag("owner", "one");
        let two = tag("owner", "two");
        let document = json!({ one.to_string(): "", two.to_string(): "NuLl" });
        let mut selection =
            Selection::read(Preference::default(), true, &document).expect("a flat object");

        assert_eq!(selection.recorded(&one), Some(&Recorded::Absent));
        assert_eq!(selection.recorded(&two), Some(&Recorded::Absent));
        assert_eq!(
            selection.representative(&one, &[key("alpha", "pick")], None),
            Some(key("alpha", "pick"))
        );
    }

    #[test]
    fn the_pointer_key_is_skipped_and_written_back() {
        let document = json!({ POINTER_KEY: "docs" });
        let selection =
            Selection::read(Preference::default(), true, &document).expect("a flat object");
        assert!(selection.recorded.is_empty());
        assert_eq!(selection.write("docs")[POINTER_KEY], "docs");
    }

    #[test]
    fn healing_drops_an_entry_the_registry_no_longer_holds_and_asks_for_one_rewrite() {
        let subject = tag("owner", "ingot");
        let document = json!({ subject.to_string(): key("gone", "entry").to_string() });
        let mut selection =
            Selection::read(Preference::default(), true, &document).expect("a flat object");

        assert!(selection.heal(&|entry| entry.id().namespace().as_str() != "gone"));
        assert_eq!(selection.recorded(&subject), Some(&Recorded::Absent));
        assert!(selection.rewrite_pending());
        assert!(
            !selection.heal(&|_| true),
            "a second pass has nothing left to heal"
        );
    }

    #[test]
    fn a_record_that_is_not_a_flat_object_is_refused_and_selection_still_answers() {
        let document = json!(["not", "an", "object"]);
        assert_eq!(
            Selection::read(Preference::default(), true, &document).map(|_| ()),
            Err(RecordError::Shape)
        );
        let mut selection = Selection::read_or_select(Preference::default(), true, &document);
        assert_eq!(
            selection.representative(&tag("owner", "ingot"), &[key("alpha", "one")], None),
            Some(key("alpha", "one")),
            "an unreadable record does not leave every tag-resulted recipe without a result"
        );
    }

    #[test]
    fn with_self_heal_off_an_entry_the_registry_lost_is_kept_and_nothing_is_rewritten() {
        let subject = tag("owner", "ingot");
        let document = json!({ subject.to_string(): key("gone", "entry").to_string() });
        let mut selection =
            Selection::read(Preference::default(), false, &document).expect("a flat object");

        assert!(!selection.heal(&|entry| entry.id().namespace().as_str() != "gone"));
        assert_eq!(
            selection.recorded(&subject),
            Some(&Recorded::Entry(key("gone", "entry"))),
            "the setting being off is what keeps the recorded answer"
        );
        assert!(!selection.rewrite_pending());
        assert_eq!(
            selection.representative(&subject, &[key("alpha", "one")], None),
            Some(key("gone", "entry")),
            "the kept answer is still what the tag stands for"
        );
    }

    #[test]
    fn an_unreadable_entry_asks_for_a_rewrite_only_where_self_heal_is_on() {
        let subject = tag("owner", "ingot");
        let document = json!({ subject.to_string(): "not an identifier" });

        let healing =
            Selection::read(Preference::default(), true, &document).expect("a flat object");
        assert_eq!(healing.recorded(&subject), Some(&Recorded::Absent));
        assert!(healing.rewrite_pending());

        let holding =
            Selection::read(Preference::default(), false, &document).expect("a flat object");
        assert_eq!(holding.recorded(&subject), Some(&Recorded::Absent));
        assert!(!holding.rewrite_pending());
    }

    #[test]
    fn a_bare_null_means_no_representative_and_keeps_the_rest_of_the_record() {
        let bare = tag("owner", "bare");
        let held = tag("owner", "held");
        let document = json!({
            bare.to_string(): JsonValue::Null,
            held.to_string(): key("alpha", "one").to_string(),
        });
        let selection =
            Selection::read(Preference::default(), true, &document).expect("a flat object");

        assert_eq!(selection.recorded(&bare), Some(&Recorded::Absent));
        assert_eq!(
            selection.recorded(&held),
            Some(&Recorded::Entry(key("alpha", "one"))),
            "one unanswered tag does not discard every other recorded answer"
        );
    }

    #[test]
    fn a_fresh_choice_is_recorded_so_it_does_not_move_afterwards() {
        let subject = tag("owner", "ingot");
        let mut selection = Selection::new(Preference::parse("beta"), true);
        let first =
            selection.representative(&subject, &[key("alpha", "one"), key("beta", "two")], None);
        assert_eq!(first, Some(key("beta", "two")));

        let mut later = Selection::new(Preference::parse("alpha"), true);
        later.recorded = selection.recorded.clone();
        assert_eq!(
            later.representative(&subject, &[key("alpha", "one"), key("beta", "two")], None),
            first,
            "a recorded answer survives a change to the preference order"
        );
    }

    #[test]
    fn a_repeated_or_blank_namespace_does_not_shift_the_order() {
        let preference = Preference::parse(" alpha , , alpha , beta ");
        assert_eq!(preference.namespaces(), ["alpha", "beta"]);
    }
}
