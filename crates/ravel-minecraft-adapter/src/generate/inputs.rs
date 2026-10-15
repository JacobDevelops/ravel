//! Loading and validation of pinned generator inputs.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{GenerateError, SELECTION, jar};

mod schema;

pub(super) use schema::*;

/// Every pinned version's data lives here, relative to the repository root.
const VERSIONS: &str = "minecraft/versions";

/// The one version this build generates against, and everything tracked for it.
#[derive(Clone, Debug)]
pub(super) struct PinnedVersion {
    pub(super) directory: PathBuf,
    pub(super) adapter: Adapter,
    pub(super) inventory: Inventory,
    pub(super) seeds: Seeds,
    pub(super) surface: Reflection,
}

/// Reads the tracked data of the version this build generates against.
pub(super) fn pinned(root: &Path, selected: Option<&str>) -> Result<PinnedVersion, GenerateError> {
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

    let name = choose(&names, selected)?;
    let directory = versions.join(name);
    let adapter: Adapter = parse(&directory.join("adapter.toml"))?;
    if adapter.minecraft != name {
        return Err(GenerateError::Drift(format!(
            "the version directory is named {name} but its adapter inputs pin {}",
            adapter.minecraft
        )));
    }
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
    check_non_goals(&inventory)?;
    check_title_menu_row(&inventory)?;

    verify_input(root, &adapter.inputs.stub_seeds)?;
    let seeds: Seeds = parse(&root.join(&adapter.inputs.stub_seeds.path))?;
    if seeds.minecraft != adapter.minecraft || seeds.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the stub seeds pin {} at schema {} but the adapter inputs pin {} at schema {}",
            seeds.minecraft, seeds.schema_version, adapter.minecraft, adapter.schema_version
        )));
    }
    check_roles(&seeds)?;

    let surface = reflection(root, &adapter)?;

    pinned_sources(&adapter)?;

    verify_java_release(root, &adapter)?;

    Ok(PinnedVersion {
        directory,
        adapter,
        inventory,
        seeds,
        surface,
    })
}

fn pinned_sources(adapter: &Adapter) -> Result<(), GenerateError> {
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
        (true, true) | (false, false) => {}
    }

    for input in [
        &adapter.inputs.client_jar,
        &adapter.inputs.data_fixer_jar,
        &adapter.inputs.version_manifest,
        &adapter.inputs.hook_inventory,
        &adapter.inputs.stub_seeds,
        &adapter.inputs.reflective_surface,
    ]
    .into_iter()
    .chain(adapter.inputs.client_mappings.as_ref())
    {
        if let Some(upstream) = &input.upstream
            && !upstream.starts_with("https://")
        {
            return Err(GenerateError::Input(format!(
                "{} names an upstream that is not an https address",
                file_name(&input.path)
            )));
        }
    }
    Ok(())
}

/// Checks the declared Java release against an available version manifest.
///
/// # Errors
///
/// Returns an error if the manifest is unreadable or its Java release is missing or inconsistent.
fn verify_java_release(root: &Path, adapter: &Adapter) -> Result<(), GenerateError> {
    let path = root.join(&adapter.inputs.version_manifest.path);
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&path).map_err(|error| {
        GenerateError::Input(format!("{} is unreadable: {error}", path.display()))
    })?;
    let declared = major_version(&text).ok_or_else(|| {
        GenerateError::Input(format!(
            "{} states no javaVersion.majorVersion, so nothing says what release this version runs",
            file_name(&adapter.inputs.version_manifest.path)
        ))
    })?;
    if declared != adapter.java_release {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare javaRelease {} but {} states {declared}",
            adapter.java_release,
            file_name(&adapter.inputs.version_manifest.path)
        )));
    }
    Ok(())
}

/// Checks the declared pack formats against the pinned jar.
///
/// # Errors
///
/// Returns an error if the jar's pack formats are missing or inconsistent with the declaration.
pub(super) fn verify_pack_format(
    adapter: &Adapter,
    archive: &jar::Archive,
) -> Result<(), GenerateError> {
    let text = jar_text(archive, JAR_VERSION)?;
    let declared = adapter.pack_format;
    let stated = pack_versions(&text).ok_or_else(|| {
        GenerateError::Input(format!(
            "{JAR_VERSION} in the pinned jar states no pack_version, so nothing says which pack \
             formats this version reads"
        ))
    })?;
    let expected = (
        declared.resource,
        declared.resource_minor,
        declared.data,
        declared.data_minor,
    );
    if stated != expected {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat {expected:?} but {JAR_VERSION} in the pinned \
             jar states {stated:?}, as resource major, resource minor, data major, data minor"
        )));
    }
    verify_spelling(declared.spelling, archive)
}

/// Checks the declared metadata spelling against a pack in the pinned jar.
///
/// # Errors
///
/// Returns an error if the jar contains no pack or its metadata uses another spelling.
fn verify_spelling(spelling: Spelling, archive: &jar::Archive) -> Result<(), GenerateError> {
    let Some(name) = archive.names().find(|name| name.ends_with(PACK_METADATA)) else {
        return Err(GenerateError::Input(format!(
            "the pinned jar ships no {PACK_METADATA} of its own, so nothing says which shape a \
             pack this version reads is written in"
        )));
    };
    let name = name.to_owned();
    let text = jar_text(archive, &name)?;
    if !text.contains(spelling.key()) {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat spelling {} but {name} in the pinned jar \
             carries no {}",
            spelling.as_str(),
            spelling.key()
        )));
    }
    Ok(())
}

/// One named entry of the pinned jar, as text.
fn jar_text(archive: &jar::Archive, name: &str) -> Result<String, GenerateError> {
    let bytes = archive.entry(name)?;
    String::from_utf8(bytes).map_err(|error| {
        GenerateError::Input(format!("{name} in the pinned jar is not UTF-8: {error}"))
    })
}

/// The document every pack, Ravel's included, declares its format in.
const PACK_METADATA: &str = "pack.mcmeta";

/// The document inside a client jar that states what the build is.
const JAR_VERSION: &str = "version.json";

/// The resource and data pack formats one jar's `version.json` states.
fn pack_versions(text: &str) -> Option<(u16, u16, u16, u16)> {
    let named = text.find("\"pack_version\"")?;
    let open = text[named..].find('{')? + named;
    let close = text[open..].find('}')? + open;
    let scope = &text[open..close];
    if let Some(resource) = number_after(scope, "resource") {
        return Some((resource, 0, number_after(scope, "data")?, 0));
    }
    Some((
        number_after(scope, "resource_major")?,
        number_after(scope, "resource_minor")?,
        number_after(scope, "data_major")?,
        number_after(scope, "data_minor")?,
    ))
}

/// The number one JSON key is set to, where the key is spelled exactly.
fn number_after(text: &str, key: &str) -> Option<u16> {
    let quoted = format!("\"{key}\"");
    let at = text.find(&quoted)? + quoted.len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The `javaVersion.majorVersion` one version manifest states.
fn major_version(text: &str) -> Option<u16> {
    let object = text.find("\"javaVersion\"")?;
    let at = text[object..].find("\"majorVersion\"")? + object + "\"majorVersion\"".len();
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The seam whose shape decides whether a title-menu row has to be declared.
pub(super) const TITLE_MENU_SEAM: &str = "screen.title-menu-options";

/// Checks that the title-menu row is declared exactly where it is needed.
///
/// # Errors
///
/// Returns an error if a row is missing, redundant, or declared on another hook.
fn check_title_menu_row(inventory: &Inventory) -> Result<(), GenerateError> {
    for hook in &inventory.hooks {
        if hook.title_menu_row.is_some() && hook.id != TITLE_MENU_SEAM {
            return Err(GenerateError::Drift(format!(
                "{} declares a title-menu row and nothing reads one for it; only {TITLE_MENU_SEAM} \
                 places a row",
                hook.id
            )));
        }
    }
    let Some(seam) = inventory
        .hooks
        .iter()
        .find(|hook| hook.id == TITLE_MENU_SEAM)
    else {
        return Ok(());
    };
    let returns_void = seam.descriptor.ends_with(")V");
    match (returns_void, seam.title_menu_row) {
        (true, None) => Err(GenerateError::Drift(format!(
            "{TITLE_MENU_SEAM} returns void here, so it hands back no row and nothing else says \
             where one is free; declare titleMenuRow, read off this version's title screen"
        ))),
        (false, Some(row)) => Err(GenerateError::Drift(format!(
            "{TITLE_MENU_SEAM} hands back the row it took, so the declared titleMenuRow of {row} \
             is a second answer to a question this version already answers"
        ))),
        _ => Ok(()),
    }
}

/// Checks that a version's declared non-goals say something the hooks do not.
///
/// # Errors
///
/// Returns an error if a non-goal is duplicated, has no reason, or is also a hook.
fn check_non_goals(inventory: &Inventory) -> Result<(), GenerateError> {
    let mut seen = BTreeSet::new();
    for non_goal in &inventory.non_goals {
        if inventory.hooks.iter().any(|hook| hook.id == non_goal.id) {
            return Err(GenerateError::Drift(format!(
                "the inventory declares {} as a hook and as a non-goal, and one id is either \
                 carried or not",
                non_goal.id
            )));
        }
        if non_goal.reason.trim().is_empty() {
            return Err(GenerateError::Drift(format!(
                "the inventory declares {} a non-goal with no reason, which tells a reader who \
                 went looking for it nothing",
                non_goal.id
            )));
        }
        if !seen.insert(non_goal.id.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the inventory declares the non-goal {} twice",
                non_goal.id
            )));
        }
    }
    Ok(())
}

/// Checks that every role a screen template could name is declared exactly once.
///
/// # Errors
///
/// Returns an error if a class, member, or role is invalid or duplicated.
fn check_roles(seeds: &Seeds) -> Result<(), GenerateError> {
    let mut seen = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for seed in &seeds.stubs {
        if !role_reference(&seed.role) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare {} as a class role, which is not a name a template can \
                 write",
                seed.role
            )));
        }
        if !class_name(&seed.class) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare {} as a class, which is not one",
                seed.class
            )));
        }
        if !seen.insert(seed.role.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare the class role {} twice, and one role names one class",
                seed.role
            )));
        }
        if !classes.insert(seed.class.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds bind two roles to {}, and one class is stubbed once",
                seed.class
            )));
        }
        let mut members = BTreeSet::new();
        let mut bound = BTreeSet::new();
        for member in &seed.members {
            if !member_reference(member.role()) || !member_reference(member.member()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds bind {}'s member role {} to {}, and one of the two is not a \
                     name a template can write",
                    seed.role,
                    member.role(),
                    member.member()
                )));
            }
            if !members.insert(member.role()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds declare {}'s member role {} twice, and one role names one \
                     member",
                    seed.role,
                    member.role()
                )));
            }
            if !bound.insert(member.member()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds bind two of {}'s member roles to {}, which stubs it twice",
                    seed.role,
                    member.member()
                )));
            }
        }
    }
    Ok(())
}

/// Whether a value is spelled like a role a screen template can write.
fn role_reference(value: &str) -> bool {
    !value.is_empty()
        && value.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
}

/// Whether a value is spelled like a member a seed list can bind.
fn member_reference(value: &str) -> bool {
    matches!(value, "<init>" | "<clinit>") || role_reference(value)
}

/// Reads the reviewed set of classes the agent jar's adapters resolve by name.
///
/// # Errors
///
/// Returns an error if the surface is missing, unpinned, inconsistent, or invalid.
fn reflection(root: &Path, adapter: &Adapter) -> Result<Reflection, GenerateError> {
    verify_input(root, &adapter.inputs.reflective_surface)?;
    let surface: Reflection = parse(&root.join(&adapter.inputs.reflective_surface.path))?;
    if surface.minecraft != adapter.minecraft || surface.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the reflective surface pins {} at schema {} but the adapter inputs pin {} at schema \
             {}",
            surface.minecraft, surface.schema_version, adapter.minecraft, adapter.schema_version
        )));
    }
    verify_surface(&surface)?;
    Ok(surface)
}

/// Refuses a surface that cannot be rendered or cannot be checked.
///
/// # Errors
///
/// Returns an error if a name, role, or member declaration is invalid or duplicated.
fn verify_surface(surface: &Reflection) -> Result<(), GenerateError> {
    for reflected in &surface.classes {
        for (what, value, legal) in [
            ("role", &reflected.name, role_name(&reflected.name)),
            (
                "runtime name",
                &reflected.runtime,
                class_name(&reflected.runtime),
            ),
            (
                "reviewed name",
                &reflected.class,
                class_name(&reflected.class),
            ),
        ] {
            if !legal {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {value} as a {what}, which is not one"
                )));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for reflected in &surface.classes {
        if !seen.insert(reflected.name.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the reflective surface declares the role {} twice, and one role names one class",
                reflected.name
            )));
        }
    }
    for reflected in &surface.classes {
        let mut declared = BTreeSet::new();
        for member in &reflected.members {
            if !member_name(&member.name) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {} of {} as a member, which is not a member \
                     name",
                    member.name, reflected.class
                )));
            }
            let shape = match member.lookup().map_err(|detail| {
                GenerateError::Drift(format!(
                    "the reflective surface declares a member of {}: {detail}",
                    reflected.class
                ))
            })? {
                Lookup::Descriptor(descriptor) => descriptor.to_owned(),
                Lookup::Parameters(count) => format!("/{count}"),
            };
            if !declared.insert(format!("{}{shape}", member.name)) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {}{shape} of {} twice",
                    member.name, reflected.class
                )));
            }
        }
    }
    Ok(())
}

/// Whether a value is a member name and nothing else.
fn member_name(value: &str) -> bool {
    matches!(value, "<init>" | "<clinit>")
        || (!value.is_empty()
            && !value.starts_with(|first: char| first.is_ascii_digit())
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_'))
}

/// Whether a role is spelled the way the generated constants are.
fn role_name(value: &str) -> bool {
    value.starts_with(|first: char| first.is_ascii_uppercase())
        && value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Whether a value is a binary class name and nothing else.
fn class_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with(|first: char| first.is_ascii_digit())
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
                })
        })
}

/// Picks the one version directory this build reads.
fn choose<'a>(names: &'a [String], selected: Option<&str>) -> Result<&'a str, GenerateError> {
    if let Some(wanted) = selected {
        return names
            .iter()
            .find(|name| name.as_str() == wanted)
            .map(String::as_str)
            .ok_or_else(|| {
                let what = if wanted.is_empty() {
                    "names nothing".to_owned()
                } else {
                    format!("names {wanted}")
                };
                GenerateError::Input(format!(
                    "{SELECTION} {what}, but the version data holds {}",
                    listed(names)
                ))
            });
    }
    match names {
        [] => Err(GenerateError::Input(
            "the version data holds no version directory to generate against".to_owned(),
        )),
        [only] => Ok(only),
        _ => Err(GenerateError::Input(format!(
            "the adapter emits one version per build and the version data holds {}, so \
             {SELECTION} has to name the one this build means",
            listed(names)
        ))),
    }
}

/// Names the version directories a refusal found, for a reader of that refusal.
fn listed(names: &[String]) -> String {
    if names.is_empty() {
        "no version directory".to_owned()
    } else {
        names.join(", ")
    }
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
pub(super) fn verify_input(root: &Path, input: &Input) -> Result<(), GenerateError> {
    // Omit paths from recorded evidence to avoid exposing machine-local details.
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
        && u64::try_from(bytes.len()).expect("input length fits in u64") != expected
    {
        return Err(GenerateError::Drift(format!(
            "{name} is {} bytes but the adapter inputs pin {expected}",
            bytes.len()
        )));
    }
    Ok(())
}

/// Rewrites what the adapter inputs pin for the two generated locks.
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
        let length = u64::try_from(bytes.len()).expect("lock length fits in u64");
        document = replace_pin(&document, table, &sha256(&bytes), length)?;
    }
    fs::write(&path, document)
        .map_err(|error| GenerateError::Input(format!("cannot write adapter.toml: {error}")))
}

/// Replaces one table's hash and length without discarding TOML comments.
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
/// Every version directory this repository tracks, in name order.
pub(super) fn tracked_versions(root: &Path) -> Vec<String> {
    let mut names = fs::read_dir(root.join(VERSIONS))
        .expect("the version data should be readable")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert!(!names.is_empty(), "the repository tracks a version");
    names
}

/// The first tracked version whose adapter inputs have the shape a test needs.
///
/// # Panics
///
/// Panics when no tracked version has the requested shape.
#[cfg(test)]
pub(super) fn tracked_version_where(
    root: &Path,
    wanted: impl Fn(&Adapter) -> bool,
) -> PinnedVersion {
    for version in tracked_versions(root) {
        let pinned = pinned(root, Some(&version)).expect("the tracked version data should parse");
        if wanted(&pinned.adapter) {
            return pinned;
        }
    }
    panic!("no tracked version has the shape this test needs");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        Adapter, Input, Kind, MappingSource, Reflection, file_name, pinned, sha256, verify_input,
        verify_surface,
    };

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>")
    }

    fn bootstrap_sources(root: &Path) -> String {
        fn walk(directory: &Path, read: &mut String) {
            let mut entries: Vec<_> = std::fs::read_dir(directory)
                .expect("the agent jar's source tree is readable")
                .map(|entry| entry.expect("a readable directory entry").path())
                .collect();
            entries.sort();
            for entry in entries {
                if entry.is_dir() {
                    walk(&entry, read);
                } else if entry.extension().is_some_and(|suffix| suffix == "java") {
                    read.push_str(&std::fs::read_to_string(&entry).expect("a readable source"));
                }
            }
        }

        let mut read = String::new();
        walk(&root.join("jvm/bootstrap"), &mut read);
        read
    }

    #[test]
    fn the_java_release_is_read_from_the_java_version_object() {
        let manifest = r#"{
  "downloads": {"client": {"majorVersion": 3, "size": 1}},
  "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21}
}"#;

        assert_eq!(super::major_version(manifest), Some(21));
    }

    #[test]
    fn both_spellings_of_a_jars_pack_version_are_read_and_neither_reads_the_other() {
        let joined = r#"{"data": 9, "pack_version": {"resource": 34, "data": 48}}"#;
        assert_eq!(super::pack_versions(joined), Some((34, 0, 48, 0)));

        let split = r#"{"pack_version": {"resource_major": 84, "resource_minor": 0,
          "data_major": 101, "data_minor": 1}, "world_version": 4790}"#;
        assert_eq!(super::pack_versions(split), Some((84, 0, 101, 1)));

        assert_eq!(super::pack_versions(r#"{"world_version": 4790}"#), None);
        assert_eq!(
            super::pack_versions(r#"{"pack_version": {"data": 48}}"#),
            None
        );
    }

    #[test]
    fn the_tracked_version_data_reads_back_as_a_reviewed_inventory() {
        let root = repository_root();
        for version in super::tracked_versions(root) {
            let pinned =
                pinned(root, Some(&version)).expect("the tracked version data should parse");

            assert_eq!(pinned.adapter.schema_version, 1);
            assert_eq!(
                pinned.adapter.mapping_source.needs_mappings(),
                pinned.adapter.inputs.client_mappings.is_some()
            );
            assert_eq!(pinned.inventory.minecraft, pinned.adapter.minecraft);
            assert!(
                pinned.directory.ends_with(&pinned.adapter.minecraft),
                "the directory names the version it pins"
            );
            assert!(
                pinned.inventory.hooks.len() >= 6,
                "the reviewed inventory should carry at least six resolved hook targets"
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

            let descriptors = pinned
                .surface
                .classes
                .iter()
                .flat_map(|reflected| &reflected.members)
                .filter_map(|member| member.descriptor.as_deref())
                .collect::<Vec<_>>();
            let agent = bootstrap_sources(root);
            for reflected in &pinned.surface.classes {
                if reflected.members.is_empty() {
                    let named = format!("L{};", reflected.class.replace('.', "/"));
                    let resolved = format!("GameNames.{}", reflected.name);
                    assert!(
                        descriptors
                            .iter()
                            .any(|descriptor| descriptor.contains(&named))
                            || agent.contains(&resolved),
                        "{} is reflected, declares no member the agent jar reaches, and is \
                         named by neither a member's descriptor nor the agent jar itself",
                        reflected.class
                    );
                }
            }

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
    }

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

        assert!(named_by_another_stub(lock, "net.minecraft.Above"));
        assert!(named_by_another_stub(lock, "net.minecraft.Parameter"));

        assert!(!named_by_another_stub(lock, "net.minecraft.Unreferenced"));
        assert!(!named_by_another_stub(lock, "net.minecraft.Absent"));
    }

    #[test]
    fn a_version_tree_with_no_pinned_version_is_refused_rather_than_guessed() {
        let error =
            pinned(Path::new("/nonexistent-ravel-root"), None).expect_err("this must not parse");

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
                .join(&super::tracked_versions(root)[0])
                .join("adapter.toml"),
        )
        .expect("the tracked adapter inputs should parse");

        let drifted = Input {
            path: adapter.inputs.hook_inventory.path,
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
        let root = repository_root();
        for version in super::tracked_versions(root) {
            let pinned =
                pinned(root, Some(&version)).expect("the tracked version data should parse");
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

    fn document(source: &str, mappings: &str) -> String {
        format!(
            "schemaVersion = 1\n\
             minecraft = \"0.0.0-test\"\n\
             javaRelease = 25\n\
             mappingSource = \"{source}\"\n\
             [packFormat]\nresource = 7\nresourceMinor = 0\ndata = 11\ndataMinor = 0\n\
             spelling = \"pack-format\"\n\
             {mappings}\
             [inputs.clientJar]\npath = \"a\"\nsha256 = \"0\"\n\
             [inputs.dataFixerJar]\npath = \"b\"\nsha256 = \"0\"\n\
             [inputs.versionManifest]\npath = \"c\"\nsha256 = \"0\"\n\
             [inputs.hookInventory]\npath = \"d\"\nsha256 = \"0\"\n\
             [inputs.stubSeeds]\npath = \"e\"\nsha256 = \"0\"\n\
             [inputs.reflectiveSurface]\npath = \"h\"\nsha256 = \"0\"\n\
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

    fn reflected(members: &str) -> Reflection {
        toml::from_str(&format!(
            "schemaVersion = 1\nminecraft = \"0.0.0-test\"\n\
             [[reflected]]\nname = \"ALPHA\"\n\
             class = \"com.example.Alpha\"\nruntime = \"a\"\n{members}"
        ))
        .expect("a surface in the shape the version files take")
    }

    #[test]
    fn a_member_name_that_is_not_one_is_refused_before_a_jar_is_opened() {
        verify_surface(&reflected(
            "[[reflected.member]]\nname = \"<init>\"\ndescriptor = \"()V\"\n",
        ))
        .expect("a constructor is a member the agent jar reaches like any other");

        let error = verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick()\"\ndescriptor = \"()V\"\n",
        ))
        .expect_err("a name carrying punctuation is not a member name");

        assert!(error.to_string().contains("tick()"), "{error}");
        assert!(error.to_string().contains("not a member name"), "{error}");
    }

    #[test]
    fn one_member_declared_twice_is_refused_and_two_overloads_are_not() {
        verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick\"\ndescriptor = \"()V\"\n\
             [[reflected.member]]\nname = \"tick\"\ndescriptor = \"(I)V\"\n",
        ))
        .expect("two descriptors are two members");

        let error = verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick\"\nparameters = 1\n\
             [[reflected.member]]\nname = \"tick\"\nparameters = 1\n",
        ))
        .expect_err("one member declared twice says one thing twice");

        assert!(error.to_string().contains("tick/1"), "{error}");
        assert!(error.to_string().contains("twice"), "{error}");
    }
}
