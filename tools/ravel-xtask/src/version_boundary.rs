//! Version data and common-source boundary checks.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ravel_minecraft_generate::Eligibility;

use crate::inventory::hook_counts;
use crate::java_shape::REFLECTION_FILE;
use crate::tracked::collect_files;

pub(crate) const VERSION_DATA: &str = "minecraft/versions";

pub(crate) const VERSION_FILES: [&str; 7] = [
    "README.md",
    "adapter.toml",
    "hooks.toml",
    "mappings.lock",
    "reflection.toml",
    "stubs.lock",
    "stubs.toml",
];

pub(crate) const VERSION_PROSE_FILE: &str = "README.md";

fn version_directories(root: &Path, directories: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut versions = Vec::new();
    for directory in directories {
        let entries = fs::read_dir(root.join(directory))
            .map_err(|error| format!("cannot read {directory}: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot read {directory} entry: {error}"))?;
            if entry.path().is_dir() {
                versions.push(entry.path());
            }
        }
    }
    versions.sort();
    Ok(versions)
}

pub(crate) fn tracked_versions(root: &Path) -> Result<Vec<String>, String> {
    Ok(version_directories(root, &[VERSION_DATA])?
        .into_iter()
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect())
}

pub(crate) fn adapter_eligibility(
    parsed: &toml::Table,
    version: &str,
) -> Result<Eligibility, String> {
    let eligibility = parsed
        .get("eligibility")
        .map(|value| value.clone().try_into::<Eligibility>())
        .transpose()
        .map_err(|error| format!("Minecraft {version} has invalid eligibility: {error}"))?
        .unwrap_or_default();
    if eligibility == Eligibility::GenerationOnly && version != "1.12.2" {
        return Err(format!(
            "Minecraft {version} must retain live eligibility; generation-only is reserved \
             for the unproven 1.12.2 port"
        ));
    }
    Ok(eligibility)
}

pub(crate) type VersionEligibility = BTreeMap<String, Eligibility>;

/// Reads each tracked adapter once, retaining diagnostics for invalid declarations.
#[cfg(test)]
pub(crate) fn version_eligibility(root: &Path, violations: &mut Vec<String>) -> VersionEligibility {
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return BTreeMap::new();
        }
    };
    versions
        .into_iter()
        .filter_map(|version| {
            match read_tracked_adapter(root, &version)
                .and_then(|parsed| adapter_eligibility(&parsed, &version))
            {
                Ok(eligibility) => Some((version, eligibility)),
                Err(error) => {
                    violations.push(error);
                    None
                }
            }
        })
        .collect()
}

pub(crate) fn read_adapter(root: &Path, version: &str) -> Result<toml::Table, String> {
    if !tracked_versions(root)?
        .iter()
        .any(|tracked| tracked == version)
    {
        return Err(format!(
            "Minecraft {version} has no tracked version directory"
        ));
    }
    read_tracked_adapter(root, version)
}

pub(crate) fn read_tracked_adapter(root: &Path, version: &str) -> Result<toml::Table, String> {
    let path = format!("{VERSION_DATA}/{version}/adapter.toml");
    let text = fs::read_to_string(root.join(&path))
        .map_err(|error| format!("cannot read {path}: {error}"))?;
    let parsed: toml::Table =
        toml::from_str(&text).map_err(|error| format!("cannot parse {path}: {error}"))?;
    if parsed.get("minecraft").and_then(toml::Value::as_str) != Some(version) {
        return Err(format!("{path} does not declare Minecraft {version}"));
    }
    Ok(parsed)
}

/// Reports matrix eligibility from typed manifest data and its actual mapping input.
pub fn version_matrix(root: &Path, version: &str) -> Result<String, String> {
    let parsed = read_adapter(root, version)?;
    let eligibility = adapter_eligibility(&parsed, version)?;
    if let Some(path) = missing_mapping_input(root, &parsed) {
        return Ok(format!("unavailable: {path} is absent"));
    }
    Ok(match eligibility {
        Eligibility::Live => "live",
        Eligibility::GenerationOnly => "generation-only",
    }
    .to_owned())
}

pub(crate) fn missing_mapping_input<'a>(root: &Path, parsed: &'a toml::Table) -> Option<&'a str> {
    ["clientMappings", "mappingIoJar"]
        .into_iter()
        .find_map(|input| {
            parsed
                .get("inputs")
                .and_then(|inputs| inputs.get(input))
                .and_then(|input| input.get("path"))
                .and_then(toml::Value::as_str)
                .filter(|path| !root.join(path).is_file())
        })
}

/// Validates version files and returns eligibility from their already-parsed adapters.
pub(crate) fn verify_version_directories(
    root: &Path,
    violations: &mut Vec<String>,
) -> VersionEligibility {
    let mut eligibility = BTreeMap::new();
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return eligibility;
        }
    };
    if versions.is_empty() {
        violations.push(format!(
            "{VERSION_DATA} holds no version directory, so every check derived from version data \
             sweeps nothing and passes by having nothing to say"
        ));
        return eligibility;
    }

    for version in &versions {
        for file in VERSION_FILES {
            let relative = format!("{VERSION_DATA}/{version}/{file}");
            if !root.join(&relative).is_file() {
                violations.push(format!("missing required file: {relative}"));
                continue;
            }
            if file == VERSION_PROSE_FILE {
                continue;
            }
            let text = match fs::read_to_string(root.join(&relative)) {
                Ok(text) => text,
                Err(error) => {
                    violations.push(format!("cannot read {relative}: {error}"));
                    continue;
                }
            };
            let parsed = match toml::from_str::<toml::Table>(&text) {
                Ok(parsed) => parsed,
                Err(error) => {
                    violations.push(format!("cannot parse {relative} as TOML: {error}"));
                    continue;
                }
            };
            match parsed.get("minecraft").and_then(toml::Value::as_str) {
                Some(declared) if declared == version => {}
                Some(declared) => violations.push(format!(
                    "{relative} declares Minecraft {declared} while its directory is named \
                     {version}; a directory copied from another version and never re-pinned \
                     serves one version's data under another version's name"
                )),
                None => violations.push(format!(
                    "{relative} declares no `minecraft` version, so nothing holds it to the \
                     version {VERSION_DATA}/{version} is named for"
                )),
            }
            if file == "hooks.toml"
                && let Err(error) = hook_counts(&text)
            {
                violations.push(format!("cannot count {relative}: {error}"));
            }
            if file == "adapter.toml" {
                verify_version_directories_adapter(&parsed, &relative, violations);
                if parsed.get("minecraft").and_then(toml::Value::as_str) == Some(version) {
                    match adapter_eligibility(&parsed, version) {
                        Ok(value) => {
                            eligibility.insert(version.clone(), value);
                        }
                        Err(error) => violations.push(format!("{relative}: {error}")),
                    }
                }
            }
        }
    }
    eligibility
}

const RETIRED_HARNESS_TABLE: &str = "harness.notRun";

const RETIRED_HARNESS_ROOT: &str = "harness";

fn missing_pack_format(parsed: &toml::Table) -> Option<String> {
    let Some(scope) = parsed.get("packFormat").and_then(toml::Value::as_table) else {
        return Some(
            "declares no `[packFormat]`, so nothing says which pack formats this version reads \
             and a pack written for it would carry a number from another version"
                .to_owned(),
        );
    };

    let mut absent: Vec<&str> = ["resource", "resourceMinor", "data", "dataMinor"]
        .into_iter()
        .filter(|key| {
            scope
                .get(*key)
                .and_then(toml::Value::as_integer)
                .and_then(|value| u32::try_from(value).ok())
                .is_none()
        })
        .collect();
    if scope
        .get("spelling")
        .and_then(toml::Value::as_str)
        .is_none_or(str::is_empty)
    {
        absent.push("spelling");
    }
    if absent.is_empty() {
        return None;
    }
    Some(format!(
        "declares `[packFormat]` without {}, and a pack format is every one of them because a \
         version states them separately",
        absent.join(" or ")
    ))
}

pub(crate) struct HookInventory {
    pub(crate) version: String,
    pub(crate) path: String,
    pub(crate) text: String,
}

pub(crate) fn hook_inventories(root: &Path) -> Vec<HookInventory> {
    let Ok(versions) = tracked_versions(root) else {
        return Vec::new();
    };
    versions
        .into_iter()
        .filter_map(|version| {
            let path = format!("{VERSION_DATA}/{version}/hooks.toml");
            let text = fs::read_to_string(root.join(&path)).ok()?;
            Some(HookInventory {
                version,
                path,
                text,
            })
        })
        .collect()
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

pub(crate) fn verify_version_boundary(root: &Path, violations: &mut Vec<String>) {
    verify_version_boundary_crate_names(root, violations);

    let Some((paths, versions)) = swept_versions(root, violations) else {
        return;
    };
    let classes = version_class_names(&paths, violations);
    let excuses = VERSION_CLASS_EXCUSES
        .iter()
        .flat_map(|(file, excused, _)| excused.iter().map(move |class| (*file, *class)))
        .collect::<Vec<_>>();
    let mut excuse_matched = vec![false; excuses.len()];

    let mut files = Vec::new();
    collect_files(&root.join("crates"), &mut files);
    for path in files {
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml")
        ) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if versions.iter().any(|version| source.contains(version)) {
            violations.push(format!(
                "hardcoded Minecraft version outside version data ({SWEPT_SCOPE}): {relative}"
            ));
        }

        for class in &classes {
            let dotted = class.replace('/', ".");
            if !source.contains(class.as_str()) && !source.contains(&dotted) {
                continue;
            }
            if let Some(index) = excuses
                .iter()
                .position(|(file, excused)| *file == relative && *excused == class.as_str())
            {
                excuse_matched[index] = true;
                continue;
            }
            violations.push(format!(
                "game class name from version data outside version data ({SWEPT_SCOPE}): \
                 {relative} names {class}"
            ));
        }
    }

    for (index, matched) in excuse_matched.iter().enumerate() {
        if !matched {
            let (file, class) = excuses[index];
            violations.push(format!(
                "stale version-class excuse: {file} no longer names {class}, \
                 so the excuse is unearned and has to go"
            ));
        }
    }
}

fn swept_versions(
    root: &Path,
    violations: &mut Vec<String>,
) -> Option<(Vec<PathBuf>, BTreeSet<String>)> {
    let directories = [VERSION_DATA, "tests/fixtures/public/versions"]
        .into_iter()
        .filter(|directory| root.join(directory).is_dir())
        .collect::<Vec<_>>();
    let paths = version_directories(root, &directories)
        .map_err(|error| violations.push(error))
        .ok()?;
    let versions = paths
        .iter()
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    Some((paths, versions))
}

fn version_class_names(directories: &[PathBuf], violations: &mut Vec<String>) -> Vec<String> {
    let mut classes = BTreeSet::new();
    let mut references = Vec::new();
    for directory in directories {
        for file in ["hooks.toml", "stubs.toml", "mappings.lock", REFLECTION_FILE] {
            let path = directory.join(file);
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    violations.push(format!("cannot read {}: {error}", path.display()));
                    continue;
                }
            };
            match toml::from_str::<toml::Value>(&text) {
                Ok(value) => collect_version_classes(&value, &mut classes, &mut references),
                Err(error) => {
                    violations.push(format!("cannot parse {} as TOML: {error}", path.display()));
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
        toml::Value::String(text) => references.extend(descriptor_classes(text)),
        toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
}

fn descriptor_classes(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    for (start, _) in text.match_indices('L') {
        let name = start + 1;
        let mut end = name;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'/' | b'$' | b'_'))
        {
            end += 1;
        }
        if bytes.get(end) == Some(&b';') && text[name..end].contains('/') {
            found.push(text[name..end].to_owned());
        }
    }
    found
}

#[cfg(test)]
mod tests;

fn verify_version_directories_adapter(
    parsed: &toml::Table,
    relative: &str,
    violations: &mut Vec<String>,
) {
    if let Some(missing) = missing_pack_format(parsed) {
        violations.push(format!("{relative} {missing}"));
    }
    if parsed.contains_key(RETIRED_HARNESS_ROOT) {
        violations.push(format!(
            "{relative} declares a `{RETIRED_HARNESS_ROOT}` table, and \
             `[{RETIRED_HARNESS_TABLE}]` is retired: every live-eligible version runs \
             every harness section, so an absent one is a break rather than \
             something a version may write its way out of. A section a version \
             genuinely cannot run is a work package in STATUS.md and ROADMAP.md, \
             where somebody owns closing it"
        ));
    }
}

fn verify_version_boundary_crate_names(root: &Path, violations: &mut Vec<String>) {
    let crates = root.join("crates");
    if let Ok(entries) = fs::read_dir(&crates) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(suffix) = name.strip_prefix("ravel-minecraft-")
                && !matches!(suffix, "adapter" | "generate")
            {
                violations.push(format!("unexpected per-version Minecraft crate: {name}"));
            }
        }
    }
}
