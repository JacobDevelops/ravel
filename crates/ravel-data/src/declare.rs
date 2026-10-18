use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ContentId, ContentIdError, ContentKind, ModId, RegistryKey, RelativePath};
use serde::{Deserialize, Serialize};

use crate::generate::{
    GeneratedData, blockstate_document, item_definition_document, item_model_document,
    loot_table_document, sprite_model_document, tag_document, translation_document,
};
use crate::tags::TagId;

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

/// One mod's registry entries, tag memberships, translations, and supplied resources.
///
/// Duplicate entries and output paths are refused locally; cross-mod claims and layering
/// are resolved by the registration barrier, [`crate::TagSet`], and [`crate::ResourceSet`].
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentId, ContentKind, ModId};
/// use ravel_data::{ContentDeclaration, ContentDeclarationError, TagId};
///
/// let owner: ModId = "example_mod".parse().expect("a legal mod id");
/// let mut declaration = ContentDeclaration::new(owner);
///
/// // A registry entry is named in the declaring mod's own namespace.
/// let block = declaration.entry(ContentKind::Block, "copper_lantern")?;
/// let item = declaration.entry(ContentKind::Item, "copper_lantern")?;
/// assert_eq!(block.to_string(), "block:example_mod:copper_lantern");
///
/// // The same name in two registries is two entries, not a duplicate.
/// assert_ne!(block, item);
///
/// // A tag in the shared convention namespace, holding this mod's item.
/// let lanterns = TagId::new(
///     ContentKind::Item,
///     ContentId::new("c".parse().expect("a legal mod id"), "lanterns")
///         .expect("a legal content path"),
/// )
/// .expect("items are taggable");
/// declaration.tag(&lanterns, &item)?;
///
/// // A name for it, in one locale.
/// declaration.translation("en_us", "block.example_mod.copper_lantern", "Copper Lantern")?;
///
/// // Generate tag, translation, and conventional block/item files.
/// let generated = declaration.generate();
/// assert_eq!(
///     generated.paths().collect::<Vec<_>>(),
///     [
///         "assets/example_mod/blockstates/copper_lantern.json",
///         "assets/example_mod/items/copper_lantern.json",
///         "assets/example_mod/lang/en_us.json",
///         "assets/example_mod/models/item/copper_lantern.json",
///         "data/c/tags/item/lanterns.json",
///         "data/example_mod/loot_table/blocks/copper_lantern.json",
///     ]
/// );
/// # Ok::<(), ContentDeclarationError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDeclaration {
    owner: ModId,
    entries: Vec<RegistryKey>,
    tags: BTreeMap<TagId, Vec<RegistryKey>>,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    resources: BTreeMap<RelativePath, Vec<u8>>,
    produced: BTreeMap<RelativePath, Producer>,
}

/// The producer claiming an output path; one tag or locale may add to its own claim.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Producer {
    /// The members of one tag.
    Tag(TagId),
    /// Every translation in one locale.
    Translations(String),
    /// Bytes the mod supplied itself.
    Supplied,
}

impl fmt::Display for Producer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag(tag) => write!(formatter, "the members of `{tag}`"),
            Self::Translations(locale) => write!(formatter, "the `{locale}` translations"),
            Self::Supplied => formatter.write_str("bytes the mod supplied"),
        }
    }
}

impl ContentDeclaration {
    /// Starts an empty declaration owned by `owner`.
    #[must_use]
    pub fn new(owner: ModId) -> Self {
        Self {
            owner,
            entries: Vec::new(),
            tags: BTreeMap::new(),
            translations: BTreeMap::new(),
            resources: BTreeMap::new(),
            produced: BTreeMap::new(),
        }
    }

    /// Returns the mod this declaration belongs to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Declares one registry entry of `kind`, at `path` in the owner's namespace.
    ///
    /// # Errors
    ///
    /// Returns an invalid-path or duplicate-entry error.
    pub fn entry(
        &mut self,
        kind: ContentKind,
        path: &str,
    ) -> Result<RegistryKey, ContentDeclarationError> {
        let id = ContentId::new(self.owner.clone(), path)
            .map_err(|source| ContentDeclarationError::Path { source })?;
        let key = RegistryKey::new(kind, id);
        if self.entries.contains(&key) {
            return Err(ContentDeclarationError::DuplicateEntry { key });
        }
        self.entries.push(key.clone());
        Ok(key)
    }

    /// Returns every entry this mod claims, in declaration order.
    #[must_use]
    pub fn entries(&self) -> &[RegistryKey] {
        &self.entries
    }

    /// Takes every claimed entry in declaration order without copying its identifiers.
    #[must_use]
    pub fn into_entries(self) -> Vec<RegistryKey> {
        self.entries
    }

    /// Declares `member` a member of `tag`.
    ///
    /// Members may belong to other mods.
    ///
    /// # Errors
    ///
    /// Returns kind mismatch, duplicate membership, or output-path collision errors.
    pub fn tag(
        &mut self,
        tag: &TagId,
        member: &RegistryKey,
    ) -> Result<(), ContentDeclarationError> {
        if !tag.admits(member) {
            return Err(ContentDeclarationError::TagKind {
                tag: tag.clone(),
                member: member.clone(),
            });
        }
        if self
            .tags
            .get(tag)
            .is_some_and(|members| members.contains(member))
        {
            return Err(ContentDeclarationError::DuplicateTagMember {
                tag: tag.clone(),
                member: member.clone(),
            });
        }
        // Claim before mutation so refusal leaves the declaration unchanged.
        Self::claim(
            &mut self.produced,
            tag.generated_path(),
            &Producer::Tag(tag.clone()),
        )?;
        self.tags
            .entry(tag.clone())
            .or_default()
            .push(member.clone());
        Ok(())
    }

    /// Returns every tag this mod contributes to, in name order.
    pub fn tags(&self) -> impl Iterator<Item = &TagId> + '_ {
        self.tags.keys()
    }

    /// Returns the members this mod puts in `tag`, in declaration order.
    #[must_use]
    pub fn tag_members(&self, tag: &TagId) -> &[RegistryKey] {
        self.tags.get(tag).map_or(&[], Vec::as_slice)
    }

    /// Declares the text one translation key has in one locale.
    ///
    /// Translation keys are supplied by the author, not derived from version conventions.
    ///
    /// # Errors
    ///
    /// Returns invalid locale, empty/duplicate key, or output-path collision errors.
    pub fn translation(
        &mut self,
        locale: &str,
        key: &str,
        text: &str,
    ) -> Result<(), ContentDeclarationError> {
        if !is_locale(locale) {
            return Err(ContentDeclarationError::Locale {
                candidate: locale.to_owned(),
            });
        }
        if key.is_empty() {
            return Err(ContentDeclarationError::EmptyTranslationKey);
        }

        if self
            .translations
            .get(locale)
            .is_some_and(|entries| entries.contains_key(key))
        {
            return Err(ContentDeclarationError::DuplicateTranslation {
                locale: locale.to_owned(),
                key: key.to_owned(),
            });
        }
        Self::claim(
            &mut self.produced,
            Self::translation_path(&self.owner, locale),
            &Producer::Translations(locale.to_owned()),
        )?;
        self.translations
            .entry(locale.to_owned())
            .or_default()
            .insert(key.to_owned(), text.to_owned());
        Ok(())
    }

    /// Returns the locales this mod supplies translations for, in name order.
    pub fn locales(&self) -> impl Iterator<Item = &str> + '_ {
        self.translations.keys().map(String::as_str)
    }

    /// Returns the text this mod gave `key` in `locale`, if it gave it any.
    #[must_use]
    pub fn translated(&self, locale: &str, key: &str) -> Option<&str> {
        self.translations
            .get(locale)
            .and_then(|entries| entries.get(key))
            .map(String::as_str)
    }

    /// Supplies `bytes` as the content of `path` in the generated pack.
    ///
    /// # Errors
    ///
    /// Returns duplicate-resource or output-path collision errors within this declaration.
    pub fn resource(
        &mut self,
        path: RelativePath,
        bytes: Vec<u8>,
    ) -> Result<(), ContentDeclarationError> {
        if self.resources.contains_key(&path) {
            return Err(ContentDeclarationError::DuplicateResource { path });
        }
        Self::claim(&mut self.produced, path.clone(), &Producer::Supplied)?;
        self.resources.insert(path, bytes);
        Ok(())
    }

    /// Returns the paths this mod supplies bytes for, in path order.
    pub fn resources(&self) -> impl Iterator<Item = &RelativePath> + '_ {
        self.resources.keys()
    }

    /// Generates the same path-ordered files regardless of declaration order.
    #[must_use]
    pub fn generate(&self) -> GeneratedData {
        let mut data = GeneratedData::default();

        // Supplied resources replace conventional defaults below.
        for (path, bytes) in self.conventional() {
            data.insert(path, bytes);
        }
        for (tag, members) in &self.tags {
            data.insert(
                tag.generated_path(),
                tag_document(
                    members
                        .iter()
                        .map(|member| member.id().to_string())
                        .collect(),
                ),
            );
        }
        for (locale, entries) in &self.translations {
            data.insert(
                Self::translation_path(&self.owner, locale),
                translation_document(entries),
            );
        }
        for (path, bytes) in &self.resources {
            data.insert(path.clone(), bytes.clone());
        }
        data
    }

    /// Conventional block/item files, including both supported item-model layouts.
    ///
    /// Authors may replace any of these defaults with supplied resources.
    fn conventional(&self) -> Vec<(RelativePath, Vec<u8>)> {
        let namespace = &self.owner;
        let blocks: BTreeSet<&str> = self
            .entries
            .iter()
            .filter(|entry| entry.kind() == ContentKind::Block)
            .map(|entry| entry.id().path())
            .collect();

        let mut files = Vec::new();
        for path in &blocks {
            files.push((
                Self::asset(namespace, &format!("blockstates/{path}.json")),
                blockstate_document(namespace.as_str(), path),
            ));
            files.push((
                Self::data(namespace, &format!("loot_table/blocks/{path}.json")),
                loot_table_document(namespace.as_str(), path),
            ));
        }
        for entry in &self.entries {
            if entry.kind() != ContentKind::Item {
                continue;
            }
            let path = entry.id().path();
            // An item sharing a block's name places that block, so it shows the
            // block's model rather than a flat sprite.
            let model = if blocks.contains(path) {
                format!("{namespace}:block/{path}")
            } else {
                format!("{namespace}:item/{path}")
            };
            files.push((
                Self::asset(namespace, &format!("models/item/{path}.json")),
                if blocks.contains(path) {
                    item_model_document(&model)
                } else {
                    sprite_model_document(&model)
                },
            ));
            files.push((
                Self::asset(namespace, &format!("items/{path}.json")),
                item_definition_document(&model),
            ));
        }
        files
    }

    /// Returns the conventional paths replaced by supplied resources.
    #[must_use]
    pub fn supplied_defaults(&self) -> Vec<RelativePath> {
        self.conventional()
            .into_iter()
            .map(|(path, _)| path)
            .filter(|path| self.resources.contains_key(path))
            .collect()
    }

    /// Builds an asset path from validated namespace and content components.
    fn asset(owner: &ModId, rest: &str) -> RelativePath {
        RelativePath::try_from(format!("assets/{owner}/{rest}"))
            .expect("a mod id and a content path cannot spell an illegal path")
    }

    /// Builds a data path from validated namespace and content components.
    fn data(owner: &ModId, rest: &str) -> RelativePath {
        RelativePath::try_from(format!("data/{owner}/{rest}"))
            .expect("a mod id and a content path cannot spell an illegal path")
    }

    /// Builds a translation path from a validated owner and locale.
    fn translation_path(owner: &ModId, locale: &str) -> RelativePath {
        RelativePath::try_from(format!("assets/{owner}/lang/{locale}.json"))
            .expect("a mod id and a locale cannot spell an illegal path")
    }

    /// Records that `writer` produces `path`, refusing a second writer for it.
    fn claim(
        table: &mut BTreeMap<RelativePath, Producer>,
        path: RelativePath,
        writer: &Producer,
    ) -> Result<(), ContentDeclarationError> {
        match table.get(&path) {
            Some(existing) if existing == writer => Ok(()),
            Some(existing) => Err(ContentDeclarationError::ProducedPath {
                path,
                existing: existing.to_string(),
                incoming: writer.to_string(),
            }),
            None => {
                table.insert(path, writer.clone());
                Ok(())
            }
        }
    }
}

/// Invalid or conflicting declarations within one mod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentDeclarationError {
    /// The declared path is not a legal content path.
    Path {
        /// Why the path was refused.
        source: ContentIdError,
    },
    /// The declaration already holds this entry.
    DuplicateEntry {
        /// The entry declared twice.
        key: RegistryKey,
    },
    /// The member is not of the kind the tag groups.
    TagKind {
        /// The tag that would not hold it.
        tag: TagId,
        /// The member of the wrong kind.
        member: RegistryKey,
    },
    /// This mod already put the member in the tag.
    DuplicateTagMember {
        /// The tag the member is already in.
        tag: TagId,
        /// The member declared twice.
        member: RegistryKey,
    },
    /// The locale is outside the pack format's `language[_region]` shape.
    Locale {
        /// The rejected spelling.
        candidate: String,
    },
    /// A translation was declared for an unnamed key.
    EmptyTranslationKey,
    /// This mod already gave the key text in the locale.
    DuplicateTranslation {
        /// The locale the key already has text in.
        locale: String,
        /// The key declared twice.
        key: String,
    },
    /// This mod already supplied bytes for the path.
    DuplicateResource {
        /// The path supplied twice.
        path: RelativePath,
    },
    /// Two of this mod's declarations would write the same file.
    ProducedPath {
        /// The contested output path.
        path: RelativePath,
        /// What already writes it.
        existing: String,
        /// What asked to write it.
        incoming: String,
    },
}

impl fmt::Display for ContentDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path { source } => write!(formatter, "{source}"),
            Self::DuplicateEntry { key } => write!(
                formatter,
                "`{key}` is already declared by this mod, so claiming it twice would contest it \
                 with itself"
            ),
            Self::TagKind { tag, member } => write!(
                formatter,
                "`{tag}` groups `{}` entries, so it cannot hold `{member}`",
                tag.kind()
            ),
            Self::DuplicateTagMember { tag, member } => {
                write!(formatter, "`{member}` is already a member of `{tag}`")
            }
            Self::Locale { candidate } => write!(
                formatter,
                "`{candidate}` is not a locale; a locale is a lowercase language and an optional \
                 `_region`, such as `en_us`, `es_419` or `lzh`"
            ),
            Self::EmptyTranslationKey => {
                formatter.write_str("a translation names a key, so the key cannot be empty")
            }
            Self::DuplicateTranslation { locale, key } => write!(
                formatter,
                "`{key}` already has `{locale}` text in this declaration"
            ),
            Self::DuplicateResource { path } => write!(
                formatter,
                "this mod already supplies `{path}`, and one path holds one file"
            ),
            Self::ProducedPath {
                path,
                existing,
                incoming,
            } => write!(
                formatter,
                "`{path}` would be written from {existing} and from {incoming}, and one path holds \
                 one file"
            ),
        }
    }
}

impl std::error::Error for ContentDeclarationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path { source } => Some(source),
            Self::DuplicateEntry { .. }
            | Self::TagKind { .. }
            | Self::DuplicateTagMember { .. }
            | Self::Locale { .. }
            | Self::EmptyTranslationKey
            | Self::DuplicateTranslation { .. }
            | Self::DuplicateResource { .. }
            | Self::ProducedPath { .. } => None,
        }
    }
}

/// Accepts ASCII lowercase language names with an optional lowercase/digit region.
fn is_locale(candidate: &str) -> bool {
    let mut segments = candidate.split('_');
    let language = segments.next().unwrap_or_default();
    let region = segments.next();
    if segments.next().is_some() {
        return false;
    }
    let language_is_a_language =
        !language.is_empty() && language.chars().all(|letter| letter.is_ascii_lowercase());
    let region_is_a_region = region.is_none_or(|region| {
        !region.is_empty()
            && region
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    });
    language_is_a_language && region_is_a_region
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, ContentIdError, ContentKind, ModId, RegistryKey, RelativePath};

    use super::{ContentDeclaration, ContentDeclarationError};
    use crate::generate::GeneratedFile;
    use crate::tags::TagId;

    fn owner() -> ModId {
        "example_mod".parse().expect("a legal mod id")
    }

    fn declaration() -> ContentDeclaration {
        ContentDeclaration::new(owner())
    }

    fn tag(kind: ContentKind, path: &str) -> TagId {
        TagId::new(
            kind,
            ContentId::new("c".parse().expect("a legal mod id"), path)
                .expect("a legal content path"),
        )
        .expect("the kind is taggable")
    }

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    #[test]
    fn an_empty_declaration_owns_its_mod_and_declares_nothing() {
        let declaration = declaration();
        assert_eq!(declaration.owner(), &owner());
        assert_eq!(declaration.entries(), []);
        assert_eq!(declaration.tags().count(), 0);
        assert_eq!(declaration.locales().count(), 0);
        assert_eq!(declaration.resources().count(), 0);
        assert!(declaration.generate().is_empty());
    }

    #[test]
    fn an_entry_is_named_in_the_owners_namespace_and_kept_in_declaration_order() {
        let mut declaration = declaration();
        let recipe = declaration
            .entry(ContentKind::Recipe, "crafting/lantern")
            .expect("a legal entry");
        let block = declaration
            .entry(ContentKind::Block, "lantern")
            .expect("a legal entry");

        assert_eq!(recipe.id().namespace(), &owner());
        assert_eq!(recipe.to_string(), "recipe:example_mod:crafting/lantern");
        assert_eq!(declaration.entries(), [recipe, block]);
    }

    #[test]
    fn the_same_path_in_two_registries_is_two_entries_and_the_same_one_twice_is_refused() {
        let mut declaration = declaration();
        let block = declaration
            .entry(ContentKind::Block, "lantern")
            .expect("a legal entry");
        declaration
            .entry(ContentKind::Item, "lantern")
            .expect("an item is a different registry");

        let error = declaration
            .entry(ContentKind::Block, "lantern")
            .expect_err("the block is already declared");
        assert_eq!(
            error,
            ContentDeclarationError::DuplicateEntry { key: block }
        );
        assert!(error.to_string().contains("with itself"), "{error}");
        assert_eq!(
            declaration.entries().len(),
            2,
            "the refusal recorded nothing"
        );
    }

    #[test]
    fn an_illegal_content_path_is_refused_with_the_reason_the_charset_gives() {
        let error = declaration()
            .entry(ContentKind::Block, "Copper")
            .expect_err("an uppercase path is illegal");
        assert_eq!(
            error,
            ContentDeclarationError::Path {
                source: ContentIdError::PathCharacter {
                    character: 'C',
                    index: 0,
                },
            }
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn a_tag_holds_the_members_of_its_own_kind_and_refuses_the_others() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let mut declaration = declaration();
        let block = declaration
            .entry(ContentKind::Block, "lantern")
            .expect("a legal entry");
        let item = declaration
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");

        declaration
            .tag(&lanterns, &item)
            .expect("an item joins an item tag");

        let error = declaration
            .tag(&lanterns, &block)
            .expect_err("a block does not join an item tag");
        assert_eq!(
            error,
            ContentDeclarationError::TagKind {
                tag: lanterns.clone(),
                member: block,
            }
        );
        assert!(error.to_string().contains("item"), "{error}");
        assert_eq!(declaration.tag_members(&lanterns), [item]);
    }

    #[test]
    fn a_member_declared_twice_in_one_tag_is_refused() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let mut declaration = declaration();
        let item = declaration
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");
        declaration.tag(&lanterns, &item).expect("the first join");

        let error = declaration
            .tag(&lanterns, &item)
            .expect_err("the second join");
        assert_eq!(
            error,
            ContentDeclarationError::DuplicateTagMember {
                tag: lanterns.clone(),
                member: item.clone(),
            }
        );
        assert_eq!(declaration.tag_members(&lanterns), [item]);
    }

    #[test]
    fn a_member_this_mod_did_not_declare_may_still_join_its_tag() {
        // Adding somebody else's item to a tag is the ordinary case.
        let lanterns = tag(ContentKind::Item, "lanterns");
        let foreign = RegistryKey::new(
            ContentKind::Item,
            ContentId::new("other_mod".parse().expect("a legal mod id"), "lantern")
                .expect("a legal content path"),
        );

        let mut declaration = declaration();
        declaration
            .tag(&lanterns, &foreign)
            .expect("a tag is not a claim on its members");
        assert_eq!(declaration.entries(), []);
        assert_eq!(declaration.tag_members(&lanterns), [foreign]);
    }

    #[test]
    fn a_tag_nothing_declared_has_no_members_rather_than_no_answer() {
        assert_eq!(
            declaration().tag_members(&tag(ContentKind::Item, "lanterns")),
            []
        );
    }

    #[test]
    fn a_locale_outside_the_pack_formats_shape_is_refused() {
        for candidate in [
            "",
            "en-US",
            "EN_US",
            "en us",
            "en1",
            // A third segment, an empty one, and a digit where a language goes:
            // the message promises a language and one optional region, so these
            // are the spellings that promise would be a lie about.
            "en_us_extra",
            "___",
            "_us",
            "en_",
            "419_es",
        ] {
            let error = declaration()
                .translation(candidate, "block.example_mod.lantern", "Lantern")
                .expect_err(candidate);
            assert_eq!(
                error,
                ContentDeclarationError::Locale {
                    candidate: candidate.to_owned(),
                }
            );
            assert!(error.to_string().contains("en_us"), "{error}");
        }
    }

    #[test]
    fn every_locale_shape_the_pack_format_ships_is_accepted() {
        // Cover digit regions and language-only names, not just `en_us`.
        for candidate in ["en_us", "fr_fr", "pt_br", "es_419", "lzh", "enws", "nds_de"] {
            declaration()
                .translation(candidate, "block.example_mod.lantern", "Lantern")
                .unwrap_or_else(|error| panic!("`{candidate}` is a locale: {error}"));
        }
    }

    #[test]
    fn a_translation_names_a_key_and_the_same_key_twice_is_refused() {
        let mut declaration = declaration();
        assert_eq!(
            declaration.translation("en_us", "", "Lantern"),
            Err(ContentDeclarationError::EmptyTranslationKey)
        );

        declaration
            .translation("en_us", "block.example_mod.lantern", "Lantern")
            .expect("the first text");
        declaration
            .translation("fr_fr", "block.example_mod.lantern", "Lanterne")
            .expect("another locale is another file");

        let error = declaration
            .translation("en_us", "block.example_mod.lantern", "Lamp")
            .expect_err("the key already has English text");
        assert_eq!(
            error,
            ContentDeclarationError::DuplicateTranslation {
                locale: "en_us".to_owned(),
                key: "block.example_mod.lantern".to_owned(),
            }
        );

        assert_eq!(
            declaration.translated("en_us", "block.example_mod.lantern"),
            Some("Lantern"),
            "the refusal left the first text in place"
        );
        assert_eq!(
            declaration.locales().collect::<Vec<_>>(),
            ["en_us", "fr_fr"]
        );
        assert_eq!(declaration.translated("de_de", "anything"), None);
    }

    #[test]
    fn a_supplied_path_is_kept_once_and_the_second_supply_is_refused() {
        let model = path("assets/example_mod/models/block/lantern.json");
        let mut declaration = declaration();
        declaration
            .resource(model.clone(), b"{}".to_vec())
            .expect("the first supply");

        let error = declaration
            .resource(model.clone(), b"{\"changed\": true}".to_vec())
            .expect_err("the second supply");
        assert_eq!(
            error,
            ContentDeclarationError::DuplicateResource {
                path: model.clone()
            }
        );
        assert_eq!(declaration.resources().collect::<Vec<_>>(), [&model]);
        assert_eq!(
            declaration
                .generate()
                .file(&model)
                .map(GeneratedFile::bytes),
            Some(b"{}".as_slice()),
            "the refusal did not overwrite the first supply"
        );
    }

    #[test]
    fn a_supplied_path_a_tag_would_be_generated_into_is_refused_in_either_order() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let generated = lanterns.generated_path();
        let member = RegistryKey::new(
            ContentKind::Item,
            ContentId::new(owner(), "lantern").expect("a legal content path"),
        );

        // Supplied first, tagged second.
        let mut supplied_first = declaration();
        supplied_first
            .resource(generated.clone(), b"{}".to_vec())
            .expect("nothing writes it yet");
        let error = supplied_first
            .tag(&lanterns, &member)
            .expect_err("the file is already supplied");
        assert!(matches!(
            error,
            ContentDeclarationError::ProducedPath { ref path, .. } if path == &generated
        ));
        assert!(
            supplied_first.tags().next().is_none(),
            "a refused tag left no half-declared group behind"
        );

        // Tagged first, supplied second.
        let mut tagged_first = declaration();
        tagged_first
            .tag(&lanterns, &member)
            .expect("nothing writes it yet");
        let error = tagged_first
            .resource(generated, b"{}".to_vec())
            .expect_err("the tag already writes it");
        let text = error.to_string();
        assert!(text.contains("one path holds one file"), "{text}");
        assert!(text.contains("tag:item:c:lanterns"), "{text}");
        assert_eq!(tagged_first.resources().count(), 0);
    }

    #[test]
    fn a_supplied_path_a_locale_would_be_generated_into_is_refused() {
        let lang = path("assets/example_mod/lang/en_us.json");
        let mut declaration = declaration();
        declaration
            .translation("en_us", "block.example_mod.lantern", "Lantern")
            .expect("the first text");

        let error = declaration
            .resource(lang.clone(), b"{}".to_vec())
            .expect_err("the locale already writes it");
        assert!(matches!(
            error,
            ContentDeclarationError::ProducedPath { ref path, .. } if path == &lang
        ));
        assert!(
            error.to_string().contains("en_us"),
            "the refusal names the producer: {error}"
        );
    }

    #[test]
    fn two_tags_of_different_kinds_sharing_a_name_are_generated_into_two_files() {
        let items = tag(ContentKind::Item, "lanterns");
        let blocks = tag(ContentKind::Block, "lanterns");
        let mut declaration = declaration();
        let item = declaration
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");
        let block = declaration
            .entry(ContentKind::Block, "lantern")
            .expect("a legal entry");

        declaration.tag(&items, &item).expect("an item tag");
        declaration.tag(&blocks, &block).expect("a block tag");

        // The two tag files are the claim, and the four beside them are the
        // conventional files a block and its item need, which this declaration
        // gets for having declared them.
        assert_eq!(
            declaration.generate().paths().collect::<Vec<_>>(),
            [
                "assets/example_mod/blockstates/lantern.json",
                "assets/example_mod/items/lantern.json",
                "assets/example_mod/models/item/lantern.json",
                "data/c/tags/block/lanterns.json",
                "data/c/tags/item/lanterns.json",
                "data/example_mod/loot_table/blocks/lantern.json",
            ]
        );
    }

    #[test]
    fn an_author_supplied_default_replaces_the_generated_one_and_is_recorded() {
        let owner: ModId = "example_mod".parse().expect("a legal mod id");
        let mut declaration = ContentDeclaration::new(owner);
        declaration
            .entry(ContentKind::Block, "lantern")
            .expect("a legal entry");
        let path = RelativePath::try_from("assets/example_mod/blockstates/lantern.json".to_owned())
            .expect("a legal path");
        let mine = b"{\n  \"variants\": {\n    \"lit=true\": {}\n  }\n}\n".to_vec();
        declaration
            .resource(path.clone(), mine.clone())
            .expect("nothing else in this declaration writes that path");

        let generated = declaration.generate();

        assert_eq!(
            generated.file(&path).map(GeneratedFile::bytes),
            Some(mine.as_slice()),
            "the author's bytes, not the generated ones"
        );
        assert_eq!(declaration.supplied_defaults(), [path]);
        // The loot table beside it was not supplied, so it is still Ravel's.
        let loot =
            RelativePath::try_from("data/example_mod/loot_table/blocks/lantern.json".to_owned())
                .expect("a legal path");
        assert!(generated.file(&loot).is_some());
    }

    #[test]
    fn a_plain_item_is_generated_as_a_sprite_and_a_blocks_item_as_the_block() {
        let owner: ModId = "example_mod".parse().expect("a legal mod id");
        let mut declaration = ContentDeclaration::new(owner);
        declaration
            .entry(ContentKind::Block, "lantern")
            .expect("a legal entry");
        declaration
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");
        declaration
            .entry(ContentKind::Item, "dust")
            .expect("a legal entry");

        let generated = declaration.generate();
        let read = |path: &str| {
            let path = RelativePath::try_from(path.to_owned()).expect("a legal path");
            String::from_utf8(
                generated
                    .file(&path)
                    .unwrap_or_else(|| panic!("{path} is generated"))
                    .bytes()
                    .to_vec(),
            )
            .expect("generated JSON is UTF-8")
        };

        assert!(
            read("assets/example_mod/models/item/lantern.json")
                .contains("\"parent\": \"example_mod:block/lantern\""),
            "an item sharing a block's name shows that block"
        );
        assert!(
            read("assets/example_mod/models/item/dust.json")
                .contains("\"parent\": \"minecraft:item/generated\""),
            "an item no block claims is a flat sprite"
        );
        assert!(
            read("assets/example_mod/models/item/dust.json")
                .contains("\"layer0\": \"example_mod:item/dust\"")
        );
        assert!(
            read("assets/example_mod/items/dust.json")
                .contains("\"model\": \"example_mod:item/dust\"")
        );
    }

    #[test]
    fn a_declared_recipe_generates_nothing_because_its_body_is_a_versions_shape() {
        let owner: ModId = "example_mod".parse().expect("a legal mod id");
        let mut declaration = ContentDeclaration::new(owner);
        declaration
            .entry(ContentKind::Recipe, "crafting/lantern")
            .expect("a legal entry");
        declaration
            .entry(ContentKind::Command, "lanterns")
            .expect("a legal entry");

        assert!(declaration.generate().is_empty());
    }
}
