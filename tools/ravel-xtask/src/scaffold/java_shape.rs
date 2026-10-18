//! Java hooks, injected instruction shapes, and generated templates.

use std::fs;
use std::path::{Path, PathBuf};

use ravel_minecraft_generate::Kind;

use crate::hooks::HookInventory;
use crate::text::{read_text, relative_label};
use crate::tracked::collect_files;
use crate::version_boundary::{VERSION_DATA, VersionAdapters, tracked_versions};

mod reflection;

use reflection::GAME_PACKAGES;
pub(crate) use reflection::verify_persistence_reflection;

const AGENT_JAR_SOURCES: &str = "jvm/bootstrap";

pub(crate) const REFLECTION_FILE: &str = "reflection.toml";

fn java_sources(directory: &Path, violations: &mut Vec<String>) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    if let Err(error) = collect_files(directory, &mut sources) {
        violations.push(error);
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
    if let Err(error) = agent_source_manifest(AGENT_SOURCE_MANIFEST) {
        violations.push(error);
        return;
    }

    for path in java_sources(&root.join(AGENT_JAR_SOURCES), violations) {
        let relative = relative_label(root, &path);
        let selected = Path::new(&relative);
        if !compiled_bootstrap_source(selected, false) && !compiled_bootstrap_source(selected, true)
        {
            continue;
        }
        let source = match read_text(root, &path) {
            Ok(source) => source,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        if compiled_bootstrap_source(selected, false) {
            let modern = source
                .split(HOOK_NAME_MATCH)
                .skip(1)
                .filter_map(|tail| tail.split('"').next());
            verify_hook_literals(&relative, modern, false, inventories, adapters, violations);
        }
        if compiled_bootstrap_source(selected, true) {
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
    literals.sort_unstable();
    literals.dedup();
    literals
}

fn uses_modern_bootstrap(adapters: &VersionAdapters, inventory: &HookInventory) -> bool {
    adapters
        .get(&inventory.version)
        .is_some_and(|facts| !facts.legacy_adapter)
}

const AGENT_SOURCE_MANIFEST: &str = include_str!("../../../../jvm/build/agent-sources.txt");

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
fn compiled_bootstrap_source(path: &Path, legacy: bool) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if parent != Path::new("jvm/bootstrap/dev/ravel/jvm")
        || path.extension().is_none_or(|extension| extension != "java")
    {
        return false;
    }
    let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
        return false;
    };
    AGENT_SOURCE_MANIFEST.lines().any(|line| {
        let mut fields = line.split_whitespace();
        let mode = fields.next();
        fields.next() == Some(name) && (mode == Some("core") || !legacy)
    }) || legacy && name.starts_with("Legacy")
}

pub(crate) const TRANSFORMER_SHAPE_RECORD: &str = "target/ravel-jvm/transformer-shape.txt";

#[derive(Clone, Copy)]
enum Parameter {
    At(usize),
    Last,
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Reference,
    Boolean,
    Int,
    Void,
}

struct Seam {
    id: &'static str,
    receiver: bool,
    loads: &'static [(Parameter, Shape)],
    returns: &'static [Shape],
}

const TRANSFORMER_SEAMS: [Seam; 5] = [
    Seam {
        id: "world.open-persistence",
        receiver: true,
        loads: &[
            (Parameter::At(0), Shape::Reference),
            (Parameter::At(2), Shape::Reference),
            (Parameter::Last, Shape::Boolean),
        ],
        returns: &[Shape::Void],
    },
    Seam {
        id: "world.create-pack-repository",
        receiver: false,
        loads: &[(Parameter::At(0), Shape::Reference)],
        returns: &[],
    },
    Seam {
        id: "screen.create-world-init",
        receiver: true,
        loads: &[],
        returns: &[],
    },
    Seam {
        id: "world.save-persistence",
        receiver: true,
        loads: &[],
        returns: &[],
    },
    Seam {
        id: "screen.title-menu-options",
        receiver: true,
        loads: &[
            (Parameter::At(0), Shape::Int),
            (Parameter::At(1), Shape::Int),
        ],
        returns: &[Shape::Int, Shape::Void],
    },
];

const MENU_ROW_CONSTANT: &str = "MenuLayout.RAVEL_ROW";

const MENU_COLUMN_BRIDGE: &str = "addTitleMenuColumn";

pub(crate) fn verify_menu_row_placement(root: &Path, violations: &mut Vec<String>) {
    let path = root
        .join(AGENT_JAR_SOURCES)
        .join("dev/ravel/jvm/Transformer.java");
    let source = match read_text(root, &path) {
        Ok(source) => source,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(at) = source.find(MENU_COLUMN_BRIDGE) else {
        violations.push(format!(
            "Transformer.java declares no {MENU_COLUMN_BRIDGE}, so the column shape that hands \
             back no row has no bridge and this check has no subject"
        ));
        return;
    };
    let body = &source[at..];
    let end = body.find("\n    }").map_or(body.len(), |end| end + 1);
    let body = &body[..end];
    if !body.contains(MENU_ROW_CONSTANT) {
        violations.push(format!(
            "Transformer.java's {MENU_COLUMN_BRIDGE} does not place from {MENU_ROW_CONSTANT}; a \
             row worked out from the column's own base and spacing is one version's layout \
             asserted over the rest, and it draws Ravel's row on top of the game's own options"
        ));
    }
}

pub(crate) fn verify_transformer_shape(
    root: &Path,
    inventories: &[HookInventory],
    adapters: &VersionAdapters,
    violations: &mut Vec<String>,
) {
    let inventories: Vec<_> = inventories
        .iter()
        .filter(|inventory| uses_modern_bootstrap(adapters, inventory))
        .collect();
    if inventories.is_empty() {
        return;
    }
    let mut held = Vec::new();
    let mut named = 0;
    for inventory in inventories {
        let before = violations.len();
        let mut seams = 0;
        for seam in &TRANSFORMER_SEAMS {
            let Some((kind, descriptor)) = declared_hook(inventory, seam.id) else {
                continue;
            };
            seams += 1;
            named += 1;
            verify_seam_shape(inventory, seam, kind, descriptor, violations);
        }
        if violations.len() == before && seams > 0 {
            held.push(format!(
                "held: Minecraft {} against {seams} transformer seams",
                inventory.version
            ));
        }
    }

    if named == 0 {
        violations.push(format!(
            "no tracked version declares a seam {AGENT_JAR_SOURCES}'s transformer injects into, \
             so its parameter positions and return kinds were checked against nothing"
        ));
    }

    let record = root.join(TRANSFORMER_SHAPE_RECORD);
    let written = record
        .parent()
        .map_or_else(|| Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&record, held.join("\n") + "\n"));
    if let Err(error) = written {
        violations.push(format!(
            "cannot record the verified transformer shapes in \
             {TRANSFORMER_SHAPE_RECORD}: {error}"
        ));
    }
}

fn verify_seam_shape(
    inventory: &HookInventory,
    seam: &Seam,
    kind: Kind,
    descriptor: &str,
    violations: &mut Vec<String>,
) {
    let path = &inventory.path;
    let id = seam.id;
    if seam.receiver && kind != Kind::InstanceMethod {
        violations.push(format!(
            "{path} declares {id} as a {kind} and the transformer loads its receiver; local zero \
             is the first argument on a static method, so the injected call would hand the \
             adapter an argument in place of the object the seam is about",
            kind = kind.as_str()
        ));
    }

    let Ok(parameters) = ravel_minecraft_generate::descriptor_parameters(descriptor) else {
        violations.push(format!(
            "{path} declares {id} at `{descriptor}`, which is not a method descriptor, so the \
             transformer's slots are derived from nothing"
        ));
        return;
    };

    for (position, shape) in seam.loads {
        let (index, declared) = match position {
            Parameter::At(index) => (*index, parameters.get(*index)),
            Parameter::Last => (parameters.len().saturating_sub(1), parameters.last()),
        };
        let Some(declared) = declared else {
            violations.push(format!(
                "{path} declares {id} with {} parameters and the transformer loads parameter \
                 {index}; a load past the declared list is a local the method never received",
                parameters.len()
            ));
            continue;
        };
        if !shape.matches(declared) {
            violations.push(format!(
                "{path} declares parameter {index} of {id} as `{declared}` and the transformer \
                 loads {} there; the load and the local disagree, which the class file verifier \
                 refuses at load time rather than anything here refusing at build time",
                shape.instruction()
            ));
        }
    }

    if !seam.returns.is_empty() {
        let returned = descriptor
            .split_once(')')
            .map_or(descriptor, |(_, returned)| returned);
        if !seam.returns.iter().any(|shape| shape.matches(returned)) {
            violations.push(format!(
                "{path} declares {id} returning `{returned}` and the transformer injects at its \
                 return for {}; a sequence written for another return kind either matches no \
                 return instruction and is silently never applied, or leaves the stack the \
                 verifier refuses",
                seam.returns
                    .iter()
                    .copied()
                    .map(Shape::instruction)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        }
    }
}

fn declared_hook<'a>(inventory: &'a HookInventory, id: &str) -> Option<(Kind, &'a str)> {
    let hook = inventory.hooks.iter().find(|hook| hook.id == id)?;
    Some((hook.kind, &hook.descriptor))
}

const SCREEN_JAR_SOURCES: &str = "jvm/screen/template";

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

impl Shape {
    fn matches(self, declared: &str) -> bool {
        match self {
            Self::Reference => declared.starts_with('L') || declared.starts_with('['),
            Self::Boolean => declared == "Z",
            Self::Int => declared == "I",
            Self::Void => declared == "V",
        }
    }

    fn instruction(self) -> &'static str {
        match self {
            Self::Reference => "a reference",
            Self::Boolean => "a boolean",
            Self::Int => "an int",
            Self::Void => "nothing",
        }
    }
}

#[cfg(test)]
mod tests;
