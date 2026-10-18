//! Checks reviewed inventory, seed, and reflection declarations before jar resolution.
//! These checks validate names and relationships, not serialized shape or file integrity.

use std::collections::BTreeSet;

use super::{GenerateError, Inventory, Lookup, Reflection, Seeds};

/// The seam whose shape decides whether a title-menu row has to be declared.
pub(crate) const TITLE_MENU_SEAM: &str = "screen.title-menu-options";

/// Checks that the title-menu row is declared exactly where it is needed.
///
/// # Errors
///
/// Returns an error if a row is missing, redundant, or declared on another hook.
pub(super) fn verify_title_menu_row(inventory: &Inventory) -> Result<(), GenerateError> {
    for hook in &inventory.hooks {
        if hook.title_menu_row.is_some() && hook.id != TITLE_MENU_SEAM {
            return Err(GenerateError::Drift(format!(
                "{} declares a title-menu row and nothing reads one for it; only {TITLE_MENU_SEAM} \
                 places a row",
                hook.id
            )));
        }
    }
    let Some(seam) = inventory
        .hooks
        .iter()
        .find(|hook| hook.id == TITLE_MENU_SEAM)
    else {
        return Ok(());
    };
    let returns_void = seam.descriptor.ends_with(")V");
    match (returns_void, seam.title_menu_row) {
        (true, None) => Err(GenerateError::Drift(format!(
            "{TITLE_MENU_SEAM} returns void here, so it hands back no row and nothing else says \
             where one is free; declare titleMenuRow, read off this version's title screen"
        ))),
        (false, Some(row)) => Err(GenerateError::Drift(format!(
            "{TITLE_MENU_SEAM} hands back the row it took, so the declared titleMenuRow of {row} \
             is a second answer to a question this version already answers"
        ))),
        _ => Ok(()),
    }
}

/// Checks that a version's declared non-goals say something the hooks do not.
///
/// # Errors
///
/// Returns an error if a non-goal is duplicated, has no reason, or is also a hook.
pub(super) fn verify_non_goals(inventory: &Inventory) -> Result<(), GenerateError> {
    let mut seen = BTreeSet::new();
    for non_goal in &inventory.non_goals {
        if inventory.hooks.iter().any(|hook| hook.id == non_goal.id) {
            return Err(GenerateError::Drift(format!(
                "the inventory declares {} as a hook and as a non-goal, and one id is either \
                 carried or not",
                non_goal.id
            )));
        }
        if non_goal.reason.trim().is_empty() {
            return Err(GenerateError::Drift(format!(
                "the inventory declares {} a non-goal with no reason, which tells a reader who \
                 went looking for it nothing",
                non_goal.id
            )));
        }
        if !seen.insert(non_goal.id.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the inventory declares the non-goal {} twice",
                non_goal.id
            )));
        }
    }
    Ok(())
}

/// Checks that every role a screen template could name is declared exactly once.
///
/// # Errors
///
/// Returns an error if a class, member, or role is invalid or duplicated.
pub(super) fn verify_roles(seeds: &Seeds) -> Result<(), GenerateError> {
    let mut seen = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for seed in &seeds.stubs {
        if !role_reference(&seed.role) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare {} as a class role, which is not a name a template can \
                 write",
                seed.role
            )));
        }
        if !class_name(&seed.class) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare {} as a class, which is not one",
                seed.class
            )));
        }
        if !seen.insert(seed.role.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds declare the class role {} twice, and one role names one class",
                seed.role
            )));
        }
        if !classes.insert(seed.class.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the stub seeds bind two roles to {}, and one class is stubbed once",
                seed.class
            )));
        }
        let mut members = BTreeSet::new();
        let mut bound = BTreeSet::new();
        for member in &seed.members {
            if !member_reference(member.role()) || !member_reference(member.member()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds bind {}'s member role {} to {}, and one of the two is not a \
                     name a template can write",
                    seed.role,
                    member.role(),
                    member.member()
                )));
            }
            if !members.insert(member.role()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds declare {}'s member role {} twice, and one role names one \
                     member",
                    seed.role,
                    member.role()
                )));
            }
            if !bound.insert(member.member()) {
                return Err(GenerateError::Drift(format!(
                    "the stub seeds bind two of {}'s member roles to {}, which stubs it twice",
                    seed.role,
                    member.member()
                )));
            }
        }
    }
    Ok(())
}

/// Whether a value is spelled like a role a screen template can write.
fn role_reference(value: &str) -> bool {
    !value.is_empty()
        && value.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_')
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
}

/// Whether a value is spelled like a member a seed list can bind.
fn member_reference(value: &str) -> bool {
    matches!(value, "<init>" | "<clinit>") || role_reference(value)
}

/// Refuses a surface that cannot be rendered or cannot be checked.
///
/// # Errors
///
/// Returns an error if a name, role, or member declaration is invalid or duplicated.
pub(super) fn verify_surface(surface: &Reflection) -> Result<(), GenerateError> {
    for reflected in &surface.classes {
        for (what, value, legal) in [
            ("role", &reflected.name, role_name(&reflected.name)),
            (
                "runtime name",
                &reflected.runtime,
                class_name(&reflected.runtime),
            ),
            (
                "reviewed name",
                &reflected.class,
                class_name(&reflected.class),
            ),
        ] {
            if !legal {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {value} as a {what}, which is not one"
                )));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for reflected in &surface.classes {
        if !seen.insert(reflected.name.as_str()) {
            return Err(GenerateError::Drift(format!(
                "the reflective surface declares the role {} twice, and one role names one class",
                reflected.name
            )));
        }
    }
    for reflected in &surface.classes {
        let mut declared = BTreeSet::new();
        for member in &reflected.members {
            if !member_name(&member.name) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {} of {} as a member, which is not a member \
                     name",
                    member.name, reflected.class
                )));
            }
            let shape = match &member.lookup {
                Lookup::Descriptor(descriptor) => descriptor.to_owned(),
                Lookup::Parameters(count) => format!("/{count}"),
            };
            if !declared.insert(format!("{}{shape}", member.name)) {
                return Err(GenerateError::Drift(format!(
                    "the reflective surface declares {}{shape} of {} twice",
                    member.name, reflected.class
                )));
            }
        }
    }
    Ok(())
}

/// Whether a value is a member name and nothing else.
fn member_name(value: &str) -> bool {
    matches!(value, "<init>" | "<clinit>")
        || (!value.is_empty()
            && !value.starts_with(|first: char| first.is_ascii_digit())
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_'))
}

/// Whether a role is spelled the way the generated constants are.
fn role_name(value: &str) -> bool {
    value.starts_with(|first: char| first.is_ascii_uppercase())
        && value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Whether a value is a binary class name and nothing else.
fn class_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with(|first: char| first.is_ascii_digit())
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::{Reflection, verify_surface};
    fn reflected(members: &str) -> Reflection {
        toml::from_str(&format!(
            "schemaVersion = 1\nminecraft = \"0.0.0-test\"\n\
             [[reflected]]\nname = \"ALPHA\"\n\
             class = \"com.example.Alpha\"\nruntime = \"a\"\n{members}"
        ))
        .expect("a surface in the shape the version files take")
    }

    #[test]
    fn a_member_name_that_is_not_one_is_refused_before_a_jar_is_opened() {
        verify_surface(&reflected(
            "[[reflected.member]]\nname = \"<init>\"\ndescriptor = \"()V\"\n",
        ))
        .expect("a constructor is a member the agent jar reaches like any other");

        let error = verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick()\"\ndescriptor = \"()V\"\n",
        ))
        .expect_err("a name carrying punctuation is not a member name");

        assert!(error.to_string().contains("tick()"), "{error}");
        assert!(error.to_string().contains("not a member name"), "{error}");
    }

    #[test]
    fn one_member_declared_twice_is_refused_and_two_overloads_are_not() {
        verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick\"\ndescriptor = \"()V\"\n\
             [[reflected.member]]\nname = \"tick\"\ndescriptor = \"(I)V\"\n",
        ))
        .expect("two descriptors are two members");

        let error = verify_surface(&reflected(
            "[[reflected.member]]\nname = \"tick\"\nparameters = 1\n\
             [[reflected.member]]\nname = \"tick\"\nparameters = 1\n",
        ))
        .expect_err("one member declared twice says one thing twice");

        assert!(error.to_string().contains("tick/1"), "{error}");
        assert!(error.to_string().contains("twice"), "{error}");
    }
}
