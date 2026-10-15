//! Named booleans whose value is a live predicate over resolved configuration.
//!
//! A flag is published into one table under an identifier. A dependent that only
//! knows the identifier holds a reference instead, and the reference binds to
//! whatever the table holds at the moment it is first asked.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::{JsonValue, ModId, ModIdError, json};

/// What a flag answers with, asked afresh on every ask.
pub type Predicate = Arc<dyn Fn() -> bool + Send + Sync>;

/// The identity a flag is published and referenced under.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlagId(ContentId);

impl FlagId {
    /// Builds an identity from an owning namespace and a name.
    ///
    /// # Errors
    ///
    /// Returns [`FlagIdError`] when the namespace is not a legal mod id or the
    /// name is not a legal content path.
    pub fn new(namespace: &str, name: &str) -> Result<Self, FlagIdError> {
        let namespace: ModId = namespace.parse().map_err(FlagIdError::Namespace)?;
        let id = ContentId::new(namespace, name).map_err(FlagIdError::Name)?;
        Ok(Self(id))
    }

    /// Reads an identity written as `namespace:name`.
    ///
    /// # Errors
    ///
    /// Returns [`FlagIdError::Shape`] when the text does not carry exactly one
    /// separator, and otherwise whatever the two halves refuse.
    pub fn parse(text: &str) -> Result<Self, FlagIdError> {
        let mut halves = text.split(':');
        let (Some(namespace), Some(name), None) = (halves.next(), halves.next(), halves.next())
        else {
            return Err(FlagIdError::Shape);
        };
        Self::new(namespace, name)
    }

    /// Returns the namespace that owns this flag.
    #[must_use]
    pub const fn namespace(&self) -> &ModId {
        self.0.namespace()
    }

    /// Returns the name this flag carries inside its namespace.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.path()
    }
}

impl fmt::Display for FlagId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.0.namespace(), self.0.path())
    }
}

/// Why an identity was refused.
#[derive(Debug)]
pub enum FlagIdError {
    /// The text was not one namespace and one name separated by a colon.
    Shape,
    /// The namespace is not a legal mod id.
    Namespace(ModIdError),
    /// The name is not a legal content path.
    Name(ContentIdError),
}

impl fmt::Display for FlagIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape => write!(formatter, "a flag identity is written `namespace:name`"),
            Self::Namespace(error) => write!(formatter, "the namespace is refused: {error}"),
            Self::Name(error) => write!(formatter, "the name is refused: {error}"),
        }
    }
}

impl std::error::Error for FlagIdError {}

/// The one table a flag is published into.
#[derive(Clone, Default)]
pub struct FlagTable {
    published: BTreeMap<FlagId, Predicate>,
}

impl fmt::Debug for FlagTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FlagTable")
            .field("published", &self.published.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl FlagTable {
    /// Builds a table holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes a predicate under an identity, replacing any earlier one.
    ///
    /// A duplicate is not refused; the earlier predicate is returned so a caller
    /// that cares can say a replacement happened.
    pub fn publish(&mut self, id: FlagId, predicate: Predicate) -> Option<Predicate> {
        self.published.insert(id, predicate)
    }

    /// Publishes a predicate that always answers the same way.
    pub fn publish_constant(&mut self, id: FlagId, value: bool) -> Option<Predicate> {
        self.publish(id, Arc::new(move || value))
    }

    /// Returns whether an identity has been published.
    #[must_use]
    pub fn holds(&self, id: &FlagId) -> bool {
        self.published.contains_key(id)
    }

    /// Returns every published identity, in identifier order.
    pub fn published(&self) -> impl Iterator<Item = &FlagId> + '_ {
        self.published.keys()
    }

    /// Asks a published flag directly, without binding a reference to it.
    #[must_use]
    pub fn enabled(&self, id: &FlagId) -> bool {
        self.published.get(id).is_some_and(|predicate| predicate())
    }
}

/// A dependent's handle on a flag it knows only by identity.
#[derive(Clone)]
pub struct FlagReference {
    id: FlagId,
    bound: Option<Predicate>,
    missing_at_bind: bool,
}

impl fmt::Debug for FlagReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FlagReference")
            .field("id", &self.id)
            .field("bound", &self.bound.is_some())
            .field("missing_at_bind", &self.missing_at_bind)
            .finish()
    }
}

impl FlagReference {
    /// Builds a reference that carries no value until it is first asked.
    #[must_use]
    pub const fn new(id: FlagId) -> Self {
        Self {
            id,
            bound: None,
            missing_at_bind: false,
        }
    }

    /// Returns the identity this reference names.
    #[must_use]
    pub const fn id(&self) -> &FlagId {
        &self.id
    }

    /// Returns whether the first ask has happened.
    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.bound.is_some()
    }

    /// Asks the flag, binding permanently to the table on the first ask.
    ///
    /// A later publication under the same identity is not seen: the reference
    /// adopts one predicate and never looks again. An identity absent at the
    /// first ask binds a constant-false predicate, which is the case
    /// [`Self::permanently_disabled`] reports.
    pub fn enabled(&mut self, table: &FlagTable) -> bool {
        if self.bound.is_none() {
            if let Some(predicate) = table.published.get(&self.id) {
                self.bound = Some(Arc::clone(predicate));
            } else {
                self.missing_at_bind = true;
                self.bound = Some(Arc::new(|| false));
            }
        }
        self.bound.as_ref().is_some_and(|predicate| predicate())
    }

    /// Returns whether this reference bound to nothing and can never answer true.
    ///
    /// The original neither logs nor refuses this case; reporting it is a
    /// deliberate divergence so a dependent can diagnose a flag stuck off.
    #[must_use]
    pub const fn permanently_disabled(&self) -> bool {
        self.missing_at_bind
    }
}

/// The declaration a data-pack condition would carry, as data.
///
/// Ravel publishes no conditioned data-pack loading, so this is the shape and
/// nothing evaluates it at load. It is here so a dependent can write the
/// condition down and a test can hold its shape.
#[must_use]
pub fn condition_declaration(id: &FlagId) -> JsonValue {
    json!({ "condition": "flag_enabled", "flag": id.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn id(name: &str) -> FlagId {
        FlagId::parse(name).expect("the test identities are legal")
    }

    #[test]
    fn an_identity_is_one_namespace_and_one_name() {
        let parsed = id("rvl_042_counterpart:tooltip");
        assert_eq!(parsed.namespace().as_str(), "rvl_042_counterpart");
        assert_eq!(parsed.name(), "tooltip");
        assert_eq!(parsed.to_string(), "rvl_042_counterpart:tooltip");
    }

    #[test]
    fn a_malformed_identity_is_refused_rather_than_accepted() {
        assert!(FlagId::parse("no_separator").is_err());
        assert!(FlagId::parse("too:many:parts").is_err());
        assert!(FlagId::parse("Upper:name").is_err());
        assert!(FlagId::parse("owner:").is_err());
    }

    #[test]
    fn a_second_publication_under_one_identity_replaces_the_first() {
        let mut table = FlagTable::new();
        assert!(table.publish_constant(id("owner:one"), false).is_none());
        assert!(table.publish_constant(id("owner:one"), true).is_some());
        assert!(table.enabled(&id("owner:one")));
        assert_eq!(table.published().count(), 1);
    }

    #[test]
    fn a_reference_asked_before_publication_is_permanently_disabled() {
        let mut table = FlagTable::new();
        let mut reference = FlagReference::new(id("owner:late"));

        assert!(!reference.enabled(&table));
        assert!(reference.permanently_disabled());

        table.publish_constant(id("owner:late"), true);
        assert!(
            !reference.enabled(&table),
            "the first ask bound the reference and it never looks again"
        );
        assert!(reference.permanently_disabled());
    }

    #[test]
    fn a_reference_bound_to_a_published_flag_ignores_a_later_replacement() {
        let mut table = FlagTable::new();
        table.publish_constant(id("owner:early"), true);
        let mut reference = FlagReference::new(id("owner:early"));

        assert!(reference.enabled(&table));
        table.publish_constant(id("owner:early"), false);
        assert!(reference.enabled(&table));
        assert!(!reference.permanently_disabled());
    }

    #[test]
    fn a_bound_predicate_is_evaluated_live_on_every_ask() {
        let live = Arc::new(AtomicBool::new(false));
        let reader = Arc::clone(&live);
        let mut table = FlagTable::new();
        table.publish(
            id("owner:live"),
            Arc::new(move || reader.load(Ordering::SeqCst)),
        );

        let mut reference = FlagReference::new(id("owner:live"));
        assert!(!reference.enabled(&table));
        live.store(true, Ordering::SeqCst);
        assert!(reference.enabled(&table));
    }

    #[test]
    fn a_reference_carries_no_value_until_it_is_asked() {
        let reference = FlagReference::new(id("owner:unasked"));
        assert!(!reference.is_bound());
        assert!(!reference.permanently_disabled());
    }

    #[test]
    fn the_condition_declaration_names_the_flag_it_holds_on() {
        let declaration = condition_declaration(&id("owner:gate"));
        assert_eq!(declaration["condition"], "flag_enabled");
        assert_eq!(declaration["flag"], "owner:gate");
    }
}
