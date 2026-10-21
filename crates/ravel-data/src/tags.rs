use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ContentId, ContentKind, ModId, RegistryKey, RelativePath};

use crate::declare::ContentDeclaration;
use crate::generate::GeneratedData;

/// A named group of entries in one taggable registry; commands are not taggable.
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

    /// Returns `data/<namespace>/tags/<registry>/<path>.json` for this tag.
    ///
    /// # Panics
    ///
    /// Panics if validated content components produce an invalid relative path.
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

/// Launch and pack tag documents merged in caller-supplied load order.
///
/// Generated and raw tags share this rule. Replacement discards earlier members;
/// unknown document fields come from the last contributor. Member order is stable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TagResources {
    documents: BTreeMap<RelativePath, TagContribution>,
    replacements: Vec<crate::Override>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TagContribution {
    owner: ModId,
    contributors: Vec<ModId>,
    document: TagDocument,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct TagDocument {
    #[serde(default)]
    replace: bool,
    values: Vec<serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl TagResources {
    /// Merges generated and supplied tags in the given declaration load order.
    ///
    /// # Errors
    /// Returns the first malformed tag, attributed to its declaration's owner.
    pub fn merged<'a>(
        declarations: impl IntoIterator<Item = &'a ContentDeclaration>,
    ) -> Result<Self, TagResourceError> {
        let mut tags = Self::default();
        for declaration in declarations {
            tags.append(Self::from_generated(
                declaration.owner(),
                &declaration.generate(),
            )?);
        }
        Ok(tags)
    }

    /// Parses every tag before admitting any files from this owner.
    ///
    /// # Errors
    /// Returns the supplying mod, path, and reason when any tag has invalid JSON
    /// or invalid member, `values`, or `replace` field types.
    pub fn from_generated(owner: &ModId, files: &GeneratedData) -> Result<Self, TagResourceError> {
        let mut tags = Self::default();
        for file in files
            .files()
            .iter()
            .filter(|file| Self::is_tag(file.path()))
        {
            let document = parse_tag(file.bytes()).map_err(|reason| TagResourceError {
                mod_id: owner.clone(),
                path: file.path().clone(),
                reason,
            })?;
            tags.documents.insert(
                file.path().clone(),
                TagContribution {
                    owner: owner.clone(),
                    contributors: vec![owner.clone()],
                    document,
                },
            );
        }
        Ok(tags)
    }

    /// Recognizes Minecraft JSON tag resource paths, including nested tag names.
    #[must_use]
    pub fn is_tag(path: &RelativePath) -> bool {
        let mut parts = path.as_str().split('/');
        parts.next() == Some("data")
            && parts.next().is_some()
            && parts.next() == Some("tags")
            && parts.next().is_some()
            && parts.next().is_some()
            && std::path::Path::new(path.as_str()).extension() == Some(std::ffi::OsStr::new("json"))
    }

    /// Appends later contributions, recording replacements but not ordinary merges.
    pub fn append(&mut self, later: Self) {
        self.replacements.extend(later.replacements);
        for (path, mut incoming) in later.documents {
            if let Some(mut earlier) = self.documents.remove(&path) {
                if incoming.document.replace {
                    self.replacements.push(crate::Override {
                        path: path.clone(),
                        winner: incoming.owner.clone(),
                        shadowed: earlier.contributors,
                    });
                } else {
                    earlier
                        .document
                        .values
                        .reserve(incoming.document.values.len());
                    for value in incoming.document.values {
                        if !earlier.document.values.contains(&value) {
                            earlier.document.values.push(value);
                        }
                    }
                    incoming.document.values = earlier.document.values;
                    incoming.document.replace = earlier.document.replace;
                    for contributor in incoming.contributors {
                        if !earlier.contributors.contains(&contributor) {
                            earlier.contributors.push(contributor);
                        }
                    }
                    incoming.contributors = earlier.contributors;
                }
            }
            self.documents.insert(path, incoming);
        }
    }

    /// Returns the final contributor used to attribute a merged output path.
    #[must_use]
    pub fn supplier(&self, path: &RelativePath) -> Option<&ModId> {
        self.documents.get(path).map(|tag| &tag.owner)
    }

    /// Returns members whose values remain in this tag, in supplied order.
    #[must_use]
    pub fn contributors(&self, path: &RelativePath) -> &[ModId] {
        self.documents
            .get(path)
            .map_or(&[], |tag| tag.contributors.as_slice())
    }

    /// Returns raw JSON member values in merged load order, or an empty absent group.
    #[must_use]
    pub fn values(&self, path: &RelativePath) -> &[serde_json::Value] {
        self.documents
            .get(path)
            .map_or(&[], |tag| tag.document.values.as_slice())
    }

    /// Returns explicit replacement events, each naming the mod that discarded members.
    #[must_use]
    pub fn replacements(&self) -> &[crate::Override] {
        &self.replacements
    }

    /// Generates the shared output bytes used by launch and validated packs.
    #[must_use]
    pub fn generate(&self) -> GeneratedData {
        let mut files = GeneratedData::default();
        for (path, tag) in &self.documents {
            let mut bytes = serde_json::to_vec_pretty(&tag.document)
                .expect("a validated tag document always serializes");
            bytes.push(b'\n');
            files.insert(path.clone(), bytes);
        }
        files
    }
}

fn parse_tag(bytes: &[u8]) -> Result<TagDocument, String> {
    use serde_json::Value;
    let document: TagDocument = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    for value in &document.values {
        if value.is_string() {
            continue;
        }
        let valid = value.as_object().is_some_and(|entry| {
            entry.get("id").is_some_and(Value::is_string)
                && entry.get("required").is_none_or(Value::is_boolean)
        });
        if !valid {
            return Err("a tag value must be an id string or an object with an id string and optional required boolean".to_owned());
        }
    }
    Ok(document)
}

/// A malformed tag, attributed to its supplying mod and resource path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagResourceError {
    /// The supplying mod.
    pub mod_id: ModId,
    /// The malformed tag path.
    pub path: RelativePath,
    /// The JSON or tag-schema error.
    pub reason: String,
}

impl fmt::Display for TagResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` supplied invalid tag `{}`: {}",
            self.mod_id, self.path, self.reason
        )
    }
}

impl std::error::Error for TagResourceError {}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, ContentKind, ModId, RegistryKey};

    use super::{TagError, TagId, TagResources};
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
    fn noncanonical_tag_ids_are_refused_before_membership_or_path_generation() {
        let valid = tag(ContentKind::Item, "kept");
        let mut declaration = contributor("example_mod", ContentKind::Item, "kept", &valid);
        let member = declaration.entries()[0].clone();
        for candidate in ["a//b", "./x", "a/./b", ".", "a/."] {
            let admitted =
                ContentId::new("c".parse().expect("a legal mod id"), candidate).map(|id| {
                    let tag = TagId::new(ContentKind::Item, id).expect("items are taggable");
                    declaration.tag(&tag, &member).expect("a new tag");
                    tag.generated_path()
                });
            assert!(admitted.is_err(), "{candidate}");
            assert_eq!(declaration.tags().collect::<Vec<_>>(), [&valid]);
            assert_eq!(
                declaration.tag_members(&valid),
                std::slice::from_ref(&member)
            );
            let generated = TagResources::merged([&declaration])
                .expect("valid tags")
                .generate();
            assert_eq!(
                generated.paths().collect::<Vec<_>>(),
                ["data/c/tags/item/kept.json"]
            );
        }
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
    fn shared_tags_keep_members_and_distinct_contributors_in_load_order() {
        let lanterns = tag(ContentKind::Item, "lanterns");
        let first = contributor("zzz_mod", ContentKind::Item, "copper_lantern", &lanterns);
        let second = contributor("aaa_mod", ContentKind::Item, "tin_lantern", &lanterns);
        let path = lanterns.generated_path();
        let forwards = TagResources::merged([&first, &second, &first]).expect("valid tags");
        let backwards = TagResources::merged([&second, &first]).expect("valid tags");
        assert_eq!(
            forwards.values(&path),
            serde_json::json!(["zzz_mod:copper_lantern", "aaa_mod:tin_lantern"])
                .as_array()
                .expect("an array")
        );
        assert_eq!(
            backwards.values(&path),
            serde_json::json!(["aaa_mod:tin_lantern", "zzz_mod:copper_lantern"])
                .as_array()
                .expect("an array")
        );
        assert_eq!(
            forwards.contributors(&path),
            [first.owner().clone(), second.owner().clone()]
        );
        assert_eq!(
            backwards.contributors(&path),
            [second.owner().clone(), first.owner().clone()]
        );
        let generated = forwards.generate();
        assert_eq!(generated.paths().collect::<Vec<_>>(), [path.as_str()]);
        let document: serde_json::Value =
            serde_json::from_slice(generated.file(&path).expect("a tag").bytes())
                .expect("tag JSON");
        assert_eq!(
            document["values"].as_array().expect("values"),
            forwards.values(&path)
        );
    }

    #[test]
    fn absent_shared_tags_have_no_values_contributors_or_generated_files() {
        let tags =
            TagResources::merged(Vec::<&ContentDeclaration>::new()).expect("no malformed tags");
        let path = tag(ContentKind::Item, "lanterns").generated_path();
        let empty: &[serde_json::Value] = &[];
        assert_eq!(tags.values(&path), empty);
        assert_eq!(tags.contributors(&path), []);
        assert!(tags.generate().is_empty());
    }
}
