use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::identity::ModId;

const NAMESPACE_SEPARATOR: char = ':';

/// A registry kind; the same content name may occur in different registries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentKind {
    /// A block placed in the world.
    Block,
    /// An item held in an inventory.
    Item,
    /// A recipe that converts items.
    Recipe,
    /// A command's root literal; arguments and subcommands belong to its owner.
    Command,
}

impl ContentKind {
    /// Every kind, in the order this enum declares them.
    pub const ALL: [Self; 4] = [Self::Block, Self::Item, Self::Recipe, Self::Command];

    /// Returns the kind's canonical serialized name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Item => "item",
            Self::Recipe => "recipe",
            Self::Command => "command",
        }
    }
}

impl fmt::Display for ContentKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ContentKind {
    type Err = ContentKindError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == candidate)
            .ok_or_else(|| ContentKindError {
                candidate: candidate.to_owned(),
            })
    }
}

/// A candidate string that names no [`ContentKind`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentKindError {
    /// The rejected spelling.
    pub candidate: String,
}

impl fmt::Display for ContentKindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let known: Vec<&str> = ContentKind::ALL.iter().map(|kind| kind.as_str()).collect();
        write!(
            formatter,
            "`{}` is not a content kind; Ravel registers {}",
            self.candidate,
            known.join(", ")
        )
    }
}

impl std::error::Error for ContentKindError {}

/// A `namespace:path` identifier whose namespace is a [`ModId`].
///
/// Paths use ASCII lowercase letters, digits, `_`, `.`, `-`, and `/`, without
/// leading or trailing `/` or `..` segments. Registrants may use another mod's namespace.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentId, ContentIdError};
///
/// let id: ContentId = "example_mod:copper_ingot".parse()?;
/// assert_eq!(id.namespace().as_str(), "example_mod");
/// assert_eq!(id.path(), "copper_ingot");
/// assert_eq!(id.to_string(), "example_mod:copper_ingot");
///
/// let nested: ContentId = "example_mod:smelting/copper".parse()?;
/// assert_eq!(nested.path(), "smelting/copper");
///
/// assert!("no_namespace".parse::<ContentId>().is_err());
/// assert!("Example_Mod:copper".parse::<ContentId>().is_err());
/// assert!("example_mod:".parse::<ContentId>().is_err());
/// # Ok::<(), ContentIdError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct ContentId {
    namespace: ModId,
    path: String,
}

impl ContentId {
    /// Builds an identifier from an already-valid namespace and a path.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] for an empty, malformed, or traversing path.
    pub fn new(namespace: ModId, path: impl Into<String>) -> Result<Self, ContentIdError> {
        let path = path.into();
        Self::validate_path(&path)?;
        Ok(Self { namespace, path })
    }

    /// Returns the mod identity that owns this content's namespace.
    #[must_use]
    pub const fn namespace(&self) -> &ModId {
        &self.namespace
    }

    /// Returns the path that names the content inside its namespace.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    fn validate_path(path: &str) -> Result<(), ContentIdError> {
        if path.is_empty() {
            return Err(ContentIdError::EmptyPath);
        }
        if path.starts_with('/') || path.ends_with('/') {
            return Err(ContentIdError::PathBoundary);
        }
        if crate::paths::is_traversal(path) {
            return Err(ContentIdError::PathTraversal);
        }

        for (index, character) in path.chars().enumerate() {
            let allowed = character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '.' | '-' | '/');
            if !allowed {
                return Err(ContentIdError::PathCharacter { character, index });
            }
        }

        Ok(())
    }

    fn parse(candidate: &str) -> Result<Self, ContentIdError> {
        let (namespace, path) = candidate
            .split_once(NAMESPACE_SEPARATOR)
            .ok_or(ContentIdError::MissingNamespace)?;
        let namespace = namespace
            .parse::<ModId>()
            .map_err(|source| ContentIdError::Namespace { source })?;
        Self::new(namespace, path)
    }
}

impl FromStr for ContentId {
    type Err = ContentIdError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::parse(candidate)
    }
}

impl TryFrom<String> for ContentId {
    type Error = ContentIdError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::parse(&candidate)
    }
}

impl From<ContentId> for String {
    fn from(id: ContentId) -> Self {
        id.to_string()
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{NAMESPACE_SEPARATOR}{}",
            self.namespace, self.path
        )
    }
}

/// Why a candidate string is not a valid [`ContentId`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentIdError {
    /// The candidate holds no `:`, so it names no namespace.
    MissingNamespace,
    /// The text before the `:` is not a legal [`ModId`].
    Namespace {
        /// Why the namespace was refused.
        source: crate::identity::ModIdError,
    },
    /// The text after the `:` is empty.
    EmptyPath,
    /// The path starts or ends with `/`, so it names a directory.
    PathBoundary,
    /// The path holds a `..` segment.
    PathTraversal,
    /// A path character is outside the documented charset.
    PathCharacter {
        /// The rejected character.
        character: char,
        /// Zero-based character index of the rejection, within the path.
        index: usize,
    },
}

impl fmt::Display for ContentIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNamespace => formatter
                .write_str("a content id is `namespace:path` and this candidate holds no `:`"),
            Self::Namespace { source } => {
                write!(formatter, "a content id's namespace is a mod id: {source}")
            }
            Self::EmptyPath => formatter.write_str("a content id's path cannot be empty"),
            Self::PathBoundary => formatter.write_str(
                "a content id's path names an entry, so it cannot start or end with `/`",
            ),
            Self::PathTraversal => formatter.write_str(
                "a content id's path names something inside its own namespace, so it cannot hold \
                 a `..` segment",
            ),
            Self::PathCharacter { character, index } => write!(
                formatter,
                "a content id's path holds only `a`-`z`, `0`-`9`, `_`, `.`, `-`, and `/`, but \
                 character {index} is `{character}`"
            ),
        }
    }
}

impl std::error::Error for ContentIdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Namespace { source } => Some(source),
            Self::MissingNamespace
            | Self::EmptyPath
            | Self::PathBoundary
            | Self::PathTraversal
            | Self::PathCharacter { .. } => None,
        }
    }
}

/// A unique registry entry, written as `kind:namespace:path`.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentKind, RegistryKey, RegistryKeyError};
///
/// let block: RegistryKey = "block:example_mod:copper".parse()?;
/// let item: RegistryKey = "item:example_mod:copper".parse()?;
///
/// assert_ne!(block, item);
/// assert_eq!(block.kind(), ContentKind::Block);
/// assert_eq!(block.id().path(), "copper");
/// assert_eq!(block.to_string(), "block:example_mod:copper");
/// # Ok::<(), RegistryKeyError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct RegistryKey {
    kind: ContentKind,
    id: ContentId,
}

impl RegistryKey {
    /// Names one entry of `kind` called `id`.
    #[must_use]
    pub const fn new(kind: ContentKind, id: ContentId) -> Self {
        Self { kind, id }
    }

    /// Returns which registry the entry belongs to.
    #[must_use]
    pub const fn kind(&self) -> ContentKind {
        self.kind
    }

    /// Returns what the entry is called.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.id
    }

    fn parse(candidate: &str) -> Result<Self, RegistryKeyError> {
        let (kind, id) = candidate
            .split_once(NAMESPACE_SEPARATOR)
            .ok_or(RegistryKeyError::MissingKind)?;
        let kind = kind
            .parse::<ContentKind>()
            .map_err(|source| RegistryKeyError::Kind { source })?;
        let id = id
            .parse::<ContentId>()
            .map_err(|source| RegistryKeyError::Id { source })?;
        Ok(Self { kind, id })
    }
}

impl FromStr for RegistryKey {
    type Err = RegistryKeyError;

    fn from_str(candidate: &str) -> Result<Self, Self::Err> {
        Self::parse(candidate)
    }
}

impl TryFrom<String> for RegistryKey {
    type Error = RegistryKeyError;

    fn try_from(candidate: String) -> Result<Self, Self::Error> {
        Self::parse(&candidate)
    }
}

impl From<RegistryKey> for String {
    fn from(key: RegistryKey) -> Self {
        key.to_string()
    }
}

impl fmt::Display for RegistryKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{NAMESPACE_SEPARATOR}{}", self.kind, self.id)
    }
}

/// Why a candidate string is not a valid [`RegistryKey`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryKeyError {
    /// The candidate holds no `:`, so it names no kind.
    MissingKind,
    /// The text before the first `:` names no [`ContentKind`].
    Kind {
        /// Why the kind was refused.
        source: ContentKindError,
    },
    /// The text after the first `:` is not a legal [`ContentId`].
    Id {
        /// Why the identifier was refused.
        source: ContentIdError,
    },
}

impl fmt::Display for RegistryKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKind => formatter.write_str(
                "a registry key is `kind:namespace:path` and this candidate holds no `:`",
            ),
            Self::Kind { source } => write!(formatter, "{source}"),
            Self::Id { source } => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for RegistryKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingKind => None,
            Self::Kind { source } => Some(source),
            Self::Id { source } => Some(source),
        }
    }
}

/// Primitive block properties, defaulting to a stone-like block.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct BlockBehaviour {
    /// How long the block takes to break. Negative means unbreakable.
    pub hardness: f32,
    /// How well the block resists an explosion.
    pub explosion_resistance: f32,
    /// Whether the block drops nothing when broken with the wrong tool.
    pub requires_correct_tool: bool,
    /// Whether the block is offered a random tick.
    pub random_ticks: bool,
}

impl Default for BlockBehaviour {
    fn default() -> Self {
        Self {
            hardness: 1.5,
            explosion_resistance: 6.0,
            requires_correct_tool: false,
            random_ticks: false,
        }
    }
}

/// Primitive item properties.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemBehaviour {
    /// How many of the item one stack holds.
    pub stack_size: u8,
    /// Whether the item survives fire and lava.
    pub fire_resistant: bool,
}

impl Default for ItemBehaviour {
    fn default() -> Self {
        Self {
            stack_size: 64,
            fire_resistant: false,
        }
    }
}

/// A block or item description, separate from its immutable registry identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentBehaviour {
    /// A block's behaviour.
    Block(BlockBehaviour),
    /// An item's behaviour.
    Item(ItemBehaviour),
}

impl ContentBehaviour {
    /// Returns the kind of content this describes.
    #[must_use]
    pub const fn kind(&self) -> ContentKind {
        match self {
            Self::Block(_) => ContentKind::Block,
            Self::Item(_) => ContentKind::Item,
        }
    }

    /// Returns whether this describes content of `kind`.
    #[must_use]
    pub const fn describes(&self, kind: ContentKind) -> bool {
        matches!(
            (self, kind),
            (Self::Block(_), ContentKind::Block) | (Self::Item(_), ContentKind::Item)
        )
    }

    /// Returns default block or item behaviour, or `None` for recipes and commands.
    #[must_use]
    pub fn default_for(kind: ContentKind) -> Option<Self> {
        match kind {
            ContentKind::Block => Some(Self::Block(BlockBehaviour::default())),
            ContentKind::Item => Some(Self::Item(ItemBehaviour::default())),
            ContentKind::Recipe | ContentKind::Command => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentId, ContentIdError, ContentKind, RegistryKey, RegistryKeyError};
    use crate::identity::ModId;
    use crate::paths::{RelativePath, RelativePathError};

    fn id(text: &str) -> ContentId {
        text.parse().expect("a legal content id")
    }

    #[test]
    fn every_kind_round_trips_through_its_canonical_name_and_its_serde_form() {
        for kind in ContentKind::ALL {
            assert_eq!(
                kind.as_str().parse::<ContentKind>().expect("a known kind"),
                kind
            );
            assert_eq!(kind.to_string(), kind.as_str());

            let encoded = serde_json::to_string(&kind).expect("should serialize");
            assert_eq!(encoded, format!("\"{}\"", kind.as_str()));
            assert_eq!(
                serde_json::from_str::<ContentKind>(&encoded).expect("should deserialize"),
                kind
            );
        }
    }

    #[test]
    fn an_unknown_kind_is_refused_and_the_refusal_names_what_ravel_registers() {
        let error = "entity".parse::<ContentKind>().expect_err("not a kind");
        assert_eq!(error.candidate, "entity");
        let text = error.to_string();
        assert!(text.contains("entity"), "{text}");
        for kind in ContentKind::ALL {
            assert!(text.contains(kind.as_str()), "{text}");
        }
    }

    #[test]
    fn a_content_id_splits_into_the_mod_that_owns_the_namespace_and_the_path() {
        let parsed = id("example_mod:copper_ingot");
        assert_eq!(
            parsed.namespace(),
            &"example_mod".parse::<ModId>().expect("a legal mod id")
        );
        assert_eq!(parsed.path(), "copper_ingot");
        assert_eq!(parsed.to_string(), "example_mod:copper_ingot");
    }

    #[test]
    fn a_path_may_nest_but_never_names_a_directory() {
        assert_eq!(id("example_mod:smelting/copper").path(), "smelting/copper");
        assert_eq!(
            "example_mod:/copper".parse::<ContentId>(),
            Err(ContentIdError::PathBoundary)
        );
        assert_eq!(
            "example_mod:copper/".parse::<ContentId>(),
            Err(ContentIdError::PathBoundary)
        );
    }

    #[test]
    fn a_path_that_would_leave_its_own_namespace_is_refused_by_both_path_types() {
        let namespace: ModId = "example_mod".parse().expect("a legal mod id");
        for candidate in ["../elsewhere", "a/../b", "smelting/.."] {
            assert_eq!(
                ContentId::new(namespace.clone(), candidate).expect_err("reject path traversal"),
                ContentIdError::PathTraversal,
                "{candidate}"
            );
        }

        assert_eq!(
            RelativePath::try_from("data/example_mod/../elsewhere.json".to_owned()),
            Err(RelativePathError::Traversal)
        );

        let dotted = ContentId::new(namespace, "a..b").expect("`a..b` is not a traversal");
        assert_eq!(dotted.path(), "a..b");
        assert!(RelativePath::try_from("data/example_mod/a..b.json".to_owned()).is_ok());
    }

    #[test]
    fn a_malformed_content_id_is_refused_with_the_reason_it_was_refused() {
        assert_eq!(
            "no_namespace".parse::<ContentId>(),
            Err(ContentIdError::MissingNamespace)
        );
        assert_eq!(
            "example_mod:".parse::<ContentId>(),
            Err(ContentIdError::EmptyPath)
        );
        assert_eq!(
            "example_mod:Copper".parse::<ContentId>(),
            Err(ContentIdError::PathCharacter {
                character: 'C',
                index: 0,
            })
        );
        assert!(matches!(
            "Example:copper".parse::<ContentId>(),
            Err(ContentIdError::Namespace { .. })
        ));
    }

    #[test]
    fn a_content_id_round_trips_through_its_string_form() {
        for text in [
            "example_mod:copper",
            "example_mod:smelting/copper",
            "a:b",
            "example-mod.two:deep/nested.path-1",
        ] {
            let parsed = id(text);
            assert_eq!(parsed.to_string(), text);

            let encoded = serde_json::to_string(&parsed).expect("should serialize");
            assert_eq!(
                serde_json::from_str::<ContentId>(&encoded).expect("should deserialize"),
                parsed
            );
        }
    }

    #[test]
    fn the_same_name_in_two_registries_is_two_distinct_keys() {
        let block = RegistryKey::new(ContentKind::Block, id("example_mod:copper"));
        let item = RegistryKey::new(ContentKind::Item, id("example_mod:copper"));

        assert_ne!(block, item);
        assert_eq!(block.id(), item.id());
        assert_eq!(block.kind(), ContentKind::Block);
        assert_eq!(block.to_string(), "block:example_mod:copper");
        assert_eq!(item.to_string(), "item:example_mod:copper");
    }

    #[test]
    fn a_registry_key_round_trips_through_its_string_form() {
        for kind in ContentKind::ALL {
            let key = RegistryKey::new(kind, id("example_mod:smelting/copper"));
            let text = key.to_string();
            assert_eq!(text.parse::<RegistryKey>().expect("a legal key"), key);

            let encoded = serde_json::to_string(&key).expect("should serialize");
            assert_eq!(
                serde_json::from_str::<RegistryKey>(&encoded).expect("should deserialize"),
                key
            );
        }
    }

    #[test]
    fn a_malformed_registry_key_names_which_half_was_refused() {
        assert_eq!(
            "block".parse::<RegistryKey>(),
            Err(RegistryKeyError::MissingKind)
        );
        assert!(matches!(
            "entity:example_mod:zombie".parse::<RegistryKey>(),
            Err(RegistryKeyError::Kind { .. })
        ));
        assert!(matches!(
            "block:no_namespace".parse::<RegistryKey>(),
            Err(RegistryKeyError::Id { .. })
        ));
    }
}
