//! The content registration of §4: what a dependent may declare, and what
//! declaring it produces.
//!
//! A dependent declares blocks, items, block-entity types, data-component types
//! and capabilities in one namespace, and running the declarations registers what
//! Ravel registers under identifiers formed from the namespace and the declared
//! path. The order that run walks the kinds in is this package's own choice, made
//! so that what a dependent declared and what the barrier claims are one sequence
//! a test can state; §4 says the observable contract is the set of declarable
//! kinds and the identifier composition, and nothing here depends on the order.
//!
//! # The changes declared here
//!
//! - **A block's item form is optional.** The original registers a default block
//!   item for every declared block that supplied no factory, and there is no way
//!   to declare a block without one. A block entity with no obtainable form is
//!   ordinary, and the original could not express it. [`ItemForm::None`] can.
//! - **Capability registration needs no wiring.** The original exposes a routine
//!   that walks the declarations and attaches capabilities, subscribes it to
//!   nothing, and gives a dependent that forgets to wire it no capabilities and
//!   no diagnostic. [`Collection::run`] attaches them, so there is nothing to
//!   forget. Attaching walks every capability against every declaration, so both
//!   capability lists are bounded at [`MAX_CAPABILITIES`] and refuse a name
//!   twice: a dependent declaring in a loop cannot grow either the memory or the
//!   per-run cost without limit.
//! - **A block-entity type is keyed by class identity.** The original keys a
//!   class by its fully-qualified name string, so two classes with one name
//!   loaded by different loaders collide and the second silently resolves to the
//!   first. [`ClassId`] is a type's identity and its name is carried only so a
//!   report can print it.
//! - **A run cannot refuse.** Every refusal belongs to the declaring call — an
//!   illegal path, a duplicate, a ceiling, and a block item form that would
//!   carry a declared item's identifier — so running a collection that was built
//!   always answers. A surface asked what this mod declares therefore reports it
//!   rather than swallowing a refusal to answer nothing.
//!
//! # Declarable only
//!
//! **Block-entity types, data components and capabilities.** `ravel_sdk::content`
//! publishes four content kinds — blocks, items, recipes and commands — and none
//! of the other three. What is implemented is the whole observable contract §4
//! says to preserve: the set of declarable kinds, the identifier composition, the
//! order, and which capability attaches to which declaration.
//! [`Registered::declarable`] is what a host would register once it publishes the
//! kinds, named and ordered, and nothing about it is stubbed.

use std::any::TypeId;
use std::fmt;

use ravel_sdk::ModId;
use ravel_sdk::content::{ContentDeclaration, ContentId, ContentIdError, ContentKind, RegistryKey};

/// The most declarations of one kind a collection will hold.
pub const MAX_PER_KIND: usize = 4096;

/// The most capabilities of one sort a collection will hold.
///
/// Attaching is a walk of every capability against every declaration, so an
/// unbounded capability list is an unbounded cost as well as unbounded memory.
/// The upgrade path is a larger constant, or an index keyed by marker.
pub const MAX_CAPABILITIES: usize = 256;

/// A kind a dependent declares.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Kind {
    /// A block placed in the world.
    Block,
    /// An item held in an inventory.
    Item,
    /// A block-entity type.
    BlockEntity,
    /// A data-component type.
    Component,
}

impl Kind {
    /// Returns the kind's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Item => "item",
            Self::BlockEntity => "block-entity",
            Self::Component => "component",
        }
    }

    /// Returns the Ravel kind this registers as, and absent for the two Ravel
    /// publishes no registration for.
    #[must_use]
    pub const fn published(self) -> Option<ContentKind> {
        match self {
            Self::Block => Some(ContentKind::Block),
            Self::Item => Some(ContentKind::Item),
            Self::BlockEntity | Self::Component => None,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Whether a declared block has an item form, and which.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemForm {
    /// No item at all. The block is not obtainable, which is ordinary and which
    /// the original cannot express.
    None,
    /// The default block item, with default properties.
    Default,
    /// An item the dependent builds itself.
    Supplied,
}

/// A type's identity.
///
/// Its name is carried for a report and is never what it is keyed by, which is
/// the whole of the difference from the original's name-keyed lookup.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClassId {
    id: TypeId,
    name: &'static str,
}

impl ClassId {
    /// The identity of one type.
    #[must_use]
    pub fn of<T: 'static>() -> Self {
        Self {
            id: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
        }
    }

    /// Returns the type's name, for a report.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

/// A marker a capability attaches through.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Marker(pub &'static str);

/// One declared capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capability {
    /// What the capability is called.
    pub name: String,
    /// The marker a declaration carries to receive it.
    pub marker: Marker,
    /// What provides it, named.
    pub provider: String,
}

/// One capability attached to one declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    /// Which capability.
    pub capability: String,
    /// Which declaration it attached to.
    pub to: ContentId,
    /// Which kind that declaration is.
    pub kind: Kind,
}

/// Why a declaration was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclareRefused {
    /// The path is not a legal content path.
    Path(ContentIdError),
    /// The same kind and path are already declared.
    Duplicate {
        /// Which kind.
        kind: Kind,
        /// Which path.
        path: String,
    },
    /// A block-entity type is already declared for this class.
    ClassDeclared(&'static str),
    /// A kind already holds [`MAX_PER_KIND`] declarations.
    Full(Kind),
    /// A block's item form and a declared item would be one identifier.
    ItemFormTaken {
        /// The path both name.
        path: String,
    },
    /// A capability of this name and sort is already declared.
    CapabilityDeclared {
        /// Which name.
        name: String,
    },
    /// A sort of capability already holds [`MAX_CAPABILITIES`].
    CapabilitiesFull,
}

impl fmt::Display for DeclareRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(source) => write!(formatter, "{source}"),
            Self::Duplicate { kind, path } => {
                write!(formatter, "a {kind} is already declared at `{path}`")
            }
            Self::ClassDeclared(name) => {
                write!(formatter, "`{name}` already has a block-entity type")
            }
            Self::Full(kind) => write!(formatter, "at most {MAX_PER_KIND} of {kind} are declared"),
            Self::ItemFormTaken { path } => write!(
                formatter,
                "a block's item form and an item both name `{path}`"
            ),
            Self::CapabilityDeclared { name } => {
                write!(formatter, "a capability is already declared as `{name}`")
            }
            Self::CapabilitiesFull => {
                write!(
                    formatter,
                    "at most {MAX_CAPABILITIES} capabilities are declared"
                )
            }
        }
    }
}

impl std::error::Error for DeclareRefused {}

#[derive(Clone, Debug)]
struct BlockDeclaration {
    path: String,
    id: ContentId,
    item: ItemForm,
    markers: Vec<Marker>,
}

#[derive(Clone, Debug)]
struct ItemDeclaration {
    path: String,
    id: ContentId,
    markers: Vec<Marker>,
}

#[derive(Clone, Debug)]
struct BlockEntityDeclaration {
    path: String,
    id: ContentId,
    class: ClassId,
    markers: Vec<Marker>,
}

/// What a dependent declares into.
#[derive(Clone, Debug)]
pub struct Collection {
    namespace: ModId,
    blocks: Vec<BlockDeclaration>,
    items: Vec<ItemDeclaration>,
    block_entities: Vec<BlockEntityDeclaration>,
    components: Vec<(String, ContentId)>,
    block_capabilities: Vec<Capability>,
    item_capabilities: Vec<Capability>,
}

impl Collection {
    /// An empty collection in one namespace.
    #[must_use]
    pub const fn new(namespace: ModId) -> Self {
        Self {
            namespace,
            blocks: Vec::new(),
            items: Vec::new(),
            block_entities: Vec::new(),
            components: Vec::new(),
            block_capabilities: Vec::new(),
            item_capabilities: Vec::new(),
        }
    }

    /// Returns the namespace every identifier here is formed in.
    #[must_use]
    pub const fn namespace(&self) -> &ModId {
        &self.namespace
    }

    /// Declares a block, with or without an item form.
    ///
    /// # Errors
    ///
    /// Returns [`DeclareRefused::Path`] for an illegal path,
    /// [`DeclareRefused::Duplicate`] for a path already declared as a block,
    /// [`DeclareRefused::ItemFormTaken`] when this block's item form would carry
    /// the identifier a declared item already has, and [`DeclareRefused::Full`]
    /// past [`MAX_PER_KIND`].
    pub fn block(
        &mut self,
        path: &str,
        item: ItemForm,
        markers: &[Marker],
    ) -> Result<ContentId, DeclareRefused> {
        let id = self.id(path)?;
        if self.blocks.iter().any(|held| held.path == path) {
            return Err(DeclareRefused::Duplicate {
                kind: Kind::Block,
                path: path.to_owned(),
            });
        }
        if item != ItemForm::None && self.items.iter().any(|held| held.path == path) {
            return Err(DeclareRefused::ItemFormTaken {
                path: path.to_owned(),
            });
        }
        if self.blocks.len() >= MAX_PER_KIND {
            return Err(DeclareRefused::Full(Kind::Block));
        }
        self.blocks.push(BlockDeclaration {
            path: path.to_owned(),
            id: id.clone(),
            item,
            markers: markers.to_vec(),
        });
        Ok(id)
    }

    /// Declares an item.
    ///
    /// # Errors
    ///
    /// As [`Collection::block`], for items, and
    /// [`DeclareRefused::ItemFormTaken`] when a block at this path already has
    /// an item form, whose identifier is the one this item would take.
    pub fn item(&mut self, path: &str, markers: &[Marker]) -> Result<ContentId, DeclareRefused> {
        let id = self.id(path)?;
        if self.items.iter().any(|held| held.path == path) {
            return Err(DeclareRefused::Duplicate {
                kind: Kind::Item,
                path: path.to_owned(),
            });
        }
        if self
            .blocks
            .iter()
            .any(|held| held.path == path && held.item != ItemForm::None)
        {
            return Err(DeclareRefused::ItemFormTaken {
                path: path.to_owned(),
            });
        }
        if self.items.len() >= MAX_PER_KIND {
            return Err(DeclareRefused::Full(Kind::Item));
        }
        self.items.push(ItemDeclaration {
            path: path.to_owned(),
            id: id.clone(),
            markers: markers.to_vec(),
        });
        Ok(id)
    }

    /// Declares a block-entity type for one class.
    ///
    /// # Errors
    ///
    /// As [`Collection::block`], and [`DeclareRefused::ClassDeclared`] when this
    /// class already has one. Two classes that share a simple name do not
    /// collide, because the key is identity.
    pub fn block_entity(
        &mut self,
        path: &str,
        class: ClassId,
        markers: &[Marker],
    ) -> Result<ContentId, DeclareRefused> {
        let id = self.id(path)?;
        if self.block_entities.iter().any(|held| held.path == path) {
            return Err(DeclareRefused::Duplicate {
                kind: Kind::BlockEntity,
                path: path.to_owned(),
            });
        }
        if let Some(held) = self
            .block_entities
            .iter()
            .find(|held| held.class.id == class.id)
        {
            return Err(DeclareRefused::ClassDeclared(held.class.name));
        }
        if self.block_entities.len() >= MAX_PER_KIND {
            return Err(DeclareRefused::Full(Kind::BlockEntity));
        }
        self.block_entities.push(BlockEntityDeclaration {
            path: path.to_owned(),
            id: id.clone(),
            class,
            markers: markers.to_vec(),
        });
        Ok(id)
    }

    /// Declares a data-component type.
    ///
    /// # Errors
    ///
    /// As [`Collection::block`], for components.
    pub fn component(&mut self, path: &str) -> Result<ContentId, DeclareRefused> {
        let id = self.id(path)?;
        if self.components.iter().any(|(held, _)| held == path) {
            return Err(DeclareRefused::Duplicate {
                kind: Kind::Component,
                path: path.to_owned(),
            });
        }
        if self.components.len() >= MAX_PER_KIND {
            return Err(DeclareRefused::Full(Kind::Component));
        }
        self.components.push((path.to_owned(), id.clone()));
        Ok(id)
    }

    /// Declares a capability that attaches to every block-entity type carrying
    /// its marker.
    ///
    /// # Errors
    ///
    /// Returns [`DeclareRefused::CapabilityDeclared`] for a name already
    /// declared as a block capability and [`DeclareRefused::CapabilitiesFull`]
    /// past [`MAX_CAPABILITIES`].
    pub fn block_capability(
        &mut self,
        name: &str,
        marker: Marker,
        provider: &str,
    ) -> Result<(), DeclareRefused> {
        Self::declare_capability(&mut self.block_capabilities, name, marker, provider)
    }

    /// Declares a capability that attaches to every item carrying its marker.
    ///
    /// # Errors
    ///
    /// As [`Collection::block_capability`], for item capabilities.
    pub fn item_capability(
        &mut self,
        name: &str,
        marker: Marker,
        provider: &str,
    ) -> Result<(), DeclareRefused> {
        Self::declare_capability(&mut self.item_capabilities, name, marker, provider)
    }

    fn declare_capability(
        into: &mut Vec<Capability>,
        name: &str,
        marker: Marker,
        provider: &str,
    ) -> Result<(), DeclareRefused> {
        if into.iter().any(|held| held.name == name) {
            return Err(DeclareRefused::CapabilityDeclared {
                name: name.to_owned(),
            });
        }
        if into.len() >= MAX_CAPABILITIES {
            return Err(DeclareRefused::CapabilitiesFull);
        }
        into.push(Capability {
            name: name.to_owned(),
            marker,
            provider: provider.to_owned(),
        });
        Ok(())
    }

    /// Returns the class a block-entity type was declared for.
    ///
    /// Keyed by identity: a type that shares a name with another type is a
    /// different key and resolves to its own declaration.
    #[must_use]
    pub fn class_of(&self, path: &str) -> Option<ClassId> {
        self.block_entities
            .iter()
            .find(|held| held.path == path)
            .map(|held| held.class)
    }

    /// Runs the collection: registers what Ravel registers, names what it does
    /// not, and attaches every capability whose marker a declaration carries.
    ///
    /// It walks blocks, then items — a block's item form among them, in its
    /// block's order — then block-entity types, then data components. That order
    /// is this package's own, chosen so a run is one stated sequence rather than
    /// an arbitrary one; §4 asks a counterpart for the declarable kinds and the
    /// identifier composition, and for whatever order the host requires.
    ///
    /// This cannot refuse. Every refusal a run could raise is raised where the
    /// declaration is made instead: an illegal path, a duplicate, a ceiling, and
    /// a block item form colliding with a declared item are all answered by the
    /// declaring call, so a collection that was built is a collection that runs.
    /// That is what lets [`crate::Shared::content_declaration`] report what this
    /// mod declares rather than swallowing a refusal to answer nothing.
    ///
    /// # Panics
    ///
    /// Never. The entries are the paths this collection already accepted, each
    /// carrying the identifier it was accepted under, and no two of them are the
    /// same kind and path.
    #[must_use]
    pub fn run(&self) -> Registered {
        let mut declaration = ContentDeclaration::new(self.namespace.clone());
        let mut entry = |kind: ContentKind, path: &str| {
            declaration
                .entry(kind, path)
                .expect("every declared path was accepted, once, under this kind");
        };
        for block in &self.blocks {
            entry(ContentKind::Block, &block.path);
        }
        for block in &self.blocks {
            if block.item != ItemForm::None {
                entry(ContentKind::Item, &block.path);
            }
        }
        for item in &self.items {
            entry(ContentKind::Item, &item.path);
        }

        let mut declarable = Vec::new();
        for held in &self.block_entities {
            declarable.push((Kind::BlockEntity, held.id.clone()));
        }
        for (_, id) in &self.components {
            declarable.push((Kind::Component, id.clone()));
        }

        let mut attached = Vec::new();
        for capability in &self.block_capabilities {
            for held in &self.block_entities {
                if held.markers.contains(&capability.marker) {
                    attached.push(Attachment {
                        capability: capability.name.clone(),
                        to: held.id.clone(),
                        kind: Kind::BlockEntity,
                    });
                }
            }
        }
        for capability in &self.item_capabilities {
            for held in &self.items {
                if held.markers.contains(&capability.marker) {
                    attached.push(Attachment {
                        capability: capability.name.clone(),
                        to: held.id.clone(),
                        kind: Kind::Item,
                    });
                }
            }
            for block in &self.blocks {
                if block.item != ItemForm::None && block.markers.contains(&capability.marker) {
                    attached.push(Attachment {
                        capability: capability.name.clone(),
                        to: block.id.clone(),
                        kind: Kind::Item,
                    });
                }
            }
        }

        Registered {
            declaration,
            declarable,
            attached,
        }
    }

    fn id(&self, path: &str) -> Result<ContentId, DeclareRefused> {
        ContentId::new(self.namespace.clone(), path).map_err(DeclareRefused::Path)
    }
}

/// What running a collection produced.
#[derive(Clone, Debug)]
pub struct Registered {
    declaration: ContentDeclaration,
    declarable: Vec<(Kind, ContentId)>,
    attached: Vec<Attachment>,
}

impl Registered {
    /// Returns the Ravel declaration, which [`ravel_sdk::content::claim_all`]
    /// claims from.
    #[must_use]
    pub const fn declaration(&self) -> &ContentDeclaration {
        &self.declaration
    }

    /// Returns every key Ravel claims, in registration order.
    #[must_use]
    pub fn entries(&self) -> &[RegistryKey] {
        self.declaration.entries()
    }

    /// Returns the kinds Ravel publishes no registration for, named and in
    /// order.
    #[must_use]
    pub fn declarable(&self) -> &[(Kind, ContentId)] {
        &self.declarable
    }

    /// Returns every capability attachment, which needed no wiring.
    #[must_use]
    pub fn attached(&self) -> &[Attachment] {
        &self.attached
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::ModId;
    use ravel_sdk::content::{ContentKind, Registrar, RegistrationRefused, RegistryKey, claim_all};

    use super::{
        ClassId, Collection, DeclareRefused, ItemForm, Kind, MAX_CAPABILITIES, MAX_PER_KIND, Marker,
    };

    mod machines {
        /// One dependent's type.
        pub(super) struct Furnace;
    }

    mod terminals {
        /// Another dependent's type under the same simple name.
        pub(super) struct Furnace;
    }

    const HOLDS_ITEMS: Marker = Marker("holds-items");
    const HOLDS_POWER: Marker = Marker("holds-power");

    fn namespace() -> ModId {
        "rvl_146_counterpart"
            .parse()
            .expect("the declared identity is a legal mod id")
    }

    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    #[test]
    fn an_identifier_is_the_namespace_and_the_declared_path() {
        let mut collection = Collection::new(namespace());
        let id = collection
            .block("crusher", ItemForm::Default, &[])
            .expect("a legal path");
        assert_eq!(id.to_string(), "rvl_146_counterpart:crusher");
        assert_eq!(collection.namespace(), &namespace());
    }

    #[test]
    fn a_block_may_be_declared_with_no_item_form_at_all() {
        let mut collection = Collection::new(namespace());
        collection
            .block("crusher", ItemForm::Default, &[])
            .expect("a legal path");
        collection
            .block("cable", ItemForm::None, &[])
            .expect("a legal path");
        let registered = collection.run();
        let keys: Vec<String> = registered
            .entries()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            keys,
            [
                "block:rvl_146_counterpart:crusher",
                "block:rvl_146_counterpart:cable",
                "item:rvl_146_counterpart:crusher",
            ],
            "the block with no item form has no item"
        );
    }

    #[test]
    fn the_registration_order_is_blocks_then_items_then_the_two_ravel_does_not_publish() {
        let mut collection = Collection::new(namespace());
        collection
            .block("crusher", ItemForm::Supplied, &[])
            .expect("a legal path");
        collection.item("dust", &[]).expect("a legal path");
        collection
            .block_entity("crusher", ClassId::of::<machines::Furnace>(), &[])
            .expect("a legal path");
        collection.component("charge").expect("a legal path");

        let registered = collection.run();
        assert_eq!(
            registered
                .entries()
                .iter()
                .map(|key| (key.kind(), key.id().path().to_owned()))
                .collect::<Vec<_>>(),
            [
                (ContentKind::Block, "crusher".to_owned()),
                (ContentKind::Item, "crusher".to_owned()),
                (ContentKind::Item, "dust".to_owned()),
            ]
        );
        assert_eq!(
            registered
                .declarable()
                .iter()
                .map(|(kind, id)| (*kind, id.path().to_owned()))
                .collect::<Vec<_>>(),
            [
                (Kind::BlockEntity, "crusher".to_owned()),
                (Kind::Component, "charge".to_owned()),
            ]
        );
    }

    #[test]
    fn what_a_collection_runs_is_what_ravels_barrier_claims() {
        let mut collection = Collection::new(namespace());
        collection
            .block("crusher", ItemForm::Default, &[])
            .expect("a legal path");
        let registered = collection.run();
        let mut registrar = Accepting::default();
        claim_all(registered.declaration(), &mut registrar).expect("an accepting registrar");
        assert_eq!(registrar.claimed(), registered.entries());
    }

    #[test]
    fn a_capability_attaches_with_no_wiring_call_of_any_kind() {
        let mut collection = Collection::new(namespace());
        collection
            .block_entity(
                "crusher",
                ClassId::of::<machines::Furnace>(),
                &[HOLDS_ITEMS],
            )
            .expect("a legal path");
        collection
            .block_entity("mixer", ClassId::of::<terminals::Furnace>(), &[HOLDS_POWER])
            .expect("a legal path");
        collection
            .item("cell", &[HOLDS_POWER])
            .expect("a legal path");
        collection
            .block_capability("inventory", HOLDS_ITEMS, "the crusher's inventory")
            .expect("a capability declared once");
        collection
            .item_capability("charge", HOLDS_POWER, "the cell's charge")
            .expect("a capability declared once");

        let registered = collection.run();
        let attached: Vec<(String, String, Kind)> = registered
            .attached()
            .iter()
            .map(|attachment| {
                (
                    attachment.capability.clone(),
                    attachment.to.path().to_owned(),
                    attachment.kind,
                )
            })
            .collect();
        assert_eq!(
            attached,
            [
                (
                    "inventory".to_owned(),
                    "crusher".to_owned(),
                    Kind::BlockEntity
                ),
                ("charge".to_owned(), "cell".to_owned(), Kind::Item),
            ],
            "a marker nothing declared attaches nothing, and no routine was wired"
        );
    }

    #[test]
    fn an_item_capability_reaches_a_blocks_item_form_and_not_a_block_without_one() {
        let mut collection = Collection::new(namespace());
        collection
            .block("crusher", ItemForm::Default, &[HOLDS_POWER])
            .expect("a legal path");
        collection
            .block("cable", ItemForm::None, &[HOLDS_POWER])
            .expect("a legal path");
        collection
            .item_capability("charge", HOLDS_POWER, "the charge")
            .expect("a capability declared once");
        let registered = collection.run();
        assert_eq!(
            registered
                .attached()
                .iter()
                .map(|attachment| attachment.to.path().to_owned())
                .collect::<Vec<_>>(),
            ["crusher".to_owned()]
        );
    }

    /// The collision the original's name-keyed lookup has cannot be built here
    /// at all: no two Rust types share the fully-qualified name [`ClassId`]
    /// carries, so a
    /// test of "two types with one name resolve to one declaration" has no
    /// fixture. That is the §4 change rather than a gap in the test — keying by
    /// identity is what makes the condition unconstructible — so what is pinned
    /// here is the half that can be: the key is the identity and never the name,
    /// and the name is carried only for a report.
    #[test]
    fn a_block_entity_type_is_keyed_by_identity_and_the_name_is_only_carried() {
        let mut collection = Collection::new(namespace());
        collection
            .block_entity("crusher", ClassId::of::<machines::Furnace>(), &[])
            .expect("a legal path");
        collection
            .block_entity("mixer", ClassId::of::<terminals::Furnace>(), &[])
            .expect("a second class with the same simple name");
        assert_eq!(
            collection.class_of("crusher"),
            Some(ClassId::of::<machines::Furnace>())
        );
        assert_eq!(
            collection.class_of("mixer"),
            Some(ClassId::of::<terminals::Furnace>()),
            "the second declaration holds its own class rather than the first's"
        );
        assert_ne!(
            ClassId::of::<machines::Furnace>(),
            ClassId::of::<terminals::Furnace>()
        );
        assert!(
            ClassId::of::<machines::Furnace>()
                .name()
                .ends_with("Furnace")
                && ClassId::of::<terminals::Furnace>()
                    .name()
                    .ends_with("Furnace"),
            "both names end in the simple name the original would key by"
        );
        assert!(collection.class_of("vanish").is_none());
    }

    #[test]
    fn one_class_holds_one_block_entity_type() {
        let mut collection = Collection::new(namespace());
        collection
            .block_entity("crusher", ClassId::of::<machines::Furnace>(), &[])
            .expect("a legal path");
        assert!(matches!(
            collection.block_entity("mixer", ClassId::of::<machines::Furnace>(), &[]),
            Err(DeclareRefused::ClassDeclared(_))
        ));
    }

    #[test]
    fn an_illegal_path_and_a_duplicate_are_refused_where_they_are_written() {
        let mut collection = Collection::new(namespace());
        assert!(matches!(
            collection.block("Not A Path", ItemForm::Default, &[]),
            Err(DeclareRefused::Path(_))
        ));
        collection
            .block("crusher", ItemForm::Default, &[])
            .expect("a legal path");
        assert_eq!(
            collection.block("crusher", ItemForm::None, &[]),
            Err(DeclareRefused::Duplicate {
                kind: Kind::Block,
                path: "crusher".to_owned()
            })
        );
        collection.item("dust", &[]).expect("a legal path");
        assert_eq!(
            collection.item("dust", &[]),
            Err(DeclareRefused::Duplicate {
                kind: Kind::Item,
                path: "dust".to_owned()
            })
        );
        collection.component("charge").expect("a legal path");
        assert_eq!(
            collection.component("charge"),
            Err(DeclareRefused::Duplicate {
                kind: Kind::Component,
                path: "charge".to_owned()
            })
        );
    }

    #[test]
    fn a_block_and_an_item_may_share_a_path_because_they_are_two_registries() {
        let mut collection = Collection::new(namespace());
        collection
            .block("crusher", ItemForm::None, &[])
            .expect("a legal path");
        collection.item("crusher", &[]).expect("two registries");
        let registered = collection.run();
        assert_eq!(registered.entries().len(), 2);
    }

    #[test]
    fn a_kind_past_its_ceiling_is_refused() {
        let mut collection = Collection::new(namespace());
        for index in 0..MAX_PER_KIND {
            collection
                .component(&format!("one{index}"))
                .expect("inside the ceiling");
        }
        assert_eq!(
            collection.component("past"),
            Err(DeclareRefused::Full(Kind::Component))
        );
    }

    #[test]
    fn a_block_item_form_and_an_item_at_one_path_are_refused_where_they_are_declared() {
        // Both would carry the one identifier, so a run holding both would be
        // the refusal a surface reporting the declaration has nowhere to put.
        // It is answered by the declaring call instead, in both orders.
        let mut first = Collection::new(namespace());
        first
            .block("crusher", ItemForm::Default, &[])
            .expect("a legal path");
        assert_eq!(
            first.item("crusher", &[]),
            Err(DeclareRefused::ItemFormTaken {
                path: "crusher".to_owned()
            })
        );

        let mut second = Collection::new(namespace());
        second.item("crusher", &[]).expect("a legal path");
        assert_eq!(
            second.block("crusher", ItemForm::Default, &[]),
            Err(DeclareRefused::ItemFormTaken {
                path: "crusher".to_owned()
            })
        );
    }

    #[test]
    fn a_capability_is_declared_once_and_the_lists_are_bounded() {
        let mut collection = Collection::new(namespace());
        collection
            .block_capability("inventory", HOLDS_ITEMS, "the crusher's inventory")
            .expect("a capability declared once");
        assert_eq!(
            collection.block_capability("inventory", HOLDS_POWER, "something else"),
            Err(DeclareRefused::CapabilityDeclared {
                name: "inventory".to_owned()
            })
        );
        collection
            .item_capability("inventory", HOLDS_ITEMS, "the cell's inventory")
            .expect("the two sorts are separate lists");

        for index in 1..MAX_CAPABILITIES {
            collection
                .block_capability(&format!("held{index}"), HOLDS_POWER, "a provider")
                .expect("inside the ceiling");
        }
        assert_eq!(
            collection.block_capability("past", HOLDS_POWER, "a provider"),
            Err(DeclareRefused::CapabilitiesFull),
            "attaching walks every capability against every declaration, so the \
             list a dependent grows in a loop is bounded"
        );
    }

    #[test]
    fn the_two_kinds_ravel_publishes_map_to_ravels_own_and_the_other_two_do_not() {
        assert_eq!(Kind::Block.published(), Some(ContentKind::Block));
        assert_eq!(Kind::Item.published(), Some(ContentKind::Item));
        assert_eq!(Kind::BlockEntity.published(), None);
        assert_eq!(Kind::Component.published(), None);
        assert_eq!(Kind::BlockEntity.to_string(), "block-entity");
    }
}
