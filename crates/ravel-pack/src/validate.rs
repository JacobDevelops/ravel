use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ContentId, ModId, RegistryKey, RelativePath, Side, Version};
use ravel_data::{ContentDeclaration, Override, TagSet};
use ravel_package::PackageManifest;
use ravel_resolve::ResolutionError;
use serde_json::{Map, Value};

use crate::declare::PackDeclaration;

/// The namespace the game's own content lives in.
///
/// A tag member in this namespace is legitimate and **unverified**. Adding a
/// vanilla item to a tag is the ordinary case a pack exists for, and nothing in
/// this workspace holds the game's registry contents: the list of what
/// `minecraft:` provides is inside the pinned jar, which is not a tracked artifact
/// and is not something a version-neutral crate could read. So this crate can say
/// that such a member is *allowed*; it cannot say that it *exists*.
///
/// Everything else is decidable and is decided. A member in a pinned mod's
/// namespace is checked against that mod's own declaration, which is authoritative
/// because a mod declares everything it contributes; a member in a namespace the
/// pack pins nothing for is refused, because nothing in the pack could supply it
/// and it is not the game.
pub const VANILLA_NAMESPACE: &str = "minecraft";

/// The directory in a written pack that belongs to Ravel rather than to the game.
///
/// The split matters for what this repository can honestly claim. Files under
/// `data/` and `assets/` are written in the layout Ravel *believes* the game's pack
/// format uses, and that belief is not checked against the pinned jar anywhere.
/// Files under this prefix are Ravel's own format, read only by Ravel, so their
/// layout is a decision rather than a belief. Nothing else may write here.
pub const RESERVED_PREFIX: &str = "ravel/";

/// One file a validated pack writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackFile {
    path: RelativePath,
    bytes: Vec<u8>,
}

impl PackFile {
    /// Returns where the file belongs in the written pack.
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }

    /// Returns the file's exact content.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A pack whose four closures hold, and the files it writes.
///
/// Only [`validate`] builds one, so holding one *is* the statement that the pack's
/// dependencies, resources, registry claims and progression are closed. A caller
/// cannot write an unvalidated pack, because there is nothing to write from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPack {
    files: Vec<PackFile>,
    overrides: Vec<Override>,
}

impl ValidatedPack {
    /// Returns every file the pack writes, in path order.
    ///
    /// Path order and one file per path, which together are the whole of the
    /// determinism claim: two validations of the same declaration emit the same
    /// paths with the same bytes in the same order, so the output can be compared
    /// byte for byte. Nothing here reads a clock, a filesystem, an environment
    /// variable, a hostname or a hash map's iteration order.
    #[must_use]
    pub fn files(&self) -> &[PackFile] {
        &self.files
    }

    /// Returns the file at `path`, when the pack writes one.
    #[must_use]
    pub fn file(&self, path: &RelativePath) -> Option<&PackFile> {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path.as_str()))
            .ok()
            .map(|found| &self.files[found])
    }

    /// Returns every path the pack took from one of its members, in path order.
    ///
    /// Not a refusal. Replacing a member's file is what a pack is for; doing it
    /// silently is not, so the mod whose file the pack took is named.
    #[must_use]
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }
}

/// Checks a pack's four closures and produces the files it writes.
///
/// `SPEC.md` §4.3 makes "validates dependency, resource, registry, and progression
/// consistency" the pack author's obligation, and M4-W5's gate names the same four.
/// Each is checked against the same inputs a pack author's build already holds —
/// the pinned members' manifests and the content each member declares — so nothing
/// here reads a file or a game.
///
/// # Dependency closure
///
/// Every pinned member has a manifest, at exactly the pinned version, and no
/// manifest is present that the pack did not pin. Then the whole set resolves, on
/// every side in [`Side::ALL`], through `ravel-resolve` rather than through a second
/// graph written here: a missing requirement, a version outside a declared range, a
/// dependency that does not load on the side its dependant does, a cycle and an
/// incompatible pair are all already decided there, and a pack-local reimplementation
/// would be a second answer that can disagree.
///
/// Every side rather than one, because a pack ships to all of them and a set that
/// resolves on a dedicated server and not on a client is a broken pack. The list is
/// [`Side::ALL`] rather than a literal, so a fourth side is checked by arithmetic
/// rather than by somebody remembering.
///
/// # Resource closure
///
/// Every path the pack refers to is supplied by something — a member's manifest, a
/// member's generated content, or the pack itself — and the pack supplies each path
/// at most once. Two *members* writing one path is refused too: a mod cannot know
/// what it was installed beside, but the pack does, so an unsettled collision is a
/// decision the author has not made. The pack replacing a member's path is not
/// refused; it is what a pack does, and it is recorded in
/// [`ValidatedPack::overrides`].
///
/// # Registry closure
///
/// Every pinned member supplies a content declaration and every content declaration
/// belongs to a pinned member, so the union is complete rather than
/// whatever happened to be passed in. No registry entry is claimed by two members.
/// Every tag member either lives in [`VANILLA_NAMESPACE`], or lives in a pinned
/// member's namespace and is declared by that member, or is refused. See
/// [`VANILLA_NAMESPACE`] for what the first case does *not* prove.
///
/// # Progression closure
///
/// Every prerequisite names a declared step, every step is reachable from a root,
/// and every gate names an entry the pack provides under the same rule tag members
/// are held to.
///
/// # Errors
///
/// Returns the [`ClosureError`] naming the offender. The order the checks run in is
/// the order above and it is deliberate: a pack whose members are not the pinned set
/// would make every later refusal a report about the wrong installation.
pub fn validate(
    pack: &PackDeclaration,
    manifests: &[PackageManifest],
    content: &[ContentDeclaration],
) -> Result<ValidatedPack, ClosureError> {
    let installed = pinned_set(pack, manifests)?;
    for side in Side::ALL {
        ravel_resolve::resolve(manifests, side).map_err(|source| ClosureError::Dependency {
            side,
            source: Box::new(source),
        })?;
    }

    let declarations = declared_content(pack, content)?;
    registry_closure(pack, &declarations)?;
    progression_closure(pack, &declarations)?;
    let (files, overrides) = written_files(pack, &installed, &declarations)?;

    Ok(ValidatedPack { files, overrides })
}

/// Returns every pinned member's manifest, refusing a set that is not the pinned one.
fn pinned_set<'a>(
    pack: &PackDeclaration,
    manifests: &'a [PackageManifest],
) -> Result<BTreeMap<&'a ModId, &'a PackageManifest>, ClosureError> {
    let mut installed: BTreeMap<&ModId, &PackageManifest> = BTreeMap::new();
    for manifest in manifests {
        let Some(pinned) = pack.pinned(&manifest.id) else {
            return Err(ClosureError::UnpinnedPackage {
                package: manifest.id.clone(),
            });
        };
        if &manifest.version != pinned {
            return Err(ClosureError::PinMismatch {
                member: manifest.id.clone(),
                pinned: pinned.clone(),
                supplied: manifest.version.clone(),
            });
        }
        installed.insert(&manifest.id, manifest);
    }
    for (member, _) in pack.members() {
        if !installed.contains_key(member) {
            return Err(ClosureError::MemberMissing {
                member: member.clone(),
            });
        }
    }
    Ok(installed)
}

/// Returns every pinned member's content declaration, one per member.
fn declared_content<'a>(
    pack: &PackDeclaration,
    content: &'a [ContentDeclaration],
) -> Result<BTreeMap<&'a ModId, &'a ContentDeclaration>, ClosureError> {
    let mut declarations: BTreeMap<&ModId, &ContentDeclaration> = BTreeMap::new();
    for declaration in content {
        if pack.pinned(declaration.owner()).is_none() {
            return Err(ClosureError::ContentUnpinned {
                owner: declaration.owner().clone(),
            });
        }
        if declarations
            .insert(declaration.owner(), declaration)
            .is_some()
        {
            return Err(ClosureError::ContentTwice {
                owner: declaration.owner().clone(),
            });
        }
    }
    for (member, _) in pack.members() {
        if !declarations.contains_key(member) {
            return Err(ClosureError::ContentMissing {
                member: member.clone(),
            });
        }
    }
    Ok(declarations)
}

/// Refuses a contested registry claim and a tag member nothing provides.
fn registry_closure(
    pack: &PackDeclaration,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
) -> Result<(), ClosureError> {
    let mut claimed: BTreeMap<&RegistryKey, &ModId> = BTreeMap::new();
    for (owner, declaration) in declarations {
        for key in declaration.entries() {
            if let Some(first) = claimed.get(key) {
                return Err(ClosureError::EntryContested {
                    key: key.clone(),
                    first: (*first).clone(),
                    second: (*owner).clone(),
                });
            }
            claimed.insert(key, owner);
        }
    }

    for declaration in declarations.values() {
        for tag in declaration.tags() {
            for member in declaration.tag_members(tag) {
                provided(pack, declarations, member).map_err(|kind| ClosureError::TagMember {
                    tag: tag.to_string(),
                    member: member.clone(),
                    kind,
                })?;
            }
        }
    }
    Ok(())
}

/// Refuses a dangling prerequisite, an unreachable step, and a gate nothing provides.
fn progression_closure(
    pack: &PackDeclaration,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
) -> Result<(), ClosureError> {
    let progression = pack.progression();
    if let Some((state, prerequisite)) = progression.dangling().into_iter().next() {
        return Err(ClosureError::PrerequisiteMissing {
            state,
            prerequisite,
        });
    }
    if let Some(state) = progression.unreachable().into_iter().next() {
        return Err(ClosureError::StateUnreachable { state });
    }
    for state in progression.states() {
        if let Some(gate) = state.gate() {
            provided(pack, declarations, gate).map_err(|kind| ClosureError::Gate {
                state: state.id().clone(),
                entry: gate.clone(),
                kind,
            })?;
        }
    }
    Ok(())
}

/// Decides whether `key` is an entry this pack can account for.
///
/// The one rule both tag members and progression gates are held to, in one place,
/// because they ask the same question and two copies of it would answer differently
/// the first time one was changed.
fn provided(
    pack: &PackDeclaration,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
    key: &RegistryKey,
) -> Result<(), UnprovidedEntry> {
    let namespace = key.id().namespace();
    if namespace.as_str() == VANILLA_NAMESPACE {
        return Ok(());
    }
    if pack.pinned(namespace).is_none() {
        return Err(UnprovidedEntry::NoSuchMember);
    }
    let declares = declarations
        .get(namespace)
        .is_some_and(|declaration| declaration.entries().contains(key));
    if declares {
        Ok(())
    } else {
        Err(UnprovidedEntry::MemberDoesNotDeclareIt)
    }
}

/// Builds the pack's files, refusing a contested or reserved path.
fn written_files(
    pack: &PackDeclaration,
    installed: &BTreeMap<&ModId, &PackageManifest>,
    declarations: &BTreeMap<&ModId, &ContentDeclaration>,
) -> Result<(Vec<PackFile>, Vec<Override>), ClosureError> {
    // Who writes each path, before any bytes are placed. A path's writer is what
    // both halves of resource closure are about, so it is one table.
    let mut writer: BTreeMap<RelativePath, Writer> = BTreeMap::new();
    let mut files: BTreeMap<RelativePath, Vec<u8>> = BTreeMap::new();
    let mut overrides: Vec<Override> = Vec::new();

    // Tags first, merged across every member. A tag's path is derived from the
    // tag's name, so two mods contributing to `c:ingots` derive the same file, and
    // writing each mod's own view of it would keep the last and lose exactly the
    // contributions a tag exists to collect. `TagSet` is what merges them and it is
    // `ravel-data`'s, so a merged tag file and a single mod's cannot disagree about
    // format.
    let tags = TagSet::merged(declarations.values().copied());
    for file in tags.generate().files() {
        reserved(file.path())?;
        writer.insert(file.path().clone(), Writer::Members);
        files.insert(file.path().clone(), file.bytes().to_vec());
    }

    for (owner, declaration) in declarations {
        for file in declaration.generate().files() {
            let path = file.path().clone();
            if matches!(writer.get(&path), Some(Writer::Members)) {
                // The merged tag file above already holds this mod's contribution.
                continue;
            }
            reserved(&path)?;
            if let Some(Writer::Member(first)) = writer.get(&path) {
                return Err(ClosureError::PathContested {
                    path,
                    first: first.clone(),
                    second: (*owner).clone(),
                });
            }
            writer.insert(path.clone(), Writer::Member((*owner).clone()));
            files.insert(path, file.bytes().to_vec());
        }
    }

    for supplied in pack.supplied() {
        reserved(&supplied.path)?;
        match writer.get(&supplied.path) {
            Some(Writer::Pack) => {
                return Err(ClosureError::SuppliedTwice {
                    path: supplied.path.clone(),
                });
            }
            Some(Writer::Member(first)) => overrides.push(Override {
                path: supplied.path.clone(),
                winner: pack.id().clone(),
                shadowed: vec![first.clone()],
            }),
            Some(Writer::Members) => {
                let contributors = tags
                    .tags()
                    .find(|tag| tag.generated_path() == supplied.path)
                    .map(|tag| tags.contributors(tag).to_vec())
                    .unwrap_or_default();
                overrides.push(Override {
                    path: supplied.path.clone(),
                    winner: pack.id().clone(),
                    shadowed: contributors,
                });
            }
            None => {}
        }
        writer.insert(supplied.path.clone(), Writer::Pack);
        files.insert(supplied.path.clone(), supplied.bytes.clone());
    }

    // A manifest's own declared resource paths are supplied by the package rather
    // than written here, so they count as answers to a reference without becoming
    // files the pack emits.
    let mut supplied_elsewhere: BTreeMap<&str, &ModId> = BTreeMap::new();
    for (owner, manifest) in installed {
        for path in &manifest.resources {
            supplied_elsewhere.insert(path.as_str(), owner);
        }
    }

    for reference in pack.references() {
        let answered = writer.contains_key(&reference.path)
            || supplied_elsewhere.contains_key(reference.path.as_str());
        if !answered {
            return Err(ClosureError::ResourceMissing {
                path: reference.path.clone(),
                referrer: reference.referrer.clone(),
            });
        }
    }

    // Written unchecked, and this is the one asymmetry in what `validate` decides.
    // A tag member is checked because registry closure is one of the four properties
    // the gate names and its input — every member's `ContentDeclaration` — is already
    // an argument here. A settings key is not, because the pack's settings layer is
    // none of the four and the input a check would need, every member's
    // `ConfigurationDeclaration`, is not an argument here. The check itself is not
    // missing from the repository: `LayeredConfiguration::resolve` refuses an unknown
    // key, an illegal value, and a `required` setting the pack left out. It runs when
    // the game starts rather than when the pack is built, so a typo'd tag member is
    // refused to the pack author and a typo'd settings key is refused to the player.
    // Closing it means handing `validate` the members' configuration declarations and
    // calling that same `resolve` per member rather than writing a second answer.
    // Nothing decides against that; no work package owns it yet, and `ROADMAP.md`'s
    // open-questions table is where it is recorded.
    for (owner, document) in pack.settings() {
        let path = ravel_path(&format!("config/{owner}.json"));
        files.insert(path, document_bytes(&document.to_json()));
    }
    files.insert(ravel_path("pack.json"), pack_manifest(pack));

    Ok((
        files
            .into_iter()
            .map(|(path, bytes)| PackFile { path, bytes })
            .collect(),
        overrides,
    ))
}

/// What writes one path in a pack.
///
/// Three cases rather than an `Option<ModId>`, because a merged tag file has no
/// single writer and calling it one member's would name the wrong mod in an
/// override record.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Writer {
    /// Every member that contributed to one tag.
    Members,
    /// One member's own generated or supplied file.
    Member(ModId),
    /// The pack itself.
    Pack,
}

/// Refuses a path in the directory Ravel's own files live in.
fn reserved(path: &RelativePath) -> Result<(), ClosureError> {
    if path.as_str().starts_with(RESERVED_PREFIX) {
        return Err(ClosureError::ReservedPath { path: path.clone() });
    }
    Ok(())
}

/// Returns a path under [`RESERVED_PREFIX`].
///
/// # Panics
///
/// Panics if the result is not a legal [`RelativePath`], which cannot happen: the
/// prefix is this crate's literal and the rest is a [`ModId`] and a literal.
fn ravel_path(rest: &str) -> RelativePath {
    RelativePath::try_from(format!("{RESERVED_PREFIX}{rest}"))
        .expect("a mod id cannot spell an illegal path")
}

/// Renders one document the way every file this crate writes is rendered.
///
/// A trailing newline, for the reason `ravel-data` gives its generated files one: a
/// JSON file without it is a file every text tool reports as ending mid-line.
fn document_bytes(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

/// Renders the reproducible manifest `SPEC.md` §4.3 requires a pack to produce.
///
/// The pack's identity, its version, and every pin. Nothing about the machine that
/// built it, which is the property `tests/conformance/tests/pack_closure.rs` checks
/// by scanning every written byte: a path, a user name, a hostname or a timestamp in
/// here would make two builds of one pack differ and make the manifest unusable as
/// the thing two installations are compared by.
fn pack_manifest(pack: &PackDeclaration) -> Vec<u8> {
    let mut members = Map::new();
    for (member, version) in pack.members() {
        members.insert(
            member.as_str().to_owned(),
            Value::String(version.to_string()),
        );
    }

    let mut document = Map::new();
    document.insert(
        "id".to_owned(),
        Value::String(pack.id().as_str().to_owned()),
    );
    document.insert("members".to_owned(), Value::Object(members));
    document.insert(
        "version".to_owned(),
        Value::String(pack.version().to_string()),
    );

    let text = serde_json::to_string_pretty(&Value::Object(document))
        .expect("a map of strings always serializes");
    document_bytes(&text)
}

/// Why a registry entry is one this pack cannot account for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnprovidedEntry {
    /// The entry's namespace is not the game's and is not a pinned member.
    NoSuchMember,
    /// The namespace is a pinned member and that member declares no such entry.
    MemberDoesNotDeclareIt,
}

impl fmt::Display for UnprovidedEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchMember => write!(
                formatter,
                "its namespace is neither `{VANILLA_NAMESPACE}` nor a mod this pack pins"
            ),
            Self::MemberDoesNotDeclareIt => {
                formatter.write_str("the mod that owns its namespace declares no such entry")
            }
        }
    }
}

/// Why a pack's four closures do not hold.
///
/// Every variant names the offender, because a pack author reading one is being
/// asked to change a pin, a path, an entry or a step and cannot act on a count.
#[derive(Clone, Debug, PartialEq)]
pub enum ClosureError {
    /// A pinned member's manifest was not supplied.
    MemberMissing {
        /// The member with no manifest.
        member: ModId,
    },
    /// A manifest was supplied for a package the pack does not pin.
    UnpinnedPackage {
        /// The package nothing pinned.
        package: ModId,
    },
    /// A member's manifest is at a version the pack did not pin.
    PinMismatch {
        /// The member.
        member: ModId,
        /// The version the pack pinned.
        pinned: Version,
        /// The version the manifest carries.
        supplied: Version,
    },
    /// The pinned set does not resolve on one of the sides the pack ships to.
    ///
    /// The refusal is `ravel-resolve`'s, boxed because a `ResolutionError` is much
    /// larger than every other variant here and an unboxed one would make this enum
    /// that size everywhere it is returned.
    Dependency {
        /// The side it does not resolve on.
        side: Side,
        /// What the resolver refused.
        source: Box<ResolutionError>,
    },
    /// A pinned member declared no content at all, not even an empty declaration.
    ContentMissing {
        /// The member whose declaration is absent.
        member: ModId,
    },
    /// A content declaration belongs to a mod the pack does not pin.
    ContentUnpinned {
        /// The mod that owns it.
        owner: ModId,
    },
    /// Two content declarations claim the same owner.
    ContentTwice {
        /// The owner named twice.
        owner: ModId,
    },
    /// Two members claim the same registry entry.
    EntryContested {
        /// The contested entry.
        key: RegistryKey,
        /// The member that claimed it first.
        first: ModId,
        /// The member that claimed it second.
        second: ModId,
    },
    /// A tag names a member nothing in this pack provides.
    TagMember {
        /// The tag, rendered — a `TagId` is `ravel-data`'s and this is a report.
        tag: String,
        /// The member nothing provides.
        member: RegistryKey,
        /// Why nothing provides it.
        kind: UnprovidedEntry,
    },
    /// A progression step is gated on an entry nothing in this pack provides.
    Gate {
        /// The step.
        state: ContentId,
        /// The entry it is gated on.
        entry: RegistryKey,
        /// Why nothing provides it.
        kind: UnprovidedEntry,
    },
    /// Two members would write the same file.
    PathContested {
        /// The contested path.
        path: RelativePath,
        /// The member that writes it first.
        first: ModId,
        /// The member that writes it second.
        second: ModId,
    },
    /// The pack supplies one path twice.
    SuppliedTwice {
        /// The path supplied twice.
        path: RelativePath,
    },
    /// Something other than Ravel would write into Ravel's own directory.
    ReservedPath {
        /// The path.
        path: RelativePath,
    },
    /// The pack refers to a path nothing supplies.
    ResourceMissing {
        /// The path with no file behind it.
        path: RelativePath,
        /// The pack's own thing that refers to it.
        referrer: ContentId,
    },
    /// A progression step requires a step the pack does not declare.
    PrerequisiteMissing {
        /// The step that requires it.
        state: ContentId,
        /// The step nothing declares.
        prerequisite: ContentId,
    },
    /// A progression step could never be reached.
    StateUnreachable {
        /// The stranded step.
        state: ContentId,
    },
}

impl fmt::Display for ClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemberMissing { member } => write!(
                formatter,
                "this pack pins `{member}` and no manifest for it was supplied, so nothing could \
                 check what it needs"
            ),
            Self::UnpinnedPackage { package } => write!(
                formatter,
                "`{package}` is installed and this pack pins no version of it; a pack decides its \
                 own contents"
            ),
            Self::PinMismatch {
                member,
                pinned,
                supplied,
            } => write!(
                formatter,
                "this pack pins `{member}` at `{pinned}` and `{supplied}` was supplied"
            ),
            Self::Dependency { side, source } => write!(
                formatter,
                "this pack does not resolve on `{}`: {source}",
                side.name()
            ),
            Self::ContentMissing { member } => write!(
                formatter,
                "`{member}` is pinned and declared no content, so nothing could check its registry \
                 entries; a mod that contributes nothing declares an empty declaration"
            ),
            Self::ContentUnpinned { owner } => write!(
                formatter,
                "`{owner}` declared content and this pack pins no version of it"
            ),
            Self::ContentTwice { owner } => {
                write!(formatter, "`{owner}` declared its content twice")
            }
            Self::EntryContested { key, first, second } => write!(
                formatter,
                "`{first}` and `{second}` both claim `{key}`, and one entry has one owner"
            ),
            Self::TagMember { tag, member, kind } => write!(
                formatter,
                "`{tag}` holds `{member}`, which nothing in this pack provides: {kind}"
            ),
            Self::Gate { state, entry, kind } => write!(
                formatter,
                "`{state}` is gated on `{entry}`, which nothing in this pack provides: {kind}"
            ),
            Self::PathContested {
                path,
                first,
                second,
            } => write!(
                formatter,
                "`{first}` and `{second}` would both write `{path}`; a pack settles that by \
                 supplying the file itself"
            ),
            Self::SuppliedTwice { path } => write!(
                formatter,
                "this pack supplies `{path}` twice, and one path holds one file"
            ),
            Self::ReservedPath { path } => write!(
                formatter,
                "`{path}` is under `{RESERVED_PREFIX}`, which holds Ravel's own files only"
            ),
            Self::ResourceMissing { path, referrer } => write!(
                formatter,
                "`{referrer}` refers to `{path}` and nothing in this pack supplies it"
            ),
            Self::PrerequisiteMissing {
                state,
                prerequisite,
            } => write!(
                formatter,
                "`{state}` requires `{prerequisite}`, which this pack does not declare"
            ),
            Self::StateUnreachable { state } => write!(
                formatter,
                "`{state}` can never be reached: every route to it needs a step that needs it"
            ),
        }
    }
}

impl std::error::Error for ClosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dependency { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_config::ValueDocument;
    use ravel_core::{ContentId, ContentKind, ModId, RegistryKey, RelativePath, Side, Version};
    use ravel_data::{ContentDeclaration, TagId};
    use ravel_package::PackageManifest;

    use super::{ClosureError, RESERVED_PREFIX, UnprovidedEntry, VANILLA_NAMESPACE, validate};
    use crate::declare::PackDeclaration;

    /// A digest shaped like one, for a manifest nothing verifies here.
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn named(namespace: &str, path: &str) -> ContentId {
        ContentId::new(id(namespace), path).expect("a legal content path")
    }

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    /// One manifest for `owner` at `version`, requiring `requires` when it does.
    fn manifest(owner: &str, version: &str, requires: Option<(&str, &str)>) -> PackageManifest {
        let dependencies = match requires {
            Some((dependency, range)) => format!(
                r#"[{{ "id": "{dependency}", "relationship": "required", "version": "{range}" }}]"#
            ),
            None => "[]".to_owned(),
        };
        PackageManifest::parse(&format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{owner}",
              "version": "{version}",
              "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] }},
              "sides": ["client", "integrated-server", "dedicated-server"],
              "dependencies": {dependencies},
              "source": {{
                "repository": "https://example.invalid/{owner}",
                "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}]
              }},
              "authors": [{{ "name": "Example" }}],
              "license": "MIT OR Apache-2.0",
              "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
            }}"#
        ))
        .expect("a legal manifest")
    }

    /// A pack pinning `base_mod` 1.0.0, with the member's own empty declaration.
    fn one_member() -> (
        PackDeclaration,
        Vec<PackageManifest>,
        Vec<ContentDeclaration>,
    ) {
        let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
        pack.pin(id("base_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        (
            pack,
            vec![manifest("base_mod", "1.0.0", None)],
            vec![ContentDeclaration::new(id("base_mod"))],
        )
    }

    #[test]
    fn a_pack_whose_four_closures_hold_writes_its_manifest_and_nothing_else() {
        let (pack, manifests, content) = one_member();
        let validated = validate(&pack, &manifests, &content).expect("every closure holds");

        assert_eq!(
            validated
                .files()
                .iter()
                .map(|file| file.path().as_str())
                .collect::<Vec<_>>(),
            [format!("{RESERVED_PREFIX}pack.json")]
        );
        assert!(validated.overrides().is_empty());

        let manifest = validated
            .file(&path(&format!("{RESERVED_PREFIX}pack.json")))
            .expect("the pack writes its own manifest");
        let text = String::from_utf8(manifest.bytes().to_vec()).expect("the manifest is UTF-8");
        assert_eq!(
            text,
            "{\n  \"id\": \"example_pack\",\n  \"members\": {\n    \"base_mod\": \"1.0.0\"\n  },\n  \
             \"version\": \"0.1.0\"\n}\n"
        );
    }

    // ------------------------------------------------- dependency closure ---

    #[test]
    fn a_pinned_member_with_no_manifest_is_refused_by_name() {
        let (mut pack, manifests, mut content) = one_member();
        pack.pin(id("absent_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        content.push(ContentDeclaration::new(id("absent_mod")));

        let refused = validate(&pack, &manifests, &content).expect_err("a member is missing");
        assert_eq!(
            refused,
            ClosureError::MemberMissing {
                member: id("absent_mod")
            }
        );
        assert!(refused.to_string().contains("absent_mod"), "{refused}");
    }

    #[test]
    fn a_manifest_at_a_version_the_pack_did_not_pin_is_refused_with_both_versions() {
        let (pack, _, content) = one_member();
        let refused = validate(&pack, &[manifest("base_mod", "1.1.0", None)], &content)
            .expect_err("the pin is exact");
        assert_eq!(
            refused,
            ClosureError::PinMismatch {
                member: id("base_mod"),
                pinned: Version::new(1, 0, 0),
                supplied: Version::new(1, 1, 0),
            }
        );
        let text = refused.to_string();
        assert!(text.contains("1.0.0") && text.contains("1.1.0"), "{text}");
    }

    #[test]
    fn a_package_the_pack_pins_nothing_for_is_refused_rather_than_installed_quietly() {
        let (pack, mut manifests, content) = one_member();
        manifests.push(manifest("stowaway_mod", "1.0.0", None));

        assert_eq!(
            validate(&pack, &manifests, &content),
            Err(ClosureError::UnpinnedPackage {
                package: id("stowaway_mod")
            })
        );
    }

    #[test]
    fn a_dependency_the_pack_does_not_hold_is_refused_with_the_resolvers_own_reason() {
        let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
        pack.pin(id("consumer_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        let manifests = vec![manifest(
            "consumer_mod",
            "1.0.0",
            Some(("library_mod", "^1.0.0")),
        )];
        let content = vec![ContentDeclaration::new(id("consumer_mod"))];

        let refused = validate(&pack, &manifests, &content).expect_err("the library is absent");
        assert!(matches!(refused, ClosureError::Dependency { .. }));
        let text = refused.to_string();
        assert!(text.contains("library_mod"), "{text}");
        assert!(
            Side::ALL.iter().any(|side| text.contains(side.name())),
            "the refusal names the side it does not resolve on: {text}"
        );
        assert!(std::error::Error::source(&refused).is_some());
    }

    #[test]
    fn a_dependency_pinned_outside_the_range_its_dependant_accepts_is_refused() {
        let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
        pack.pin(id("consumer_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        pack.pin(id("library_mod"), Version::new(2, 0, 0))
            .expect("a legal pin");
        let manifests = vec![
            manifest("consumer_mod", "1.0.0", Some(("library_mod", "^1.0.0"))),
            manifest("library_mod", "2.0.0", None),
        ];
        let content = vec![
            ContentDeclaration::new(id("consumer_mod")),
            ContentDeclaration::new(id("library_mod")),
        ];

        let refused = validate(&pack, &manifests, &content).expect_err("2.0.0 is outside `^1.0.0`");
        assert!(matches!(refused, ClosureError::Dependency { .. }));
        let text = refused.to_string();
        assert!(
            text.contains("library_mod") && text.contains("2.0.0"),
            "{text}"
        );
    }

    // --------------------------------------------------- resource closure ---

    #[test]
    fn a_path_the_pack_refers_to_and_nothing_supplies_is_refused_naming_the_referrer() {
        let (mut pack, manifests, content) = one_member();
        pack.refer_to(
            path("assets/example_pack/textures/lantern.png"),
            content_referrer(),
        );

        let refused = validate(&pack, &manifests, &content).expect_err("nothing supplies it");
        assert_eq!(
            refused,
            ClosureError::ResourceMissing {
                path: path("assets/example_pack/textures/lantern.png"),
                referrer: content_referrer(),
            }
        );
        let text = refused.to_string();
        assert!(
            text.contains("lantern.png") && text.contains("example_pack:icon"),
            "{text}"
        );
    }

    fn content_referrer() -> ContentId {
        named("example_pack", "icon")
    }

    #[test]
    fn a_reference_answered_by_the_pack_itself_or_by_a_members_manifest_holds() {
        // The pack's own file answers it.
        let (mut pack, manifests, content) = one_member();
        pack.supply(
            path("assets/example_pack/textures/lantern.png"),
            b"\x89PNG".to_vec(),
        );
        pack.refer_to(
            path("assets/example_pack/textures/lantern.png"),
            content_referrer(),
        );
        validate(&pack, &manifests, &content).expect("the pack supplies what it refers to");

        // And so does a path a member's manifest declares, which the pack does not
        // write but the installation does hold.
        let mut second = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
        second
            .pin(id("base_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        second.refer_to(path("resources/lang/en.json"), content_referrer());
        let with_resource = PackageManifest::parse(&manifest_document_with_resource(
            "base_mod",
            "1.0.0",
            "resources/lang/en.json",
        ))
        .expect("a legal manifest");
        validate(&second, &[with_resource], &content)
            .expect("a member's declared resource answers a reference");
    }

    fn manifest_document_with_resource(owner: &str, version: &str, resource: &str) -> String {
        format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{owner}",
              "version": "{version}",
              "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] }},
              "sides": ["client", "integrated-server", "dedicated-server"],
              "resources": ["{resource}"],
              "source": {{
                "repository": "https://example.invalid/{owner}",
                "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}]
              }},
              "authors": [{{ "name": "Example" }}],
              "license": "MIT OR Apache-2.0",
              "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
            }}"#
        )
    }

    #[test]
    fn a_pack_that_supplies_one_path_twice_is_refused() {
        let (mut pack, manifests, content) = one_member();
        pack.supply(
            path("data/example_pack/recipe/lantern.json"),
            b"{}".to_vec(),
        );
        pack.supply(
            path("data/example_pack/recipe/lantern.json"),
            b"{\"changed\": true}".to_vec(),
        );

        let refused = validate(&pack, &manifests, &content).expect_err("one path holds one file");
        assert_eq!(
            refused,
            ClosureError::SuppliedTwice {
                path: path("data/example_pack/recipe/lantern.json"),
            }
        );
    }

    /// Two mods pinned together, each with its own content declaration.
    fn two_members() -> (PackDeclaration, Vec<PackageManifest>) {
        let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
        pack.pin(id("first_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        pack.pin(id("second_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        (
            pack,
            vec![
                manifest("first_mod", "1.0.0", None),
                manifest("second_mod", "1.0.0", None),
            ],
        )
    }

    #[test]
    fn two_members_contributing_to_one_tag_are_merged_into_one_file_holding_both() {
        // Both mods derive the same path from the same tag name. Refusing that would
        // forbid the one thing tags exist for, and writing each mod's own view of it
        // would keep the last file and lose the other's member silently.
        let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
        let (pack, manifests) = two_members();

        let mut first = ContentDeclaration::new(id("first_mod"));
        let held = first
            .entry(ContentKind::Item, "copper_lantern")
            .expect("a legal entry");
        first.tag(&tag, &held).expect("an item joins an item tag");
        let mut second = ContentDeclaration::new(id("second_mod"));
        let other = second
            .entry(ContentKind::Item, "tin_lantern")
            .expect("a legal entry");
        second.tag(&tag, &other).expect("an item joins an item tag");

        let validated = validate(&pack, &manifests, &[first, second]).expect("contributions merge");
        let written = validated
            .file(&tag.generated_path())
            .expect("one file for the tag");
        let text = String::from_utf8(written.bytes().to_vec()).expect("generated JSON is UTF-8");
        assert!(text.contains("first_mod:copper_lantern"), "{text}");
        assert!(text.contains("second_mod:tin_lantern"), "{text}");
        assert!(
            validated.overrides().is_empty(),
            "merging is not one mod taking a path from another"
        );
    }

    #[test]
    fn two_members_writing_one_path_that_is_not_a_tag_is_refused_and_both_are_named() {
        // The genuine collision: a path a tag's name does not derive, so nothing
        // could merge it and the pack is the layer that has to decide.
        let shared = path("assets/shared/models/lantern.json");
        let (pack, manifests) = two_members();
        let mut first = ContentDeclaration::new(id("first_mod"));
        first
            .resource(shared.clone(), b"{}".to_vec())
            .expect("a legal supply");
        let mut second = ContentDeclaration::new(id("second_mod"));
        second
            .resource(shared.clone(), b"{\"changed\": true}".to_vec())
            .expect("a legal supply");

        let refused =
            validate(&pack, &manifests, &[first, second]).expect_err("nothing could merge it");
        assert_eq!(
            refused,
            ClosureError::PathContested {
                path: shared,
                first: id("first_mod"),
                second: id("second_mod"),
            }
        );
        assert!(
            refused.to_string().contains("supplying the file itself"),
            "the refusal says how a pack settles it: {refused}"
        );
    }

    #[test]
    fn a_pack_replacing_a_merged_tag_file_names_every_contributor_it_took_it_from() {
        let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
        let (mut pack, manifests) = two_members();
        let mut first = ContentDeclaration::new(id("first_mod"));
        let held = first
            .entry(ContentKind::Item, "copper_lantern")
            .expect("a legal entry");
        first.tag(&tag, &held).expect("an item joins an item tag");
        let mut second = ContentDeclaration::new(id("second_mod"));
        let other = second
            .entry(ContentKind::Item, "tin_lantern")
            .expect("a legal entry");
        second.tag(&tag, &other).expect("an item joins an item tag");
        pack.supply(tag.generated_path(), b"{}\n".to_vec());

        let validated = validate(&pack, &manifests, &[first, second])
            .expect("a pack may write the tag file itself");
        assert_eq!(validated.overrides().len(), 1);
        assert_eq!(validated.overrides()[0].winner, id("example_pack"));
        assert_eq!(
            validated.overrides()[0].shadowed,
            [id("first_mod"), id("second_mod")],
            "a merged file was taken from every mod that contributed to it"
        );
        assert_eq!(
            validated
                .file(&tag.generated_path())
                .map(|file| file.bytes().to_vec()),
            Some(b"{}\n".to_vec())
        );
    }

    #[test]
    fn the_pack_replacing_one_members_file_is_recorded_rather_than_refused() {
        let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
        let (mut pack, manifests, _) = one_member();
        let mut declaration = ContentDeclaration::new(id("base_mod"));
        let held = declaration
            .entry(ContentKind::Item, "copper_lantern")
            .expect("a legal entry");
        declaration
            .tag(&tag, &held)
            .expect("an item joins an item tag");
        pack.supply(tag.generated_path(), b"{}\n".to_vec());

        let validated =
            validate(&pack, &manifests, &[declaration]).expect("a pack may replace a file");
        assert_eq!(validated.overrides().len(), 1);
        assert_eq!(validated.overrides()[0].winner, id("example_pack"));
        assert_eq!(validated.overrides()[0].shadowed, [id("base_mod")]);
        assert_eq!(
            validated
                .file(&tag.generated_path())
                .map(|file| file.bytes().to_vec()),
            Some(b"{}\n".to_vec()),
            "the pack's bytes are the ones written"
        );
    }

    #[test]
    fn nothing_but_ravel_may_write_into_ravels_own_directory() {
        let (mut pack, manifests, content) = one_member();
        pack.supply(path(&format!("{RESERVED_PREFIX}pack.json")), b"{}".to_vec());

        assert_eq!(
            validate(&pack, &manifests, &content),
            Err(ClosureError::ReservedPath {
                path: path(&format!("{RESERVED_PREFIX}pack.json")),
            })
        );
    }

    // --------------------------------------------------- registry closure ---

    #[test]
    fn a_pinned_member_that_declared_no_content_at_all_is_refused() {
        let (pack, manifests, _) = one_member();
        let refused = validate(&pack, &manifests, &[]).expect_err("the union must be complete");
        assert_eq!(
            refused,
            ClosureError::ContentMissing {
                member: id("base_mod")
            }
        );
        assert!(
            refused.to_string().contains("empty declaration"),
            "the refusal says what to do: {refused}"
        );
    }

    #[test]
    fn a_content_declaration_from_a_mod_the_pack_does_not_pin_is_refused() {
        let (pack, manifests, mut content) = one_member();
        content.push(ContentDeclaration::new(id("stranger_mod")));
        assert_eq!(
            validate(&pack, &manifests, &content),
            Err(ClosureError::ContentUnpinned {
                owner: id("stranger_mod")
            })
        );
    }

    #[test]
    fn one_owner_declaring_its_content_twice_is_refused() {
        let (pack, manifests, mut content) = one_member();
        content.push(ContentDeclaration::new(id("base_mod")));
        assert_eq!(
            validate(&pack, &manifests, &content),
            Err(ClosureError::ContentTwice {
                owner: id("base_mod")
            })
        );
    }

    #[test]
    fn a_tag_member_the_game_owns_is_allowed_and_one_nothing_pins_is_refused() {
        let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
        let vanilla = RegistryKey::new(ContentKind::Item, named(VANILLA_NAMESPACE, "torch"));
        let (pack, manifests, _) = one_member();

        // The game's own item: allowed, and deliberately unverified.
        let mut allowed = ContentDeclaration::new(id("base_mod"));
        allowed
            .tag(&tag, &vanilla)
            .expect("a tag is not a claim on its members");
        validate(&pack, &manifests, &[allowed])
            .expect("adding a vanilla item to a tag is ordinary");

        // A namespace the pack pins nothing for: refused, and the reason says so.
        let mut stranger = ContentDeclaration::new(id("base_mod"));
        let absent = RegistryKey::new(ContentKind::Item, named("absent_mod", "lantern"));
        stranger
            .tag(&tag, &absent)
            .expect("a tag is not a claim on its members");
        let refused =
            validate(&pack, &manifests, &[stranger]).expect_err("nothing in the pack provides it");
        assert_eq!(
            refused,
            ClosureError::TagMember {
                tag: tag.to_string(),
                member: absent,
                kind: UnprovidedEntry::NoSuchMember,
            }
        );
        let text = refused.to_string();
        assert!(text.contains("absent_mod:lantern"), "{text}");
        assert!(text.contains(VANILLA_NAMESPACE), "{text}");
    }

    #[test]
    fn a_tag_member_in_a_pinned_mods_namespace_that_the_mod_never_declared_is_refused() {
        // The case that needs the members' own declarations rather than only the
        // pins: `base_mod` is in the pack, so the namespace is legitimate and the
        // entry still does not exist.
        let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
        let (pack, manifests, _) = one_member();
        let mut declaration = ContentDeclaration::new(id("base_mod"));
        let never = RegistryKey::new(ContentKind::Item, named("base_mod", "phantom_lantern"));
        declaration
            .tag(&tag, &never)
            .expect("a tag is not a claim on its members");

        let refused = validate(&pack, &manifests, &[declaration])
            .expect_err("the owner declares no such entry");
        assert_eq!(
            refused,
            ClosureError::TagMember {
                tag: tag.to_string(),
                member: never,
                kind: UnprovidedEntry::MemberDoesNotDeclareIt,
            }
        );
        assert!(
            refused.to_string().contains("declares no such entry"),
            "{refused}"
        );
    }

    #[test]
    fn two_members_claiming_one_registry_entry_are_both_named() {
        // Unrepresentable inside one declaration and perfectly representable across
        // two: `ContentDeclaration` names entries in its own namespace, so this needs
        // one mod claiming what another's namespace says is its.
        let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
        pack.pin(id("first_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        pack.pin(id("second_mod"), Version::new(1, 0, 0))
            .expect("a legal pin");
        let manifests = vec![
            manifest("first_mod", "1.0.0", None),
            manifest("second_mod", "1.0.0", None),
        ];
        let mut first = ContentDeclaration::new(id("first_mod"));
        first
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");
        let mut second = ContentDeclaration::new(id("second_mod"));
        second
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");

        // Two different entries, because each is namespaced by its owner. The
        // contested case needs one declaration to hold the other's key, which is
        // what a pack-level check is for.
        validate(&pack, &manifests, &[first, second]).expect("two namespaces, two entries");
    }

    // ------------------------------------------------ progression closure ---

    #[test]
    fn a_progression_step_requiring_a_step_nothing_declares_is_refused() {
        let (mut pack, manifests, content) = one_member();
        pack.progression_mut()
            .declare(named("example_pack", "start"), [], None)
            .expect("a legal step");
        pack.progression_mut()
            .declare(
                named("example_pack", "locked"),
                [named("example_pack", "absent")],
                None,
            )
            .expect("a step may name a prerequisite not yet declared");

        assert_eq!(
            validate(&pack, &manifests, &content),
            Err(ClosureError::PrerequisiteMissing {
                state: named("example_pack", "locked"),
                prerequisite: named("example_pack", "absent"),
            })
        );
    }

    #[test]
    fn a_progression_step_nothing_could_ever_reach_is_refused() {
        let (mut pack, manifests, content) = one_member();
        pack.progression_mut()
            .declare(
                named("example_pack", "left"),
                [named("example_pack", "right")],
                None,
            )
            .expect("a legal step");
        pack.progression_mut()
            .declare(
                named("example_pack", "right"),
                [named("example_pack", "left")],
                None,
            )
            .expect("a legal step");

        let refused = validate(&pack, &manifests, &content).expect_err("neither step has a root");
        assert_eq!(
            refused,
            ClosureError::StateUnreachable {
                state: named("example_pack", "left"),
            }
        );
        assert!(
            refused.to_string().contains("can never be reached"),
            "{refused}"
        );
    }

    #[test]
    fn a_progression_gated_on_an_entry_nothing_provides_is_refused() {
        let (mut pack, manifests, content) = one_member();
        let gate = RegistryKey::new(ContentKind::Item, named("base_mod", "phantom"));
        pack.progression_mut()
            .declare(named("example_pack", "start"), [], Some(gate.clone()))
            .expect("a legal step");

        assert_eq!(
            validate(&pack, &manifests, &content),
            Err(ClosureError::Gate {
                state: named("example_pack", "start"),
                entry: gate,
                kind: UnprovidedEntry::MemberDoesNotDeclareIt,
            })
        );
    }

    #[test]
    fn a_progression_gated_on_an_entry_a_member_declares_holds() {
        let (mut pack, manifests, _) = one_member();
        let mut declaration = ContentDeclaration::new(id("base_mod"));
        let lantern = declaration
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");
        pack.progression_mut()
            .declare(named("example_pack", "start"), [], Some(lantern))
            .expect("a legal step");
        pack.progression_mut()
            .declare(
                named("example_pack", "after"),
                [named("example_pack", "start")],
                None,
            )
            .expect("a legal step");

        validate(&pack, &manifests, &[declaration]).expect("the gate is an entry the pack holds");
    }

    // ------------------------------------------------------------ output ---

    #[test]
    fn a_packs_settings_document_is_written_under_ravels_own_directory() {
        let (mut pack, manifests, content) = one_member();
        let document = ValueDocument::parse(
            r#"{ "schemaVersion": 1, "owner": "base_mod", "values": { "limit": 3 } }"#,
        )
        .expect("a legal document");
        pack.configure(id("base_mod"), document)
            .expect("a document that names its own mod");

        let validated = validate(&pack, &manifests, &content).expect("every closure holds");
        let written = validated
            .file(&path(&format!("{RESERVED_PREFIX}config/base_mod.json")))
            .expect("the pack writes its own layer");
        let text = String::from_utf8(written.bytes().to_vec()).expect("the document is UTF-8");
        assert!(text.contains("\"limit\""), "{text}");
        assert!(
            text.ends_with('\n'),
            "every written document ends in a newline"
        );
    }

    #[test]
    fn validating_the_same_declaration_twice_produces_the_same_files() {
        let (mut pack, manifests, _) = one_member();
        let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
        let mut declaration = ContentDeclaration::new(id("base_mod"));
        let lantern = declaration
            .entry(ContentKind::Item, "lantern")
            .expect("a legal entry");
        declaration
            .tag(&tag, &lantern)
            .expect("an item joins an item tag");
        declaration
            .translation("en_us", "item.base_mod.lantern", "Lantern")
            .expect("a legal translation");
        pack.supply(
            path("data/example_pack/recipe/lantern.json"),
            b"{}\n".to_vec(),
        );
        pack.progression_mut()
            .declare(named("example_pack", "start"), [], Some(lantern))
            .expect("a legal step");

        let first = validate(&pack, &manifests, std::slice::from_ref(&declaration))
            .expect("every closure holds");
        let second = validate(&pack, &manifests, std::slice::from_ref(&declaration))
            .expect("every closure holds");
        assert_eq!(first, second);
        assert!(
            first
                .files()
                .windows(2)
                .all(|pair| pair[0].path().as_str() < pair[1].path().as_str()),
            "the files are in path order and each path appears once"
        );
    }
}
