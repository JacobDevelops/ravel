use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{
    DependencyRelationship, FailureKind, LifecycleState, ModId, Side, StructuredFailure, Version,
    VersionRange,
};
use ravel_package::{Dependency, DependencyOrder, PackageManifest};

use crate::plan::{Discouraged, LoadPlan};

/// Resolves a validated package set into the order it loads in.
///
/// Every package handed here has already been admitted by `ravel-package`: its
/// manifest parsed, its integrity held, and its compatibility with the running
/// host was accepted. What is left is the graph the packages declare about each
/// other, and `SPEC.md` §14 requires it to be accepted before any mod code
/// runs. A package set that resolves reaches [`LifecycleState::Resolved`]; one
/// that does not reaches [`LifecycleState::Failed`] with an attribution a
/// player can act on, and `SPEC.md` §9.2 requires that to happen before world
/// access rather than during it.
///
/// The resolution is per-`side`. A package declares the sides it loads on, so
/// the same installation resolves to a different plan on a client than on a
/// dedicated server, and a package that does not declare `side` is simply not
/// part of this plan.
///
/// The result is deterministic for an identical package set, which is the
/// `SPEC.md` §14 invariant this function exists to produce. Nothing here
/// iterates a hash map, every collection is ordered by [`ModId`], and the
/// returned order is the lexicographically smallest topological order of the
/// dependency graph. Two hosts given the same mods in different discovery
/// orders load them in the same order, and so does the same host twice.
///
/// # Errors
///
/// Fails closed on an invalid graph, reporting the first failure in a fixed
/// order of checks so that an identical package set always reports an identical
/// failure: [`ResolutionError::DuplicateIdentity`] first, then the declared
/// relationships in mod-id order, then [`ResolutionError::Cycle`].
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

/// One resolution in progress.
///
/// The two indexes are deliberately different. `installed` holds every package
/// in the set, because an identity collision is a property of the installation
/// rather than of a side, and because a required dependency that exists but
/// does not load here is a side mismatch rather than a missing mod. `loading`
/// holds only the packages that declare this side, because they are the ones
/// that will actually be ordered.
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

    /// Walks every declaration the loading packages make, in mod-id order and
    /// then in the order each manifest declared them.
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

    /// Applies one declaration, which is where each of the four relationship
    /// kinds gets its distinct meaning.
    fn declare(
        &mut self,
        dependant: &'a ModId,
        dependency: &'a Dependency,
    ) -> Result<(), ResolutionError> {
        match dependency.relationship {
            DependencyRelationship::Required => self.require(dependant, dependency),
            DependencyRelationship::Optional => self.prefer(dependant, dependency),
            DependencyRelationship::Incompatible => self.refuse(dependant, dependency),
            DependencyRelationship::Discouraged => {
                self.discourage(dependant, dependency);
                Ok(())
            }
        }
    }

    /// A required dependency has to be there, on this side, in range.
    ///
    /// The three ways it can fail are reported as three different outcomes
    /// because `SPEC.md` §9.2 names them separately and a player fixes them
    /// differently: a missing mod is installed, a side mismatch is a mod
    /// installed for the wrong side, and a version outside the range is a mod
    /// upgraded or downgraded.
    fn require(
        &mut self,
        dependant: &'a ModId,
        dependency: &'a Dependency,
    ) -> Result<(), ResolutionError> {
        let Some(installed) = self.installed.get(&dependency.id) else {
            return Err(ResolutionError::MissingDependency {
                dependant: dependant.clone(),
                dependency: dependency.id.clone(),
                required: dependency.version.clone(),
            });
        };
        if !self.loading.contains_key(&dependency.id) {
            return Err(ResolutionError::SideMismatch {
                dependant: dependant.clone(),
                dependency: dependency.id.clone(),
                side: self.side,
            });
        }
        check_version(dependant, dependency, &installed.version)?;
        self.order(
            dependant,
            dependency,
            dependency.order.unwrap_or(DependencyOrder::After),
        );
        Ok(())
    }

    /// An optional dependency may be absent, but not present and unsupported.
    ///
    /// Absence is the whole point of the kind, so a mod that is not installed
    /// or not loading on this side costs nothing. A mod that *is* loading is
    /// held to the declared range, because a package that declares an optional
    /// dependency has written integration code against that range: ignoring a
    /// version it says it does not support would let that code run against one,
    /// which is the failure `SPEC.md` §9.2 requires to be reported before world
    /// access rather than discovered after it.
    fn prefer(
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

    /// An incompatible pair that is both loading fails the graph closed.
    ///
    /// The declared range selects which versions the refusal is about, so a mod
    /// that names a range its rival has grown out of does not fail on it.
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

    /// A discouraged pair is recorded and both mods still load.
    ///
    /// Ordering is applied only when the declaration asks for it. A mod that
    /// discourages another states nothing about which of them goes first unless
    /// it says so, and inventing an edge would constrain a graph the package
    /// model left free.
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

    /// Returns the loading version of the named mod when the declared range
    /// selects it.
    fn matching(&self, dependency: &Dependency) -> Option<Version> {
        self.loading
            .get(&dependency.id)
            .map(|installed| installed.version.clone())
            .filter(|version| dependency.version.matches(version))
    }

    /// Records that one mod loads before another.
    ///
    /// `DependencyOrder::After` is the sense a dependency has by default: the
    /// declaring package loads second, after the mod it depends on. `Before`
    /// inverts it, which is how a package that has to install something ahead
    /// of a dependency it still needs says so.
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

/// Returns whether a declaration applies on the side being resolved.
///
/// A dependency may narrow to a subset of sides, which is how a package says
/// that it needs a rendering library on the client and nothing on a server. A
/// declaration that names no sides applies everywhere.
fn applies_on(dependency: &Dependency, side: Side) -> bool {
    dependency
        .sides
        .as_ref()
        .is_none_or(|sides| sides.loads_on(side))
}

/// Holds a loading dependency to the range that was declared for it.
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

/// Sorts the graph into the one order this crate is allowed to return.
///
/// This is Kahn's algorithm with an ordered ready set, so the mod taken next is
/// always the smallest [`ModId`] that has nothing left to wait for. That makes
/// the result the lexicographically smallest topological order, which is unique
/// for a given graph: it does not depend on which mod was inserted first, and a
/// graph with no edges at all comes back in identity order.
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

/// Names one concrete cycle among the mods that could not be ordered.
///
/// `SPEC.md` §9.2 requires a cycle to be reported, and a report that says only
/// that one exists leaves a player nothing to remove. Every mod left unplaced
/// by Kahn's algorithm still has an unplaced mod waiting ahead of it, so
/// walking backwards from the smallest one — always to the smallest predecessor
/// — reaches a repeat, and the walk between the two occurrences is a real
/// cycle. The result is rotated to start at its smallest member so that one
/// cycle has one spelling.
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

/// Why a validated package set has no load order.
///
/// The variants are the `SPEC.md` §9.2 outcomes a player is shown before world
/// access, kept apart because each one is a different thing to fix. They carry
/// what was declared and what is installed for the same reason
/// [`ravel_package::CompatibilityError`] does: nobody can act on
/// "unresolvable". No variant carries a filesystem path, because a resolution
/// failure is about mod ids and versions and never about where a package sits
/// on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolutionError {
    /// Two packages claim the same mod id.
    DuplicateIdentity {
        /// The claimed identity.
        id: ModId,
    },
    /// A required dependency is not installed at all.
    MissingDependency {
        /// The mod that declared the requirement.
        dependant: ModId,
        /// The mod it requires.
        dependency: ModId,
        /// The versions it would have accepted.
        required: VersionRange,
    },
    /// A required dependency is installed but does not load on this side.
    SideMismatch {
        /// The mod that declared the requirement.
        dependant: ModId,
        /// The mod it requires.
        dependency: ModId,
        /// The side being resolved.
        side: Side,
    },
    /// A dependency is loading at a version outside the declared range.
    UnsatisfiedVersion {
        /// The mod that declared the dependency.
        dependant: ModId,
        /// The mod it depends on.
        dependency: ModId,
        /// The versions it accepts.
        required: VersionRange,
        /// The version that is installed.
        installed: Version,
    },
    /// Two mods that declare each other incompatible are both loading.
    IncompatiblePair {
        /// The mod that declared the incompatibility.
        declaring: ModId,
        /// The mod it refuses to load with.
        rejected: ModId,
        /// The versions it refuses.
        refused: VersionRange,
        /// The version that is installed.
        installed: Version,
    },
    /// The ordering constraints form a cycle.
    Cycle {
        /// The mods in the cycle, in load order, starting at the smallest.
        members: Vec<ModId>,
    },
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity { id } => {
                write!(formatter, "two packages claim the mod id `{id}`")
            }
            Self::MissingDependency {
                dependant,
                dependency,
                required,
            } => write!(
                formatter,
                "`{dependant}` requires `{dependency}` `{required}`, which is not installed"
            ),
            Self::SideMismatch {
                dependant,
                dependency,
                side,
            } => write!(
                formatter,
                "`{dependant}` requires `{dependency}`, which does not load on the {} side",
                side_name(*side)
            ),
            Self::UnsatisfiedVersion {
                dependant,
                dependency,
                required,
                installed,
            } => write!(
                formatter,
                "`{dependant}` requires `{dependency}` `{required}`, but `{dependency}` \
                 {installed} is installed"
            ),
            Self::IncompatiblePair {
                declaring,
                rejected,
                refused,
                installed,
            } => write!(
                formatter,
                "`{declaring}` cannot load with `{rejected}` `{refused}`, and `{rejected}` \
                 {installed} is installed"
            ),
            Self::Cycle { members } => write!(
                formatter,
                "the dependency cycle {} cannot be ordered",
                cycle_path(members)
            ),
        }
    }
}

impl std::error::Error for ResolutionError {}

impl StructuredFailure for ResolutionError {
    /// Every resolution failure is one of the `SPEC.md` §9.2 outcomes.
    ///
    /// Unlike a manifest or integrity failure, which happens before there is a
    /// graph to describe, everything here *is* the graph, so every variant has
    /// a kind. Two of them share one: `SPEC.md` §9.2 names "incompatible
    /// versions" once, and both a dependency outside its range and a pair that
    /// refuses to load together are exactly that. The variants stay distinct so
    /// a caller can still tell the two apart.
    fn failure_kind(&self) -> Option<FailureKind> {
        Some(match self {
            Self::DuplicateIdentity { .. } => FailureKind::DuplicateIdentity,
            Self::MissingDependency { .. } => FailureKind::MissingRequiredDependency,
            Self::SideMismatch { .. } => FailureKind::SideMismatch,
            Self::UnsatisfiedVersion { .. } | Self::IncompatiblePair { .. } => {
                FailureKind::IncompatibleVersion
            }
            Self::Cycle { .. } => FailureKind::DependencyCycle,
        })
    }

    /// A package that fails resolution was already validated.
    ///
    /// `SPEC.md` §14 puts metadata, target, integrity, and schema acceptance in
    /// [`LifecycleState::Validated`] and the dependency graph in
    /// [`LifecycleState::Resolved`], so a set that reaches this crate has
    /// passed the first and failed on its way to the second. It has still run
    /// no mod code.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Validated
    }
}

/// Renders a cycle as the loop it is, closing back on its first member.
fn cycle_path(members: &[ModId]) -> String {
    let mut rendered: Vec<String> = members.iter().map(|id| format!("`{id}`")).collect();
    if let Some(first) = rendered.first().cloned() {
        rendered.push(first);
    }
    rendered.join(" -> ")
}

/// Names a side the way `schemas/package.schema.json` spells it.
///
/// `Side` is a `ravel-core` type with no rendering of its own, and the schema
/// spelling is the one a mod author wrote in their manifest, so it is the one
/// to show them.
fn side_name(side: Side) -> &'static str {
    match side {
        Side::Client => "client",
        Side::IntegratedServer => "integrated-server",
        Side::DedicatedServer => "dedicated-server",
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{FailureKind, LifecycleState, ModId, Redaction, Side, StructuredFailure};
    use ravel_package::PackageManifest;

    use super::{ResolutionError, resolve};

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Builds a manifest that differs from its siblings only where a test needs
    /// it to: identity, version, the sides it loads on, and what it declares
    /// about other mods.
    fn package(id: &str, version: &str, sides: &str, dependencies: &str) -> PackageManifest {
        let document = format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{id}",
              "version": "{version}",
              "compatibility": {{ "ravel": "*", "minecraft": ["26.0.0"] }},
              "sides": {sides},
              "dependencies": {dependencies},
              "authors": [{{ "name": "Example" }}],
              "license": "MIT",
              "source": {{ "repository": "https://example.invalid/{id}", "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}] }},
              "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
            }}"#
        );
        PackageManifest::parse(&document).expect("the fixture manifest should parse")
    }

    /// A client-side package at 1.0.0 declaring `dependencies`.
    fn client(id: &str, dependencies: &str) -> PackageManifest {
        package(id, "1.0.0", r#"["client"]"#, dependencies)
    }

    fn ids(plan: &[ModId]) -> Vec<String> {
        plan.iter().map(ToString::to_string).collect()
    }

    fn order(packages: &[PackageManifest], side: Side) -> Vec<String> {
        ids(resolve(packages, side)
            .expect("the graph should resolve")
            .order())
    }

    fn failure(packages: &[PackageManifest], side: Side) -> ResolutionError {
        resolve(packages, side).expect_err("the graph should fail closed")
    }

    fn id(text: &str) -> ModId {
        text.parse().expect("a valid mod id")
    }

    /// Every ordering of `packages`, so a test can assert that none of them
    /// changes the answer.
    fn permutations(packages: &[PackageManifest]) -> Vec<Vec<PackageManifest>> {
        if packages.is_empty() {
            return vec![Vec::new()];
        }
        let mut orderings = Vec::new();
        for (index, package) in packages.iter().enumerate() {
            let mut rest = packages.to_vec();
            rest.remove(index);
            for mut ordering in permutations(&rest) {
                ordering.insert(0, package.clone());
                orderings.push(ordering);
            }
        }
        orderings
    }

    #[test]
    fn a_package_set_that_declares_nothing_loads_in_identity_order() {
        let packages = [client("example_mod", "[]"), client("aardvark", "[]")];
        assert_eq!(order(&packages, Side::Client), ["aardvark", "example_mod"]);
    }

    #[test]
    fn an_empty_package_set_resolves_to_an_empty_plan() {
        let plan = resolve(&[], Side::Client).expect("nothing to resolve");
        assert!(plan.order().is_empty());
        assert!(plan.discouraged().is_empty());
        assert_eq!(plan.side(), Side::Client);
    }

    #[test]
    fn a_required_dependency_loads_before_the_mod_that_requires_it() {
        let packages = [
            client(
                "aardvark",
                r#"[{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]"#,
            ),
            client("example_library", "[]"),
        ];
        assert_eq!(
            order(&packages, Side::Client),
            ["example_library", "aardvark"],
            "the requirement must beat identity order"
        );
    }

    #[test]
    fn a_required_dependency_that_is_not_installed_fails_the_graph_closed() {
        let packages = [client(
            "example_mod",
            r#"[{ "id": "example_library", "relationship": "required", "version": "^1.0.0" }]"#,
        )];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::MissingDependency {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                required: "^1.0.0".parse().expect("a valid range"),
            }
        );
    }

    #[test]
    fn a_required_dependency_outside_the_declared_range_is_refused() {
        let packages = [
            client(
                "example_mod",
                r#"[{ "id": "example_library", "relationship": "required", "version": "^2.0.0" }]"#,
            ),
            client("example_library", "[]"),
        ];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::UnsatisfiedVersion {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                required: "^2.0.0".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            }
        );
    }

    #[test]
    fn every_comparator_of_the_range_grammar_decides_a_requirement() {
        for (range, version, resolves) in [
            (">=1.2, <2.0", "1.4.0", true),
            (">=1.2, <2.0", "2.0.0", false),
            ("~1.2.3", "1.2.9", true),
            ("~1.2.3", "1.3.0", false),
            ("=1.0.0", "1.0.0", true),
            ("=1.0.0", "1.0.1", false),
            ("*", "9.9.9", true),
        ] {
            let packages = [
                client(
                    "example_mod",
                    &format!(
                        r#"[{{ "id": "example_library", "relationship": "required",
                             "version": "{range}" }}]"#
                    ),
                ),
                package("example_library", version, r#"["client"]"#, "[]"),
            ];
            assert_eq!(
                resolve(&packages, Side::Client).is_ok(),
                resolves,
                "{range} vs {version}"
            );
        }
    }

    #[test]
    fn a_required_dependency_that_loads_on_another_side_is_a_side_mismatch() {
        let packages = [
            package(
                "example_mod",
                "1.0.0",
                r#"["client", "dedicated-server"]"#,
                r#"[{ "id": "example_library", "relationship": "required", "version": "*" }]"#,
            ),
            client("example_library", "[]"),
        ];

        assert_eq!(
            order(&packages, Side::Client),
            ["example_library", "example_mod"]
        );
        assert_eq!(
            failure(&packages, Side::DedicatedServer),
            ResolutionError::SideMismatch {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                side: Side::DedicatedServer,
            },
            "an installed mod that does not load here is not a missing mod"
        );
    }

    #[test]
    fn a_requirement_narrowed_to_another_side_does_not_apply_on_this_one() {
        let packages = [package(
            "example_mod",
            "1.0.0",
            r#"["client", "dedicated-server"]"#,
            r#"[{
                "id": "example_library",
                "relationship": "required",
                "version": "*",
                "sides": ["client"]
            }]"#,
        )];

        assert_eq!(order(&packages, Side::DedicatedServer), ["example_mod"]);
        assert!(
            matches!(
                failure(&packages, Side::Client),
                ResolutionError::MissingDependency { .. }
            ),
            "the same declaration still binds on the side it names"
        );
    }

    #[test]
    fn a_mod_that_does_not_declare_this_side_is_absent_from_the_plan() {
        let packages = [
            client("example_mod", "[]"),
            package(
                "example_server_mod",
                "1.0.0",
                r#"["dedicated-server"]"#,
                "[]",
            ),
        ];
        assert_eq!(order(&packages, Side::Client), ["example_mod"]);
        assert_eq!(
            order(&packages, Side::DedicatedServer),
            ["example_server_mod"]
        );
        assert!(order(&packages, Side::IntegratedServer).is_empty());
    }

    #[test]
    fn an_optional_dependency_is_absent_without_failing_and_orders_when_present() {
        let optional = r#"[{ "id": "aardvark", "relationship": "optional", "version": "^1.0.0" }]"#;
        let alone = [client("example_mod", optional)];
        assert_eq!(order(&alone, Side::Client), ["example_mod"]);

        let together = [client("example_mod", optional), client("aardvark", "[]")];
        assert_eq!(order(&together, Side::Client), ["aardvark", "example_mod"]);
    }

    #[test]
    fn an_optional_dependency_present_outside_its_range_is_refused() {
        let packages = [
            client(
                "example_mod",
                r#"[{ "id": "example_library", "relationship": "optional", "version": "^2.0.0" }]"#,
            ),
            client("example_library", "[]"),
        ];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::UnsatisfiedVersion {
                dependant: id("example_mod"),
                dependency: id("example_library"),
                required: "^2.0.0".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            }
        );
    }

    #[test]
    fn an_optional_dependency_loading_on_another_side_costs_nothing() {
        let packages = [
            package(
                "example_mod",
                "1.0.0",
                r#"["client", "dedicated-server"]"#,
                r#"[{ "id": "example_library", "relationship": "optional", "version": "^2.0.0" }]"#,
            ),
            client("example_library", "[]"),
        ];
        assert_eq!(order(&packages, Side::DedicatedServer), ["example_mod"]);
    }

    #[test]
    fn two_incompatible_mods_loading_together_fail_the_graph_closed() {
        let packages = [
            client(
                "example_mod",
                r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "*" }]"#,
            ),
            client("example_rival", "[]"),
        ];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::IncompatiblePair {
                declaring: id("example_mod"),
                rejected: id("example_rival"),
                refused: "*".parse().expect("a valid range"),
                installed: "1.0.0".parse().expect("a valid version"),
            }
        );
    }

    #[test]
    fn an_incompatibility_that_names_a_range_only_fires_inside_it() {
        let declaration =
            r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "<1.0.0" }]"#;
        let outgrown = [
            client("example_mod", declaration),
            package("example_rival", "1.0.0", r#"["client"]"#, "[]"),
        ];
        assert_eq!(
            order(&outgrown, Side::Client),
            ["example_mod", "example_rival"]
        );

        let inside = [
            client("example_mod", declaration),
            package("example_rival", "0.9.0", r#"["client"]"#, "[]"),
        ];
        assert!(matches!(
            failure(&inside, Side::Client),
            ResolutionError::IncompatiblePair { .. }
        ));
    }

    #[test]
    fn an_incompatible_mod_that_is_not_loading_here_is_not_a_conflict() {
        let packages = [
            package(
                "example_mod",
                "1.0.0",
                r#"["client", "dedicated-server"]"#,
                r#"[{ "id": "example_rival", "relationship": "incompatible", "version": "*" }]"#,
            ),
            client("example_rival", "[]"),
        ];
        assert_eq!(order(&packages, Side::DedicatedServer), ["example_mod"]);
        assert!(matches!(
            failure(&packages, Side::Client),
            ResolutionError::IncompatiblePair { .. }
        ));
    }

    #[test]
    fn a_discouraged_combination_is_reported_and_still_loads() {
        let packages = [
            client(
                "example_mod",
                r#"[{ "id": "example_rival", "relationship": "discouraged", "version": "*" }]"#,
            ),
            client("example_rival", "[]"),
        ];

        let plan = resolve(&packages, Side::Client).expect("discouragement does not fail");
        assert_eq!(ids(plan.order()), ["example_mod", "example_rival"]);
        assert_eq!(plan.discouraged().len(), 1);
        assert_eq!(plan.discouraged()[0].declaring, id("example_mod"));
        assert_eq!(plan.discouraged()[0].subject, id("example_rival"));
        assert_eq!(
            plan.discouraged()[0].installed,
            "1.0.0".parse().expect("a valid version")
        );
    }

    #[test]
    fn a_discouraged_mod_that_is_absent_is_not_reported() {
        let packages = [client(
            "example_mod",
            r#"[{ "id": "example_rival", "relationship": "discouraged", "version": "*" }]"#,
        )];
        let plan = resolve(&packages, Side::Client).expect("nothing to discourage");
        assert!(plan.discouraged().is_empty());
    }

    #[test]
    fn a_discouraged_declaration_orders_only_when_it_asks_to() {
        let unordered = [
            client(
                "zebra",
                r#"[{ "id": "aardvark", "relationship": "discouraged", "version": "*" }]"#,
            ),
            client("aardvark", "[]"),
        ];
        assert_eq!(order(&unordered, Side::Client), ["aardvark", "zebra"]);

        let ordered = [
            client(
                "zebra",
                r#"[{
                    "id": "aardvark",
                    "relationship": "discouraged",
                    "version": "*",
                    "order": "before"
                }]"#,
            ),
            client("aardvark", "[]"),
        ];
        assert_eq!(
            order(&ordered, Side::Client),
            ["zebra", "aardvark"],
            "a declared order beats identity order"
        );
    }

    #[test]
    fn a_dependency_declaring_order_before_puts_the_declaring_mod_first() {
        let packages = [
            client(
                "zebra",
                r#"[{
                    "id": "aardvark",
                    "relationship": "required",
                    "version": "*",
                    "order": "before"
                }]"#,
            ),
            client("aardvark", "[]"),
        ];
        assert_eq!(order(&packages, Side::Client), ["zebra", "aardvark"]);
    }

    #[test]
    fn a_dependency_cycle_names_every_mod_in_it() {
        let packages = [
            client(
                "example_mod",
                r#"[{ "id": "example_library", "relationship": "required", "version": "*" }]"#,
            ),
            client(
                "example_library",
                r#"[{ "id": "example_mod", "relationship": "required", "version": "*" }]"#,
            ),
        ];

        let error = failure(&packages, Side::Client);
        assert_eq!(
            error,
            ResolutionError::Cycle {
                members: vec![id("example_library"), id("example_mod")],
            }
        );
        assert_eq!(
            error.to_string(),
            "the dependency cycle `example_library` -> `example_mod` -> `example_library` \
             cannot be ordered"
        );
    }

    #[test]
    fn a_longer_cycle_is_reported_in_load_order_from_its_smallest_member() {
        let requires = |id: &str| {
            format!(r#"[{{ "id": "{id}", "relationship": "required", "version": "*" }}]"#)
        };
        let packages = [
            client("beetle", &requires("aardvark")),
            client("crane", &requires("beetle")),
            client("aardvark", &requires("crane")),
            client("dormouse", "[]"),
        ];

        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::Cycle {
                members: vec![id("aardvark"), id("beetle"), id("crane")],
            }
        );
    }

    #[test]
    fn a_mod_that_depends_on_itself_is_a_cycle_of_one() {
        let packages = [client(
            "example_mod",
            r#"[{ "id": "example_mod", "relationship": "required", "version": "*" }]"#,
        )];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::Cycle {
                members: vec![id("example_mod")],
            }
        );
    }

    #[test]
    fn contradictory_ordering_between_two_mods_is_a_cycle() {
        let packages = [
            client(
                "aardvark",
                r#"[{
                    "id": "beetle",
                    "relationship": "required",
                    "version": "*",
                    "order": "after"
                }]"#,
            ),
            client(
                "beetle",
                r#"[{
                    "id": "aardvark",
                    "relationship": "required",
                    "version": "*",
                    "order": "after"
                }]"#,
            ),
        ];
        assert!(matches!(
            failure(&packages, Side::Client),
            ResolutionError::Cycle { .. }
        ));
    }

    #[test]
    fn two_packages_claiming_one_identity_fail_the_graph_closed() {
        let packages = [
            client("example_mod", "[]"),
            package("example_mod", "2.0.0", r#"["dedicated-server"]"#, "[]"),
        ];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::DuplicateIdentity {
                id: id("example_mod"),
            },
            "a collision is an installation fault, not a per-side one"
        );
    }

    #[test]
    fn a_duplicate_is_reported_by_identity_rather_than_by_arrival() {
        let packages = [
            client("zebra", "[]"),
            client("aardvark", "[]"),
            client("zebra", "[]"),
            client("aardvark", "[]"),
        ];
        assert_eq!(
            failure(&packages, Side::Client),
            ResolutionError::DuplicateIdentity { id: id("aardvark") }
        );
    }

    #[test]
    fn the_same_package_set_resolves_identically_from_every_input_order() {
        let packages = [
            client(
                "zebra",
                r#"[{ "id": "beetle", "relationship": "required", "version": "^1.0.0" }]"#,
            ),
            client(
                "beetle",
                r#"[{ "id": "aardvark", "relationship": "optional", "version": "*" }]"#,
            ),
            client("aardvark", "[]"),
            client(
                "crane",
                r#"[{ "id": "aardvark", "relationship": "discouraged", "version": "*" }]"#,
            ),
            client("dormouse", "[]"),
        ];

        let orderings = permutations(&packages);
        assert_eq!(orderings.len(), 120, "every ordering of five packages");

        let expected = resolve(&packages, Side::Client).expect("the graph should resolve");
        for ordering in orderings {
            assert_eq!(
                resolve(&ordering, Side::Client).expect("the graph should resolve"),
                expected,
                "{:?}",
                ids(&ordering
                    .iter()
                    .map(|package| package.id.clone())
                    .collect::<Vec<_>>())
            );
        }
        assert_eq!(
            ids(expected.order()),
            ["aardvark", "beetle", "crane", "dormouse", "zebra"]
        );
    }

    #[test]
    fn a_failure_is_the_same_failure_from_every_input_order() {
        let packages = [
            client(
                "zebra",
                r#"[{ "id": "beetle", "relationship": "required", "version": "^2.0.0" }]"#,
            ),
            client("beetle", "[]"),
            client(
                "aardvark",
                r#"[{ "id": "crane", "relationship": "required", "version": "*" }]"#,
            ),
        ];

        let expected = failure(&packages, Side::Client);
        for ordering in permutations(&packages) {
            assert_eq!(failure(&ordering, Side::Client), expected);
        }
        assert_eq!(
            expected,
            ResolutionError::MissingDependency {
                dependant: id("aardvark"),
                dependency: id("crane"),
                required: "*".parse().expect("a valid range"),
            },
            "the smallest identity reports first, whatever order they arrived in"
        );
    }

    #[test]
    fn every_failure_reports_its_spec_outcome_and_the_state_it_failed_in() {
        for (error, expected) in [
            (
                ResolutionError::DuplicateIdentity { id: id("a") },
                FailureKind::DuplicateIdentity,
            ),
            (
                ResolutionError::MissingDependency {
                    dependant: id("a"),
                    dependency: id("b"),
                    required: "*".parse().expect("a valid range"),
                },
                FailureKind::MissingRequiredDependency,
            ),
            (
                ResolutionError::SideMismatch {
                    dependant: id("a"),
                    dependency: id("b"),
                    side: Side::Client,
                },
                FailureKind::SideMismatch,
            ),
            (
                ResolutionError::UnsatisfiedVersion {
                    dependant: id("a"),
                    dependency: id("b"),
                    required: "*".parse().expect("a valid range"),
                    installed: "1.0.0".parse().expect("a valid version"),
                },
                FailureKind::IncompatibleVersion,
            ),
            (
                ResolutionError::IncompatiblePair {
                    declaring: id("a"),
                    rejected: id("b"),
                    refused: "*".parse().expect("a valid range"),
                    installed: "1.0.0".parse().expect("a valid version"),
                },
                FailureKind::IncompatibleVersion,
            ),
            (
                ResolutionError::Cycle {
                    members: vec![id("a")],
                },
                FailureKind::DependencyCycle,
            ),
        ] {
            assert_eq!(error.failure_kind(), Some(expected), "{error}");
            assert_eq!(error.lifecycle_state(), LifecycleState::Validated);
            assert_eq!(error.diagnostic().redaction(), Redaction::Raw);
            assert_eq!(error.diagnostic().text(), error.to_string());
            assert!(!error.quotes_untrusted_text(), "{error}");
            assert!(!error.to_string().contains('/'), "{error}");
        }
    }

    #[test]
    fn every_failure_renders_the_rule_it_breaks() {
        for (error, expected) in [
            (
                ResolutionError::DuplicateIdentity {
                    id: id("example_mod"),
                },
                "two packages claim the mod id `example_mod`",
            ),
            (
                ResolutionError::MissingDependency {
                    dependant: id("example_mod"),
                    dependency: id("example_library"),
                    required: "^1.0.0".parse().expect("a valid range"),
                },
                "`example_mod` requires `example_library` `^1.0.0`, which is not installed",
            ),
            (
                ResolutionError::SideMismatch {
                    dependant: id("example_mod"),
                    dependency: id("example_library"),
                    side: Side::IntegratedServer,
                },
                "`example_mod` requires `example_library`, which does not load on the \
                 integrated-server side",
            ),
            (
                ResolutionError::UnsatisfiedVersion {
                    dependant: id("example_mod"),
                    dependency: id("example_library"),
                    required: "^2.0.0".parse().expect("a valid range"),
                    installed: "1.0.0".parse().expect("a valid version"),
                },
                "`example_mod` requires `example_library` `^2.0.0`, but `example_library` 1.0.0 \
                 is installed",
            ),
            (
                ResolutionError::IncompatiblePair {
                    declaring: id("example_mod"),
                    rejected: id("example_rival"),
                    refused: "*".parse().expect("a valid range"),
                    installed: "1.0.0".parse().expect("a valid version"),
                },
                "`example_mod` cannot load with `example_rival` `*`, and `example_rival` 1.0.0 \
                 is installed",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn every_side_is_named_the_way_the_package_schema_spells_it() {
        for (side, expected) in [
            (Side::Client, "client"),
            (Side::IntegratedServer, "integrated-server"),
            (Side::DedicatedServer, "dedicated-server"),
        ] {
            assert_eq!(super::side_name(side), expected);
        }
    }

    #[test]
    fn a_mod_may_declare_several_relationships_at_once() {
        let packages = [
            client(
                "zebra",
                r#"[
                    { "id": "aardvark", "relationship": "required", "version": "^1.0.0" },
                    { "id": "beetle", "relationship": "optional", "version": "*" },
                    { "id": "crane", "relationship": "discouraged", "version": "*" },
                    { "id": "emu", "relationship": "incompatible", "version": "*" }
                ]"#,
            ),
            client("aardvark", "[]"),
            client("beetle", "[]"),
            client("crane", "[]"),
        ];

        let plan = resolve(&packages, Side::Client).expect("the graph should resolve");
        assert_eq!(ids(plan.order()), ["aardvark", "beetle", "crane", "zebra"]);
        assert_eq!(plan.discouraged().len(), 1);
    }

    #[test]
    fn a_relationship_declared_twice_orders_once() {
        let packages = [
            client(
                "zebra",
                r#"[
                    { "id": "aardvark", "relationship": "required", "version": "^1.0.0" },
                    { "id": "aardvark", "relationship": "optional", "version": "*" }
                ]"#,
            ),
            client("aardvark", "[]"),
        ];
        assert_eq!(order(&packages, Side::Client), ["aardvark", "zebra"]);
    }
}
