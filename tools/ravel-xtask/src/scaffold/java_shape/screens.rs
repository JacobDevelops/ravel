//! Shared screen templates and version-specific API overrides.

use std::fs;
use std::path::{Path, PathBuf};

use super::reflection::GAME_PACKAGES;
use crate::text::{read_text, relative_label};
use crate::tracked::collect_files;
use crate::version_boundary::{VERSION_DATA, tracked_versions};

pub(super) const SCREEN_JAR_SOURCES: &str = "jvm/screen/template";

pub(crate) fn verify_screen_templates(root: &Path, violations: &mut Vec<String>) {
    let directory = root.join(SCREEN_JAR_SOURCES);
    let templates = screen_templates(&directory, violations);
    if templates.is_empty() {
        violations.push(format!(
            "{SCREEN_JAR_SOURCES} holds no screen template, so the screen jar is generated from \
             nothing"
        ));
        return;
    }

    for path in templates {
        let relative = relative_label(root, &path);
        let source = match read_text(root, &path) {
            Ok(source) => source,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        for (index, line) in source.lines().enumerate() {
            for spelled in game_names_outside_placeholders(line) {
                violations.push(format!(
                    "{relative}:{} spells the game name {spelled} itself. A screen template is one \
                     source for every tracked version and a game name written here is one \
                     version's spelling asserted over all of them — the obfuscated name of none. \
                     Name it inside a placeholder, which the generator renders into the name that \
                     version links",
                    index + 1
                ));
            }
        }
    }
}

pub(crate) fn verify_screen_overrides(root: &Path, violations: &mut Vec<String>) {
    let directory = root.join(SCREEN_JAR_SOURCES);
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(format!("cannot read {SCREEN_JAR_SOURCES}: {error}"));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                violations.push(format!("cannot read {SCREEN_JAR_SOURCES} entry: {error}"));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !versions.contains(&name) {
            violations.push(format!(
                "{SCREEN_JAR_SOURCES}/{name} is named for no tracked version, so every template \
                 in it is read by no build and refused by nothing"
            ));
            continue;
        }
        verify_version_overrides(root, &directory, &path, &name, violations);
    }
}

fn verify_version_overrides(
    root: &Path,
    directory: &Path,
    path: &Path,
    name: &str,
    violations: &mut Vec<String>,
) {
    let overrides = match fs::read_dir(path) {
        Ok(overrides) => overrides,
        Err(error) => {
            violations.push(format!("cannot read {SCREEN_JAR_SOURCES}/{name}: {error}"));
            return;
        }
    };
    for over in overrides {
        let over = match over {
            Ok(over) => over,
            Err(error) => {
                violations.push(format!(
                    "cannot read {SCREEN_JAR_SOURCES}/{name} entry: {error}"
                ));
                continue;
            }
        };
        let file = over.file_name().to_string_lossy().into_owned();
        if over.path().is_dir() {
            violations.push(format!(
                "{SCREEN_JAR_SOURCES}/{name}/{file} is a directory, and a version's templates \
                     are read from one level only, so anything inside it is left out silently"
            ));
            continue;
        }
        if !file.ends_with(SCREEN_TEMPLATE_SUFFIX) {
            continue;
        }
        let shared = directory.join(&file);
        if !shared.is_file() {
            violations.push(format!(
                "{SCREEN_JAR_SOURCES}/{name}/{file} overrides no shared template, and the \
                     agent jar opens the same screens under every version"
            ));
            continue;
        }
        let (left, right) = match (read_text(root, &shared), read_text(root, &over.path())) {
            (Ok(left), Ok(right)) => (left, right),
            (left, right) => {
                violations.extend(left.err());
                violations.extend(right.err());
                continue;
            }
        };
        if outside_placeholders(&left) == outside_placeholders(&right) {
            violations.push(format!(
                "{SCREEN_JAR_SOURCES}/{name}/{file} differs from the shared template only \
                     inside its placeholders, which is a rename. Bind the roles in \
                     {VERSION_DATA}/{name}/stubs.toml instead; a per-version source is for an API \
                     that genuinely differs, and this one buys a second copy to maintain"
            ));
        }
    }
}

fn outside_placeholders(source: &str) -> String {
    let mut text = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(index) = rest.find("{{") {
        text.push_str(&rest[..index]);
        text.push('\u{0}');
        let after = &rest[index + 2..];
        match after.find("}}") {
            Some(end) => rest = &after[end + 2..],
            None => return text,
        }
    }
    text.push_str(rest);
    text
}

fn screen_templates(directory: &Path, violations: &mut Vec<String>) -> Vec<PathBuf> {
    let mut templates = Vec::new();
    if let Err(error) = collect_files(directory, &mut templates) {
        violations.push(error);
    }
    templates.retain(|path| {
        path.to_str()
            .is_some_and(|name| name.ends_with(SCREEN_TEMPLATE_SUFFIX))
    });
    templates.sort();
    templates
}

const SCREEN_TEMPLATE_SUFFIX: &str = ".java.in";

fn game_names_outside_placeholders(line: &str) -> Vec<String> {
    let mut spelled = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let (outside, remainder) = match rest.split_once("{{") {
            Some((outside, inside)) => (
                outside,
                inside.split_once("}}").map_or("", |(_, after)| after),
            ),
            None => (rest, ""),
        };
        for package in GAME_PACKAGES {
            let mut search = outside;
            while let Some(index) = search.find(package) {
                let after = &search[index..];
                let end = after
                    .find(|character: char| {
                        !(character.is_alphanumeric() || character == '.' || character == '_')
                    })
                    .unwrap_or(after.len());
                spelled.push(after[..end].trim_end_matches('.').to_owned());
                search = &after[end..];
            }
        }
        rest = remainder;
    }
    spelled
}
