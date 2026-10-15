//! Finding a model declaration, the specification's §4.1.
//!
//! Two paths reach a declaration and only the first is portable.
//!
//! **Path one** collects every JSON document under one fixed top-level
//! directory across all loaded resource packs, keyed by the identifier the file
//! path yields. It is what this module implements.
//!
//! **Path two** rewrites the requested identifier into the path of the game's
//! own state-to-model file and walks every candidate document there in pack
//! order. It needs a reader over the model bakery's already-parsed documents,
//! which Ravel does not publish, so it is recorded as host-blocked and
//! [`Catalogue::find`] says where the fall-through would go.
//!
//! # Path one short-circuits, and that is load-bearing
//!
//! If the directory holds a document under the requested identifier at all,
//! that document is checked once and its verdict is final. A missing or
//! differing kind value ends the lookup with no result rather than falling
//! through. A counterpart that treated path one as a fallible first attempt
//! would resolve declarations the original leaves unresolved, which is a
//! behaviour difference a pack author would see.

use std::collections::BTreeMap;

use ravel_sdk::JsonValue;

use crate::conditions::qualify;

/// The most documents one reload may leave in the table.
///
/// A resource pack decides how many files it ships, so this is untrusted input.
/// The cap is far above any pack a dependent writes.
pub const MAX_DOCUMENTS: usize = 4_096;

/// The top-level directory declaration documents are collected from.
pub const DIRECTORY: &str = "joined_models";

/// The declaration key whose value names the model kind.
pub const KIND_KEY: &str = "shape";

/// What a lookup answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup<'a> {
    /// A document under that identifier declares that kind.
    Found(&'a JsonValue),
    /// A document under that identifier exists and does not declare that kind.
    ///
    /// The lookup ends here. Path two is not tried.
    Refused,
    /// No document under that identifier. Path two would be tried here.
    Absent,
}

impl<'a> Lookup<'a> {
    /// Returns the document, if one was found.
    #[must_use]
    pub const fn document(self) -> Option<&'a JsonValue> {
        match self {
            Self::Found(document) => Some(document),
            Self::Refused | Self::Absent => None,
        }
    }

    /// Returns whether the fall-through to path two is reached.
    ///
    /// Only [`Lookup::Absent`] reaches it. This is the short-circuit written
    /// down as one function so a test can pin it.
    #[must_use]
    pub const fn falls_through(self) -> bool {
        matches!(self, Self::Absent)
    }
}

/// What a reload left behind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RebuildReport {
    /// How many documents the table now holds.
    pub held: usize,
    /// How many the previous table held and this one does not.
    pub dropped: usize,
    /// Documents skipped for an identifier that is not a name.
    pub unreadable: usize,
    /// Documents skipped because the table was already full.
    pub over_cap: usize,
}

/// The documents one reload collected, keyed by identifier.
#[derive(Clone, Debug, Default)]
pub struct Catalogue {
    documents: BTreeMap<String, JsonValue>,
}

impl Catalogue {
    /// Builds an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the resource path a declaration for `identifier` is read from.
    ///
    /// Returns `None` when the identifier is not a name.
    #[must_use]
    pub fn path_of(identifier: &str) -> Option<String> {
        let qualified = qualify(identifier)?;
        let (namespace, path) = qualified.split_once(':')?;
        Some(format!("{namespace}/{DIRECTORY}/{path}.json"))
    }

    /// Replaces the whole table with what this reload collected.
    ///
    /// A reload *replaces*: a document a pack shipped before and does not ship
    /// now is gone rather than left behind. That is what makes disabling a pack
    /// take effect, and it is why this takes the whole set rather than adding
    /// to one.
    pub fn rebuild<I>(&mut self, documents: I) -> RebuildReport
    where
        I: IntoIterator<Item = (String, JsonValue)>,
    {
        let previous = std::mem::take(&mut self.documents);
        let mut report = RebuildReport::default();
        for (identifier, document) in documents {
            if self.documents.len() >= MAX_DOCUMENTS {
                report.over_cap += 1;
                continue;
            }
            let Some(qualified) = qualify(&identifier) else {
                report.unreadable += 1;
                continue;
            };
            self.documents.insert(qualified, document);
        }
        report.held = self.documents.len();
        report.dropped = previous
            .keys()
            .filter(|held| !self.documents.contains_key(*held))
            .count();
        report
    }

    /// Returns how many documents the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Returns whether the table holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Looks a declaration up, checking it once and answering finally.
    ///
    /// A document is accepted only if the kind key is present *and* its value
    /// equals the kind being asked about.
    #[must_use]
    pub fn find(&self, identifier: &str, kind: &str) -> Lookup<'_> {
        let Some(qualified) = qualify(identifier) else {
            return Lookup::Absent;
        };
        let Some(document) = self.documents.get(&qualified) else {
            return Lookup::Absent;
        };
        match document.get(KIND_KEY).and_then(JsonValue::as_str) {
            Some(declared) if declared == kind => Lookup::Found(document),
            _ => Lookup::Refused,
        }
    }

    /// Returns the identifiers the table holds, in a deterministic order.
    pub fn identifiers(&self) -> impl Iterator<Item = &str> {
        self.documents.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::{JsonValue, json};

    use super::{Catalogue, DIRECTORY, KIND_KEY, Lookup, MAX_DOCUMENTS};

    fn declaring(kind: &str) -> JsonValue {
        json!({ "shape": kind, "textures": { "alone": "base:one" } })
    }

    #[test]
    fn a_document_is_accepted_only_when_it_declares_the_kind_being_asked_about() {
        let mut catalogue = Catalogue::new();
        catalogue.rebuild([("base:wall".to_owned(), declaring("joined_cube"))]);

        assert!(matches!(
            catalogue.find("base:wall", "joined_cube"),
            Lookup::Found(_)
        ));
        assert_eq!(
            catalogue.find("base:wall", "joined_mat"),
            Lookup::Refused,
            "a differing kind value ends the lookup"
        );
    }

    #[test]
    fn only_an_absent_document_falls_through_to_the_second_path() {
        let mut catalogue = Catalogue::new();
        catalogue.rebuild([
            ("base:wall".to_owned(), declaring("joined_cube")),
            ("base:blank".to_owned(), json!({ "textures": {} })),
            ("base:wrong".to_owned(), json!({ "shape": 7 })),
        ]);

        assert!(!catalogue.find("base:wall", "joined_cube").falls_through());
        assert!(
            !catalogue.find("base:wall", "joined_mat").falls_through(),
            "a document that exists and differs is final"
        );
        assert!(
            !catalogue.find("base:blank", "joined_cube").falls_through(),
            "so is one with no kind key at all"
        );
        assert!(
            !catalogue.find("base:wrong", "joined_cube").falls_through(),
            "so is one whose kind value is not a string"
        );
        assert!(
            catalogue
                .find("base:nothing", "joined_cube")
                .falls_through(),
            "only an identifier nothing shipped reaches the second path"
        );
    }

    #[test]
    fn a_reload_replaces_the_table_rather_than_adding_to_it() {
        let mut catalogue = Catalogue::new();
        catalogue.rebuild([
            ("base:first".to_owned(), declaring("joined_cube")),
            ("base:second".to_owned(), declaring("joined_cube")),
        ]);
        assert_eq!(catalogue.len(), 2);

        let report = catalogue.rebuild([("base:second".to_owned(), declaring("joined_mat"))]);
        assert_eq!(report.held, 1);
        assert_eq!(report.dropped, 1, "the pack that stopped shipping is gone");
        assert_eq!(catalogue.len(), 1);
        assert!(catalogue.find("base:first", "joined_cube").falls_through());
        assert!(matches!(
            catalogue.find("base:second", "joined_mat"),
            Lookup::Found(_)
        ));

        let report = catalogue.rebuild(std::iter::empty());
        assert_eq!(report.held, 0);
        assert!(
            catalogue.is_empty(),
            "and an empty reload empties the table"
        );
    }

    #[test]
    fn a_reload_shipping_more_than_the_cap_is_bounded_rather_than_held() {
        let mut catalogue = Catalogue::new();
        let report = catalogue.rebuild(
            (0..MAX_DOCUMENTS * 4)
                .map(|index| (format!("base:d{index}"), declaring("joined_cube"))),
        );

        assert_eq!(report.held, MAX_DOCUMENTS);
        assert_eq!(report.over_cap, MAX_DOCUMENTS * 3);
        assert_eq!(catalogue.len(), MAX_DOCUMENTS);
    }

    #[test]
    fn an_identifier_that_is_not_a_name_is_counted_and_never_looked_up() {
        let mut catalogue = Catalogue::new();
        let report = catalogue.rebuild([
            ("not a name".to_owned(), declaring("joined_cube")),
            ("base:good".to_owned(), declaring("joined_cube")),
        ]);
        assert_eq!(report.unreadable, 1);
        assert_eq!(report.held, 1);
        assert_eq!(catalogue.find("not a name", "joined_cube"), Lookup::Absent);
        assert_eq!(
            catalogue.identifiers().collect::<Vec<_>>(),
            ["base:good"],
            "and the identifiers come back qualified and ordered"
        );
    }

    #[test]
    fn a_bare_identifier_is_qualified_the_same_way_going_in_and_coming_out() {
        let mut catalogue = Catalogue::new();
        catalogue.rebuild([("wall".to_owned(), declaring("joined_cube"))]);
        assert!(matches!(
            catalogue.find("base:wall", "joined_cube"),
            Lookup::Found(_)
        ));
        assert!(matches!(
            catalogue.find("wall", "joined_cube"),
            Lookup::Found(_)
        ));
    }

    #[test]
    fn the_reload_path_names_the_directory_and_refuses_a_name_it_cannot_read() {
        let expected = format!("base/{DIRECTORY}/wall.json");
        assert_eq!(
            Catalogue::path_of("base:wall").as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(
            Catalogue::path_of("wall").as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(Catalogue::path_of("not a name"), None);
        assert_eq!(
            KIND_KEY, "shape",
            "the key the tests above write by hand is the one the module publishes"
        );
    }
}
