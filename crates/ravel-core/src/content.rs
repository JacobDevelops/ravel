use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::identity::ModId;

/// The character that separates a [`ContentId`]'s namespace from its path.
const NAMESPACE_SEPARATOR: char = ':';

/// A kind of registered content.
///
/// `SPEC.md` §14 makes registry identifiers immutable after one registration
/// barrier, and `ROADMAP.md`'s M3-W1 row names the three kinds the prototype
/// registers. The kind is part of the identity rather than a property of it,
/// because the game keeps one registry per kind: a block and an item may share
/// the name `example_mod:copper` without either claim conflicting with the
/// other, and a barrier keyed on the path alone would refuse the second.
///
/// This list grows with the capabilities that land. It is deliberately not an
/// open string: an unknown kind names a registry no adapter can commit to, and
/// failing to parse it is how that is caught before a mod claims anything.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentKind {
    /// A block placed in the world.
    Block,
    /// An item held in an inventory.
    Item,
    /// A recipe that converts items.
    Recipe,
}

impl ContentKind {
    /// Every kind, in the order this enum declares them.
    pub const ALL: [Self; 3] = [Self::Block, Self::Item, Self::Recipe];

    /// Returns the kind's canonical lowercase name.
    ///
    /// This is the same spelling the serde representation uses, so a kind reads
    /// the same in a [`RegistryKey`], in a report, and in a manifest.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Item => "item",
            Self::Recipe => "recipe",
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
    /// The rejected spelling, kept so a report can quote what was asked for.
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

/// A canonical identifier for one piece of registered content.
///
/// The shape is `namespace:path`, where the namespace is a [`ModId`] and the
/// path names the content inside it. Reusing [`ModId`] for the namespace is not
/// a convenience: `SPEC.md` §14 requires persistent data to be owned by a
/// stable canonical mod id, and content identifiers end up in saved worlds, so
/// the half of the identifier that says who owns it is exactly the type that is
/// already stable for the life of a mod.
///
/// The namespace is *not* required to be the mod that registers the content.
/// A pack legitimately replaces a recipe it did not declare, and refusing that
/// here would decide a pack-authoring question this milestone has no evidence
/// for. What stops two mods owning the same content is the registration
/// barrier's conflict check, which reports both claimants, rather than a rule
/// in this type.
///
/// The path grammar is Minecraft's resource-location path charset, narrowed to
/// what a class file will accept: ASCII lowercase letters, ASCII digits, and
/// the separators `_`, `.`, `-`, and `/`. A path is never empty and never
/// starts or ends with `/`, because a trailing slash names a directory rather
/// than an entry.
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
/// // A recipe path may be nested.
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
    /// Returns [`ContentIdError`] when the path is empty, holds a character
    /// outside the documented charset, or starts or ends with `/`.
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
            _ => None,
        }
    }
}

/// One entry in one of the game's registries.
///
/// A [`ContentId`] says what the content is called and a [`ContentKind`] says
/// which registry it belongs to. Only the pair is unique, so the pair is what
/// the registration barrier keys on and what a conflict report names.
///
/// The canonical string form is `kind:namespace:path`. It is unambiguous
/// because no kind holds a `:`, so the first separator always ends the kind.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentKind, RegistryKey, RegistryKeyError};
///
/// let block: RegistryKey = "block:example_mod:copper".parse()?;
/// let item: RegistryKey = "item:example_mod:copper".parse()?;
///
/// // The same name in two registries is two different claims.
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

#[cfg(test)]
mod tests {
    use super::{ContentId, ContentIdError, ContentKind, RegistryKey, RegistryKeyError};
    use crate::ModId;

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
