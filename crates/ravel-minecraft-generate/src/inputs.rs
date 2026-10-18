//! Selects a pinned version and loads its reviewed generator documents in validation order.
//! Schema, declaration semantics, Java manifests, and byte integrity live in child modules.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{GenerateError, SELECTION};

mod admission;
#[cfg(test)]
use crate::test_fixtures as fixtures;
mod integrity;
mod metadata;
mod schema;
mod semantics;

use admission::verify_pinned_sources;
#[cfg(test)]
pub(super) use integrity::replace_pin;
pub(super) use integrity::{file_name, repin_locks, sha256, verify_input};
use metadata::verify_java_release;
pub(super) use semantics::TITLE_MENU_SEAM;
use semantics::{verify_non_goals, verify_roles, verify_title_menu_row};

pub use schema::{Eligibility, Kind};

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

/// Validated adapter facts available without opening jars or generation documents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterFacts {
    /// Runtime admission policy declared by this version.
    pub eligibility: Eligibility,
    /// Mapping documents followed by any required mapping normalizer tool JAR.
    pub mapping_inputs: Vec<String>,
}

/// The reviewed hook facts consumed by repository tooling.
#[derive(Clone, Debug)]
pub struct HookInventory {
    /// Version whose adapter owns this inventory.
    pub version: String,
    /// Resolved inventory location, relative to the repository when inside it.
    pub path: String,
    /// Strictly parsed hook declarations.
    pub hooks: Vec<HookFact>,
    /// Number of explicitly reviewed non-goals.
    pub non_goals: usize,
}

/// A hook's identity and JVM shape, without generator-only resolution details.
#[derive(Clone, Debug)]
pub struct HookFact {
    /// Stable reviewed hook identity.
    pub id: String,
    /// Reviewed declaring class, as spelled by the inventory.
    pub class: String,
    /// Declared target shape.
    pub kind: Kind,
    /// JVM target descriptor.
    pub descriptor: String,
}

/// Reads the strict inventory at the path owned by the selected adapter.
///
/// # Errors
/// Returns an error for unreadable inventories, strict-schema violations,
/// or declarations inconsistent with the adapter. Byte pins are checked at generation.
pub fn hook_inventory(root: &Path, version: &str) -> Result<HookInventory, GenerateError> {
    hook_inventory_at(root, &root.join(VERSIONS).join(version))
}

/// Reads the adapter-owned inventory from a tracked or public-fixture directory.
///
/// Paths under `minecraft/versions/<this version>` follow the supplied adapter
/// directory, so a public fixture remains a drop-in version directory. Other
/// declared inventory paths remain relative to `root`.
///
/// # Errors
///
/// Returns an error for invalid directory names, malformed declarations, or identity drift.
pub fn hook_inventory_at(root: &Path, directory: &Path) -> Result<HookInventory, GenerateError> {
    let version = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            GenerateError::Input("the version directory has no UTF-8 name".to_owned())
        })?;
    let adapter_label = directory
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(version))
        .join("adapter.toml");
    let adapter = read_adapter_at(directory, version)
        .map_err(|error| in_context(error, &adapter_label.to_string_lossy()))?;
    let path = inventory_path(root, directory, &adapter);
    let label = path
        .strip_prefix(root)
        .unwrap_or_else(|_| {
            path.file_name()
                .map_or_else(|| Path::new("inventory"), Path::new)
        })
        .to_string_lossy();
    let inventory = read_inventory(&path, &adapter)
        .map_err(|error| in_context(error, &format!("version {version}, {label}")))?;
    Ok(HookInventory {
        version: adapter.minecraft,
        path: label.into_owned(),
        hooks: inventory
            .hooks
            .into_iter()
            .map(|hook| HookFact {
                id: hook.id,
                class: hook.class,
                kind: hook.kind,
                descriptor: hook.descriptor,
            })
            .collect(),
        non_goals: inventory.non_goals.len(),
    })
}

/// Relocates only an inventory declared inside this adapter's own version tree.
fn inventory_path(root: &Path, directory: &Path, adapter: &Adapter) -> PathBuf {
    let declared = Path::new(&adapter.inputs.hook_inventory.path);
    let canonical = Path::new(VERSIONS).join(&adapter.minecraft);
    declared
        .strip_prefix(canonical)
        .map_or_else(|_| root.join(declared), |suffix| directory.join(suffix))
}

/// Reads the typed adapter and applies the generator's admission policy.
///
/// # Errors
///
/// Returns an error for malformed adapters, identity drift, or invalid admission.
pub fn adapter_facts(root: &Path, version: &str) -> Result<AdapterFacts, GenerateError> {
    let adapter = read_adapter(root, version)
        .and_then(|adapter| {
            verify_pinned_sources(&adapter)?;
            Ok(adapter)
        })
        .map_err(|error| in_context(error, &format!("{VERSIONS}/{version}/adapter.toml")))?;
    let mapping_inputs = adapter
        .inputs
        .client_mappings
        .iter()
        .chain(adapter.inputs.mapping_io_jar.iter())
        .map(|input| input.path.clone())
        .collect();
    Ok(AdapterFacts {
        eligibility: adapter.eligibility,
        mapping_inputs,
    })
}

/// Adds a tracked source label without inventing a location for declared inputs.
fn in_context(error: GenerateError, label: &str) -> GenerateError {
    let context = |detail| format!("{label}: {detail}");
    match error {
        GenerateError::Input(detail) => GenerateError::Input(context(detail)),
        GenerateError::Drift(detail) => GenerateError::Drift(context(detail)),
        GenerateError::Unresolved(detail) => GenerateError::Unresolved(context(detail)),
        GenerateError::Extraction(detail) => GenerateError::Extraction(context(detail)),
        GenerateError::Unsupported(detail) => GenerateError::Unsupported(context(detail)),
    }
}

fn read_adapter(root: &Path, version: &str) -> Result<Adapter, GenerateError> {
    read_adapter_at(&root.join(VERSIONS).join(version), version)
}

/// Shares adapter identity checks between generation and fixture inventory readers.
fn read_adapter_at(directory: &Path, version: &str) -> Result<Adapter, GenerateError> {
    let adapter: Adapter = parse(&directory.join("adapter.toml"))?;
    if adapter.minecraft != version {
        return Err(GenerateError::Drift(format!(
            "the version directory is named {version} but its adapter inputs pin {}",
            adapter.minecraft
        )));
    }
    Ok(adapter)
}

/// Reads the tracked data of the version this build generates against.
pub(super) fn pinned(root: &Path, selected: Option<&str>) -> Result<PinnedVersion, GenerateError> {
    let versions = root.join(VERSIONS);
    let mut names = Vec::new();
    let entries = fs::read_dir(&versions)
        .map_err(|error| GenerateError::Input(format!("cannot read the version data: {error}")))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            GenerateError::Input(format!("cannot read a version data entry: {error}"))
        })?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();

    let name = choose(&names, selected)?;
    let directory = versions.join(name);
    let adapter = read_adapter(root, name)?;
    verify_input(root, &adapter.inputs.hook_inventory)?;
    let inventory = read_inventory(&root.join(&adapter.inputs.hook_inventory.path), &adapter)?;

    verify_input(root, &adapter.inputs.stub_seeds)?;
    let seeds: Seeds = parse(&root.join(&adapter.inputs.stub_seeds.path))?;
    if seeds.minecraft != adapter.minecraft || seeds.schema_version != adapter.schema_version {
        return Err(GenerateError::Drift(format!(
            "the stub seeds pin {} at schema {} but the adapter inputs pin {} at schema {}",
            seeds.minecraft, seeds.schema_version, adapter.minecraft, adapter.schema_version
        )));
    }
    verify_roles(&seeds)?;

    let surface = reflection(root, &adapter)?;

    verify_pinned_sources(&adapter)?;

    verify_java_release(root, &adapter)?;

    Ok(PinnedVersion {
        directory,
        adapter,
        inventory,
        seeds,
        surface,
    })
}

/// Shares inventory identity and declaration checks with repository tooling.
fn read_inventory(path: &Path, adapter: &Adapter) -> Result<Inventory, GenerateError> {
    let inventory: Inventory = parse(path)?;
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
    verify_non_goals(&inventory)?;
    verify_title_menu_row(&inventory)?;
    Ok(inventory)
}

/// Reads the pinned reflection declaration and verifies its adapter identity.
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
    semantics::verify_surface(&surface)?;
    Ok(surface)
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

#[cfg(test)]
/// Every version directory this repository tracks, in name order.
pub(super) fn tracked_versions(root: &Path) -> Vec<String> {
    let mut names = fs::read_dir(root.join(VERSIONS))
        .expect("the version data should be readable")
        .map(|entry| entry.expect("every version data entry should be readable"))
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

    use super::fixtures::{document, legacy_adapter, repository_root};
    use super::{Adapter, Kind, MappingSource, file_name, pinned};

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
                .filter_map(|member| match &member.lookup {
                    super::Lookup::Descriptor(descriptor) => Some(descriptor.as_str()),
                    super::Lookup::Parameters(_) => None,
                })
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
    fn the_tracked_adapter_inputs_pin_every_input_they_declare() {
        let root = repository_root();
        for version in super::tracked_versions(root) {
            let pinned =
                pinned(root, Some(&version)).expect("the tracked version data should parse");
            let inputs = &pinned.adapter.inputs;

            for input in [
                &inputs.client_jar,
                &inputs.version_manifest,
                &inputs.hook_inventory,
                &inputs.stub_seeds,
                &inputs.mappings_lock,
                &inputs.stubs_lock,
            ]
            .into_iter()
            .chain(inputs.client_mappings.as_ref())
            .chain(inputs.data_fixer_jar.as_ref())
            .chain(inputs.mapping_io_jar.as_ref())
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
    fn legacy_capability_is_explicit_and_unknown_or_malformed_values_are_refused() {
        let document = document("unobfuscated-class-files", "");
        for value in [
            "[\"legacy\"]",
            "[\"legacy-1-12-typo\"]",
            "\"legacy-1-12\"",
            "[1]",
        ] {
            let text = format!("capabilities = {value}\n{document}");
            assert!(toml::from_str::<Adapter>(&text).is_err(), "{value}");
        }
        let text = format!("capabilities = [\"legacy-1-12\"]\n{document}");
        let adapter: Adapter = toml::from_str(&text).expect("the named capability parses");
        assert_eq!(adapter.capabilities, [super::Capability::Legacy112]);
        assert!(toml::from_str::<Adapter>(&format!("unknown = true\n{document}")).is_err());
    }

    #[test]
    fn modern_dfu_omission_is_refused_and_legacy_omission_is_accepted() {
        let text = document("unobfuscated-class-files", "")
            .replace("[inputs.dataFixerJar]\npath = \"b\"\nsha256 = \"0\"\n", "");
        let modern: Adapter = toml::from_str(&text).expect("an optional library parses");
        let error = super::verify_pinned_sources(&modern).expect_err("modern inputs require DFU");
        assert!(
            error.to_string().contains("require dataFixerJar"),
            "{error}"
        );
        super::verify_pinned_sources(&legacy_adapter()).expect("legacy declares no DFU");
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
            MappingSource::FeatherTinyMappings,
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

#[cfg(test)]
mod adapter_fact_tests {
    use super::{adapter_facts, fixtures};

    #[test]
    fn facts_read_tracked_adapters_without_loading_generation_inputs() {
        let root = fixtures::repository_root();
        for version in super::tracked_versions(root) {
            let facts = adapter_facts(root, &version).expect("tracked typed adapter");
            let adapter = super::read_adapter(root, &version).expect("tracked adapter");
            assert_eq!(facts.eligibility, adapter.eligibility);
            let mut expected = Vec::new();
            if adapter.mapping_source.needs_mappings() {
                expected.push(
                    adapter
                        .inputs
                        .client_mappings
                        .expect("required mapping")
                        .path,
                );
            }
            if adapter.mapping_source == super::MappingSource::FeatherTinyMappings {
                expected.push(
                    adapter
                        .inputs
                        .mapping_io_jar
                        .expect("required normalizer")
                        .path,
                );
            }
            assert_eq!(facts.mapping_inputs, expected, "{version}");
        }
    }

    #[test]
    fn facts_reject_generation_only_without_the_reviewed_legacy_contract() {
        let root = std::env::temp_dir().join(format!("ravel-adapter-facts-{}", std::process::id()));
        let directory = root.join("minecraft/versions/0.0.0-test");
        std::fs::create_dir_all(&directory).expect("adapter directory");
        let text = fixtures::document("unobfuscated-class-files", "").replacen(
            "schemaVersion = 1",
            "schemaVersion = 1\neligibility = \"generation-only\"",
            1,
        );
        let path = directory.join("adapter.toml");
        std::fs::write(&path, text).expect("generation-only adapter");
        let error = adapter_facts(&root, "0.0.0-test").expect_err("admission rejects shortcut");
        let super::GenerateError::Input(detail) = error else {
            panic!("admission failure must be an input error: {error}");
        };
        assert_eq!(
            detail,
            "minecraft/versions/0.0.0-test/adapter.toml: \
             generation-only eligibility requires legacy-1-12"
        );
        std::fs::remove_dir_all(root).expect("remove adapter fixture");
    }
}

#[cfg(test)]
mod hook_inventory_tests {
    use std::fs;

    use super::{Kind, fixtures, hook_inventory, hook_inventory_at, sha256};

    const INVENTORY: &str = "\
schemaVersion = 1\nminecraft = \"0.0.0-test\"\n\
[[hook]]\nid = \"tick\"\nclass = \"example.Game\"\nmember = \"tick\"\n\
descriptor = \"()V\"\nkind = \"instance-method\"\ncapability = \"tick\"\n\
[[nonGoal]]\nid = \"render\"\nreason = \"not carried\"\n";

    fn write_inventory(root: &std::path::Path, inventory: &str) {
        let adapter = fixtures::document("unobfuscated-class-files", "").replace(
            "[inputs.hookInventory]\npath = \"d\"\nsha256 = \"0\"",
            &format!(
                "[inputs.hookInventory]\npath = \"reviewed.toml\"\nsha256 = \"{}\"",
                sha256(inventory.as_bytes())
            ),
        );
        fs::write(root.join("reviewed.toml"), inventory).expect("inventory");
        fs::write(
            root.join("minecraft/versions/0.0.0-test/adapter.toml"),
            adapter,
        )
        .expect("adapter");
    }

    #[test]
    fn canonical_inventory_paths_follow_relocated_adapters_and_label_the_actual_file() {
        let root =
            std::env::temp_dir().join(format!("ravel-relocated-hooks-{}", std::process::id()));
        let tracked = root.join("minecraft/versions/0.0.0-test");
        let fixture = root.join("tests/fixtures/public/versions/0.0.0-test");
        fs::create_dir_all(&tracked).expect("tracked directory");
        fs::create_dir_all(fixture.join("reviewed")).expect("fixture directory");
        write_inventory(&root, INVENTORY);
        let adapter = fs::read_to_string(tracked.join("adapter.toml")).expect("adapter");
        fs::write(
            fixture.join("adapter.toml"),
            adapter.replace(
                "reviewed.toml",
                "minecraft/versions/0.0.0-test/reviewed/hooks.toml",
            ),
        )
        .expect("relocated adapter");
        let inventory = fixture.join("reviewed/hooks.toml");
        fs::rename(root.join("reviewed.toml"), &inventory).expect("relocated inventory");
        let facts = hook_inventory_at(&root, &fixture).expect("relocated hook facts");
        let actual = "tests/fixtures/public/versions/0.0.0-test/reviewed/hooks.toml";
        assert_eq!(facts.path, actual);
        assert_eq!(facts.hooks[0].class, "example.Game");
        fs::write(
            &inventory,
            INVENTORY.replace("instance-method", "unknown-kind"),
        )
        .expect("invalid relocated inventory");
        let error = hook_inventory_at(&root, &fixture).expect_err("invalid fixture");
        let detail = error.to_string();
        assert!(
            detail.contains(&format!("version 0.0.0-test, {actual}:")),
            "{detail}"
        );
        assert!(
            !detail.contains(&root.to_string_lossy().to_string()),
            "{detail}"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn inventory_follows_adapter_path_and_refuses_schema_and_identity_drift() {
        let root = std::env::temp_dir().join(format!("ravel-hook-facts-{}", std::process::id()));
        fs::create_dir_all(root.join("minecraft/versions/0.0.0-test")).expect("version directory");
        write_inventory(&root, INVENTORY);
        let facts = hook_inventory(&root, "0.0.0-test").expect("typed inventory");
        assert_eq!(facts.version, "0.0.0-test");
        assert_eq!(facts.path, "reviewed.toml");
        assert_eq!(facts.hooks.len(), 1);
        assert_eq!(facts.hooks[0].id, "tick");
        assert_eq!(facts.hooks[0].class, "example.Game");
        assert_eq!(facts.hooks[0].kind, Kind::InstanceMethod);
        assert_eq!(facts.hooks[0].descriptor, "()V");
        assert_eq!(facts.non_goals, 1);
        for (from, to, diagnostic) in [
            ("instance-method", "unknown-kind", "unknown variant"),
            ("capability = \"tick\"", "unknown = true", "unknown field"),
            ("0.0.0-test", "other-version", "the inventory pins"),
            (
                "schemaVersion = 1",
                "schemaVersion = 2",
                "the inventory is schema",
            ),
        ] {
            write_inventory(&root, &INVENTORY.replace(from, to));
            let error = hook_inventory(&root, "0.0.0-test").expect_err("invalid inventory");
            assert!(error.to_string().contains(diagnostic), "{error}");
            let detail = error.to_string();
            assert!(
                detail.contains("version 0.0.0-test, reviewed.toml:"),
                "{detail}"
            );
            assert!(
                !detail.contains("minecraft/versions/0.0.0-test/reviewed.toml"),
                "{detail}"
            );
        }
        write_inventory(&root, INVENTORY);
        let fixture = root.join("tests/fixtures/public/versions/0.0.0-test");
        fs::create_dir_all(&fixture).expect("fixture directory");
        fs::rename(
            root.join("minecraft/versions/0.0.0-test/adapter.toml"),
            fixture.join("adapter.toml"),
        )
        .expect("relocated adapter");
        let fixture_facts = hook_inventory_at(&root, &fixture).expect("fixture inventory");
        assert_eq!(fixture_facts.path, "reviewed.toml");
        assert_eq!(fixture_facts.hooks[0].class, "example.Game");
        fs::remove_dir_all(root).expect("cleanup");
    }
}
