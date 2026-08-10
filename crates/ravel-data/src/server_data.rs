use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ContentId, ContentIdError, ModId, RelativePath};

use crate::generate::{GeneratedData, GeneratedFile};

/// A kind of entry the server resolves rather than the client.
///
/// Four kinds, because these are the four shapes a mod ships that are neither a
/// registry entry, a tag membership, a translation, nor a client-side asset. A
/// registry entry is claimed at the barrier and a server-resolved entry is not:
/// what it occupies is a path in the generated pack, and the question about it
/// is whether something already writes that path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ServerDataKind {
    /// A conversion from some items to others.
    Crafting,
    /// A table of what something drops.
    Loot,
    /// A step in a player's recorded progression.
    Progression,
    /// A named sequence of commands the server runs.
    Procedure,
}

impl ServerDataKind {
    /// Every kind, in the order this enum declares them.
    pub const ALL: [Self; 4] = [
        Self::Crafting,
        Self::Loot,
        Self::Progression,
        Self::Procedure,
    ];

    /// Returns the kind's canonical lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crafting => "crafting",
            Self::Loot => "loot",
            Self::Progression => "progression",
            Self::Procedure => "procedure",
        }
    }

    /// Returns the pack directory this kind's entries are generated into.
    ///
    /// The directory and the extension below are the pack format's conventions
    /// rather than Ravel's, and they live here for the same reason
    /// [`crate::TagId`]'s generated path does: one place derives them, so a mod
    /// cannot misfile its own data and a format change is one edit rather than a
    /// search. What is deliberately not derived here is the *content* of an
    /// entry, which a Minecraft version decides the shape of and the mod
    /// supplies as bytes.
    #[must_use]
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Crafting => "recipe",
            Self::Loot => "loot_table",
            Self::Progression => "advancement",
            Self::Procedure => "function",
        }
    }

    /// Returns the extension this kind's generated files carry.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Crafting | Self::Loot | Self::Progression => "json",
            Self::Procedure => "mcfunction",
        }
    }
}

impl fmt::Display for ServerDataKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One server-resolved entry a mod owns.
///
/// The path is the declaration's answer rather than the mod's guess, which is
/// the whole of what this type adds over handing bytes to
/// [`crate::ContentDeclaration::resource`]: a mod that shipped a table as
/// supplied bytes had to know where the pack format puts it, and was wrong
/// silently when it did not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerDataEntry {
    owner: ModId,
    kind: ServerDataKind,
    name: String,
    path: RelativePath,
}

impl ServerDataEntry {
    /// Returns the mod that owns the entry.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Returns the kind of entry this is.
    #[must_use]
    pub const fn kind(&self) -> ServerDataKind {
        self.kind
    }

    /// Returns the name the mod declared it under.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns where the entry is generated.
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }
}

/// The server-resolved data one mod owns, declared in one place.
///
/// A mod already declares its registry keys, its tag memberships and its
/// translations in a [`crate::ContentDeclaration`]. A server-resolved entry is
/// none of those — it is claimed by nobody, merged with nobody, and read by the
/// server when a world loads — so before this type the only way to ship one was
/// as supplied bytes at a path the mod had to spell itself. That is the escape
/// hatch the declaration treats as a last resort, and it is what this replaces.
///
/// One barrier answers whether a path is already claimed: every entry, of every
/// kind, goes through the same table, so two of a mod's own declarations that
/// would write one file are refused when the second is made rather than when the
/// file is written.
///
/// # Examples
///
/// ```
/// use ravel_core::ModId;
/// use ravel_data::{ServerDataDeclaration, ServerDataError, ServerDataKind};
///
/// let owner: ModId = "example_mod".parse().expect("a legal mod id");
/// let mut declared = ServerDataDeclaration::new(owner);
///
/// let plate = declared.crafting("plate_from_ingot", br#"{"result": "example_mod:plate"}"#)?;
/// assert_eq!(
///     plate.path().as_str(),
///     "data/example_mod/recipe/plate_from_ingot.json"
/// );
///
/// // The same name under two kinds is two entries, not a duplicate.
/// declared.entry(ServerDataKind::Loot, "plate_from_ingot", b"{}")?;
/// // The same name under one kind is the second claim the barrier refuses.
/// assert!(declared.crafting("plate_from_ingot", b"{}").is_err());
///
/// let generated = declared.generate();
/// assert_eq!(generated.files().count(), 2);
/// assert!(generated.contains(plate.path()));
/// # Ok::<(), ServerDataError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerDataDeclaration {
    owner: ModId,
    entries: BTreeMap<RelativePath, (ServerDataEntry, Vec<u8>)>,
}

impl ServerDataDeclaration {
    /// Starts an empty declaration owned by `owner`.
    #[must_use]
    pub fn new(owner: ModId) -> Self {
        Self {
            owner,
            entries: BTreeMap::new(),
        }
    }

    /// Returns the mod this declaration belongs to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Declares one entry of `kind`, named `name`, with `bytes` as its content.
    ///
    /// The bytes are the mod's, unread and unrewritten. What a table means is a
    /// Minecraft version's decision and modelling it here would put a version's
    /// shape into a version-neutral crate; what this crate decides is where the
    /// entry goes and whether the name is free.
    ///
    /// # Errors
    ///
    /// Returns [`ServerDataError::Name`] when `name` is not a legal content
    /// path, and [`ServerDataError::ClaimedPath`] when this declaration already
    /// writes the file the entry would be generated into.
    pub fn entry(
        &mut self,
        kind: ServerDataKind,
        name: &str,
        bytes: &[u8],
    ) -> Result<ServerDataEntry, ServerDataError> {
        let id = ContentId::new(self.owner.clone(), name)
            .map_err(|source| ServerDataError::Name { source })?;
        let path = Self::generated_path(kind, &id);
        if let Some((existing, _)) = self.entries.get(&path) {
            return Err(ServerDataError::ClaimedPath {
                path,
                kind: existing.kind,
                name: existing.name.clone(),
            });
        }
        let entry = ServerDataEntry {
            owner: self.owner.clone(),
            kind,
            name: name.to_owned(),
            path: path.clone(),
        };
        self.entries.insert(path, (entry.clone(), bytes.to_vec()));
        Ok(entry)
    }

    /// Declares one [`ServerDataKind::Crafting`] entry.
    ///
    /// The one shorthand, because a conversion between items is the entry a mod
    /// declares most and the kind is not worth spelling for it. Every other kind
    /// goes through [`ServerDataDeclaration::entry`], which is the same call.
    ///
    /// # Errors
    ///
    /// Returns what [`ServerDataDeclaration::entry`] returns.
    pub fn crafting(
        &mut self,
        name: &str,
        bytes: &[u8],
    ) -> Result<ServerDataEntry, ServerDataError> {
        self.entry(ServerDataKind::Crafting, name, bytes)
    }

    /// Returns every entry this mod declared, in path order.
    pub fn entries(&self) -> impl Iterator<Item = &ServerDataEntry> + '_ {
        self.entries.values().map(|(entry, _)| entry)
    }

    /// Returns the entry generated at `path`, if this declaration writes one.
    #[must_use]
    pub fn at(&self, path: &RelativePath) -> Option<&ServerDataEntry> {
        self.entries.get(path).map(|(entry, _)| entry)
    }

    /// Produces the files this declaration describes.
    ///
    /// The result is a pure function of what was declared and not of the order it
    /// was declared in: the entries are held in a [`BTreeMap`] keyed by their
    /// generated path and [`GeneratedData`] orders its files by path, so the same
    /// entries declared in two orders produce the same bytes in the same
    /// sequence. Nothing here reads a clock, a filesystem, an environment
    /// variable, or a hash map's iteration order.
    #[must_use]
    pub fn generate(&self) -> GeneratedServerData {
        let mut data = GeneratedData::default();
        for (path, (_, bytes)) in &self.entries {
            data.insert(path.clone(), bytes.clone());
        }
        GeneratedServerData { data }
    }

    /// Returns the path an entry of `kind` named by `id` is generated into.
    ///
    /// # Panics
    ///
    /// Panics if the derived path is not a legal [`RelativePath`], which cannot
    /// happen: a [`ModId`] and a validated [`ContentId`] hold no backslash, no
    /// line terminator and — since the traversal rule became one rule rather
    /// than two — no `..` segment either. Before it was shared, `../elsewhere`
    /// was a legal content path whose derived pack path was refused here, and
    /// this expectation was the crash that made it visible.
    fn generated_path(kind: ServerDataKind, id: &ContentId) -> RelativePath {
        RelativePath::try_from(format!(
            "data/{}/{}/{}.{}",
            id.namespace(),
            kind.directory(),
            id.path(),
            kind.extension()
        ))
        .expect("a mod id and a validated content path cannot spell an illegal path")
    }
}

/// The files a server-data declaration produces.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GeneratedServerData {
    data: GeneratedData,
}

impl GeneratedServerData {
    /// Returns every generated file, in path order.
    pub fn files(&self) -> impl Iterator<Item = &GeneratedFile> + '_ {
        self.data.files().iter()
    }

    /// Returns whether a file is generated at `path`.
    #[must_use]
    pub fn contains(&self, path: &RelativePath) -> bool {
        self.data.file(path).is_some()
    }

    /// Returns the file at `path`, if the declaration produces one.
    #[must_use]
    pub fn file(&self, path: &RelativePath) -> Option<&GeneratedFile> {
        self.data.file(path)
    }

    /// Returns how many files the declaration produces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns whether the declaration produces nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Why a mod's own server-data declaration was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerDataError {
    /// The declared name is not a legal content path.
    Name {
        /// Why the name was refused.
        source: ContentIdError,
    },
    /// Something this mod already declared writes the same file.
    ClaimedPath {
        /// The contested output path.
        path: RelativePath,
        /// The kind that already writes it.
        kind: ServerDataKind,
        /// The name it was declared under.
        name: String,
    },
}

impl fmt::Display for ServerDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { source } => write!(formatter, "{source}"),
            Self::ClaimedPath { path, kind, name } => write!(
                formatter,
                "`{path}` is already written by the `{kind}` entry `{name}`, and one path holds \
                 one file"
            ),
        }
    }
}

impl std::error::Error for ServerDataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Name { source } => Some(source),
            Self::ClaimedPath { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, RelativePath};

    use super::{ServerDataDeclaration, ServerDataError, ServerDataKind};

    const BODY: &[u8] = br#"{"result": "sample:plate"}"#;

    fn owner() -> ModId {
        "sample".parse().expect("a well formed owner id")
    }

    fn declare(order: [&str; 3]) -> Vec<(String, Vec<u8>)> {
        let mut declared = ServerDataDeclaration::new(owner());
        for name in order {
            declared
                .crafting(name, BODY)
                .expect("each name is claimed once");
        }
        declared
            .generate()
            .files()
            .map(|file| (file.path().to_string(), file.bytes().to_vec()))
            .collect()
    }

    #[test]
    fn a_declared_server_entry_reaches_generation() {
        let mut declared = ServerDataDeclaration::new(owner());

        let entry = declared
            .crafting("plate_from_ingot", BODY)
            .expect("a first claim on an unclaimed name is taken");

        assert_eq!(entry.owner(), &owner());
        assert_eq!(entry.name(), "plate_from_ingot");
        assert_eq!(entry.kind(), ServerDataKind::Crafting);

        let generated = declared.generate();
        assert_eq!(generated.files().count(), 1);
        assert!(generated.contains(entry.path()));
    }

    #[test]
    fn generation_of_nothing_is_empty_rather_than_refused() {
        let declared = ServerDataDeclaration::new(owner());
        let generated = declared.generate();

        assert_eq!(generated.files().count(), 0);
        assert!(generated.is_empty());
        assert_eq!(generated.len(), 0);
    }

    /// The path is pinned here as text a reader can check against the pack
    /// format, rather than being read back out of the declaration that derived
    /// it. Asking the declaration where the entry went would agree with the
    /// declaration whatever it decided.
    #[test]
    fn every_kind_is_generated_at_the_path_this_test_spells_out() {
        let mut declared = ServerDataDeclaration::new(owner());
        let expected = [
            (ServerDataKind::Crafting, "data/sample/recipe/thing.json"),
            (ServerDataKind::Loot, "data/sample/loot_table/thing.json"),
            (
                ServerDataKind::Progression,
                "data/sample/advancement/thing.json",
            ),
            (
                ServerDataKind::Procedure,
                "data/sample/function/thing.mcfunction",
            ),
        ];

        for (kind, path) in expected {
            let entry = declared
                .entry(kind, "thing", BODY)
                .expect("one name under four kinds is four entries");
            assert_eq!(entry.path().as_str(), path);
        }
        assert_eq!(declared.generate().files().count(), 4);
    }

    #[test]
    fn a_nested_name_keeps_its_segments_and_gains_the_kinds_extension() {
        let mut declared = ServerDataDeclaration::new(owner());
        let entry = declared
            .crafting("smelting/plate", BODY)
            .expect("a nested name is a legal content path");
        assert_eq!(
            entry.path().as_str(),
            "data/sample/recipe/smelting/plate.json"
        );
    }

    #[test]
    fn a_second_claim_on_one_name_is_refused_naming_what_already_writes_it() {
        let mut declared = ServerDataDeclaration::new(owner());
        declared
            .crafting("plate_from_ingot", BODY)
            .expect("a first claim is taken");

        let refusal = declared
            .crafting("plate_from_ingot", b"{}")
            .expect_err("a second claim on one path is refused");

        assert!(matches!(refusal, ServerDataError::ClaimedPath { .. }));
        assert!(refusal.to_string().contains("plate_from_ingot"));
    }

    #[test]
    fn a_name_that_is_not_a_content_path_is_refused_rather_than_escaped() {
        let mut declared = ServerDataDeclaration::new(owner());
        let refusal = declared
            .crafting("Uppercase", BODY)
            .expect_err("a capital is not a legal content path");
        assert!(matches!(refusal, ServerDataError::Name { .. }));
    }

    /// A content path and a pack path share one traversal rule, and this is the
    /// name that used to fall between them: legal as a content id, refused as a
    /// pack path, so deriving it panicked inside the declaration call.
    #[test]
    fn a_name_that_would_leave_the_mods_own_directory_is_refused() {
        let mut declared = ServerDataDeclaration::new(owner());
        let refusal = declared
            .crafting("../elsewhere", BODY)
            .expect_err("a traversal is refused rather than generated");
        assert!(matches!(refusal, ServerDataError::Name { .. }));
        assert!(declared.generate().is_empty());
    }

    #[test]
    fn generated_bytes_do_not_depend_on_declaration_order() {
        let forward = declare(["first", "second", "third"]);
        let backward = declare(["third", "second", "first"]);

        assert_eq!(forward, backward);
    }

    #[test]
    fn generated_bytes_do_not_change_between_runs() {
        let once = declare(["first", "second", "third"]);
        let twice = declare(["first", "second", "third"]);

        assert_eq!(once, twice);
    }

    /// The bytes a mod supplied are the bytes generated, checked against a
    /// literal rather than against the declaration that stored them.
    #[test]
    fn the_generated_file_holds_exactly_the_bytes_the_mod_supplied() {
        let mut declared = ServerDataDeclaration::new(owner());
        let entry = declared
            .crafting("plate_from_ingot", BODY)
            .expect("a first claim is taken");

        let path = RelativePath::try_from("data/sample/recipe/plate_from_ingot.json".to_owned())
            .expect("a legal relative path");
        assert_eq!(entry.path(), &path);

        let generated = declared.generate();
        let file = generated.file(&path).expect("the entry was declared");
        assert_eq!(file.bytes(), br#"{"result": "sample:plate"}"#);
    }
}
