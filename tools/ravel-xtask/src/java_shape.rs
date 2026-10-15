//! Java hooks, injected instruction shapes, and generated templates.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::inventory::hook_counts;
use crate::tracked::collect_files;
use crate::version_boundary::{HookInventory, VERSION_DATA, VERSION_PROSE_FILE, tracked_versions};

const AGENT_JAR_SOURCES: &str = "jvm/bootstrap";

const GAME_PACKAGES: [&str; 2] = ["net.minecraft.", "com.mojang."];

pub(crate) const REFLECTION_FILE: &str = "reflection.toml";

struct ReflectiveSurface {
    path: String,
    roles: Vec<Reflected>,
}

struct Reflected {
    name: String,
    class: String,
    descriptors: Vec<String>,
}

fn reflective_surfaces(root: &Path, violations: &mut Vec<String>) -> Vec<ReflectiveSurface> {
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return Vec::new();
        }
    };

    let mut surfaces = Vec::new();
    for version in versions {
        let path = format!("{VERSION_DATA}/{version}/{REFLECTION_FILE}");
        match fs::read_to_string(root.join(&path)) {
            Ok(text) => match declared_roles(&text, &path) {
                Ok(roles) => surfaces.push(ReflectiveSurface { path, roles }),
                Err(violation) => violations.push(violation),
            },
            Err(error) => violations.push(format!("cannot read {path}: {error}")),
        }
    }
    surfaces
}

fn declared_roles(text: &str, path: &str) -> Result<Vec<Reflected>, String> {
    let mut roles: Vec<Reflected> = Vec::new();
    let mut name = None;
    let mut class = None;
    let mut runtime = None;
    let mut descriptors: Vec<String> = Vec::new();
    let finish = |name: &mut Option<String>,
                  class: &mut Option<String>,
                  runtime: &mut Option<String>,
                  descriptors: &mut Vec<String>,
                  roles: &mut Vec<Reflected>|
     -> Result<(), String> {
        let carried = std::mem::take(descriptors);
        match (name.take(), class.take(), runtime.take()) {
            (None, None, None) => Ok(()),
            (Some(name), Some(class), Some(_runtime)) => {
                roles.push(Reflected {
                    name,
                    class,
                    descriptors: carried,
                });
                Ok(())
            }
            (name, class, _) => Err(format!(
                "{path} declares a reflected table without all three of name, class and runtime \
                 ({}); a role that resolves to nothing is a name held to nothing",
                name.or(class).unwrap_or_else(|| "unnamed".to_owned())
            )),
        }
    };

    let mut member = false;
    for line in text.lines().map(str::trim) {
        if line == "[[reflected]]" {
            finish(
                &mut name,
                &mut class,
                &mut runtime,
                &mut descriptors,
                &mut roles,
            )?;
            member = false;
            continue;
        }
        if line.starts_with('[') {
            member = true;
            continue;
        }
        if member {
            if let Some(value) = quoted_value(line, "descriptor") {
                descriptors.push(value.to_owned());
            }
            continue;
        }
        if let Some(value) = quoted_value(line, "name") {
            name = Some(value.to_owned());
        } else if let Some(value) = quoted_value(line, "class") {
            class = Some(value.to_owned());
        } else if let Some(value) = quoted_value(line, "runtime") {
            runtime = Some(value.to_owned());
        }
    }
    finish(
        &mut name,
        &mut class,
        &mut runtime,
        &mut descriptors,
        &mut roles,
    )?;
    Ok(roles)
}

pub(crate) fn verify_persistence_reflection(root: &Path, violations: &mut Vec<String>) {
    let surfaces = reflective_surfaces(root, violations);

    let sources = java_sources(&root.join(AGENT_JAR_SOURCES));
    if sources.is_empty() {
        violations.push(format!(
            "{AGENT_JAR_SOURCES} holds no Java source, so no game class the agent jar resolves is \
             held to a declared set"
        ));
        return;
    }

    let mut spelled: Vec<(String, String)> = Vec::new();
    let mut reached: Vec<(String, String)> = Vec::new();
    for path in &sources {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        match fs::read_to_string(path) {
            Ok(source) => {
                for name in game_class_literals(&source) {
                    violations.push(format!(
                        "{relative} spells the game class {name} as a literal; \
                         {AGENT_JAR_SOURCES} is one source tree for every tracked version, so a \
                         game name written here is one version's spelling asserted over all of \
                         them and is the obfuscated name of none. Reach it as {GAME_NAMES}.<role> \
                         and declare the role in each version's {REFLECTION_FILE}"
                    ));
                }
                spelled.extend(
                    generated_constants(&source, GAME_NAMES)
                        .into_iter()
                        .map(|role| (relative.clone(), role)),
                );
                reached.extend(
                    generated_constants(&source, GAME_MEMBERS)
                        .into_iter()
                        .map(|member| (relative.clone(), member)),
                );
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }

    for surface in &surfaces {
        let declaring = &surface.path;
        let declared: Vec<&str> = surface
            .roles
            .iter()
            .map(|role| role.name.as_str())
            .collect();

        for (relative, role) in &spelled {
            if !declared.contains(&role.as_str()) {
                violations.push(format!(
                    "{relative} resolves the game class {GAME_NAMES}.{role} and {declaring} \
                     declares no such role; a role one version answers and another does not is a \
                     shared source that compiles against one game and not the other"
                ));
            }
        }

        let mut used: Vec<&str> = Vec::new();
        for (relative, constant) in &reached {
            match declaring_role(constant, &declared) {
                Some(role) => used.push(role),
                None => violations.push(format!(
                    "{relative} resolves the game member {GAME_MEMBERS}.{constant} and {declaring} \
                     declares no role it could belong to; a member one version answers and another \
                     does not is a shared source that compiles against one game and not the other"
                )),
            }
        }

        let reached: BTreeSet<&str> = spelled
            .iter()
            .map(|(_, role)| role.as_str())
            .chain(used.iter().copied())
            .collect();

        for role in &surface.roles {
            let inside = format!("L{};", role.class.replace('.', "/"));
            let passed_to_a_reached_lookup = surface.roles.iter().any(|reaching| {
                reached.contains(reaching.name.as_str())
                    && reaching
                        .descriptors
                        .iter()
                        .any(|descriptor| descriptor.contains(&inside))
            });
            if !reached.contains(role.name.as_str()) && !passed_to_a_reached_lookup {
                violations.push(format!(
                    "{declaring} declares the role {} for {} and no source under \
                     {AGENT_JAR_SOURCES} resolves it; a declared name nothing uses says the \
                     surface drifted rather than that it is pinned",
                    role.name, role.class
                ));
            }
        }
    }
}

fn java_sources(directory: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_files(directory, &mut sources);
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
    violations: &mut Vec<String>,
) {
    let flattened = |name: &str| -> String {
        name.chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .collect()
    };

    for path in java_sources(&root.join(AGENT_JAR_SOURCES)) {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let mut rest = source.as_str();
        while let Some(at) = rest.find(HOOK_NAME_MATCH) {
            rest = &rest[at + HOOK_NAME_MATCH.len()..];
            let Some((literal, tail)) = rest.split_once('"') else {
                break;
            };
            rest = tail;
            for inventory in inventories {
                if !inventory
                    .text
                    .lines()
                    .filter_map(|line| quoted_value(line, "id"))
                    .any(|id| flattened(id) == flattened(literal))
                {
                    violations.push(format!(
                        "{relative} matches the generated hook {literal} and {} declares no hook \
                         it is rendered from; the hook names are per-version data, so a seam one \
                         version declares under another id renders another name here, every \
                         comparison is false, and the transform is silently not applied",
                        inventory.path
                    ));
                }
            }
        }
    }
}

const TRANSFORMER_SHAPE_RECORD: &str = "target/ravel-jvm/transformer-shape.txt";

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
    let Ok(source) = fs::read_to_string(&path) else {
        violations.push(format!(
            "{AGENT_JAR_SOURCES}/dev/ravel/jvm/Transformer.java cannot be read, so nothing holds \
             the title-menu bridge to the row its version declares"
        ));
        return;
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
    violations: &mut Vec<String>,
) {
    let mut held = Vec::new();
    let mut named = 0;
    for inventory in inventories {
        let before = violations.len();
        let mut seams = 0;
        for seam in &TRANSFORMER_SEAMS {
            let Some((kind, descriptor)) = declared_hook(&inventory.text, seam.id) else {
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
    kind: &str,
    descriptor: &str,
    violations: &mut Vec<String>,
) {
    let path = &inventory.path;
    let id = seam.id;
    if seam.receiver && kind != "instance-method" {
        violations.push(format!(
            "{path} declares {id} as a {kind} and the transformer loads its receiver; local zero \
             is the first argument on a static method, so the injected call would hand the \
             adapter an argument in place of the object the seam is about"
        ));
    }

    let Some(parameters) = descriptor_parameters(descriptor) else {
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

fn declared_hook<'a>(inventory: &'a str, id: &str) -> Option<(&'a str, &'a str)> {
    for block in inventory.split("[[hook]]").skip(1) {
        let body = block
            .split("\n[")
            .next()
            .expect("splitting a string yields at least one part");
        let mut declared = None;
        let mut kind = None;
        let mut descriptor = None;
        for line in body.lines().map(str::trim) {
            if let Some(value) = quoted_value(line, "id") {
                declared = Some(value);
            } else if let Some(value) = quoted_value(line, "kind") {
                kind = Some(value);
            } else if let Some(value) = quoted_value(line, "descriptor") {
                descriptor = Some(value);
            }
        }
        if declared == Some(id) {
            return Some((kind?, descriptor?));
        }
    }
    None
}

const SCREEN_JAR_SOURCES: &str = "jvm/screen/template";

pub(crate) fn verify_screen_templates(root: &Path, violations: &mut Vec<String>) {
    let directory = root.join(SCREEN_JAR_SOURCES);
    let templates = screen_templates(&directory);
    if templates.is_empty() {
        violations.push(format!(
            "{SCREEN_JAR_SOURCES} holds no screen template, so the screen jar is generated from \
             nothing"
        ));
        return;
    }

    for path in templates {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(source) = fs::read_to_string(&path) else {
            violations.push(format!("cannot read {relative}"));
            continue;
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

pub(crate) fn verify_version_readmes(root: &Path, violations: &mut Vec<String>) {
    let Ok(versions) = tracked_versions(root) else {
        return;
    };
    for version in versions {
        let directory = root.join(VERSION_DATA).join(&version);
        let Ok(inventory) = fs::read_to_string(directory.join("hooks.toml")) else {
            continue;
        };
        let Ok(counts) = hook_counts(&inventory) else {
            continue;
        };
        let non_goals = inventory.matches("[[nonGoal]]").count();
        let expected = format!(
            "carries {} reviewed hooks and records {non_goals} non-goals",
            counts.entries
        );
        let readme = format!("{VERSION_DATA}/{version}/README.md");
        let Ok(prose) = fs::read_to_string(directory.join(VERSION_PROSE_FILE)) else {
            violations.push(format!("cannot read {readme}"));
            continue;
        };
        if !prose.contains(&expected) {
            violations.push(format!(
                "{readme} does not state \"{expected}\", which is what \
                 {VERSION_DATA}/{version}/hooks.toml declares"
            ));
        }
    }
}

pub(crate) fn verify_screen_overrides(root: &Path, violations: &mut Vec<String>) {
    let directory = root.join(SCREEN_JAR_SOURCES);
    let Ok(versions) = tracked_versions(root) else {
        violations.push(format!("cannot read {VERSION_DATA}"));
        return;
    };
    let Ok(entries) = fs::read_dir(&directory) else {
        return;
    };
    for entry in entries.flatten() {
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
        let Ok(overrides) = fs::read_dir(&path) else {
            violations.push(format!("cannot read {SCREEN_JAR_SOURCES}/{name}"));
            continue;
        };
        for over in overrides.flatten() {
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
            let (Ok(left), Ok(right)) =
                (fs::read_to_string(&shared), fs::read_to_string(over.path()))
            else {
                violations.push(format!("cannot read {SCREEN_JAR_SOURCES}/{name}/{file}"));
                continue;
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

fn screen_templates(directory: &Path) -> Vec<PathBuf> {
    let mut templates = Vec::new();
    collect_files(directory, &mut templates);
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

const GAME_NAMES: &str = "GameNames";

const GAME_MEMBERS: &str = "GameMembers";

fn declaring_role<'a>(constant: &str, declared: &[&'a str]) -> Option<&'a str> {
    declared
        .iter()
        .filter(|role| {
            constant
                .strip_prefix(**role)
                .is_some_and(|rest| rest.starts_with('_'))
        })
        .max_by_key(|role| role.len())
        .copied()
}

fn game_class_literals(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*"))
        })
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .map(|literal| literal.replace('/', "."))
        .filter(|literal| {
            GAME_PACKAGES
                .iter()
                .any(|package| literal.starts_with(package))
        })
        .collect()
}

pub(crate) fn quoted_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(name)?.trim_start();
    rest.strip_prefix('=')?
        .trim_start()
        .strip_prefix('"')?
        .strip_suffix('"')
}

pub(crate) fn descriptor_handles(descriptor: &str) -> Option<usize> {
    Some(
        descriptor_parameters(descriptor)?
            .iter()
            .filter(|parameter| {
                parameter.starts_with('[')
                    || (parameter.starts_with('L') && **parameter != "Ljava/lang/String;")
            })
            .count(),
    )
}

fn descriptor_parameters(descriptor: &str) -> Option<Vec<&str>> {
    let mut rest = descriptor.strip_prefix('(')?.split_once(')')?.0;
    let mut parameters = Vec::new();
    while !rest.is_empty() {
        let arrays = rest.len() - rest.trim_start_matches('[').len();
        let named = &rest[arrays..];
        let length = if let Some(class) = named.strip_prefix('L') {
            class.split_once(';')?.0.len() + 2
        } else {
            if !matches!(
                named.as_bytes().first()?,
                b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z'
            ) {
                return None;
            }
            1
        };
        let (parameter, tail) = rest.split_at(arrays + length);
        parameters.push(parameter);
        rest = tail;
    }
    Some(parameters)
}

fn generated_constants(source: &str, class: &str) -> Vec<String> {
    let mut roles = Vec::new();
    let mut rest = source;
    let anchor = format!("{class}.");
    while let Some(index) = rest.find(&anchor) {
        let after = &rest[index + anchor.len()..];
        let end = after
            .find(|character: char| !character.is_ascii_uppercase() && character != '_')
            .unwrap_or(after.len());
        if end > 0 {
            roles.push(after[..end].to_owned());
        }
        rest = &after[end..];
    }
    roles.sort();
    roles.dedup();
    roles
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
