//! Reconciles class and member roles from stub seeds, reflection, and hook inventories.

use std::collections::BTreeMap;

use super::inputs::{Inventory, Lookup, Reflection, SeedMember, Seeds};
use super::{GenerateError, classfile};

/// Where one binding of a role was read, and what it bound the role to.
struct Binding {
    /// The tracked file that carries it.
    list: &'static str,
    /// The key that file bound the role under.
    key: &'static str,
    /// The internal class name it binds the role to.
    class: String,
}

/// One member a runtime list declares by name and descriptor.
pub(super) struct Declared {
    /// The tracked file that declares it.
    pub(super) list: &'static str,
    /// The reviewed member name, which is the descriptor's name space too.
    pub(super) member: String,
    /// The reviewed descriptor that picks it out of its overload set.
    pub(super) descriptor: String,
}

/// Every admission list one version tracks, asked as one.
pub(super) struct Roles {
    /// Every binding each role carries, in the order the lists were read.
    bound: BTreeMap<String, Vec<Binding>>,
    /// The members the runtime lists declare, by the role and the member role.
    declared: BTreeMap<(String, String), Vec<Declared>>,
    /// The members the stub seeds bind, by the role that declares them.
    /// Seeded members need the stub tree to resolve overloads; declarations carry descriptors.
    seeded: BTreeMap<String, Vec<SeedMember>>,
}

impl Roles {
    /// Collects bindings, deferring agreement checks until a role is requested.
    pub(super) fn new(seeds: &Seeds, surface: &Reflection, inventory: &Inventory) -> Self {
        let mut bound: BTreeMap<String, Vec<Binding>> = BTreeMap::new();
        let mut declared: BTreeMap<(String, String), Vec<Declared>> = BTreeMap::new();
        let mut seeded = BTreeMap::new();
        for seed in &seeds.stubs {
            bound.entry(seed.role.clone()).or_default().push(Binding {
                list: "stubs.toml",
                key: "stub role",
                class: classfile::internal_name(&seed.class),
            });
            seeded
                .entry(seed.role.clone())
                .or_insert_with(Vec::new)
                .extend(seed.members.iter().cloned());
        }
        for reflected in &surface.classes {
            bound
                .entry(reflected.name.clone())
                .or_default()
                .push(Binding {
                    list: "reflection.toml",
                    key: "reflected name",
                    class: classfile::internal_name(&reflected.class),
                });
            for member in &reflected.members {
                match &member.lookup {
                    // Argument counts do not identify a member declaration.
                    Lookup::Parameters(_) => {}
                    Lookup::Descriptor(descriptor) => {
                        declared
                            .entry((reflected.name.clone(), member.name.clone()))
                            .or_default()
                            .push(Declared {
                                list: "reflection.toml",
                                member: member.name.clone(),
                                descriptor: descriptor.to_owned(),
                            });
                    }
                }
            }
        }
        for hook in &inventory.hooks {
            bound.entry(hook.id.clone()).or_default().push(Binding {
                list: "hooks.toml",
                key: "hook id",
                class: classfile::internal_name(&hook.class),
            });
            declared
                .entry((hook.id.clone(), hook.member.clone()))
                .or_default()
                .push(Declared {
                    list: "hooks.toml",
                    member: hook.member.clone(),
                    descriptor: hook.descriptor.clone(),
                });
        }
        Self {
            bound,
            declared,
            seeded,
        }
    }

    /// The internal class name this version binds a role to.
    ///
    /// # Errors
    ///
    /// Returns an error when no admission list binds the role, and when two
    /// bind it to different classes.
    pub(super) fn class(&self, role: &str) -> Result<String, GenerateError> {
        let Some((first, rest)) = self.bound.get(role).and_then(|found| found.split_first()) else {
            return Err(GenerateError::Unresolved(format!(
                "the role {role} is bound by none of this version's stubs.toml, reflection.toml \
                 or hooks.toml, so nothing says which class it means"
            )));
        };
        if let Some(other) = rest.iter().find(|other| other.class != first.class) {
            return Err(GenerateError::Input(format!(
                "the role {role} is bound to {} by {}'s {} and to {} by {}'s {}, and one role \
                 names one class",
                classfile::source_name(&first.class),
                first.list,
                first.key,
                classfile::source_name(&other.class),
                other.list,
                other.key
            )));
        }
        Ok(first.class.clone())
    }

    /// Every role the stub seeds bind, in a stable order.
    pub(super) fn seeded(&self) -> impl Iterator<Item = (&str, &[SeedMember])> {
        self.seeded
            .iter()
            .map(|(role, members)| (role.as_str(), members.as_slice()))
    }

    /// The members the stub seeds bind under one role.
    ///
    /// Absence differs from a bound class with no matching member.
    pub(super) fn seeded_members(&self, role: &str) -> Option<&[SeedMember]> {
        self.seeded.get(role).map(Vec::as_slice)
    }

    /// Every declaration one member role carries, where a runtime list carries one.
    ///
    /// # Errors
    ///
    /// Returns an error when lists disagree on a member role's declaration.
    pub(super) fn declared_member(
        &self,
        role: &str,
        member_role: &str,
    ) -> Result<Option<&Declared>, GenerateError> {
        let key = (role.to_owned(), member_role.to_owned());
        let Some((first, rest)) = self
            .declared
            .get(&key)
            .and_then(|found| found.split_first())
        else {
            return Ok(None);
        };
        if let Some(other) = rest
            .iter()
            .find(|other| other.member != first.member || other.descriptor != first.descriptor)
        {
            return Err(GenerateError::Input(format!(
                "the member role {role}#{member_role} is declared as {}{} by {} and as {}{} by \
                 {}, and one member role names one member",
                first.member,
                first.descriptor,
                first.list,
                other.member,
                other.descriptor,
                other.list
            )));
        }
        Ok(Some(first))
    }

    /// The tracked file that binds a role's class, where one binds it.
    pub(super) fn bound_by(&self, role: &str) -> Option<&'static str> {
        self.bound
            .get(role)
            .and_then(|found| found.first())
            .map(|binding| binding.list)
    }
}

#[cfg(test)]
mod tests {
    use super::super::inputs::{Hook, Kind, ReflectedMember, Seed};
    use super::*;
    use crate::test_fixtures::reflection as surface;

    fn seeds(bound: &[(&str, &str)]) -> Seeds {
        Seeds {
            schema_version: 1,
            minecraft: String::from("0.0.0"),
            stubs: bound
                .iter()
                .map(|(role, class)| Seed {
                    role: (*role).to_owned(),
                    class: (*class).to_owned(),
                    members: Vec::new(),
                })
                .collect(),
        }
    }

    fn inventory(bound: &[(&str, &str)]) -> Inventory {
        Inventory {
            schema_version: 1,
            minecraft: String::from("0.0.0"),
            hooks: bound
                .iter()
                .map(|(id, class)| Hook {
                    id: (*id).to_owned(),
                    class: (*class).to_owned(),
                    member: String::from("run"),
                    descriptor: String::from("()V"),
                    kind: Kind::InstanceMethod,
                    capability: String::from("test"),
                    side: crate::inputs::HookSide::Shared,
                    title_menu_row: None,
                })
                .collect(),
            non_goals: Vec::new(),
        }
    }

    #[test]
    fn a_role_only_the_stub_seeds_bind_resolves() {
        let (seeds, surface, inventory) = (
            seeds(&[("Panel", "com.example.Panel")]),
            surface(&[]),
            inventory(&[]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        assert_eq!(
            roles.class("Panel").expect("the stub seeds bind it"),
            "com/example/Panel"
        );
    }

    #[test]
    fn a_role_only_the_reflective_surface_binds_resolves() {
        let (seeds, surface, inventory) = (
            seeds(&[]),
            surface(&[("PANEL", "com.example.Panel")]),
            inventory(&[]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        assert_eq!(
            roles.class("PANEL").expect("the surface binds it"),
            "com/example/Panel"
        );
    }

    #[test]
    fn a_role_only_the_hook_inventory_binds_resolves() {
        let (seeds, surface, inventory) = (
            seeds(&[]),
            surface(&[]),
            inventory(&[("panel.draw", "com.example.Panel")]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        assert_eq!(
            roles.class("panel.draw").expect("the inventory binds it"),
            "com/example/Panel"
        );
    }

    #[test]
    fn two_lists_that_agree_resolve() {
        let (seeds, surface, inventory) = (
            seeds(&[("Panel", "com.example.Panel")]),
            surface(&[("Panel", "com.example.Panel")]),
            inventory(&[("Panel", "com.example.Panel")]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        assert_eq!(
            roles.class("Panel").expect("all three agree"),
            "com/example/Panel"
        );
    }

    #[test]
    fn two_lists_that_disagree_are_refused_naming_both() {
        let (seeds, surface, inventory) = (
            seeds(&[("Panel", "com.example.Panel")]),
            surface(&[("Panel", "com.example.Frame")]),
            inventory(&[]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        let error = roles
            .class("Panel")
            .expect_err("two lists bind the role to two classes");
        let message = error.to_string();
        for named in [
            "com.example.Panel",
            "com.example.Frame",
            "stubs.toml",
            "reflection.toml",
        ] {
            assert!(message.contains(named), "{message}");
        }
    }

    #[test]
    fn a_hook_that_disagrees_with_the_seeds_is_refused_naming_both() {
        let (seeds, surface, inventory) = (
            seeds(&[("Panel", "com.example.Panel")]),
            surface(&[]),
            inventory(&[("Panel", "com.example.Frame")]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        let message = roles
            .class("Panel")
            .expect_err("the inventory names another class")
            .to_string();
        for named in ["com.example.Panel", "com.example.Frame", "hooks.toml"] {
            assert!(message.contains(named), "{message}");
        }
    }

    #[test]
    fn a_member_declared_both_ways_is_refused_rather_than_dropped() {
        for lookup in [
            "descriptor = '()V'\nparameters = 0",
            "",
            "descriptor = '()V'\nunknown = true",
        ] {
            let source = format!("name = 'draw'\n{lookup}");
            assert!(
                toml::from_str::<ReflectedMember>(&source).is_err(),
                "{source}"
            );
        }
        for lookup in ["descriptor = '()V'", "parameters = 0"] {
            let source = format!("name = 'draw'\n{lookup}");
            toml::from_str::<ReflectedMember>(&source).expect("one lookup is valid");
        }
    }

    #[test]
    fn an_unknown_role_is_refused() {
        let (seeds, surface, inventory) = (
            seeds(&[("Panel", "com.example.Panel")]),
            surface(&[]),
            inventory(&[]),
        );
        let roles = Roles::new(&seeds, &surface, &inventory);
        let message = roles
            .class("Frame")
            .expect_err("nothing binds the role")
            .to_string();
        assert!(message.contains("Frame"), "{message}");
        assert!(message.contains("stubs.toml"), "{message}");
    }
}
