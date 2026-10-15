//! Version data and common-source boundary checks.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::inventory::hook_counts;
use crate::java_shape::{REFLECTION_FILE, quoted_value};
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

fn declared_version(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find_map(|line| quoted_value(line, "minecraft"))
}

pub(crate) fn tracked_versions(root: &Path) -> Result<Vec<String>, String> {
    let entries = fs::read_dir(root.join(VERSION_DATA))
        .map_err(|error| format!("cannot read {VERSION_DATA}: {error}"))?;
    let mut versions = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    versions.sort();
    Ok(versions)
}

pub(crate) fn verify_version_directories(root: &Path, violations: &mut Vec<String>) {
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    if versions.is_empty() {
        violations.push(format!(
            "{VERSION_DATA} holds no version directory, so every check derived from version data \
             sweeps nothing and passes by having nothing to say"
        ));
        return;
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
            match declared_version(&text) {
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
                if let Some(missing) = missing_pack_format(&text) {
                    violations.push(format!("{relative} {missing}"));
                }
                match toml::from_str::<toml::Table>(&text) {
                    Ok(parsed) if parsed.contains_key(RETIRED_HARNESS_ROOT) => {
                        violations.push(format!(
                            "{relative} declares a `{RETIRED_HARNESS_ROOT}` table, and \
                             `[{RETIRED_HARNESS_TABLE}]` is retired: every tracked version runs \
                             every harness section, so an absent one is a break rather than \
                             something a version may write its way out of. A section a version \
                             genuinely cannot run is a work package in STATUS.md and ROADMAP.md, \
                             where somebody owns closing it"
                        ));
                    }
                    Ok(_) => {}
                    Err(error) => violations.push(format!(
                        "cannot parse {relative} as TOML: {error}; a file nothing can read is \
                         held to none of the requirements asked of it here"
                    )),
                }
            }
        }
    }
}

const RETIRED_HARNESS_TABLE: &str = "harness.notRun";

const RETIRED_HARNESS_ROOT: &str = "harness";

fn missing_pack_format(text: &str) -> Option<String> {
    let Some(scope) = table_scope(text, "packFormat") else {
        return Some(
            "declares no `[packFormat]`, so nothing says which pack formats this version reads \
             and a pack written for it would carry a number from another version"
                .to_owned(),
        );
    };

    let mut absent: Vec<&str> = ["resource", "resourceMinor", "data", "dataMinor"]
        .into_iter()
        .filter(|key| scoped_number(&scope, key).is_none())
        .collect();
    if scoped_text(&scope, "spelling").is_none() {
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

fn table_scope(text: &str, name: &str) -> Option<String> {
    let header = format!("[{name}]");
    let mut lines = text.lines();
    lines.find(|line| line.trim_end() == header)?;
    Some(
        lines
            .take_while(|line| !line.trim_start().starts_with('['))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn scoped_text(scope: &str, key: &str) -> Option<String> {
    scope
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| name.trim() == key)
        .map(|(_, value)| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
}

fn scoped_number(scope: &str, key: &str) -> Option<u32> {
    scope
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| name.trim() == key)
        .and_then(|(_, value)| value.trim().parse().ok())
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
        "crates/ravel-minecraft-adapter/src/generate/mod.rs",
        &[
            "net/minecraft/client/gui/components/events/AbstractContainerEventHandler",
            "net/minecraft/client/gui/screens/Screen",
        ],
        "generates from the tracked pinned inputs and asserts on the stub tree \
         they produce, so it names what they name",
    ),
    (
        "crates/ravel-minecraft-adapter/src/generate/resolve.rs",
        &["net/minecraft/util/StringUtil"],
        "resolves the pinned inventory's first hook and asserts the class that \
         inventory names it against",
    ),
    (
        "crates/ravel-minecraft-adapter/src/generate/stubs.rs",
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
    let crates = root.join("crates");
    if let Ok(entries) = fs::read_dir(&crates) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(suffix) = name.strip_prefix("ravel-minecraft-")
                && suffix != "adapter"
            {
                violations.push(format!("unexpected per-version Minecraft crate: {name}"));
            }
        }
    }

    let versions = ["minecraft/versions", "tests/fixtures/public/versions"]
        .into_iter()
        .flat_map(|directory| fs::read_dir(root.join(directory)).into_iter().flatten())
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let classes = version_class_names(root);
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

fn version_class_names(root: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    for directory in ["minecraft/versions", "tests/fixtures/public/versions"] {
        let Ok(entries) = fs::read_dir(root.join(directory)) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            for file in ["hooks.toml", "stubs.toml", "mappings.lock", REFLECTION_FILE] {
                if let Ok(text) = fs::read_to_string(entry.path().join(file)) {
                    texts.push(text);
                }
            }
        }
    }

    let mut classes = BTreeSet::new();
    for text in &texts {
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("class = \"")
                && let Some(value) = value.strip_suffix('"')
            {
                classes.insert(value.replace('.', "/"));
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

    for text in &texts {
        for reference in descriptor_classes(text) {
            if roots.iter().any(|prefix| reference.starts_with(prefix)) {
                classes.insert(reference);
            }
        }
    }

    classes.into_iter().collect()
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
