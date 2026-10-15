use std::collections::{BTreeMap, BTreeSet};

use ravel_core::{DependencyRelationship, ModId, Side, Version};
use ravel_package::{Dependency, DependencyOrder, PackageManifest};

use crate::error::ResolutionError;
use crate::plan::{Discouraged, LoadPlan};

/// Resolves one side's validated packages into the lexicographically smallest load order.
///
/// # Errors
///
/// Checks duplicate identities first, then relationships in mod-id and declaration order, then cycles.
///
/// # Examples
///
/// ```
/// use ravel_core::Side;
/// use ravel_package::PackageManifest;
/// use ravel_resolve::resolve;
///
/// # fn manifest(id: &str, dependencies: &str) -> PackageManifest {
/// #     let digest = "0".repeat(64);
/// #     let document = format!(
/// #         r#"{{
/// #           "schemaVersion": 1,
/// #           "id": "{id}",
/// #           "version": "1.0.0",
/// #           "compatibility": {{ "ravel": "*", "minecraft": ["26.0.0"] }},
/// #           "sides": ["client"],
/// #           "dependencies": {dependencies},
/// #           "authors": [{{ "name": "Example" }}],
/// #           "license": "MIT",
/// #           "source": {{ "repository": "https://example.invalid/{id}", "files": [{{ "path": "src/lib.rs", "sha256": "{digest}" }}] }},
/// #           "provenance": {{ "record": "provenance.json", "sha256": "{digest}" }}
/// #         }}"#
/// #     );
/// #     PackageManifest::parse(&document).expect("a valid manifest")
/// # }
/// let packages = [
///     manifest(
///         "example_mod",
///         r#"[{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]"#,
///     ),
///     manifest("example_library", "[]"),
/// ];
///
/// let plan = resolve(&packages, Side::Client)?;
/// assert_eq!(
///     plan.order().iter().map(ToString::to_string).collect::<Vec<_>>(),
///     ["example_library", "example_mod"]
/// );
/// # Ok::<(), ravel_resolve::ResolutionError>(())
/// ```
pub fn resolve(packages: &[PackageManifest], side: Side) -> Result<LoadPlan, ResolutionError> {
    let mut resolution = Resolution::new(packages, side)?;
    resolution.declare_every_relationship()?;
    resolution.into_plan()
}

// Keep all installed identities to distinguish missing dependencies from side mismatches.
struct Resolution<'a> {
    side: Side,
    installed: BTreeMap<&'a ModId, &'a PackageManifest>,
    loading: BTreeMap<&'a ModId, &'a PackageManifest>,
    successors: BTreeMap<&'a ModId, BTreeSet<&'a ModId>>,
    discouraged: Vec<Discouraged>,
}

impl<'a> Resolution<'a> {
    fn new(packages: &'a [PackageManifest], side: Side) -> Result<Self, ResolutionError> {
        let mut counts: BTreeMap<&ModId, usize> = BTreeMap::new();
        for package in packages {
            *counts.entry(&package.id).or_default() += 1;
        }
        if let Some((id, _)) = counts.iter().find(|(_, count)| **count > 1) {
            return Err(ResolutionError::DuplicateIdentity { id: (*id).clone() });
        }

        let installed: BTreeMap<&ModId, &PackageManifest> = packages
            .iter()
            .map(|package| (&package.id, package))
            .collect();
        let loading: BTreeMap<&ModId, &PackageManifest> = installed
            .iter()
            .filter(|(_, package)| package.sides.loads_on(side))
            .map(|(id, package)| (*id, *package))
            .collect();

        Ok(Self {
            side,
            installed,
            successors: loading.keys().map(|id| (*id, BTreeSet::new())).collect(),
            loading,
            discouraged: Vec::new(),
        })
    }

    fn declare_every_relationship(&mut self) -> Result<(), ResolutionError> {
        let declarations: Vec<(&ModId, &Dependency)> = self
            .loading
            .iter()
            .flat_map(|(id, package)| {
                package
                    .dependencies
                    .iter()
                    .map(move |dependency| (*id, dependency))
            })
            .collect();

        for (dependant, dependency) in declarations {
            if !applies_on(dependency, self.side) {
                continue;
            }
            self.declare(dependant, dependency)?;
        }
        Ok(())
    }

    fn declare(
        &mut self,
        dependant: &'a ModId,
        dependency: &'a Dependency,
    ) -> Result<(), ResolutionError> {
        match dependency.relationship {
            DependencyRelationship::Required => self.require(dependant, dependency),
            DependencyRelationship::Optional => self.check_and_order(dependant, dependency),
            DependencyRelationship::Incompatible => self.refuse(dependant, dependency),
            DependencyRelationship::Discouraged => {
                self.discourage(dependant, dependency);
                Ok(())
            }
        }
    }

    fn require(
        &mut self,
        dependant: &'a ModId,
        dependency: &'a Dependency,
    ) -> Result<(), ResolutionError> {
        if !self.installed.contains_key(&dependency.id) {
            return Err(ResolutionError::MissingDependency {
                dependant: dependant.clone(),
                dependency: dependency.id.clone(),
                required: dependency.version.clone(),
            });
        }
        if !self.loading.contains_key(&dependency.id) {
            return Err(ResolutionError::SideMismatch {
                dependant: dependant.clone(),
                dependency: dependency.id.clone(),
                side: self.side,
            });
        }
        self.check_and_order(dependant, dependency)
    }

    // Optional dependencies may be absent, but a loading version must satisfy its range.
    fn check_and_order(
        &mut self,
        dependant: &'a ModId,
        dependency: &'a Dependency,
    ) -> Result<(), ResolutionError> {
        let Some(installed) = self.loading.get(&dependency.id) else {
            return Ok(());
        };
        check_version(dependant, dependency, &installed.version)?;
        self.order(
            dependant,
            dependency,
            dependency.order.unwrap_or(DependencyOrder::After),
        );
        Ok(())
    }

    fn refuse(
        &mut self,
        dependant: &'a ModId,
        dependency: &'a Dependency,
    ) -> Result<(), ResolutionError> {
        let Some(installed) = self.matching(dependency) else {
            return Ok(());
        };
        Err(ResolutionError::IncompatiblePair {
            declaring: dependant.clone(),
            rejected: dependency.id.clone(),
            refused: dependency.version.clone(),
            installed,
        })
    }

    // Discouragement adds no ordering constraint unless one was declared.
    fn discourage(&mut self, dependant: &'a ModId, dependency: &'a Dependency) {
        let Some(installed) = self.matching(dependency) else {
            return;
        };
        self.discouraged.push(Discouraged {
            declaring: dependant.clone(),
            subject: dependency.id.clone(),
            installed,
        });
        if let Some(order) = dependency.order {
            self.order(dependant, dependency, order);
        }
    }

    fn matching(&self, dependency: &Dependency) -> Option<Version> {
        self.loading
            .get(&dependency.id)
            .filter(|installed| dependency.version.matches(&installed.version))
            .map(|installed| installed.version.clone())
    }

    fn order(&mut self, dependant: &'a ModId, dependency: &'a Dependency, order: DependencyOrder) {
        let Some((other, _)) = self.loading.get_key_value(&dependency.id) else {
            return;
        };
        let (first, second) = match order {
            DependencyOrder::After => (*other, dependant),
            DependencyOrder::Before => (dependant, *other),
        };
        self.successors.entry(first).or_default().insert(second);
    }

    fn into_plan(self) -> Result<LoadPlan, ResolutionError> {
        let order = load_order(&self.successors)?;
        Ok(LoadPlan::new(self.side, order, self.discouraged))
    }
}

fn applies_on(dependency: &Dependency, side: Side) -> bool {
    dependency
        .sides
        .as_ref()
        .is_none_or(|sides| sides.loads_on(side))
}

fn check_version(
    dependant: &ModId,
    dependency: &Dependency,
    installed: &Version,
) -> Result<(), ResolutionError> {
    if dependency.version.matches(installed) {
        return Ok(());
    }
    Err(ResolutionError::UnsatisfiedVersion {
        dependant: dependant.clone(),
        dependency: dependency.id.clone(),
        required: dependency.version.clone(),
        installed: installed.clone(),
    })
}

// Kahn's algorithm with an ordered ready set breaks ties by mod id.
fn load_order(
    successors: &BTreeMap<&ModId, BTreeSet<&ModId>>,
) -> Result<Vec<ModId>, ResolutionError> {
    let mut waiting_on: BTreeMap<&ModId, usize> = successors.keys().map(|id| (*id, 0)).collect();
    for followers in successors.values() {
        for follower in followers {
            *waiting_on.entry(follower).or_default() += 1;
        }
    }

    let mut ready: BTreeSet<&ModId> = waiting_on
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order: Vec<ModId> = Vec::with_capacity(successors.len());
    while let Some(next) = ready.pop_first() {
        order.push(next.clone());
        for follower in successors.get(next).into_iter().flatten() {
            let count = waiting_on.entry(follower).or_default();
            *count -= 1;
            if *count == 0 {
                ready.insert(follower);
            }
        }
    }

    if order.len() == successors.len() {
        return Ok(order);
    }
    let placed: BTreeSet<&ModId> = order.iter().collect();
    let stuck: BTreeSet<&ModId> = successors
        .keys()
        .filter(|id| !placed.contains(*id))
        .copied()
        .collect();
    Err(ResolutionError::Cycle {
        members: find_cycle(&stuck, successors),
    })
}

// Walk smallest predecessors to a repeat, then rotate the cycle to its smallest member.
fn find_cycle(
    stuck: &BTreeSet<&ModId>,
    successors: &BTreeMap<&ModId, BTreeSet<&ModId>>,
) -> Vec<ModId> {
    let mut predecessors: BTreeMap<&ModId, BTreeSet<&ModId>> =
        stuck.iter().map(|id| (*id, BTreeSet::new())).collect();
    for (first, followers) in successors {
        for follower in followers {
            if let Some(entry) = predecessors.get_mut(follower)
                && stuck.contains(first)
            {
                entry.insert(first);
            }
        }
    }

    let mut walk: Vec<&ModId> = vec![stuck.first().copied().expect("a cycle has a member")];
    let repeat = loop {
        let current = *walk.last().expect("the walk starts with one member");
        let previous = predecessors
            .get(current)
            .and_then(BTreeSet::first)
            .copied()
            .expect("an unplaced mod always waits on another unplaced mod");
        if let Some(index) = walk.iter().position(|id| *id == previous) {
            break index;
        }
        walk.push(previous);
    };

    let mut members: Vec<ModId> = walk.split_off(repeat).into_iter().rev().cloned().collect();
    if let Some((smallest, _)) = members
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.cmp(right))
    {
        members.rotate_left(smallest);
    }
    members
}

#[cfg(test)]
mod tests;
