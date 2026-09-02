use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ContentId, ContentIdError, ContentKind, ModId, RegistryKey, RelativePath};

use crate::generate::{
    GeneratedData, blockstate_document, item_definition_document, item_model_document,
    loot_table_document, sprite_model_document, tag_document, translation_document,
};
use crate::tags::TagId;

/// Everything one mod contributes to the game's content, declared in one place.
///
/// A mod declares three kinds of thing and they behave differently once several
/// mods are installed, which is why they are one type here rather than three
/// unrelated calls:
///
/// - a **registry entry** has exactly one owner, so declaring it is a claim and
///   the second claim on it is a conflict the registration barrier refuses;
/// - a **tag membership** has no owner, so every mod's contributions to
///   `c:ingots` are kept and merged by [`crate::TagSet`];
/// - a **resource** has one supplier per path, so the last mod to declare a path
///   supplies it and [`crate::ResourceSet`] records which mods it shadowed.
///
/// Declaring is separate from claiming. This type refuses what one mod can get
/// wrong on its own — the same entry twice, a member of the wrong kind, a
/// translation the mod already wrote, a file two of its own declarations would
/// both produce — and it does so at the point the mistake is made, with no
/// loader and no game involved. What it cannot know is what *other* mods
/// declared, so every cross-mod decision stays with the barrier and the layered
/// sets, and nothing here second-guesses them.
///
/// Every refusal names the identifier or path the mod itself declared. A mod
/// that turns one into a [`ravel_core::StructuredFailure`] does so through the
/// host error whose detail is already marked as text a mod wrote, so the report
/// surface redacts it on the way in rather than trusting this crate's wording.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentId, ContentKind, ModId};
/// use ravel_data::{ContentDeclaration, DeclarationError, TagId};
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
/// // The tag and the translation, plus the files a version needs for a block and
/// // its item that nobody supplied: where the block's model is, what its item
/// // looks like in a hand, and what breaking it drops.
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
/// # Ok::<(), DeclarationError>(())
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

/// What a declaration would write one output path from.
///
/// The declaration keeps one table of output paths across all three kinds of
/// declaration, because the question "does something already write this file"
/// has one answer and asking it three times is how the answers disagree. The
/// producer is compared rather than merely counted, so a second translation in a
/// locale the mod already writes finds its own claim and a supplied resource on
/// the same path does not.
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
    /// The namespace is the owner's rather than an argument, which is what stops
    /// a mod declaring content into a namespace it does not own by accident. A
    /// pack that deliberately replaces another mod's entry is a pack-authoring
    /// question M4-W5 answers with `ravel-pack`, not something a mod does by
    /// spelling a different namespace here.
    ///
    /// # Errors
    ///
    /// Returns [`DeclarationError::Path`] when `path` is not a legal content
    /// path, and [`DeclarationError::DuplicateEntry`] when this declaration
    /// already holds the same kind and path. A duplicate is refused here rather
    /// than at the barrier because the barrier would report it as a conflict
    /// with the mod itself, which names no one to act on it.
    pub fn entry(
        &mut self,
        kind: ContentKind,
        path: &str,
    ) -> Result<RegistryKey, DeclarationError> {
        let id = ContentId::new(self.owner.clone(), path)
            .map_err(|source| DeclarationError::Path { source })?;
        let key = RegistryKey::new(kind, id);
        if self.entries.contains(&key) {
            return Err(DeclarationError::DuplicateEntry { key });
        }
        self.entries.push(key.clone());
        Ok(key)
    }

    /// Returns every entry this mod claims, in declaration order.
    ///
    /// Declaration order rather than sorted order, because this is the order the
    /// keys reach [`ravel_core::RegistryKey`]'s barrier and a mod that reads its
    /// own refusal wants the sequence it wrote.
    #[must_use]
    pub fn entries(&self) -> &[RegistryKey] {
        &self.entries
    }

    /// Declares `member` a member of `tag`.
    ///
    /// The member may be an entry this mod did not declare. Adding a vanilla
    /// item to a tag is the ordinary case, and requiring membership to be
    /// declared locally would forbid it.
    ///
    /// # Errors
    ///
    /// Returns [`DeclarationError::TagKind`] when the member is not of the kind
    /// the tag groups, [`DeclarationError::DuplicateTagMember`] when this mod
    /// already put the member in the tag, and
    /// [`DeclarationError::ProducedPath`] when something else this mod declared
    /// already writes the file the tag would be generated into.
    pub fn tag(&mut self, tag: &TagId, member: &RegistryKey) -> Result<(), DeclarationError> {
        if !tag.admits(member) {
            return Err(DeclarationError::TagKind {
                tag: tag.clone(),
                member: member.clone(),
            });
        }
        if self
            .tags
            .get(tag)
            .is_some_and(|members| members.contains(member))
        {
            return Err(DeclarationError::DuplicateTagMember {
                tag: tag.clone(),
                member: member.clone(),
            });
        }
        // Every refusal happens before anything is recorded, so a refused
        // declaration leaves no half-declared group behind.
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
    /// The key is the author's, not Ravel's. A translation key's shape is a
    /// convention the game decides — `block.<namespace>.<path>` today — and
    /// deriving it here would put a version's convention in a version-neutral
    /// crate and be wrong the first time the game changed it.
    ///
    /// # Errors
    ///
    /// Returns [`DeclarationError::Locale`] for a locale outside the lowercase
    /// `language` or `language_region` shape the pack format uses — the shape
    /// [`is_locale`] states and enforces —
    /// [`DeclarationError::EmptyTranslationKey`] for an unnamed key,
    /// [`DeclarationError::DuplicateTranslation`] when this mod already gave the
    /// key text in that locale, and [`DeclarationError::ProducedPath`] when
    /// something else this mod declared already writes the locale's file.
    pub fn translation(
        &mut self,
        locale: &str,
        key: &str,
        text: &str,
    ) -> Result<(), DeclarationError> {
        if !is_locale(locale) {
            return Err(DeclarationError::Locale {
                candidate: locale.to_owned(),
            });
        }
        if key.is_empty() {
            return Err(DeclarationError::EmptyTranslationKey);
        }

        if self
            .translations
            .get(locale)
            .is_some_and(|entries| entries.contains_key(key))
        {
            return Err(DeclarationError::DuplicateTranslation {
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
    /// This is the escape hatch for content Ravel does not model — a model, a
    /// texture, a recipe body whose shape a version decides. The bytes are
    /// carried into the same output the derived files land in, so a mod's pack
    /// is one deterministic set rather than a generated half and a hand-written
    /// half that can disagree about a path.
    ///
    /// # Errors
    ///
    /// Returns [`DeclarationError::DuplicateResource`] when this mod already
    /// supplied the path, and [`DeclarationError::ProducedPath`] when a tag or a
    /// locale this mod declared already writes it. Two mods supplying one path
    /// is not refused anywhere: that is an override, and
    /// [`crate::ResourceSet`] records it.
    pub fn resource(&mut self, path: RelativePath, bytes: Vec<u8>) -> Result<(), DeclarationError> {
        if self.resources.contains_key(&path) {
            return Err(DeclarationError::DuplicateResource { path });
        }
        Self::claim(&mut self.produced, path.clone(), &Producer::Supplied)?;
        self.resources.insert(path, bytes);
        Ok(())
    }

    /// Returns the paths this mod supplies bytes for, in path order.
    pub fn resources(&self) -> impl Iterator<Item = &RelativePath> + '_ {
        self.resources.keys()
    }

    /// Produces the pack this declaration describes.
    ///
    /// The result is a pure function of what was declared and not of the order
    /// it was declared in: every collection read here is keyed or sorted by
    /// content, and [`GeneratedData`] orders its files by path. That is what
    /// makes generating twice — in one process or in two — produce the same
    /// bytes, which `tests/conformance/tests/content_declarations.rs` shows by
    /// declaring the same content in two orders and comparing the output.
    #[must_use]
    pub fn generate(&self) -> GeneratedData {
        let mut data = GeneratedData::default();

        // First, so that anything the author supplied at the same path replaces
        // it below. That ordering is the whole of the precedence rule: a default
        // is what an author did not write, and `GeneratedData::insert` keeps the
        // last bytes recorded for a path.
        for (path, bytes) in self.conventional() {
            data.insert(path, bytes);
        }
        for (tag, members) in &self.tags {
            let mut spellings: Vec<String> = members
                .iter()
                .map(|member| member.id().to_string())
                .collect();
            spellings.sort();
            data.insert(tag.generated_path(), tag_document(&spellings));
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

    /// The files a version needs for the declared content that nobody supplied.
    ///
    /// A block is three files, not one. The author writes the block model,
    /// because its shape is theirs; what a version needs beside it is a
    /// blockstates file mapping the block's one variant to that model, an item
    /// model so the block can be held and seen in an inventory, and a loot table
    /// so breaking it drops something. None of those has a choice in it for a
    /// block with one state, so an author writing them by hand would be writing
    /// the same four lines every time and getting a placeholder texture and an
    /// empty drop when they forgot. Ravel writes them, and a declaration stays a
    /// declaration.
    ///
    /// Every path here is one an author may take back simply by supplying it:
    /// these are inserted before the supplied resources, so the author's bytes
    /// replace them. [`ContentDeclaration::supplied_defaults`] says which.
    ///
    /// # Two spellings of an item's model, on purpose
    ///
    /// The tracked versions disagree about how an item finds its model. The
    /// older reads `models/item/<name>.json` directly; the newer reads an item
    /// definition at `items/<name>.json` that names a model. Both are written,
    /// because this crate is version-neutral and has no way to ask which version
    /// is running — `docs/architecture/dependency-rules.md` is what keeps it that
    /// way. The cost is one file the running version never looks up, which is a
    /// path nothing resolves rather than a wrong answer; the alternative was
    /// threading version data into the crate a mod's declaration lives in.
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

    /// Returns the conventional paths the author supplied rather than inherited.
    ///
    /// A default nobody took back is invisible and a default an author replaced
    /// is a decision they made, so a pack that ships one of each can say which is
    /// which rather than leaving a reader to diff the bytes.
    #[must_use]
    pub fn supplied_defaults(&self) -> Vec<RelativePath> {
        self.conventional()
            .into_iter()
            .map(|(path, _)| path)
            .filter(|path| self.resources.contains_key(path))
            .collect()
    }

    /// One path below this pack's `assets` half, in the owner's namespace.
    ///
    /// # Panics
    ///
    /// Panics if the derived path is not a legal [`RelativePath`], which cannot
    /// happen: a [`ModId`] and a validated content path hold no backslash, no
    /// line terminator and no `..` component.
    fn asset(owner: &ModId, rest: &str) -> RelativePath {
        RelativePath::try_from(format!("assets/{owner}/{rest}"))
            .expect("a mod id and a content path cannot spell an illegal path")
    }

    /// One path below this pack's `data` half, in the owner's namespace.
    ///
    /// # Panics
    ///
    /// Panics for the reason [`ContentDeclaration::asset`] does, and as rarely.
    fn data(owner: &ModId, rest: &str) -> RelativePath {
        RelativePath::try_from(format!("data/{owner}/{rest}"))
            .expect("a mod id and a content path cannot spell an illegal path")
    }

    /// Returns the path a locale's translations are generated into.
    ///
    /// # Panics
    ///
    /// Panics if the derived path is not a legal [`RelativePath`], which cannot
    /// happen: a [`ModId`] and a validated locale hold no backslash, no line
    /// terminator and no `..` component.
    fn translation_path(owner: &ModId, locale: &str) -> RelativePath {
        RelativePath::try_from(format!("assets/{owner}/lang/{locale}.json"))
            .expect("a mod id and a locale cannot spell an illegal path")
    }

    /// Records that `writer` produces `path`, refusing a second writer for it.
    fn claim(
        table: &mut BTreeMap<RelativePath, Producer>,
        path: RelativePath,
        writer: &Producer,
    ) -> Result<(), DeclarationError> {
        match table.get(&path) {
            Some(existing) if existing == writer => Ok(()),
            Some(existing) => Err(DeclarationError::ProducedPath {
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

/// Why a mod's own declaration was refused.
///
/// Every variant is something one mod can get wrong without another mod
/// existing. Nothing here is a cross-mod outcome: a contested registry entry is
/// the barrier's [`ravel_core::FailureKind::ConflictingRegistryClaim`], and a
/// contested resource path is an override rather than a failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationError {
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

impl fmt::Display for DeclarationError {
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

impl std::error::Error for DeclarationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path { source } => Some(source),
            _ => None,
        }
    }
}

/// Returns whether `candidate` is a locale of the shape the pack format names
/// its language files after.
///
/// A language, optionally followed by `_` and a region: the language is ASCII
/// lowercase letters and the region is ASCII lowercase letters or digits, so
/// `en_us`, `lzh` and `es_419` are locales and `en_us_extra`, `___`, `en-US` and
/// `en1` are not.
///
/// The region admits digits and neither segment carries a length, because both
/// are the pack format's decisions rather than this crate's: a rule tighter than
/// the format's would refuse a locale the game ships. What this refuses is a
/// spelling the format has no reading for at all — an extra segment, an empty
/// one, or a character that cannot appear in either.
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

    use super::{ContentDeclaration, DeclarationError};
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
        assert!(declaration.entries().is_empty());
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
        assert_eq!(error, DeclarationError::DuplicateEntry { key: block });
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
            DeclarationError::Path {
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
            DeclarationError::TagKind {
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
            DeclarationError::DuplicateTagMember {
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
        assert!(declaration.entries().is_empty());
        assert_eq!(declaration.tag_members(&lanterns), [foreign]);
    }

    #[test]
    fn a_tag_nothing_declared_has_no_members_rather_than_no_answer() {
        assert!(
            declaration()
                .tag_members(&tag(ContentKind::Item, "lanterns"))
                .is_empty()
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
                DeclarationError::Locale {
                    candidate: candidate.to_owned(),
                }
            );
            assert!(error.to_string().contains("en_us"), "{error}");
        }
    }

    #[test]
    fn every_locale_shape_the_pack_format_ships_is_accepted() {
        // The negative control for the test above: the refusals are a shape check
        // rather than a list of two spellings. A region carries digits and a
        // locale need not carry a region at all, so a rule tighter than this one
        // would refuse a language file the game itself ships.
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
            Err(DeclarationError::EmptyTranslationKey)
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
            DeclarationError::DuplicateTranslation {
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
            DeclarationError::DuplicateResource {
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
            DeclarationError::ProducedPath { ref path, .. } if path == &generated
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
            .resource(generated.clone(), b"{}".to_vec())
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
            DeclarationError::ProducedPath { ref path, .. } if path == &lang
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

    /// A default an author supplied is the author's, and is named as such.
    ///
    /// The precedence is the point. A mod that ships a blockstates file with two
    /// variants has decided something Ravel cannot, so the generated one must not
    /// be what lands in the pack.
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
            generated.file(&path).map(crate::GeneratedFile::bytes),
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

    /// An item nothing else claims is a flat sprite rather than a block.
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

    /// A recipe entry writes no file, and that is the decision rather than a gap.
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
