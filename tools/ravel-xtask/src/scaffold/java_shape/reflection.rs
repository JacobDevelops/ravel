//! Declared reflective roles and their reachability from the compiled Java adapter.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use super::{
    AGENT_JAR_SOURCES, AGENT_SOURCE_MANIFEST, REFLECTION_FILE, agent_source_manifest,
    compiled_bootstrap_source, java_sources,
};
use crate::text::{read_text, relative_label};
use crate::version_boundary::{VERSION_DATA, VersionAdapters};

pub(super) const GAME_PACKAGES: [&str; 2] = ["net.minecraft.", "com.mojang."];

pub(super) struct ReflectiveSurface {
    path: String,
    roles: Vec<Reflected>,
    legacy_adapter: bool,
}

pub(super) struct Reflected {
    pub(super) name: String,
    class: String,
    runtime: String,
    pub(super) descriptors: Vec<String>,
    members: Vec<String>,
}

pub(super) fn reflective_surfaces(
    root: &Path,
    adapters: &VersionAdapters,
    violations: &mut Vec<String>,
) -> Vec<ReflectiveSurface> {
    let mut surfaces = Vec::with_capacity(adapters.len());
    for (version, facts) in adapters {
        let path = format!("{VERSION_DATA}/{version}/{REFLECTION_FILE}");
        match read_text(root, Path::new(&path)) {
            Ok(text) => match declared_roles(&text, &path) {
                Ok(roles) => surfaces.push(ReflectiveSurface {
                    path,
                    roles,
                    legacy_adapter: facts.legacy_adapter,
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
    name: Option<String>,
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
            members: role
                .member
                .iter()
                .filter_map(|member| member.name.clone())
                .collect(),
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
    manifest: &[(&str, &str)],
    spelled: &[(String, String)],
    reached: &[(String, String)],
    violations: &mut Vec<String>,
) {
    for surface in surfaces {
        if surface.legacy_adapter {
            verify_legacy_reflection(surface, violations);
        }
        let compiled = |(relative, _): &&(String, String)| {
            compiled_bootstrap_source(Path::new(relative), surface.legacy_adapter, manifest)
        };
        let declaring = &surface.path;
        let declared: Vec<&str> = surface
            .roles
            .iter()
            .map(|role| role.name.as_str())
            .collect();

        for (relative, role) in spelled.iter().filter(compiled) {
            if !declared.contains(&role.as_str()) {
                violations.push(format!(
                    "{relative} resolves the game class {GAME_NAMES}.{role} and {declaring} \
                     declares no such role; a role one version answers and another does not is a \
                     shared source that compiles against one game and not the other"
                ));
            }
        }

        let mut used: Vec<&str> = Vec::new();
        for (relative, constant) in reached.iter().filter(compiled) {
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
            .filter(compiled)
            .map(|(_, role)| role.as_str())
            .chain(used.iter().copied())
            .collect();

        verify_role_reachability(surface, &reached, violations);
    }
}

fn verify_role_reachability(
    surface: &ReflectiveSurface,
    reached: &BTreeSet<&str>,
    violations: &mut Vec<String>,
) {
    // Modern constants directly close over this tree. Legacy names also serve generated
    // screen/persistence templates and harness-only roles, validated by the generator's
    // template/member resolution and tracked-inventory test. Direct bootstrap usage alone
    // cannot establish that a legacy declaration is unused.
    for role in surface.roles.iter().filter(|_| !surface.legacy_adapter) {
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
                "{} declares the role {} for {} and no source under \
                 {AGENT_JAR_SOURCES} resolves it; a declared name nothing uses says the \
                 surface drifted rather than that it is pinned",
                surface.path, role.name, role.class
            ));
        }
    }
}

fn quoted_argument(text: &str) -> Option<&str> {
    text.trim().strip_prefix('"')?.strip_suffix('"')
}

fn verify_legacy_lookups(
    surface: &ReflectiveSurface,
    relative: &str,
    source: &str,
    violations: &mut Vec<String>,
) {
    let anchors = [
        ("", "LegacyNames.className(", 0, false),
        ("", "LegacyNames.member(", 0, true),
        ("", "LegacyAccess.type(", 0, false),
        ("", "LegacyAccess.call(", 0, true),
        ("", "LegacyAccess.field(", 0, true),
        ("", "LegacyAccess.create(", 0, false),
        ("/LegacyCommand.java", "type(", 0, false),
        ("/LegacyCommand.java", "member(", 0, true),
        ("/LegacyPersistence.java", "call(", 1, true),
    ];
    for (file, anchor, index, member_lookup) in anchors {
        if !relative.ends_with(file) {
            continue;
        }
        for (at, _) in source.match_indices(anchor) {
            if !anchor.contains('.')
                && source[..at].chars().next_back().is_some_and(|character| {
                    character == '.' || character.is_ascii_alphanumeric() || character == '_'
                })
            {
                continue;
            }
            let tail = &source[at + anchor.len()..];
            let arguments = tail.split(')').next().unwrap_or(tail);
            let mut arguments = arguments.split(',').skip(index);
            let Some(role) = arguments.next().and_then(quoted_argument) else {
                continue;
            };
            let member = if member_lookup {
                arguments.next().and_then(quoted_argument)
            } else if anchor == "LegacyAccess.create(" {
                Some("<init>")
            } else {
                None
            };
            verify_legacy_lookup(surface, relative, role, member, violations);
        }
    }
}

fn verify_legacy_lookup(
    surface: &ReflectiveSurface,
    relative: &str,
    role: &str,
    member: Option<&str>,
    violations: &mut Vec<String>,
) {
    let Some(declaration) = surface.roles.iter().find(|declared| declared.name == role) else {
        violations.push(format!(
            "{relative} resolves legacy role {role} but {} declares no such role",
            surface.path
        ));
        return;
    };
    if let Some(member) = member
        && !declaration.members.iter().any(|name| name == member)
    {
        violations.push(format!(
            "{relative} resolves legacy member {role}#{member} but {} declares no such member",
            surface.path
        ));
    }
}

fn verify_legacy_reflection(surface: &ReflectiveSurface, violations: &mut Vec<String>) {
    if surface.roles.is_empty() {
        violations.push(format!(
            "{} declares no legacy reflected roles",
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
                "{} has an empty or duplicate legacy reflected role {}",
                surface.path, role.name
            ));
        }
        for descriptor in &role.descriptors {
            if let Err(error) = verify_member_descriptor(descriptor) {
                violations.push(format!(
                    "{} role {} declares invalid descriptor {descriptor}: {error}",
                    surface.path, role.name
                ));
            }
        }
    }
}

/// Reflection descriptors name either a method or exactly one non-void field type.
fn verify_member_descriptor(
    descriptor: &str,
) -> Result<(), ravel_minecraft_generate::GenerateError> {
    ravel_minecraft_generate::descriptor_classes(descriptor).map(drop)
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
    adapters: &VersionAdapters,
    violations: &mut Vec<String>,
) {
    let manifest = match agent_source_manifest(AGENT_SOURCE_MANIFEST) {
        Ok(manifest) => manifest,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let surfaces = reflective_surfaces(root, adapters, violations);

    let sources = java_sources(root, &root.join(AGENT_JAR_SOURCES), violations);
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
        let selected = Path::new(&relative);
        let modern = compiled_bootstrap_source(selected, false, &manifest);
        let legacy = compiled_bootstrap_source(selected, true, &manifest);
        if !surfaces.iter().any(|surface| {
            (surface.legacy_adapter && legacy) || (!surface.legacy_adapter && modern)
        }) {
            continue;
        }
        match read_text(root, path) {
            Ok(source) => {
                for surface in surfaces
                    .iter()
                    .filter(|surface| surface.legacy_adapter && legacy)
                {
                    verify_legacy_lookups(surface, &relative, &source, violations);
                }
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

    verify_persistence_reflection_roles(&surfaces, &manifest, &spelled, &reached, violations);
}
