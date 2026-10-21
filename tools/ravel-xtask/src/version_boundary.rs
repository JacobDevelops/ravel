//! Version data and common-source boundary checks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ravel_minecraft_generate::{AdapterFacts, Eligibility, version_facts};

use crate::hooks::{HookInventory, hook_counts};
use crate::text::{read_text, redacted_path};
use crate::tracked::collect_files;

pub(crate) const VERSION_DATA: &str = "minecraft/versions";

pub(crate) const TOML_VERSION_FILES: [&str; 2] = ["reflection.toml", "stubs.toml"];

pub(crate) const VERSION_PROSE_FILE: &str = "README.md";
const FIXTURE_VERSIONS: &str = "tests/fixtures/public/versions";

fn version_directories(root: &Path, directories: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut versions = Vec::new();
    for directory in directories {
        for entry in crate::text::entries(root, &root.join(directory))? {
            if entry.path().is_dir() {
                versions.push(entry.path());
            }
        }
    }
    versions.sort();
    Ok(versions)
}

pub(crate) fn tracked_versions(root: &Path) -> Result<Vec<String>, String> {
    Ok(directory_names(&version_directories(root, &[VERSION_DATA])?).collect())
}

fn directory_names(paths: &[PathBuf]) -> impl Iterator<Item = String> {
    paths.iter().filter_map(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    })
}

pub(crate) type VersionAdapters = BTreeMap<String, AdapterFacts>;

/// Whether a version can run in the matrix, or which input prevents generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatrixEligibility {
    /// Admission policy declared by the version adapter.
    Eligible(Eligibility),
    /// A declared mapping input is absent.
    Unavailable {
        /// The missing repository-relative input path.
        missing: String,
    },
}

impl std::fmt::Display for MatrixEligibility {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eligible(eligibility) => formatter.write_str(eligibility.as_str()),
            Self::Unavailable { missing } => write!(formatter, "unavailable: {missing} is absent"),
        }
    }
}

/// Reports matrix eligibility from typed manifest data and its actual mapping input.
pub fn matrix_eligibility(root: &Path, version: &str) -> Result<MatrixEligibility, String> {
    if !tracked_versions(root)?
        .iter()
        .any(|tracked| tracked == version)
    {
        return Err(format!(
            "Minecraft {version} has no tracked version directory"
        ));
    }
    let facts = version_facts(root, version)
        .map_err(|error| error.to_string())?
        .adapter;
    if let Some(path) = facts
        .mapping_inputs
        .iter()
        .find(|path| !root.join(path).is_file())
    {
        return Ok(MatrixEligibility::Unavailable {
            missing: path.clone(),
        });
    }
    Ok(MatrixEligibility::Eligible(facts.eligibility))
}

/// Validates version inputs once and retains every admitted hook inventory.
pub(crate) fn load_version_inputs(
    root: &Path,
    violations: &mut Vec<String>,
) -> (VersionAdapters, Vec<HookInventory>) {
    let mut adapters = BTreeMap::new();
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return (adapters, Vec::new());
        }
    };
    if versions.is_empty() {
        violations.push(format!(
            "{VERSION_DATA} holds no version directory, so every check derived from version data \
             sweeps nothing and passes by having nothing to say"
        ));
        return (adapters, Vec::new());
    }

    let mut inventories = Vec::with_capacity(versions.len());
    for version in &versions {
        let prose = format!("{VERSION_DATA}/{version}/{VERSION_PROSE_FILE}");
        if let Err(error) = crate::text::require_file(root, &prose) {
            violations.push(error);
        }
        let adapter = format!("{VERSION_DATA}/{version}/adapter.toml");
        let facts = crate::text::require_file(root, &adapter).and_then(|_| {
            ravel_minecraft_generate::version_facts_at(root, &root.join(VERSION_DATA).join(version))
                .map_err(|error| error.to_string())
        });
        match facts {
            Ok(facts) => {
                for path in facts.toml_inputs {
                    verify_version_toml(root, version, &redacted_path(root, &path), violations);
                }
                adapters.insert(version.to_owned(), facts.adapter);
                match facts.inventory {
                    Ok(inventory) => match hook_counts(&inventory) {
                        Ok(_) => inventories.push(inventory),
                        Err(error) => {
                            violations.push(format!("cannot count {}: {error}", inventory.path));
                        }
                    },
                    Err(error) => violations.push(error.to_string()),
                }
            }
            Err(error) => {
                violations.push(error);
                verify_local_version_toml(root, version, violations);
            }
        }
    }
    (adapters, inventories)
}

fn verify_local_version_toml(root: &Path, version: &str, violations: &mut Vec<String>) {
    let directory = root.join(VERSION_DATA).join(version);
    let mut paths = match crate::text::entries(root, &directory) {
        Ok(entries) => entries
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "toml")
                    && path.file_name().is_some_and(|name| name != "adapter.toml")
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    paths.extend(TOML_VERSION_FILES.iter().map(|name| directory.join(name)));
    paths.sort();
    paths.dedup();
    for path in paths {
        verify_version_toml(root, version, &redacted_path(root, &path), violations);
    }
}

fn verify_version_toml(root: &Path, version: &str, relative: &str, violations: &mut Vec<String>) {
    let path = match crate::text::require_file(root, relative) {
        Ok(path) => path,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let text = match read_text(root, &path) {
        Ok(text) => text,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let parsed = match toml::from_str::<toml::Table>(&text) {
        Ok(parsed) => parsed,
        Err(error) => {
            violations.push(format!("cannot parse {relative} as TOML: {error}"));
            return;
        }
    };
    match parsed.get("minecraft").and_then(toml::Value::as_str) {
        Some(declared) if declared == version => {}
        Some(declared) => {
            violations.push(format!(
                "{relative} declares Minecraft {declared} while its directory is named {version}"
            ));
        }
        None => {
            violations.push(format!(
                "{relative} declares no `minecraft` version for {VERSION_DATA}/{version}"
            ));
        }
    }
}

const SWEPT_SCOPE: &str =
    "the sweep reads crates/ only; tests/ and test-mods/ are excluded by design";

const VERSION_CLASS_EXCUSES: [(&str, &[&str], &str); 3] = [
    (
        "crates/ravel-minecraft-generate/src/lib.rs",
        &[
            "net/minecraft/client/gui/components/events/AbstractContainerEventHandler",
            "net/minecraft/client/gui/screens/Screen",
        ],
        "generates from the tracked pinned inputs and asserts on the stub tree \
         they produce, so it names what they name",
    ),
    (
        "crates/ravel-minecraft-generate/src/resolve.rs",
        &["net/minecraft/util/StringUtil"],
        "resolves the pinned inventory's first hook and asserts the class that \
         inventory names it against",
    ),
    (
        "crates/ravel-minecraft-generate/src/stubs.rs",
        &[
            "net/minecraft/client/gui/components/AbstractSliderButton",
            "net/minecraft/client/gui/components/events/AbstractContainerEventHandler",
            "net/minecraft/client/gui/components/AbstractWidget",
            "net/minecraft/client/gui/components/Button",
            "net/minecraft/client/gui/components/EditBox",
            "net/minecraft/client/gui/components/events/GuiEventListener",
            "net/minecraft/client/gui/screens/Screen",
            "net/minecraft/network/chat/Component",
            "net/minecraft/network/chat/FormattedText",
            "net/minecraft/network/chat/MutableComponent",
        ],
        "asserts which supertype edges the pinned jar's own stub set keeps and \
         which it drops, and an edge is a fact about those classes",
    ),
];

pub(crate) fn verify_version_boundary(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    verify_version_boundary_crate_names(root, violations);

    let Some((paths, versions)) = swept_versions(root, violations) else {
        return;
    };
    let before_metadata = violations.len();
    let classes = version_class_names(root, &paths, inventories, violations);
    let version_data = root.join(VERSION_DATA);
    // Failed canonical admissions are absent from `inventories`; their missing classes
    // must not turn an unmatched excuse into evidence that the excuse is stale.
    let metadata_complete = violations.len() == before_metadata
        && paths.iter().all(|directory| {
            !directory.starts_with(&version_data)
                || directory.file_name().is_some_and(|version| {
                    inventories
                        .iter()
                        .any(|inventory| version == inventory.version.as_str())
                })
        });
    let mut files = Vec::new();
    let walk_complete = collect_files(root, &root.join("crates"), &mut files)
        .map_err(|error| violations.push(error))
        .is_ok();
    verify_boundary_files(
        root,
        &files,
        &versions,
        &classes,
        metadata_complete && walk_complete,
        violations,
    );
}

type ClassExcuses = BTreeMap<&'static str, BTreeMap<&'static str, (&'static str, bool)>>;

fn verify_boundary_files(
    root: &Path,
    files: &[PathBuf],
    versions: &BTreeSet<String>,
    classes: &[String],
    scan_complete: bool,
    violations: &mut Vec<String>,
) {
    let mut excuses: ClassExcuses = VERSION_CLASS_EXCUSES
        .iter()
        .map(|(file, excused, reason)| {
            let classes = excused
                .iter()
                .map(|class| (*class, (*reason, false)))
                .collect();
            (*file, classes)
        })
        .collect();

    let mut unreadable = BTreeSet::new();
    for path in files {
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml")
        ) {
            continue;
        }
        let relative = redacted_path(root, path);
        match read_text(root, path) {
            Ok(source) => verify_boundary_source(
                &relative,
                &source,
                versions,
                classes,
                &mut excuses,
                violations,
            ),
            Err(error) => {
                violations.push(error);
                unreadable.insert(relative);
            }
        }
    }

    if scan_complete {
        verify_class_excuses(&excuses, &unreadable, violations);
    }
}

fn verify_boundary_source(
    relative: &str,
    source: &str,
    versions: &BTreeSet<String>,
    classes: &[String],
    excuses: &mut ClassExcuses,
    violations: &mut Vec<String>,
) {
    if versions.iter().any(|version| source.contains(version)) {
        violations.push(format!(
            "hardcoded Minecraft version outside version data ({SWEPT_SCOPE}): {relative}"
        ));
    }
    for class in classes {
        let dotted = class.replace('/', ".");
        if !source.contains(class.as_str()) && !source.contains(&dotted) {
            continue;
        }
        if let Some((_, matched)) = excuses
            .get_mut(relative)
            .and_then(|excused| excused.get_mut(class.as_str()))
        {
            *matched = true;
            continue;
        }
        violations.push(format!(
            "game class name from version data outside version data ({SWEPT_SCOPE}): \
             {relative} names {class}"
        ));
    }
}

fn verify_class_excuses(
    excuses: &ClassExcuses,
    unreadable: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    for (&file, classes) in excuses {
        if unreadable.contains(file) {
            continue;
        }
        for (&class, &(reason, matched)) in classes {
            if !matched {
                violations.push(format!(
                    "stale version-class excuse: {file} no longer names {class}, \
                     so the excuse is unearned and has to go (reason: {reason})"
                ));
            }
        }
    }
}

fn swept_versions(
    root: &Path,
    violations: &mut Vec<String>,
) -> Option<(Vec<PathBuf>, BTreeSet<String>)> {
    let directories = [VERSION_DATA, FIXTURE_VERSIONS]
        .into_iter()
        .filter(|directory| root.join(directory).is_dir())
        .collect::<Vec<_>>();
    let paths = version_directories(root, &directories)
        .map_err(|error| violations.push(error))
        .ok()?;
    let versions = directory_names(&paths).collect();
    Some((paths, versions))
}

fn version_class_names(
    root: &Path,
    directories: &[PathBuf],
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) -> Vec<String> {
    let mut classes = BTreeSet::new();
    let mut references = Vec::new();
    for inventory in inventories {
        collect_hook_classes(inventory, &mut classes, &mut references, violations);
    }
    let fixture_versions = root.join(FIXTURE_VERSIONS);
    for directory in directories {
        let facts = match ravel_minecraft_generate::version_facts_at(root, directory) {
            Ok(facts) => facts,
            Err(error) => {
                let error = error.to_string();
                if !violations.contains(&error) {
                    violations.push(error);
                }
                continue;
            }
        };
        if directory.starts_with(&fixture_versions) {
            match facts.inventory {
                Ok(inventory) => {
                    collect_hook_classes(&inventory, &mut classes, &mut references, violations);
                }
                Err(error) => violations.push(error.to_string()),
            }
        }
        for path in facts
            .toml_inputs
            .into_iter()
            .enumerate()
            .filter_map(|(index, path)| (index != 1).then_some(path))
        {
            let text = match read_text(root, &path) {
                Ok(text) => text,
                Err(error) => {
                    violations.push(error);
                    continue;
                }
            };
            match toml::from_str::<toml::Value>(&text) {
                Ok(value) => collect_version_classes(&value, &mut classes, &mut references),
                Err(error) => {
                    violations.push(format!(
                        "cannot parse {} as TOML: {error}",
                        redacted_path(root, &path)
                    ));
                }
            }
        }
    }

    let roots = classes
        .iter()
        .filter_map(|class| {
            let mut segments = class.split('/');
            Some(format!("{}/{}/", segments.next()?, segments.next()?))
        })
        .collect::<BTreeSet<_>>();

    for reference in references {
        if roots.iter().any(|prefix| reference.starts_with(prefix)) {
            classes.insert(reference);
        }
    }

    classes.into_iter().collect()
}

fn collect_hook_classes(
    inventory: &HookInventory,
    classes: &mut BTreeSet<String>,
    references: &mut Vec<String>,
    violations: &mut Vec<String>,
) {
    for hook in &inventory.hooks {
        classes.insert(hook.class.replace('.', "/"));
        match ravel_minecraft_generate::descriptor_classes(&hook.descriptor) {
            Ok(names) => references.extend(names.into_iter().map(str::to_owned)),
            Err(error) => violations.push(format!("{}: {error}", hook.class)),
        }
    }
}

fn collect_version_classes(
    value: &toml::Value,
    classes: &mut BTreeSet<String>,
    references: &mut Vec<String>,
) {
    match value {
        toml::Value::Table(table) => {
            if let Some(class) = table.get("class").and_then(toml::Value::as_str) {
                classes.insert(class.replace('.', "/"));
            }
            for value in table.values() {
                collect_version_classes(value, classes, references);
            }
        }
        toml::Value::Array(values) => {
            for value in values {
                collect_version_classes(value, classes, references);
            }
        }
        toml::Value::String(text) => {
            // Most strings in these documents are names, not descriptors.
            if let Ok(names) = ravel_minecraft_generate::descriptor_classes(text) {
                references.extend(names.into_iter().map(str::to_owned));
            }
        }
        toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
}

fn verify_version_boundary_crate_names(root: &Path, violations: &mut Vec<String>) {
    let crates = root.join("crates");
    let entries = match crate::text::entries(root, &crates) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(suffix) = name.strip_prefix("ravel-minecraft-")
            && !matches!(suffix, "adapter" | "generate")
        {
            violations.push(format!("unexpected per-version Minecraft crate: {name}"));
        }
    }
}

pub(crate) fn verify_version_readmes(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    for inventory in inventories {
        let version = &inventory.version;
        let directory = root.join(VERSION_DATA).join(version);
        let non_goals = inventory.non_goals;
        let expected = format!(
            "carries {} reviewed hooks and records {non_goals} non-goals",
            inventory.hooks.len()
        );
        let readme = format!("{VERSION_DATA}/{version}/{VERSION_PROSE_FILE}");
        let prose = match read_text(root, &directory.join(VERSION_PROSE_FILE)) {
            Ok(text) => text,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        if !prose.contains(&expected) {
            violations.push(format!(
                "{readme} does not state \"{expected}\", which is what {} declares",
                inventory.path
            ));
        }
    }
}
#[cfg(test)]
mod tests;
