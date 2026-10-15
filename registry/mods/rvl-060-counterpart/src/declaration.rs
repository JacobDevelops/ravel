//! Declaring content, from §4.
//!
//! A dependent declares content by asking a namespace-scoped collection for an
//! entry: it supplies a path and receives a handle. The observable contract of a
//! handle is that it yields the registered object and its full identifier, and
//! that asking for the object before registration has run is an error rather
//! than an absent value. There is one handle: the original spells several handle
//! classes apart by what each one yields, and no observable behaviour follows
//! from the distinction.
//!
//! The handles themselves are Ravel's. What has behaviour of its own is the
//! nesting and the grouping, and that is what this module is:
//!
//! - a nested collection forwards every declaration to its parent and
//!   additionally remembers it, so a dependent can ask "everything I declared in
//!   this group" without a second bookkeeping structure;
//! - a further nesting keys those groups by an enumeration value;
//! - initialisation is explicit and per-collection, and a nested collection's
//!   own initialisation is deliberately inert;
//! - exactly one extension point exists, for fluids, and asking for any other
//!   kind is refused with an error rather than a plain collection.
//!
//! # The change declared here
//!
//! **A group with no declaration is empty rather than absent.** The original's
//! grouped-by-enumeration accessor returns the group for a value that has had at
//! least one declaration and null otherwise, with no empty-collection fallback.
//! [`Registry::variant`] answers an empty slice.

use std::collections::BTreeMap;

use ravel_sdk::ModId;
use ravel_sdk::content::{
    ContentDeclaration, ContentId, ContentIdError, ContentKind, DeclarationError, RegistryKey,
};

/// The kinds the loader models specially rather than as a plain registry.
///
/// Exactly one is defined at the reviewed revision. Any other kind is refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Special {
    /// The fluid registry of §9.
    Fluid,
}

impl Special {
    /// Every special kind, which is one.
    pub const ALL: [Self; 1] = [Self::Fluid];

    /// Returns the name this kind is asked for by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fluid => "fluid",
        }
    }

    /// Returns the special kind a name asks for.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::NotSpecial`] for any other name. The refusal is
    /// the point: a dependent that asks for a kind the loader does not model
    /// specially gets an error rather than a plain collection that would then
    /// register nothing.
    pub fn named(name: &str) -> Result<Self, RegistryError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
            .ok_or_else(|| RegistryError::NotSpecial(name.to_owned()))
    }
}

/// What a declaration hands back.
///
/// The identifier is available from the moment the entry is declared. The
/// *object* is Ravel's and exists only once the registration barrier has run, so
/// asking for it before initialisation is [`RegistryError::NotRegistered`] rather
/// than an absent value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Handle {
    key: RegistryKey,
}

impl Handle {
    /// Returns the entry's full identifier.
    #[must_use]
    pub const fn id(&self) -> &RegistryKey {
        &self.key
    }

    /// Returns the registered object's identifier once registration has run.
    ///
    /// **Host-blocked in one narrow place.** Ravel's registration barrier
    /// claims identifiers and hands back no object, so what a resolved handle
    /// yields here is the identifier the object was registered under. The
    /// before-and-after rule is the observable part and is what is reproduced.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::NotRegistered`] when the owning collection has not
    /// been initialised.
    pub fn resolve<'a>(&'a self, registry: &Registry) -> Result<&'a RegistryKey, RegistryError> {
        if registry.is_initialised() {
            Ok(&self.key)
        } else {
            Err(RegistryError::NotRegistered(self.key.to_string()))
        }
    }
}

/// The most entries one namespace will declare.
///
/// The two dependents the graph records register large numbers of blocks and
/// items, so this is generous. It is here because a declaration loop is a
/// dependent's and an unbounded one is an allocation this package would carry.
pub const MAX_ENTRIES: usize = 65_536;

/// The most groups, and the most variants inside one group.
pub const MAX_GROUPS: usize = 1024;

/// Why a declaration was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// The name asks for a kind the loader does not model specially.
    NotSpecial(String),
    /// The object was asked for before registration ran.
    NotRegistered(String),
    /// The declaration itself was refused.
    Declaration(String),
    /// The collection already holds [`MAX_ENTRIES`].
    Full(usize),
    /// The collection already holds [`MAX_GROUPS`] groups.
    TooManyGroups(usize),
    /// Initialisation was asked for twice.
    AlreadyInitialised,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSpecial(name) => write!(
                formatter,
                "`{name}` names no registry the loader models specially"
            ),
            Self::NotRegistered(key) => {
                write!(formatter, "`{key}` was asked for before registration ran")
            }
            Self::Declaration(detail) => write!(formatter, "{detail}"),
            Self::Full(ceiling) => write!(formatter, "a namespace holds {ceiling} entries already"),
            Self::TooManyGroups(ceiling) => {
                write!(formatter, "a namespace holds {ceiling} groups already")
            }
            Self::AlreadyInitialised => {
                write!(
                    formatter,
                    "a collection is initialised once and was asked twice"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// A namespace-scoped collection of declarations.
///
/// This is the root. Groups and variants live inside it rather than as separate
/// values, because a nested collection forwards everything to its parent
/// anyway: keeping them here is what makes "forward and additionally remember"
/// one statement rather than two structures that can disagree.
#[derive(Clone, Debug)]
pub struct Registry {
    declaration: ContentDeclaration,
    handles: Vec<Handle>,
    groups: BTreeMap<String, Vec<usize>>,
    variants: BTreeMap<(String, String), Vec<usize>>,
    initialised: bool,
}

impl Registry {
    /// Builds a collection under a namespace.
    #[must_use]
    pub fn new(owner: ModId) -> Self {
        Self {
            declaration: ContentDeclaration::new(owner),
            handles: Vec::new(),
            groups: BTreeMap::new(),
            variants: BTreeMap::new(),
            initialised: false,
        }
    }

    /// Declares an entry at the root.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Full`] at [`MAX_ENTRIES`] and
    /// [`RegistryError::Declaration`] for a path Ravel refuses or an entry already
    /// declared.
    pub fn declare(&mut self, kind: ContentKind, path: &str) -> Result<Handle, RegistryError> {
        if self.handles.len() >= MAX_ENTRIES {
            return Err(RegistryError::Full(MAX_ENTRIES));
        }
        let key = self
            .declaration
            .entry(kind, path)
            .map_err(|error: DeclarationError| RegistryError::Declaration(error.to_string()))?;
        let handle = Handle { key };
        self.handles.push(handle.clone());
        Ok(handle)
    }

    /// Declares an entry inside a named group.
    ///
    /// The declaration reaches the root exactly as an unnested one does, and the
    /// group additionally remembers it.
    ///
    /// # Errors
    ///
    /// Returns what [`Registry::declare`] returns, and
    /// [`RegistryError::TooManyGroups`] at [`MAX_GROUPS`].
    pub fn declare_in(
        &mut self,
        group: &str,
        kind: ContentKind,
        path: &str,
    ) -> Result<Handle, RegistryError> {
        if !self.groups.contains_key(group) && self.groups.len() >= MAX_GROUPS {
            return Err(RegistryError::TooManyGroups(MAX_GROUPS));
        }
        let handle = self.declare(kind, path)?;
        let index = self.handles.len() - 1;
        self.groups.entry(group.to_owned()).or_default().push(index);
        Ok(handle)
    }

    /// Declares an entry inside a group, keyed by an enumeration value.
    ///
    /// The declaration reaches both the group and the root.
    ///
    /// # Errors
    ///
    /// Returns what [`Registry::declare_in`] returns.
    pub fn declare_variant(
        &mut self,
        group: &str,
        variant: &str,
        kind: ContentKind,
        path: &str,
    ) -> Result<Handle, RegistryError> {
        if !self
            .variants
            .contains_key(&(group.to_owned(), variant.to_owned()))
            && self.variants.len() >= MAX_GROUPS
        {
            return Err(RegistryError::TooManyGroups(MAX_GROUPS));
        }
        let handle = self.declare_in(group, kind, path)?;
        let index = self.handles.len() - 1;
        self.variants
            .entry((group.to_owned(), variant.to_owned()))
            .or_default()
            .push(index);
        Ok(handle)
    }

    /// Names an entry in the one registry the loader models specially.
    ///
    /// **Host-blocked in what it claims.** Ravel has no fluid registry, so this
    /// claims nothing at the barrier and hands back the identifier the entry
    /// would carry. Claiming it as a block would put a fluid in front of Ravel's
    /// block barrier: a dependent naming fluid `x` and block `x` would then be
    /// refused for a duplicate it did not write, and the loader would hold a
    /// block that does not exist. What §4 makes observable is the refusal below,
    /// and that is kept.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::NotSpecial`] for any name other than the fluid
    /// one, which is the refusal §4 says to keep, and
    /// [`RegistryError::Declaration`] for a path that is not a legal identifier.
    pub fn declare_special(
        &self,
        kind_name: &str,
        path: &str,
    ) -> Result<(Special, ContentId), RegistryError> {
        let special = Special::named(kind_name)?;
        let id = ContentId::new(self.declaration.owner().clone(), path)
            .map_err(|error: ContentIdError| RegistryError::Declaration(error.to_string()))?;
        Ok((special, id))
    }

    /// Returns every entry declared, in declaration order.
    #[must_use]
    pub fn entries(&self) -> &[RegistryKey] {
        self.declaration.entries()
    }

    /// Returns every handle, in declaration order.
    #[must_use]
    pub fn handles(&self) -> &[Handle] {
        &self.handles
    }

    /// Returns the handles a named group remembers.
    ///
    /// A group nothing declared into is empty rather than absent.
    #[must_use]
    pub fn group(&self, group: &str) -> Vec<&Handle> {
        self.groups.get(group).map_or_else(Vec::new, |indices| {
            indices
                .iter()
                .filter_map(|i| self.handles.get(*i))
                .collect()
        })
    }

    /// Returns the handles one enumeration value's group remembers.
    ///
    /// **Declared change.** The original answers null for a value nothing was
    /// declared under. This answers an empty group.
    #[must_use]
    pub fn variant(&self, group: &str, variant: &str) -> Vec<&Handle> {
        self.variants
            .get(&(group.to_owned(), variant.to_owned()))
            .map_or_else(Vec::new, |indices| {
                indices
                    .iter()
                    .filter_map(|i| self.handles.get(*i))
                    .collect()
            })
    }

    /// Returns the group names, in order.
    #[must_use]
    pub fn group_names(&self) -> Vec<&str> {
        self.groups.keys().map(String::as_str).collect()
    }

    /// Initialises the collection.
    ///
    /// Only the root initialises: [`Registry::initialise_group`] is inert by
    /// design, which is the original's rule and is the reason a dependent's
    /// nested collection never has to be driven.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::AlreadyInitialised`] on a second call.
    pub fn initialise(&mut self) -> Result<(), RegistryError> {
        if self.initialised {
            return Err(RegistryError::AlreadyInitialised);
        }
        self.initialised = true;
        Ok(())
    }

    /// Initialises a nested collection, which does nothing.
    ///
    /// It is here rather than absent because a dependent writes the call, and a
    /// method that is deliberately inert says so where a missing one would look
    /// like an oversight.
    ///
    /// A dependent deliberately discarding the inert result writes
    /// `let _ = registry.initialise_group("nested");`: the call acknowledges
    /// the nested collection without driving its registration.
    #[must_use]
    pub fn initialise_group(&self, group: &str) -> Inert {
        let _ = group;
        Inert
    }

    /// Returns whether registration has run.
    #[must_use]
    pub const fn is_initialised(&self) -> bool {
        self.initialised
    }

    /// Returns the declaration Ravel's barrier claims from.
    #[must_use]
    pub const fn declaration(&self) -> &ContentDeclaration {
        &self.declaration
    }
}

/// What initialising a nested collection produces.
///
/// Nothing. The type exists so the call has a return value that says as much.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Inert;

#[cfg(test)]
mod tests {
    use ravel_sdk::ModId;
    use ravel_sdk::content::ContentKind;

    use super::{Handle, Registry, RegistryError, Special};

    fn registry() -> Registry {
        Registry::new("example".parse::<ModId>().expect("a legal mod id"))
    }

    #[test]
    fn a_declaration_yields_a_handle_carrying_the_full_identifier() {
        let mut registry = registry();
        let handle = registry
            .declare(ContentKind::Block, "lantern")
            .expect("a legal path");
        assert_eq!(handle.id().to_string(), "block:example:lantern");
        assert_eq!(registry.entries().len(), 1);
    }

    #[test]
    fn asking_for_the_object_before_registration_is_an_error_rather_than_an_absent_value() {
        let mut registry = registry();
        let handle = registry
            .declare(ContentKind::Item, "lantern")
            .expect("a legal path");
        assert_eq!(
            handle.resolve(&registry),
            Err(RegistryError::NotRegistered(
                "item:example:lantern".to_owned()
            ))
        );
        registry.initialise().expect("the first call initialises");
        assert_eq!(handle.resolve(&registry), Ok(handle.id()));
    }

    #[test]
    fn a_nested_collection_forwards_to_the_parent_and_additionally_remembers() {
        let mut registry = registry();
        registry
            .declare(ContentKind::Block, "loose")
            .expect("a legal path");
        registry
            .declare_in("stone", ContentKind::Block, "slab")
            .expect("a legal path");
        registry
            .declare_in("stone", ContentKind::Block, "stair")
            .expect("a legal path");

        assert_eq!(registry.entries().len(), 3, "everything reaches the root");
        let group: Vec<String> = registry
            .group("stone")
            .into_iter()
            .map(|handle| handle.id().to_string())
            .collect();
        assert_eq!(group, ["block:example:slab", "block:example:stair"]);
        assert_eq!(registry.group_names(), ["stone"]);
    }

    #[test]
    fn a_group_nothing_was_declared_into_is_empty_rather_than_absent() {
        let registry = registry();
        assert_eq!(registry.group("never").len(), 0);
    }

    #[test]
    fn a_variant_group_keys_declarations_by_an_enumeration_value() {
        let mut registry = registry();
        registry
            .declare_variant("stone", "rough", ContentKind::Block, "rough_slab")
            .expect("a legal path");
        registry
            .declare_variant("stone", "smooth", ContentKind::Block, "smooth_slab")
            .expect("a legal path");

        let rough: Vec<String> = registry
            .variant("stone", "rough")
            .into_iter()
            .map(|handle| handle.id().to_string())
            .collect();
        assert_eq!(rough, ["block:example:rough_slab"]);
        assert_eq!(
            registry.group("stone").len(),
            2,
            "a variant declaration reaches the group as well as the root"
        );
        assert_eq!(registry.entries().len(), 2);
    }

    #[test]
    fn a_variant_nothing_was_declared_under_is_an_empty_group_rather_than_null() {
        let mut registry = registry();
        registry
            .declare_variant("stone", "rough", ContentKind::Block, "rough_slab")
            .expect("a legal path");
        assert_eq!(
            registry.variant("stone", "smooth").len(),
            0,
            "this is the declared change: the original answers null here"
        );
        assert_eq!(registry.variant("absent", "rough").len(), 0);
    }

    #[test]
    fn only_the_root_initialises_and_a_nested_collection_is_inert() {
        let mut registry = registry();
        assert!(!registry.is_initialised());
        assert_eq!(registry.initialise_group("stone"), super::Inert);
        assert!(
            !registry.is_initialised(),
            "a nested collection's own initialisation is deliberately inert"
        );
        registry.initialise().expect("the first call initialises");
        assert!(registry.is_initialised());
        assert_eq!(
            registry.initialise(),
            Err(RegistryError::AlreadyInitialised),
            "nothing is registered twice"
        );
    }

    #[test]
    fn exactly_one_registry_is_modelled_specially_and_every_other_ask_is_refused() {
        assert_eq!(Special::ALL.len(), 1);
        assert_eq!(Special::named("fluid"), Ok(Special::Fluid));
        assert_eq!(
            Special::named("gas"),
            Err(RegistryError::NotSpecial("gas".to_owned())),
            "the refusal is kept: a plain collection here would register nothing"
        );
        let mut registry = registry();
        let (kind, id) = registry
            .declare_special("fluid", "molten_copper")
            .expect("the one special kind");
        assert_eq!(kind, Special::Fluid);
        assert_eq!(id.to_string(), "example:molten_copper");
        assert!(registry.declare_special("gas", "vapour").is_err());
        assert_eq!(
            registry.entries().len(),
            0,
            "a fluid does not reach Ravel's block barrier"
        );
        registry
            .declare(ContentKind::Block, "molten_copper")
            .expect("the same name as a block is a different registry and is free");
        assert!(registry.declare_special("fluid", "..").is_err());
    }

    #[test]
    fn a_duplicate_declaration_is_refused_by_ravels_own_declaration() {
        let mut registry = registry();
        registry
            .declare(ContentKind::Block, "lantern")
            .expect("a legal path");
        assert!(matches!(
            registry.declare(ContentKind::Block, "lantern"),
            Err(RegistryError::Declaration(_))
        ));
        assert!(
            registry.declare(ContentKind::Item, "lantern").is_ok(),
            "the same name in two registries is two claims"
        );
    }

    #[test]
    fn an_illegal_path_is_refused_at_the_declaration_point() {
        let mut registry = registry();
        assert!(matches!(
            registry.declare(ContentKind::Block, "NOT A PATH"),
            Err(RegistryError::Declaration(_))
        ));
    }

    #[test]
    fn the_declaration_ravels_barrier_claims_from_is_the_one_this_module_wrote() {
        let mut registry = registry();
        registry
            .declare_in("stone", ContentKind::Block, "slab")
            .expect("a legal path");
        assert_eq!(registry.declaration().entries(), registry.entries());
        assert_eq!(registry.handles().len(), 1);
    }

    #[test]
    fn a_group_count_past_the_ceiling_is_refused() {
        let mut registry = registry();
        for index in 0..super::MAX_GROUPS {
            registry
                .declare_in(
                    &format!("g{index}"),
                    ContentKind::Block,
                    &format!("b{index}"),
                )
                .expect("a legal path");
        }
        assert_eq!(
            registry.declare_in("one_more", ContentKind::Block, "extra"),
            Err(RegistryError::TooManyGroups(super::MAX_GROUPS))
        );
    }

    #[test]
    fn a_resolved_handle_names_the_entry_it_was_declared_as() {
        let mut registry = registry();
        let handle: Handle = registry
            .declare(ContentKind::Recipe, "smelting")
            .expect("a legal path");
        registry.initialise().expect("the first call initialises");
        assert_eq!(
            handle.resolve(&registry).expect("registration has run"),
            handle.id()
        );
    }
}
