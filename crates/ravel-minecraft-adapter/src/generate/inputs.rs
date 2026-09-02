//! The tracked inputs the generator is allowed to read.
//!
//! ADG-3 pins three files per version: the reviewed hook inventory, the adapter
//! inputs with their upstream hashes, and the generated mapping lock. Nothing
//! else is read, and an input whose bytes no longer match what is tracked is
//! refused before a single class is looked at.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::GenerateError;

/// Every pinned version's data lives here, relative to the repository root.
const VERSIONS: &str = "minecraft/versions";

/// Where the names the generator resolves against come from.
///
/// ADG-3's amendment admits two, and nothing else: an unknown value is refused
/// here rather than carried through as descriptive text, because this string
/// decides whether a name is translated before an archive is asked for it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MappingSource {
    /// The jar declares the names the reviewed inventory carries.
    UnobfuscatedClassFiles,
    /// The version's own published mapping file relates the two name spaces.
    OfficialProguardMappings,
}

impl MappingSource {
    /// How the adapter inputs and the generated locks write this down.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::UnobfuscatedClassFiles => "unobfuscated-class-files",
            Self::OfficialProguardMappings => "official-proguard-mappings",
        }
    }

    /// Whether this source needs a published mapping file to be read.
    pub(super) const fn needs_mappings(self) -> bool {
        matches!(self, Self::OfficialProguardMappings)
    }
}

/// Shape of a hook target, which the class file's flags have to agree with.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Kind {
    StaticMethod,
    InstanceMethod,
    StaticField,
    InstanceField,
}

impl Kind {
    /// Whether the target is a method rather than a field.
    pub(super) const fn is_method(self) -> bool {
        matches!(self, Self::StaticMethod | Self::InstanceMethod)
    }

    /// Whether the class file has to declare the target static.
    pub(super) const fn is_static(self) -> bool {
        matches!(self, Self::StaticMethod | Self::StaticField)
    }

    /// The `ravel-minecraft` variant the generated binding names.
    pub(super) const fn variant(self) -> &'static str {
        match self {
            Self::StaticMethod => "StaticMethod",
            Self::InstanceMethod => "InstanceMethod",
            Self::StaticField => "StaticField",
            Self::InstanceField => "InstanceField",
        }
    }
}

/// One reviewed hook entry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hook {
    pub(super) id: String,
    pub(super) class: String,
    pub(super) member: String,
    pub(super) descriptor: String,
    pub(super) kind: Kind,
    pub(super) capability: String,
    /// Whether this target's prologue may return from it before its body runs.
    ///
    /// ADG-1's Phase C amendment bounds the power by making the opt-in generated
    /// data rather than Java: the refusable set is exactly what this inventory
    /// names, so widening it re-pins a hash and shows up in a diff. Absent means
    /// false, which is what every observation hook is.
    #[serde(default)]
    pub(super) refuses: bool,
}

/// The tracked hook inventory.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Inventory {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    #[serde(rename = "hook")]
    pub(super) hooks: Vec<Hook>,
}

/// One class the settings screen compiles against.
///
/// The members are named and nothing else: every descriptor comes from the jar,
/// which is the whole reason these are generated rather than written by hand. A
/// name that the class overloads stubs every overload, because picking one would
/// mean writing down the descriptor that distinguishes them.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Seed {
    pub(super) class: String,
    pub(super) members: Vec<String>,
}

/// The tracked list of classes to stub.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Seeds {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    #[serde(rename = "stub")]
    pub(super) stubs: Vec<Seed>,
}

/// One pinned input file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    pub(super) path: String,
    #[serde(default)]
    pub(super) sha256: Option<String>,
    #[serde(default)]
    pub(super) bytes: Option<u64>,
    #[serde(default)]
    pub(super) upstream: Option<String>,
}

/// Inputs the generator is allowed to read.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Inputs {
    pub(super) client_jar: Input,
    /// The version's published client mapping, where the source calls for one.
    ///
    /// It is declared exactly as the jar is and never tracked, for the reasons
    /// ADG-3's amendment records: it is upstream material this repository does
    /// not own, it is derived from the build that produced the jar, and it is
    /// larger than every tracked file of a version put together. What is tracked
    /// is its hash.
    #[serde(default)]
    pub(super) client_mappings: Option<Input>,
    /// The library jar carrying the serialization types the game's signatures
    /// name. Those classes are not in the client jar, so a stub for one has to be
    /// read from the library that declares it rather than guessed at.
    pub(super) data_fixer_jar: Input,
    pub(super) version_manifest: Input,
    pub(super) hook_inventory: Input,
    pub(super) stub_seeds: Input,
    /// The generated mapping lock, pinned to the jars that produced it.
    pub(super) mappings_lock: Input,
    /// The generated stub lock, pinned to the jars that produced it.
    pub(super) stubs_lock: Input,
}

/// The tracked adapter inputs.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Adapter {
    pub(super) schema_version: u8,
    pub(super) minecraft: String,
    pub(super) mapping_source: MappingSource,
    pub(super) inputs: Inputs,
}

/// The one version this build generates against, and everything tracked for it.
#[derive(Clone, Debug)]
pub(super) struct PinnedVersion {
    pub(super) directory: PathBuf,
    pub(super) adapter: Adapter,
    pub(super) inventory: Inventory,
    pub(super) seeds: Seeds,
}

/// Reads the tracked data of the pinned version.
///
/// The adapter generates against exactly one version at a time. A second
/// directory under `minecraft/versions` is refused rather than guessed at,
/// because two versions would need two hook classes and two exported symbol
/// sets, and nothing has decided how those are told apart.
pub(super) fn pinned(root: &Path) -> Result<PinnedVersion, GenerateError> {
    let versions = root.join(VERSIONS);
    let mut names = Vec::new();
    let entries = fs::read_dir(&versions)
        .map_err(|error| GenerateError::Input(format!("cannot read the version data: {error}")))?;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();

    let [name] = names.as_slice() else {
        return Err(GenerateError::Input(format!(
            "the adapter generates one pinned version at a time, but the version data holds {}",
            names.len()
        )));
    };

    let directory = versions.join(name);
    let adapter: Adapter = parse(&directory.join("adapter.toml"))?;
    if &adapter.minecraft != name {
        return Err(GenerateError::Drift(format!(
            "the version directory is named {name} but its adapter inputs pin {}",
            adapter.minecraft
        )));
    }
    // The inventory is verified here rather than beside the jar, because it is
    // tracked and therefore present on every machine: a build that falls back to
    // the mapping lock still has to know the inventory is the reviewed one. It is
    // verified before it is parsed, so nothing reads meaning out of bytes that
    // have not been checked.
    verify_input(root, &adapter.inputs.hook_inventory)?;
    let inventory: Inventory = parse(&root.join(&adapter.inputs.hook_inventory.path))?;
    if inventory.minecraft != adapter.minecraft {
        return Err(GenerateError::Drift(format!(
            "the inventory pins {} but the adapter inputs pin {}",
            inventory.minecraft, adapter.minecraft
        )));
    }
    if inventory.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the inventory is schema {} but the adapter inputs are schema {}",
            inventory.schema_version, adapter.schema_version
        )));
    }

    // The seed list is tracked and therefore present everywhere too, for the
    // same reason: a build that generates stubs from the stub lock still has to
    // know which classes the lock was asked to cover.
    verify_input(root, &adapter.inputs.stub_seeds)?;
    let seeds: Seeds = parse(&root.join(&adapter.inputs.stub_seeds.path))?;
    if seeds.minecraft != adapter.minecraft || seeds.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the stub seeds pin {} at schema {} but the adapter inputs pin {} at schema {}",
            seeds.minecraft, seeds.schema_version, adapter.minecraft, adapter.schema_version
        )));
    }

    // The source decides whether a mapping file exists, so a pair that
    // disagrees is refused rather than resolved one way: a mapping file nothing
    // reads is a pin nobody maintains, and a source with none to read cannot
    // translate the name it is about to ask an archive for.
    match (
        adapter.mapping_source.needs_mappings(),
        adapter.inputs.client_mappings.is_some(),
    ) {
        (true, false) => {
            return Err(GenerateError::Input(format!(
                "{} reads a published mapping but the adapter inputs declare none",
                adapter.mapping_source.as_str()
            )));
        }
        (false, true) => {
            return Err(GenerateError::Input(format!(
                "{} reads names from the class files but the adapter inputs declare a mapping",
                adapter.mapping_source.as_str()
            )));
        }
        _ => {}
    }

    for input in [
        &adapter.inputs.client_jar,
        &adapter.inputs.data_fixer_jar,
        &adapter.inputs.version_manifest,
        &adapter.inputs.hook_inventory,
        &adapter.inputs.stub_seeds,
    ]
    .into_iter()
    .chain(adapter.inputs.client_mappings.as_ref())
    {
        // An input that records where it came from has to record a fetchable
        // origin, because that string is the only trail back to the bytes the
        // hash pins.
        if let Some(upstream) = &input.upstream
            && !upstream.starts_with("https://")
        {
            return Err(GenerateError::Input(format!(
                "{} names an upstream that is not an https address",
                file_name(&input.path)
            )));
        }
    }

    Ok(PinnedVersion {
        directory,
        adapter,
        inventory,
        seeds,
    })
}

/// Reads one tracked TOML document.
pub(super) fn parse<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, GenerateError> {
    let displayed = path.to_string_lossy();
    let name = file_name(&displayed);
    let text = fs::read_to_string(path)
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    toml::from_str(&text)
        .map_err(|error| GenerateError::Input(format!("cannot parse {name}: {error}")))
}

/// Refuses an input whose bytes no longer match the tracked hash.
///
/// An input that pins neither a hash nor a length is refused too. ADG-3 says
/// generation runs only from inputs whose hashes match what is tracked, and an
/// input with nothing to check against does not meet that by having nothing to
/// fail: it just makes the check vacuous, silently, for the one file nobody
/// remembered to pin.
pub(super) fn verify_input(root: &Path, input: &Input) -> Result<(), GenerateError> {
    // Messages name inputs by file name, never by path: these strings end up in
    // recorded evidence, and a path would say whose machine produced it.
    let name = file_name(&input.path);
    if input.sha256.is_none() && input.bytes.is_none() {
        return Err(GenerateError::Input(format!(
            "{name} pins neither a hash nor a length, so nothing about it is verified"
        )));
    }
    let bytes = fs::read(root.join(&input.path))
        .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
    if let Some(expected) = &input.sha256 {
        let actual = sha256(&bytes);
        if &actual != expected {
            return Err(GenerateError::Drift(format!(
                "{name} hashes {actual} but the adapter inputs pin {expected}"
            )));
        }
    }
    if let Some(expected) = input.bytes
        && bytes.len() as u64 != expected
    {
        return Err(GenerateError::Drift(format!(
            "{name} is {} bytes but the adapter inputs pin {expected}",
            bytes.len()
        )));
    }
    Ok(())
}

/// Rewrites what the adapter inputs pin for the two generated locks.
///
/// Only the generated locks are ever re-pinned, and only by the deliberate
/// regeneration path that has just written them from the jar. An upstream pin is
/// a claim about bytes nobody here produced, so nothing rewrites one.
pub(super) fn repin_locks(
    root: &Path,
    directory: &Path,
    inputs: &Inputs,
) -> Result<(), GenerateError> {
    let path = directory.join("adapter.toml");
    let mut document = fs::read_to_string(&path)
        .map_err(|error| GenerateError::Input(format!("cannot read adapter.toml: {error}")))?;
    for (table, input) in [
        ("[inputs.mappingsLock]", &inputs.mappings_lock),
        ("[inputs.stubsLock]", &inputs.stubs_lock),
    ] {
        let name = file_name(&input.path);
        let bytes = fs::read(root.join(&input.path))
            .map_err(|error| GenerateError::Input(format!("cannot read {name}: {error}")))?;
        document = replace_pin(&document, table, &sha256(&bytes), bytes.len() as u64)?;
    }
    fs::write(&path, document)
        .map_err(|error| GenerateError::Input(format!("cannot write adapter.toml: {error}")))
}

/// Replaces the hash and length one table of the adapter inputs records.
///
/// The document is edited line by line rather than deserialized and written
/// back, because every sentence of prose in `adapter.toml` explains a pin and a
/// round trip through the parser would drop all of it.
pub(super) fn replace_pin(
    document: &str,
    table: &str,
    sha256: &str,
    bytes: u64,
) -> Result<String, GenerateError> {
    let mut rewritten = String::with_capacity(document.len());
    let mut inside = false;
    let mut pinned_hash = false;
    let mut pinned_length = false;
    for line in document.lines() {
        if line.starts_with('[') {
            inside = line.trim() == table;
        }
        if inside && line.starts_with("sha256 = ") {
            rewritten.push_str("sha256 = \"");
            rewritten.push_str(sha256);
            rewritten.push('"');
            pinned_hash = true;
        } else if inside && line.starts_with("bytes = ") {
            rewritten.push_str("bytes = ");
            rewritten.push_str(&bytes.to_string());
            pinned_length = true;
        } else {
            rewritten.push_str(line);
        }
        rewritten.push('\n');
    }
    if !pinned_hash || !pinned_length {
        return Err(GenerateError::Input(format!(
            "the adapter inputs have no {table} hash and length to rewrite"
        )));
    }
    Ok(rewritten)
}

/// The last path segment, which is all a recorded message may say about a file.
pub(super) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Hex-encoded SHA-256 of some bytes.
pub(super) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    text
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Adapter, Input, Kind, MappingSource, file_name, pinned, sha256, verify_input};

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>")
    }

    #[test]
    fn the_tracked_version_data_reads_back_as_a_reviewed_inventory() {
        let pinned = pinned(repository_root()).expect("the tracked version data should parse");

        assert_eq!(pinned.adapter.schema_version, 1);
        assert_eq!(
            pinned.adapter.mapping_source,
            MappingSource::UnobfuscatedClassFiles
        );
        assert_eq!(pinned.inventory.minecraft, pinned.adapter.minecraft);
        assert!(
            pinned.directory.ends_with(&pinned.adapter.minecraft),
            "the directory names the version it pins"
        );
        assert!(
            pinned.inventory.hooks.len() >= 6,
            "the reviewed inventory should carry every entry ADG-3 resolved"
        );
        assert!(
            pinned
                .inventory
                .hooks
                .iter()
                .all(|hook| !hook.capability.is_empty()),
            "every entry carries the sentence its human review needs"
        );
        assert_eq!(pinned.seeds.minecraft, pinned.adapter.minecraft);

        // A seed with no members earns its place by being named by something
        // else in the set. Nothing calls a member on it, so the only work it can
        // do is being a type another stub already writes down: a supertype that
        // makes the classes under it assignable, or a descriptor that a member
        // of another stub declares. A seed nothing names stubs a class that
        // cannot be reached at all, which is a seed to delete.
        let lock = std::fs::read_to_string(pinned.directory.join("stubs.lock"))
            .expect("the tracked stub lock should be readable");
        for seed in &pinned.seeds.stubs {
            if !seed.members.is_empty() {
                continue;
            }
            assert!(
                named_by_another_stub(&lock, &seed.class),
                "{} is seeded with no members and nothing else in the set names it",
                seed.class
            );
        }
    }

    /// Whether anything else in `lock` writes `class` down.
    ///
    /// A supertype edge or a member descriptor, and nothing else: both are lines
    /// the generator rendered out of the jar, so neither can name a class the
    /// jar does not agree carries that relationship.
    fn named_by_another_stub(lock: &str, class: &str) -> bool {
        let internal = class.replace('.', "/");
        lock.lines().any(|line| {
            (line.starts_with("extends")
                || line.starts_with("implements")
                || line.starts_with("descriptor"))
                && line.contains(&internal)
        })
    }

    #[test]
    fn a_memberless_seed_nothing_else_names_is_refused_however_it_is_named() {
        let lock = "\
[[stub]]
class = \"net/minecraft/Named\"
extends = \"net/minecraft/Above\"

[[stub.member]]
name = \"take\"
descriptor = \"(Lnet/minecraft/Parameter;)V\"

[[stub]]
class = \"net/minecraft/Unreferenced\"
";

        // The two ways a seed earns its place, and both still earn it.
        assert!(named_by_another_stub(lock, "net.minecraft.Above"));
        assert!(named_by_another_stub(lock, "net.minecraft.Parameter"));

        // Its own declaration is not something else naming it, and neither is a
        // class the lock never mentions.
        assert!(!named_by_another_stub(lock, "net.minecraft.Unreferenced"));
        assert!(!named_by_another_stub(lock, "net.minecraft.Absent"));
    }

    #[test]
    fn a_version_tree_with_no_pinned_version_is_refused_rather_than_guessed() {
        let error = pinned(Path::new("/nonexistent-ravel-root")).expect_err("this must not parse");

        assert!(error.to_string().contains("cannot read the version data"));
    }

    #[test]
    fn every_kind_agrees_with_the_flags_a_class_file_declares() {
        for (kind, method, is_static, variant) in [
            (Kind::StaticMethod, true, true, "StaticMethod"),
            (Kind::InstanceMethod, true, false, "InstanceMethod"),
            (Kind::StaticField, false, true, "StaticField"),
            (Kind::InstanceField, false, false, "InstanceField"),
        ] {
            assert_eq!(kind.is_method(), method);
            assert_eq!(kind.is_static(), is_static);
            assert_eq!(kind.variant(), variant);
        }
    }

    #[test]
    fn an_input_that_no_longer_hashes_to_its_pin_is_refused() {
        let root = repository_root();
        let adapter: Adapter = super::parse(
            &root
                .join("minecraft/versions")
                .join(pinned(root).expect("pinned").adapter.minecraft)
                .join("adapter.toml"),
        )
        .expect("the tracked adapter inputs should parse");

        let drifted = Input {
            path: adapter.inputs.hook_inventory.path.clone(),
            sha256: Some("0".repeat(64)),
            bytes: None,
            upstream: None,
        };

        let error = verify_input(root, &drifted).expect_err("a drifted hash must be refused");
        let message = error.to_string();

        assert!(message.contains("hooks.toml"), "{message}");
        assert!(!message.contains('/'), "a message must not name a path");
    }

    #[test]
    fn an_input_of_the_wrong_length_is_refused_even_when_no_hash_is_pinned() {
        let root = repository_root();
        let wrong_length = Input {
            path: "minecraft/README.md".to_owned(),
            sha256: None,
            bytes: Some(1),
            upstream: None,
        };
        let missing = Input {
            path: "minecraft/absent.toml".to_owned(),
            sha256: None,
            bytes: Some(1),
            upstream: None,
        };

        assert!(
            verify_input(root, &wrong_length)
                .expect_err("a wrong length must be refused")
                .to_string()
                .contains("bytes but the adapter inputs pin")
        );
        assert!(
            verify_input(root, &missing)
                .expect_err("a missing input must be refused")
                .to_string()
                .contains("cannot read absent.toml")
        );
    }

    #[test]
    fn an_input_with_nothing_to_verify_against_is_refused_rather_than_passed() {
        let unpinned = Input {
            path: "minecraft/README.md".to_owned(),
            sha256: None,
            bytes: None,
            upstream: None,
        };

        let error = verify_input(repository_root(), &unpinned)
            .expect_err("an unverifiable input must be refused");

        assert!(
            error
                .to_string()
                .contains("pins neither a hash nor a length"),
            "{error}"
        );
    }

    #[test]
    fn the_tracked_adapter_inputs_pin_every_input_they_declare() {
        let pinned = pinned(repository_root()).expect("the tracked version data should parse");
        let inputs = &pinned.adapter.inputs;

        for input in [
            &inputs.client_jar,
            &inputs.data_fixer_jar,
            &inputs.version_manifest,
            &inputs.hook_inventory,
            &inputs.stub_seeds,
            &inputs.mappings_lock,
            &inputs.stubs_lock,
        ]
        .into_iter()
        .chain(inputs.client_mappings.as_ref())
        {
            assert!(
                input.sha256.is_some() || input.bytes.is_some(),
                "{} is declared with nothing to verify it against",
                file_name(&input.path)
            );
        }
        assert!(
            inputs.hook_inventory.sha256.is_some(),
            "the reviewed inventory decides what gets rewritten and must pin a hash"
        );
        assert!(
            inputs.stub_seeds.sha256.is_some(),
            "the seed list decides which game types the screen compiles against"
        );
        for lock in [&inputs.mappings_lock, &inputs.stubs_lock] {
            assert!(
                lock.sha256.is_some(),
                "{} is the only thing a jar-less machine can check the lock against",
                file_name(&lock.path)
            );
        }
    }

    #[test]
    fn repinning_rewrites_one_tables_pin_and_leaves_the_prose_alone() {
        let document = "\
# why the seeds are pinned
[inputs.stubSeeds]
path = \"stubs.toml\"
sha256 = \"aa\"
bytes = 1

# why the lock is pinned
[inputs.mappingsLock]
path = \"mappings.lock\"
sha256 = \"bb\"
bytes = 2
";

        let rewritten = super::replace_pin(document, "[inputs.mappingsLock]", "cc", 3)
            .expect("the table is there to rewrite");

        assert!(
            rewritten.contains("sha256 = \"cc\"\nbytes = 3\n"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("sha256 = \"aa\"\nbytes = 1\n"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("# why the lock is pinned"),
            "{rewritten}"
        );
    }

    #[test]
    fn repinning_a_table_that_pins_nothing_is_refused_rather_than_appended() {
        let error = super::replace_pin(
            "[inputs.stubsLock]\npath = \"a\"\n",
            "[inputs.stubsLock]",
            "cc",
            3,
        )
        .expect_err("there is no pin to rewrite");

        assert!(
            error.to_string().contains("hash and length to rewrite"),
            "{error}"
        );
    }

    #[test]
    fn a_message_names_a_file_and_never_the_directory_it_sits_in() {
        assert_eq!(file_name("a/b/client.jar"), "client.jar");
        assert_eq!(file_name("client.jar"), "client.jar");
    }

    #[test]
    fn hashing_matches_the_digest_the_adapter_inputs_pin() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"ravel"),
            sha256(b"ravel"),
            "hashing is a function of the bytes alone"
        );
        assert_ne!(sha256(b"ravel"), sha256(b"Ravel"));
    }

    /// Adapter inputs written out, with one mapping source and one mapping pin.
    fn document(source: &str, mappings: &str) -> String {
        format!(
            "schemaVersion = 1\n\
             minecraft = \"0.0.0-test\"\n\
             mappingSource = \"{source}\"\n\
             {mappings}\
             [inputs.clientJar]\npath = \"a\"\nsha256 = \"0\"\n\
             [inputs.dataFixerJar]\npath = \"b\"\nsha256 = \"0\"\n\
             [inputs.versionManifest]\npath = \"c\"\nsha256 = \"0\"\n\
             [inputs.hookInventory]\npath = \"d\"\nsha256 = \"0\"\n\
             [inputs.stubSeeds]\npath = \"e\"\nsha256 = \"0\"\n\
             [inputs.mappingsLock]\npath = \"f\"\nsha256 = \"0\"\n\
             [inputs.stubsLock]\npath = \"g\"\nsha256 = \"0\"\n"
        )
    }

    #[test]
    fn a_mapping_source_nothing_implements_is_refused_where_it_is_written() {
        let error = toml::from_str::<Adapter>(&document("community-mappings", ""))
            .expect_err("a source the generator cannot act on must be refused");

        assert!(error.to_string().contains("unknown variant"), "{error}");
        assert!(
            error.to_string().contains("unobfuscated-class-files"),
            "the refusal names the sources there are: {error}"
        );
    }

    #[test]
    fn both_admitted_sources_parse_and_write_back_the_string_they_came_from() {
        for source in [
            MappingSource::UnobfuscatedClassFiles,
            MappingSource::OfficialProguardMappings,
        ] {
            let mappings = if source.needs_mappings() {
                "[inputs.clientMappings]\npath = \"m\"\nsha256 = \"0\"\n"
            } else {
                ""
            };
            let adapter: Adapter = toml::from_str(&document(source.as_str(), mappings))
                .expect("an admitted source parses");

            assert_eq!(adapter.mapping_source, source);
            assert_eq!(
                adapter.inputs.client_mappings.is_some(),
                source.needs_mappings()
            );
        }
    }
}
