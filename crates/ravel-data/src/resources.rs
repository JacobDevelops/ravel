use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ModId, RelativePath};

/// A supplied resource path paired with its owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    /// The mod that declared it.
    pub owner: ModId,
    /// The path, exactly as the manifest declared it.
    pub path: RelativePath,
}

impl fmt::Display for Resource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "`{}` from `{}`", self.path.as_str(), self.owner)
    }
}

/// The winning supplier and shadowed owners of a contested resource path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Override {
    /// The path more than one mod declared.
    pub path: RelativePath,
    /// The mod whose copy the game reads.
    pub winner: ModId,
    /// The mods it took the path from, in load order and without repeats.
    pub shadowed: Vec<ModId>,
}

impl fmt::Display for Override {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is supplied by `{}`, which shadows {}",
            self.path.as_str(),
            self.winner,
            self.shadowed.len()
        )
    }
}

/// Resources layered in caller-supplied load order; the last declaration wins.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, RelativePath};
/// use ravel_data::ResourceSet;
///
/// let base: ModId = "base_mod".parse().expect("a legal mod id");
/// let skin: ModId = "skin_mod".parse().expect("a legal mod id");
/// let shared = RelativePath::try_from("resources/lang/en.json".to_owned())
///     .expect("a legal path");
///
/// // `skin_mod` loads later, so its copy is the one the game reads.
/// let set = ResourceSet::layered([(&base, [&shared]), (&skin, [&shared])]);
///
/// assert_eq!(set.supplier(&shared), Some(&skin));
/// assert_eq!(set.overrides().len(), 1);
/// assert_eq!(set.overrides()[0].shadowed, [base]);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceSet {
    declared: Vec<Resource>,
    resolved: BTreeMap<String, ModId>,
    overrides: Vec<Override>,
}

impl ResourceSet {
    /// Layers every mod's declared paths, in the order they are given.
    #[must_use]
    pub fn layered<'a, M, P>(declarations: M) -> Self
    where
        M: IntoIterator<Item = (&'a ModId, P)>,
        P: IntoIterator<Item = &'a RelativePath>,
    {
        let mut set = Self::default();
        for (owner, paths) in declarations {
            for path in paths {
                set.declared.push(Resource {
                    owner: owner.clone(),
                    path: path.clone(),
                });
                set.resolved.insert(path.as_str().to_owned(), owner.clone());
            }
        }
        set.overrides = set.contested();
        set
    }

    /// Builds path-ordered override records, excluding self-shadowing and repeated owners.
    fn contested(&self) -> Vec<Override> {
        let mut by_path: BTreeMap<&str, Vec<&Resource>> = BTreeMap::new();
        for resource in &self.declared {
            by_path
                .entry(resource.path.as_str())
                .or_default()
                .push(resource);
        }

        by_path
            .into_values()
            .filter_map(|declarations| {
                let (winner, earlier) = declarations.split_last()?;
                let mut shadowed: Vec<ModId> = Vec::new();
                for resource in earlier {
                    if resource.owner != winner.owner && !shadowed.contains(&resource.owner) {
                        shadowed.push(resource.owner.clone());
                    }
                }
                (!shadowed.is_empty()).then(|| Override {
                    path: winner.path.clone(),
                    winner: winner.owner.clone(),
                    shadowed,
                })
            })
            .collect()
    }

    /// Returns every declaration, in the order the set was layered.
    #[must_use]
    pub fn declared(&self) -> &[Resource] {
        &self.declared
    }

    /// Returns the mod whose copy of `path` the game reads.
    #[must_use]
    pub fn supplier(&self, path: &RelativePath) -> Option<&ModId> {
        self.resolved.get(path.as_str())
    }

    /// Returns every path more than one mod declared, in path order.
    #[must_use]
    pub fn overrides(&self) -> &[Override] {
        &self.overrides
    }

    /// Returns whether any path is supplied by a mod that took it from another.
    #[must_use]
    pub fn has_overrides(&self) -> bool {
        !self.overrides.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, RelativePath};

    use super::{Resource, ResourceSet};

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    #[test]
    fn a_set_nobody_contests_records_every_declaration_and_no_override() {
        let base = id("base_mod");
        let other = id("other_mod");
        let lang = path("resources/lang/en.json");
        let model = path("resources/models/copper.json");

        let set = ResourceSet::layered([(&base, [&lang]), (&other, [&model])]);

        assert_eq!(set.supplier(&lang), Some(&base));
        assert_eq!(set.supplier(&model), Some(&other));
        assert!(!set.has_overrides());
        assert_eq!(set.overrides(), []);
        assert_eq!(set.declared().len(), 2);
    }

    #[test]
    fn the_last_mod_to_declare_a_path_supplies_it_and_the_others_are_named() {
        let first = id("first_mod");
        let second = id("second_mod");
        let third = id("third_mod");
        let shared = path("resources/lang/en.json");

        let set = ResourceSet::layered([
            (&first, [&shared]),
            (&second, [&shared]),
            (&third, [&shared]),
        ]);

        assert_eq!(set.supplier(&shared), Some(&third));
        assert!(set.has_overrides());
        assert_eq!(set.overrides().len(), 1);

        let taken = &set.overrides()[0];
        assert_eq!(taken.winner, third);
        assert_eq!(taken.shadowed, [first, second]);
        assert_eq!(taken.path, shared);
    }

    #[test]
    fn the_same_set_layered_twice_resolves_every_path_the_same_way() {
        let first = id("first_mod");
        let second = id("second_mod");
        let shared = path("resources/lang/en.json");
        let own = path("resources/models/copper.json");

        let build =
            || ResourceSet::layered([(&first, vec![&shared, &own]), (&second, vec![&shared])]);
        assert_eq!(build(), build());
        assert_eq!(build().supplier(&shared), Some(&second));
        assert_eq!(build().supplier(&own), Some(&first));
    }

    #[test]
    fn a_path_nobody_declared_has_no_supplier() {
        let base = id("base_mod");
        let lang = path("resources/lang/en.json");
        let set = ResourceSet::layered([(&base, [&lang])]);

        assert_eq!(set.supplier(&path("resources/lang/fr.json")), None);
    }

    #[test]
    fn an_empty_set_supplies_nothing_and_contests_nothing() {
        let set = ResourceSet::default();
        assert_eq!(set.declared(), []);
        assert!(!set.has_overrides());
        assert_eq!(set.supplier(&path("resources/lang/en.json")), None);
        assert_eq!(
            set,
            ResourceSet::layered(Vec::<(&ModId, Vec<&RelativePath>)>::new())
        );
    }

    #[test]
    fn a_mod_that_declares_the_same_path_twice_does_not_contest_itself_into_a_report() {
        // A manifest could list a path twice. The mod still supplies it, and a
        // report telling a player that `base_mod` shadowed `base_mod` would name
        // a conflict nobody can act on.
        let base = id("base_mod");
        let repeated = path("resources/lang/en.json");
        let set = ResourceSet::layered([(&base, [&repeated, &repeated])]);

        assert_eq!(set.supplier(&repeated), Some(&base));
        assert_eq!(set.declared().len(), 2);
        assert!(!set.has_overrides());
    }

    #[test]
    fn a_mod_named_twice_around_another_is_shadowed_once_and_only_by_name() {
        // `first_mod` declares the path, `second_mod` takes it, `first_mod`
        // takes it back. The winner is `first_mod`, and it shadows `second_mod`
        // rather than shadowing itself as well.
        let first = id("first_mod");
        let second = id("second_mod");
        let shared = path("resources/lang/en.json");

        let set = ResourceSet::layered([
            (&first, [&shared]),
            (&second, [&shared]),
            (&first, [&shared]),
        ]);

        assert_eq!(set.supplier(&shared), Some(&first));
        assert_eq!(set.overrides().len(), 1);
        assert_eq!(set.overrides()[0].winner, first);
        assert_eq!(set.overrides()[0].shadowed, [second]);
    }

    #[test]
    fn every_recorded_value_says_which_mod_and_which_path_it_is_about() {
        let base = id("base_mod");
        let lang = path("resources/lang/en.json");
        let resource = Resource {
            owner: base.clone(),
            path: lang.clone(),
        };
        assert!(resource.to_string().contains("base_mod"));
        assert!(resource.to_string().contains("lang/en.json"));

        let set = ResourceSet::layered([(&base, [&lang]), (&id("other_mod"), [&lang])]);
        let taken = set.overrides()[0].to_string();
        assert!(taken.contains("other_mod"), "{taken}");
        assert!(taken.contains("lang/en.json"), "{taken}");
    }
}
