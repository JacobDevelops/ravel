//! Resolves reviewed placeholders against one version's class and member bindings.

use super::roles::Roles;
use super::{Placeholder, Purpose, Template, Version, in_template};
use crate::classfile;
use crate::error::GenerateError;
use crate::mapping::Mapping;
use crate::stubs::Stub;

/// Expands placeholders without changing surrounding source text.
pub(super) fn expand(
    template: &Template<'_>,
    version: &Version<'_>,
    package: Option<&str>,
) -> Result<String, GenerateError> {
    let mut source = String::with_capacity(template.text.len());
    let mut rest = template.text.as_str();
    let mut placeholders = template.placeholders.iter();
    while let Some(index) = rest.find("{{") {
        source.push_str(&rest[..index]);
        let after = &rest[index + 2..];
        let end = after.find("}}").unwrap_or(after.len());
        let placeholder = placeholders.next().ok_or_else(|| {
            GenerateError::Input(format!(
                "{} carries more placeholders than were read from it",
                template.name
            ))
        })?;
        source.push_str(
            &resolve(placeholder, version, package)
                .map_err(|error| in_template(template, error))?,
        );
        rest = &after[end + 2..];
    }
    source.push_str(rest);
    Ok(source)
}

/// What one placeholder renders as.
fn resolve(
    placeholder: &Placeholder,
    version: &Version<'_>,
    package: Option<&str>,
) -> Result<String, GenerateError> {
    let name = |role: &String| {
        version.linked.get(role).cloned().ok_or_else(|| {
            GenerateError::Unresolved(format!("no linked name was resolved for the role {role}"))
        })
    };
    match placeholder {
        // The provenance rides on the package line rather than above it, because
        // a line added at the top of a generated source moves every line below
        // it and the class files would stop being the ones this version built.
        Placeholder::Package => Ok(package.map_or_else(
            || format!(
                "// Generated from the pinned Minecraft {} inputs. Do not edit. {}",
                version.adapter.minecraft,
                match version.purpose {
                    Purpose::Driver => "A command line names this class, so it has no package.",
                    Purpose::Screen | Purpose::StandIn => {
                        "The game is in the unnamed package here, so this is too."
                    }
                }
            ),
            |package| format!(
                "package {package}; // Generated from the pinned Minecraft {} inputs. Do not edit.",
                version.adapter.minecraft
            ),
        )),
        // A class the version links in the unnamed package is imported by
        // nothing: the language refuses the import and the simple name is the
        // whole name.
        Placeholder::Import(class) => {
            let linked = classfile::alias(&name(class)?, version.aliased);
            Ok(match classfile::split(&linked).0 {
                Some(_) => format!("import {};", classfile::source_name(&linked)),
                None => String::new(),
            })
        }
        Placeholder::Class(class) => {
            let linked = name(class)?;
            Ok(classfile::split(&linked).1.to_owned())
        }
        Placeholder::Member(role, member) => {
            member_name(role, member, version.roles, version.stubs, version.mapping)
        }
        Placeholder::Ravel(name) => Ok(version
            .screens
            .map_or_else(|| name.clone(), |package| format!("{package}.{name}"))),
    }
}

/// Reconciles seeded and runtime member roles, rejecting ambiguous linked names.
pub(super) fn member_name(
    role: &str,
    member_role: &str,
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<String, GenerateError> {
    let class = roles.class(role)?;
    let reviewed = classfile::source_name(&class);
    let seeded = seeded_member_name(&class, role, member_role, roles, stubs, mapping)?;
    let declared = roles
        .declared_member(role, member_role)?
        .map(|declared| {
            mapping
                .runtime_member_or_self(&class, &declared.member, &declared.descriptor)
                .map(|linked| linked.0)
        })
        .transpose()?;
    match (seeded, declared) {
        (Some(seeded), Some(declared)) if seeded != declared => {
            Err(GenerateError::Unsupported(format!(
                "{reviewed}#{member_role} links as {seeded} through the stub seeds and as \
                 {declared} through this version's runtime lists, and one member role names one \
                 member"
            )))
        }
        (Some(name), _) | (None, Some(name)) => Ok(name),
        (None, None) => Err(GenerateError::Unresolved(roles.bound_by(role).map_or_else(
            || format!(
                "a template names {role}#{member_role} and none of this version's stubs.toml, \
                 reflection.toml or hooks.toml binds a class to that role"
            ),
            |list| format!(
                "a template names {role}#{member_role}, and this version's {list} binds a class \
                 to that role and no member of it to that member role"
            ),
        ))),
    }
}

/// Resolves a seeded member role, leaving unseeded roles to the runtime declarations.
fn seeded_member_name(
    class: &str,
    role: &str,
    member_role: &str,
    roles: &Roles,
    stubs: &[Stub],
    mapping: &Mapping,
) -> Result<Option<String>, GenerateError> {
    let Some(bound) = roles.seeded_members(role) else {
        return Ok(None);
    };
    let Some(seeded) = bound.iter().find(|seeded| seeded.role() == member_role) else {
        return Ok(None);
    };
    let reviewed = classfile::source_name(class);
    let member = seeded.member();
    let stub = stubs
        .iter()
        .find(|stub| stub.class == class)
        .ok_or_else(|| {
            GenerateError::Unresolved(format!(
                "a template names {role}#{member_role} and {reviewed} is not stubbed"
            ))
        })?;
    // Every overload is stubbed, because the seed list names a member and never
    // a descriptor. Where a version links them under one name that is the whole
    // answer; where it does not, the binding's parameter count is what separates
    // them, and a set it fails to separate is refused below rather than chosen
    // between.
    let mut candidates = Vec::new();
    for declared in stub.members.iter().filter(|held| held.name == member) {
        if let Some(wanted) = seeded.parameters() {
            let (parameters, _) = classfile::signature(&declared.descriptor).map_err(|detail| {
                GenerateError::Unsupported(format!(
                    "{reviewed}#{member} carries a descriptor that does not read: {detail}"
                ))
            })?;
            if !wanted.matches(&parameters) {
                continue;
            }
        }
        candidates.push(declared);
    }
    let mut names = candidates
        .iter()
        .map(|declared| {
            mapping
                .runtime_member_or_self(class, &declared.name, &declared.descriptor)
                .map(|linked| linked.0)
        })
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    names.dedup();
    match names.as_slice() {
        [] => Err(GenerateError::Unresolved(format!(
            "a template names {role}#{member_role}, which this version binds to \
             {reviewed}#{member}, and the stub tree does not carry that member"
        ))),
        [one] => Ok(Some(one.clone())),
        several => Err(GenerateError::Unsupported(format!(
            "{reviewed}#{member} links under {} names ({}), so no single reference names them all",
            several.len(),
            several.join(", ")
        ))),
    }
}

#[cfg(test)]
mod tests;
