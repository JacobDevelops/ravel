//! Declared reflective roles and their reachability from shared Java sources.

use std::collections::BTreeSet;
use std::path::Path;

use ravel_minecraft_generate::Eligibility;
use serde::Deserialize;

use super::{AGENT_JAR_SOURCES, REFLECTION_FILE, java_sources};
use crate::text::{read_text, relative_label};
use crate::version_boundary::{VERSION_DATA, VersionEligibility};

pub(super) const GAME_PACKAGES: [&str; 2] = ["net.minecraft.", "com.mojang."];

pub(super) struct ReflectiveSurface {
    path: String,
    roles: Vec<Reflected>,
    eligibility: Eligibility,
}

pub(super) struct Reflected {
    pub(super) name: String,
    class: String,
    runtime: String,
    pub(super) descriptors: Vec<String>,
}

pub(super) fn reflective_surfaces(
    root: &Path,
    eligibility: &VersionEligibility,
    violations: &mut Vec<String>,
) -> Vec<ReflectiveSurface> {
    let mut surfaces = Vec::with_capacity(eligibility.len());
    for (version, &eligibility) in eligibility {
        let path = format!("{VERSION_DATA}/{version}/{REFLECTION_FILE}");
        match read_text(root, Path::new(&path)) {
            Ok(text) => match declared_roles(&text, &path) {
                Ok(roles) => surfaces.push(ReflectiveSurface {
                    path,
                    roles,
                    eligibility,
                }),
                Err(violation) => violations.push(violation),
            },
            Err(error) => violations.push(error),
        }
    }
    surfaces
}

#[derive(Deserialize)]
struct DeclaredReflection {
    #[serde(default)]
    reflected: Vec<ReflectedRole>,
}

#[derive(Deserialize)]
struct ReflectedRole {
    name: String,
    class: String,
    runtime: String,
    #[serde(default)]
    member: Vec<ReflectedMember>,
}

#[derive(Deserialize)]
struct ReflectedMember {
    descriptor: Option<String>,
}

pub(super) fn declared_roles(text: &str, path: &str) -> Result<Vec<Reflected>, String> {
    let parsed: DeclaredReflection = toml::from_str(text)
        .map_err(|error| format!("cannot parse {path} as reflected TOML: {error}"))?;
    Ok(parsed
        .reflected
        .into_iter()
        .map(|role| Reflected {
            name: role.name,
            class: role.class,
            runtime: role.runtime,
            descriptors: role
                .member
                .into_iter()
                .filter_map(|member| member.descriptor)
                .collect(),
        })
        .collect())
}

pub(super) fn verify_persistence_reflection_roles(
    surfaces: &[ReflectiveSurface],
    spelled: &[(String, String)],
    reached: &[(String, String)],
    violations: &mut Vec<String>,
) {
    for surface in surfaces {
        if surface.eligibility == Eligibility::GenerationOnly {
            verify_generation_reflection(surface, violations);
            continue;
        }
        let declaring = &surface.path;
        let declared: Vec<&str> = surface
            .roles
            .iter()
            .map(|role| role.name.as_str())
            .collect();

        for (relative, role) in spelled {
            if !declared.contains(&role.as_str()) {
                violations.push(format!(
                    "{relative} resolves the game class {GAME_NAMES}.{role} and {declaring} \
                     declares no such role; a role one version answers and another does not is a \
                     shared source that compiles against one game and not the other"
                ));
            }
        }

        let mut used: Vec<&str> = Vec::new();
        for (relative, constant) in reached {
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

fn verify_generation_reflection(surface: &ReflectiveSurface, violations: &mut Vec<String>) {
    if surface.roles.is_empty() {
        violations.push(format!(
            "{} declares no generation-only reflected roles",
            surface.path
        ));
    }
    let mut names = BTreeSet::new();
    for role in &surface.roles {
        if role.name.is_empty()
            || role.class.is_empty()
            || role.runtime.is_empty()
            || !names.insert(role.name.as_str())
        {
            violations.push(format!(
                "{} has an empty or duplicate generation-only reflected role {}",
                surface.path, role.name
            ));
        }
        for descriptor in &role.descriptors {
            if let Err(error) = ravel_minecraft_generate::descriptor_parameters(descriptor) {
                violations.push(format!(
                    "{} role {} declares invalid descriptor {descriptor}: {error}",
                    surface.path, role.name
                ));
            }
        }
    }
}

pub(super) const GAME_NAMES: &str = "GameNames";

pub(super) const GAME_MEMBERS: &str = "GameMembers";

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

pub(super) fn game_class_literals(source: &str) -> Vec<String> {
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

pub(super) fn generated_constants(source: &str, class: &str) -> Vec<String> {
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

pub(crate) fn verify_persistence_reflection(
    root: &Path,
    eligibility: &VersionEligibility,
    violations: &mut Vec<String>,
) {
    let surfaces = reflective_surfaces(root, eligibility, violations);

    let sources = java_sources(&root.join(AGENT_JAR_SOURCES), violations);
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
        let relative = relative_label(root, path);
        match read_text(root, path) {
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
            Err(error) => violations.push(error),
        }
    }

    verify_persistence_reflection_roles(&surfaces, &spelled, &reached, violations);
}
