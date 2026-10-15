use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ContentId, ContentKind, ModId, RegistryKey, RelativePath};

use crate::declare::ContentDeclaration;
use crate::generate::{GeneratedData, tag_document};

/// A named group of registry entries.
///
/// A tag is the third thing a mod declares and the only one whose contributions
/// *add up*. A registry key has exactly one owner, so the second claim on it is
/// a conflict the barrier refuses. A resource path has exactly one supplier, so
/// the last mod to declare it wins and the ones it shadowed are recorded. A tag
/// has no owner at all: `c:ingots` is a name several mods contribute members to
/// on purpose, and refusing the second contribution — or letting it replace the
/// first — would break the one thing tags exist for, which is letting a recipe
/// accept an ingot nobody has written yet.
///
/// The kind is part of the name for the reason it is part of a [`RegistryKey`]:
/// the game keeps one tag namespace per registry, so a block tag and an item tag
/// may share `c:lanterns` and mean two different groups. It also fixes what a
/// member may be — an item tag holds items — which is what
/// [`crate::DeclarationError::TagKind`] refuses.
///
/// [`ContentKind::Command`] is not taggable. A tag exists so content can be
/// looked up as a group by a recipe, a loot table or a game rule, and none of
/// those looks up a command; a command tag would be a name with no reader.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentId, ContentKind, ModId};
/// use ravel_data::{TagError, TagId};
///
/// let convention: ModId = "c".parse().expect("a legal mod id");
/// let lanterns = TagId::new(
///     ContentKind::Item,
///     ContentId::new(convention, "lanterns").expect("a legal content path"),
/// )?;
/// assert_eq!(lanterns.to_string(), "tag:item:c:lanterns");
/// assert_eq!(
///     lanterns.generated_path().as_str(),
///     "data/c/tags/item/lanterns.json"
/// );
///
/// // The same name in the block registry is a different group.
/// let blocks = TagId::new(ContentKind::Block, lanterns.id().clone())?;
/// assert_ne!(lanterns, blocks);
/// # Ok::<(), TagError>(())
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TagId {
    kind: ContentKind,
    id: ContentId,
}

impl TagId {
    /// Names the group of `kind` entries called `id`.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::UntaggableKind`] for a kind nothing groups.
    pub fn new(kind: ContentKind, id: ContentId) -> Result<Self, TagError> {
        if matches!(kind, ContentKind::Command) {
            return Err(TagError::UntaggableKind { kind });
        }
        Ok(Self { kind, id })
    }

    /// Returns which registry's entries this tag groups.
    #[must_use]
    pub const fn kind(&self) -> ContentKind {
        self.kind
    }

    /// Returns what the group is called.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.id
    }

    /// Returns whether `member` may be a member of this tag.
    #[must_use]
    pub fn admits(&self, member: &RegistryKey) -> bool {
        member.kind() == self.kind
    }

    /// Returns the path in a generated pack that holds this tag's members.
    ///
    /// The layout is the data-pack layout — `data/<namespace>/tags/<registry>/<path>.json`
    /// — which is shared across the Minecraft versions Ravel targets rather than
    /// pinned to one of them, so deriving it here does not put a version detail
    /// in a version-neutral crate. Anything that *is* version-shaped stays out:
    /// this crate generates no recipe body and no translation key, because both
    /// are shapes a version decides and an author or an adapter supplies.
    ///
    /// The path is derived rather than declared, which is what makes generated
    /// output impossible to misfile: a mod names the tag and Ravel decides where
    /// it lands.
    ///
    /// # Panics
    ///
    /// Panics if the derived path is not a legal [`RelativePath`], which cannot
    /// happen: a [`ContentId`]'s charset holds no backslash, no line terminator
    /// and no `..` component, and the prefix is this crate's own literal.
    #[must_use]
    pub fn generated_path(&self) -> RelativePath {
        let path = format!(
            "data/{}/tags/{}/{}.json",
            self.id.namespace(),
            self.kind.as_str(),
            self.id.path()
        );
        RelativePath::try_from(path).expect("a content id cannot spell an illegal path")
    }
}

impl fmt::Display for TagId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "tag:{}:{}", self.kind, self.id)
    }
}

/// Why a candidate is not a valid [`TagId`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TagError {
    /// Nothing groups entries of this kind.
    UntaggableKind {
        /// The kind that has no tag namespace.
        kind: ContentKind,
    },
}

impl fmt::Display for TagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UntaggableKind { kind } => write!(
                formatter,
                "`{kind}` entries are not grouped by tags, so a `{kind}` tag would have no reader"
            ),
        }
    }
}

impl std::error::Error for TagError {}

/// Every tag an installed set declares, with the contributions merged.
///
/// This is the tag counterpart of [`crate::ResourceSet`], and the contrast is
/// the point of both. A contested resource path has one winner and the mods it
/// shadowed; a tag has no winner, because every contribution is kept. The
/// caller supplies declarations in resolved load order, which fixes the order
/// contributors are named in, and the members of one tag are ordered by their
/// canonical spelling rather than by who declared them — a tag is a set, and a
/// reader that saw a different order for the same set would be reading the load
/// order out of a value that is not about load order.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentId, ContentKind, ModId};
/// use ravel_data::{ContentDeclaration, TagId, TagSet};
///
/// fn declare(owner: &str, path: &str) -> ContentDeclaration {
///     let owner: ModId = owner.parse().expect("a legal mod id");
///     let tag = TagId::new(
///         ContentKind::Item,
///         ContentId::new("c".parse().expect("a legal mod id"), "ingots")
///             .expect("a legal content path"),
///     )
///     .expect("items are taggable");
///
///     let mut declaration = ContentDeclaration::new(owner);
///     let ingot = declaration
///         .entry(ContentKind::Item, path)
///         .expect("a legal, undeclared entry");
///     declaration.tag(&tag, &ingot).expect("an item joins an item tag");
///     declaration
/// }
///
/// let first = declare("first_mod", "copper_ingot");
/// let second = declare("second_mod", "tin_ingot");
/// let set = TagSet::merged([&first, &second]);
///
/// let tag = first.tags().next().expect("one tag was declared");
/// assert_eq!(set.members(tag).len(), 2, "both contributions are kept");
/// assert_eq!(set.contributors(tag).len(), 2);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TagSet {
    members: BTreeMap<TagId, Vec<RegistryKey>>,
    contributors: BTreeMap<TagId, Vec<ModId>>,
}

impl TagSet {
    /// Merges every declaration's tag contributions, in the order given.
    #[must_use]
    pub fn merged<'a, D>(declarations: D) -> Self
    where
        D: IntoIterator<Item = &'a ContentDeclaration>,
    {
        let mut set = Self::default();
        for declaration in declarations {
            for tag in declaration.tags() {
                let contributors = set.contributors.entry(tag.clone()).or_default();
                if !contributors.contains(declaration.owner()) {
                    contributors.push(declaration.owner().clone());
                }

                let members = set.members.entry(tag.clone()).or_default();
                for member in declaration.tag_members(tag) {
                    if !members.contains(member) {
                        members.push(member.clone());
                    }
                }
            }
        }
        for members in set.members.values_mut() {
            members.sort_by_key(RegistryKey::to_string);
        }
        set
    }

    /// Returns every tag the set declares, in name order.
    pub fn tags(&self) -> impl Iterator<Item = &TagId> + '_ {
        self.members.keys()
    }

    /// Returns the members of `tag`, in canonical spelling order.
    ///
    /// A tag nobody declared has no members rather than no answer, because a
    /// reader asking what is in `c:ingots` when no mod supplies ingots wants an
    /// empty group.
    #[must_use]
    pub fn members(&self, tag: &TagId) -> &[RegistryKey] {
        self.members.get(tag).map_or(&[], Vec::as_slice)
    }

    /// Returns the mods that contributed to `tag`, in load order.
    #[must_use]
    pub fn contributors(&self, tag: &TagId) -> &[ModId] {
        self.contributors.get(tag).map_or(&[], Vec::as_slice)
    }

    /// Returns whether the set declares any tag at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Produces one file per tag, holding every contribution.
    ///
    /// [`ContentDeclaration::generate`] renders one mod's view of a tag, which is
    /// the right answer for one mod and the wrong one for an installation: two mods
    /// contributing to `c:ingots` derive the same path, so writing both would make
    /// the later file replace the earlier and lose exactly the contributions a tag
    /// exists to collect. Merging first and rendering once is what keeps them.
    ///
    /// The rendering is the same function [`ContentDeclaration::generate`] uses, so
    /// a merged tag file and a single mod's tag file cannot disagree about format.
    #[must_use]
    pub fn generate(&self) -> GeneratedData {
        let mut data = GeneratedData::default();
        for (tag, members) in &self.members {
            let mut spellings: Vec<String> = members
                .iter()
                .map(|member| member.id().to_string())
                .collect();
            spellings.sort();
            data.insert(tag.generated_path(), tag_document(&spellings));
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, ContentKind, ModId, RegistryKey};

    use super::{TagError, TagId, TagSet};
    use crate::declare::ContentDeclaration;

    fn id(namespace: &str, path: &str) -> ContentId {
        ContentId::new(
            namespace.parse::<ModId>().expect("a legal mod id"),
            path.to_owned(),
        )
        .expect("a legal content path")
    }

    fn tag(kind: ContentKind, path: &str) -> TagId {
        TagId::new(kind, id("c", path)).expect("the kind is taggable")
    }

    /// A mod that puts one entry of `kind` into `into`.
    fn contributor(owner: &str, kind: ContentKind, path: &str, into: &TagId) -> ContentDeclaration {
        let mut declaration =
            ContentDeclaration::new(owner.parse::<ModId>().expect("a legal mod id"));
        let key = declaration
            .entry(kind, path)
            .expect("a legal, undeclared entry");
        declaration
            .tag(into, &key)
            .expect("the member matches the tag's kind");
        declaration
    }

    #[test]
    fn a_tag_is_named_by_the_registry_it_groups_and_the_id_it_carries() {
        let items = tag(ContentKind::Item, "lanterns");
        assert_eq!(items.kind(), ContentKind::Item);
        assert_eq!(items.id(), &id("c", "lanterns"));
        assert_eq!(items.to_string(), "tag:item:c:lanterns");

        let blocks =
            TagId::new(ContentKind::Block, id("c", "lanterns")).expect("blocks are tagged");
        assert_ne!(items, blocks);
        assert_ne!(items.generated_path(), blocks.generated_path());
    }

    #[test]
    fn a_generated_path_names_the_registry_directory_the_pack_format_uses() {
        for (kind, expected) in [
            (ContentKind::Block, "data/c/tags/block/lanterns.json"),
            (ContentKind::Item, "data/c/tags/item/lanterns.json"),
            (ContentKind::Recipe, "data/c/tags/recipe/lanterns.json"),
        ] {
            assert_eq!(tag(kind, "lanterns").generated_path().as_str(), expected);
        }

        // A nested tag path keeps its nesting, so two groups cannot land on one
        // generated file.
        assert_eq!(
            tag(ContentKind::Item, "metals/copper")
                .generated_path()
                .as_str(),
            "data/c/tags/item/metals/copper.json"
        );
    }

    #[test]
    fn a_command_cannot_be_tagged_and_the_refusal_says_why() {
        let error =
            TagId::new(ContentKind::Command, id("c", "homes")).expect_err("commands are untagged");
        assert_eq!(
            error,
            TagError::UntaggableKind {
                kind: ContentKind::Command
            }
        );
        let text = error.to_string();
        assert!(text.contains("command"), "{text}");
        assert!(text.contains("no reader"), "{text}");
    }

    #[test]
    fn a_tag_admits_only_the_kind_it_groups() {
        let items = tag(ContentKind::Item, "lanterns");
        assert!(items.admits(&RegistryKey::new(
            ContentKind::Item,
            id("example_mod", "lantern")
        )));
        assert!(!items.admits(&RegistryKey::new(
            ContentKind::Block,
            id("example_mod", "lantern")
        )));
    }

    #[test]
    fn two_mods_contributing_to_one_tag_keep_both_members_and_are_both_named() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let first = contributor("first_mod", ContentKind::Item, "copper_lantern", &lanterns);
        let second = contributor("second_mod", ContentKind::Item, "tin_lantern", &lanterns);

        let set = TagSet::merged([&first, &second]);
        assert!(!set.is_empty());
        assert_eq!(set.tags().collect::<Vec<_>>(), [&lanterns]);
        assert_eq!(
            set.members(&lanterns)
                .iter()
                .map(RegistryKey::to_string)
                .collect::<Vec<_>>(),
            [
                "item:first_mod:copper_lantern",
                "item:second_mod:tin_lantern"
            ]
        );
        assert_eq!(
            set.contributors(&lanterns),
            [
                "first_mod".parse::<ModId>().expect("a legal mod id"),
                "second_mod".parse::<ModId>().expect("a legal mod id"),
            ]
        );
    }

    #[test]
    fn merging_orders_members_by_spelling_rather_than_by_load_order() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let early = contributor("zzz_mod", ContentKind::Item, "lantern", &lanterns);
        let late = contributor("aaa_mod", ContentKind::Item, "lantern", &lanterns);

        let forwards = TagSet::merged([&early, &late]);
        let backwards = TagSet::merged([&late, &early]);

        assert_eq!(forwards.members(&lanterns), backwards.members(&lanterns));
        assert_eq!(
            forwards.members(&lanterns)[0].id().namespace().as_str(),
            "aaa_mod"
        );
        // Contributors are load order, so these two do differ there.
        assert_ne!(
            forwards.contributors(&lanterns),
            backwards.contributors(&lanterns)
        );
    }

    #[test]
    fn one_mod_contributing_the_same_member_through_two_declarations_is_named_once() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let declaration = contributor("first_mod", ContentKind::Item, "lantern", &lanterns);

        let set = TagSet::merged([&declaration, &declaration]);
        assert_eq!(set.members(&lanterns).len(), 1);
        assert_eq!(set.contributors(&lanterns).len(), 1);
    }

    #[test]
    fn merging_two_contributions_generates_one_file_holding_both() {
        // The defect this exists for: both mods derive the same path from the same
        // tag name, so generating each mod's own view and writing both keeps the
        // last one and silently drops the other's member.
        let lanterns = tag(ContentKind::Item, "lanterns");
        let first = contributor("first_mod", ContentKind::Item, "copper_lantern", &lanterns);
        let second = contributor("second_mod", ContentKind::Item, "tin_lantern", &lanterns);

        let merged = TagSet::merged([&first, &second]).generate();
        assert_eq!(
            merged.paths().collect::<Vec<_>>(),
            ["data/c/tags/item/lanterns.json"]
        );
        let rendered = merged
            .file(&lanterns.generated_path())
            .expect("the tag has a file");
        let text = String::from_utf8(rendered.bytes().to_vec()).expect("generated JSON is UTF-8");
        assert!(text.contains("first_mod:copper_lantern"), "{text}");
        assert!(text.contains("second_mod:tin_lantern"), "{text}");

        // Neither mod's own generated file holds the other's member, which is why
        // merging has to happen before rendering rather than after.
        let alone = first
            .generate()
            .file(&lanterns.generated_path())
            .map(|file| file.bytes().to_vec())
            .expect("one mod's own view");
        assert!(
            !String::from_utf8_lossy(&alone).contains("second_mod"),
            "one mod's view cannot hold another's contribution"
        );
        assert_eq!(TagSet::default().generate().len(), 0);
    }

    #[test]
    fn a_tag_nobody_declared_is_an_empty_group_rather_than_an_absent_answer() {
        let set = TagSet::default();
        let lanterns = tag(ContentKind::Item, "lanterns");
        assert!(set.is_empty());
        assert_eq!(set.members(&lanterns), []);
        assert_eq!(set.contributors(&lanterns), []);
        assert_eq!(set, TagSet::merged(Vec::<&ContentDeclaration>::new()));
    }
}
