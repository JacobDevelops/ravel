//! Java hooks, injected instruction shapes, and generated templates.

use std::path::{Path, PathBuf};

use crate::hooks::HookInventory;
use crate::text::{read_text, redacted_path};
use crate::tracked::collect_files;
use crate::version_boundary::VersionAdapters;

mod reflection;
mod screens;
mod transformer;

pub(crate) use reflection::verify_persistence_reflection;
pub(crate) use screens::{verify_screen_overrides, verify_screen_templates};
pub(crate) use transformer::{verify_menu_row_placement, verify_transformer_shape};

const AGENT_JAR_SOURCES: &str = "jvm/bootstrap";
const LEGACY_AGENT_SOURCES: &str = "jvm/legacy-platform/src/agent/java/dev/ravel/jvm";

pub(crate) const TRANSFORMER_SHAPE_RECORD: &str = "target/ravel-jvm/transformer-shape.txt";

pub(crate) const REFLECTION_FILE: &str = "reflection.toml";

fn java_sources(root: &Path, violations: &mut Vec<String>) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for relative in [AGENT_JAR_SOURCES, LEGACY_AGENT_SOURCES] {
        let directory = root.join(relative);
        if relative == LEGACY_AGENT_SOURCES && !directory.exists() {
            continue;
        }
        if let Err(error) = collect_files(root, &directory, &mut sources) {
            violations.push(error);
        }
    }
    sources.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "java")
    });
    sources.sort();
    sources
}

const HOOK_NAME_MATCH: &str = "hookMethod().equals(\"";

pub(crate) fn verify_hook_name_literals(
    root: &Path,
    inventories: &[HookInventory],
    adapters: &VersionAdapters,
    violations: &mut Vec<String>,
) {
    let text = match read_text(root, &root.join(AGENT_SOURCE_MANIFEST)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let manifest = match agent_source_manifest(&text) {
        Ok(manifest) => manifest,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    for path in java_sources(root, violations) {
        let relative = redacted_path(root, &path);
        let selected = Path::new(&relative);
        let modern = compiled_bootstrap_source(selected, false, &manifest);
        let legacy = compiled_bootstrap_source(selected, true, &manifest);
        if !modern && !legacy {
            continue;
        }
        let source = match read_text(root, &path) {
            Ok(source) => source,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        if modern {
            let modern = source
                .split(HOOK_NAME_MATCH)
                .skip(1)
                .filter_map(|tail| tail.split('"').next())
                .filter(|literal| !legacy || !literal.starts_with("legacy"));
            verify_hook_literals(&relative, modern, false, inventories, adapters, violations);
        }
        if legacy {
            let legacy = legacy_hook_literals(&source);
            verify_hook_literals(
                &relative,
                legacy.into_iter(),
                true,
                inventories,
                adapters,
                violations,
            );
        }
    }
}

fn verify_hook_literals<'a>(
    relative: &str,
    literals: impl Iterator<Item = &'a str>,
    legacy: bool,
    inventories: &[HookInventory],
    adapters: &VersionAdapters,
    violations: &mut Vec<String>,
) {
    let matches = |name: &str, literal: &str| {
        name.chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .eq(literal
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .flat_map(char::to_lowercase))
    };
    for literal in literals {
        for inventory in inventories.iter().filter(|inventory| {
            adapters
                .get(&inventory.version)
                .is_some_and(|facts| facts.legacy_adapter == legacy)
        }) {
            if !inventory
                .hooks
                .iter()
                .any(|hook| matches(&hook.id, literal))
            {
                violations.push(format!(
                    "{relative} matches the generated hook {literal} and {} declares no hook \
                     it is rendered from; the transform is silently not applied",
                    inventory.path
                ));
            }
        }
    }
}

fn legacy_hook_literals(source: &str) -> Vec<&str> {
    let mut literals = Vec::new();
    for anchor in ["switch (target.hookMethod())", "switch (id)"] {
        for tail in source.split(anchor).skip(1) {
            let Some((_, body)) = tail.split_once('{') else {
                continue;
            };
            let mut depth = 1;
            let end = body
                .char_indices()
                .find_map(|(index, character)| {
                    match character {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                    (depth == 0).then_some(index)
                })
                .unwrap_or(body.len());
            for case in body[..end].split("case ").skip(1) {
                let labels = case.split("->").next().unwrap_or(case);
                literals.extend(
                    labels
                        .split('"')
                        .enumerate()
                        .filter_map(|(index, literal)| (index % 2 == 1).then_some(literal)),
                );
            }
        }
    }
    for anchor in ["Agent.isInstalled(\"", "id.equals(\""] {
        literals.extend(
            source
                .split(anchor)
                .skip(1)
                .filter_map(|tail| tail.split('"').next()),
        );
    }
    // Shared Transformer code also has direct comparisons for legacy-only
    // constructor handling. Keep those checked against legacy inventories.
    literals.extend(
        source
            .split(HOOK_NAME_MATCH)
            .skip(1)
            .filter_map(|tail| tail.split('"').next())
            .filter(|literal| literal.starts_with("legacy")),
    );
    literals.sort_unstable();
    literals.dedup();
    literals
}

const AGENT_SOURCE_MANIFEST: &str = "jvm/build/agent-sources.txt";

fn agent_source_manifest(text: &str) -> Result<Vec<(&str, &str)>, String> {
    let mut sources = Vec::with_capacity(text.lines().count());
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(group), Some(name), None) = (fields.next(), fields.next(), fields.next()) else {
            return Err(format!("malformed agent source manifest row: {line}"));
        };
        if !matches!(group, "core" | "modern")
            || !name.starts_with(|character: char| character.is_ascii_alphabetic())
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
            || sources.iter().any(|(_, source)| *source == name)
        {
            return Err(format!("malformed agent source manifest row: {line}"));
        }
        sources.push((group, name));
    }
    if !["core", "modern"]
        .iter()
        .all(|group| sources.iter().any(|(mode, _)| mode == group))
    {
        return Err("agent source manifest needs core and modern sources".to_owned());
    }
    Ok(sources)
}

/// Shares build-agent.sh's manifest and its top-level-only legacy source glob.
fn compiled_bootstrap_source(path: &Path, legacy: bool, manifest: &[(&str, &str)]) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if path.extension().is_none_or(|extension| extension != "java") {
        return false;
    }
    let bootstrap = parent == Path::new("jvm/bootstrap/dev/ravel/jvm");
    let legacy_platform = parent == Path::new(LEGACY_AGENT_SOURCES);
    if !(bootstrap || legacy && legacy_platform) {
        return false;
    }
    let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
        return false;
    };
    (bootstrap
        && manifest
            .iter()
            .any(|(group, source)| *source == name && (*group == "core" || !legacy)))
        || legacy && name.starts_with("Legacy")
}

#[cfg(test)]
mod tests;
