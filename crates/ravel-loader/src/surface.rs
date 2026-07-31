use ravel_core::{FailureAttribution, LifecycleState, ModId, Side};

use crate::lifecycle::{LifecycleMachine, Requirement};

/// What a player is shown about one installed mod.
///
/// `ROADMAP.md`'s M3-W5 row asks for valid and invalid graphs to render
/// deterministic, actionable messages. This is the deterministic part: the
/// model a mod list draws from, ordered by the resolved load order so the same
/// package set always produces the same list. Drawing it is ADG-6's, because
/// nothing has chosen how Ravel puts anything on screen.
///
/// Separating the two is not tidiness. A model that can be asserted on without
/// a game running is what lets M3-W7's process matrix check the *content* of a
/// diagnostic rather than a screenshot of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModEntry {
    /// The canonical identity, which is what a report attributes anything to.
    pub mod_id: ModId,
    /// Where the mod is in its lifecycle.
    pub state: LifecycleState,
    /// Whether the plan tolerates this mod being absent.
    pub requirement: Requirement,
    /// Whether the mod loads on the side the set is running on.
    pub loads_here: bool,
    /// Why the mod failed, when it did.
    ///
    /// This is the loader's structured attribution rather than a sentence, so
    /// a renderer applies its own redaction policy instead of receiving text
    /// that has already been flattened past the point of redacting it.
    pub failure: Option<FailureAttribution>,
}

impl ModEntry {
    /// Returns whether this entry is one a player has to act on.
    ///
    /// A failed required mod is actionable because the set cannot open a world
    /// without it. A failed optional mod is reportable and not actionable: the
    /// plan already tolerates its absence, and telling a player to fix it would
    /// be asking for work the loader does not need done.
    #[must_use]
    pub const fn is_actionable(&self) -> bool {
        self.failure.is_some() && matches!(self.requirement, Requirement::Required)
    }
}

/// The whole installed set, as a mod list would show it.
///
/// The order is the resolved load order, which `SPEC.md` §14 requires to be
/// deterministic for an identical package set, so this list is too.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModSurface {
    side: Side,
    entries: Vec<ModEntry>,
}

impl ModSurface {
    /// Builds the surface from what the machine currently holds.
    ///
    /// # Panics
    ///
    /// Never. Every identity comes from the machine's own resolved order, so
    /// each lookup asks it about a mod it is already tracking.
    #[must_use]
    pub fn of(machine: &LifecycleMachine) -> Self {
        let side = machine.side();
        let entries = machine
            .order()
            .cloned()
            .collect::<Vec<ModId>>()
            .into_iter()
            .map(|mod_id| {
                let entry = machine
                    .entry(&mod_id)
                    .expect("the machine only orders mods it holds");
                ModEntry {
                    state: machine
                        .state(&mod_id)
                        .expect("the machine only orders mods it holds"),
                    failure: machine
                        .failure(&mod_id)
                        .expect("the machine only orders mods it holds")
                        .cloned(),
                    requirement: entry.requirement,
                    loads_here: entry.sides.loads_on(side),
                    mod_id,
                }
            })
            .collect();
        Self { side, entries }
    }

    /// Returns the side the set is running on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns every mod, in resolved load order.
    #[must_use]
    pub fn entries(&self) -> &[ModEntry] {
        &self.entries
    }

    /// Returns the entries a player has to act on, in resolved load order.
    pub fn actionable(&self) -> impl Iterator<Item = &ModEntry> + '_ {
        self.entries.iter().filter(|entry| entry.is_actionable())
    }

    /// Returns whether anything in the set failed, required or not.
    #[must_use]
    pub fn has_failure(&self) -> bool {
        self.entries.iter().any(|entry| entry.failure.is_some())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{
        DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, LifecycleState,
        ModId, RedactionPolicy, Side, SideSet,
    };

    use super::ModSurface;
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

    /// A policy that removes nothing, so a test can build a publishable
    /// message without depending on the crate that owns the real rules.
    struct KeepEverything;

    impl RedactionPolicy for KeepEverything {
        fn apply(&self, text: &str) -> String {
            text.to_owned()
        }
    }

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn entry(name: &str, requirement: Requirement, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement,
            saved_data_versions: BTreeSet::new(),
        }
    }

    fn attribution(mod_id: &str) -> FailureAttribution {
        FailureAttribution {
            mod_id: Some(id(mod_id)),
            version: None,
            dependency_chain: Vec::new(),
            state: LifecycleState::Resolved,
            kind: Some(FailureKind::MissingRequiredDependency),
            surface: None,
            environment: FailureEnvironment {
                ravel: "0.0.0".parse().expect("a legal version"),
                minecraft: "test-version".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
            },
            message: DiagnosticMessage::redacted(&KeepEverything, "requires `example_library`"),
        }
    }

    #[test]
    fn the_surface_lists_every_mod_in_resolved_order() {
        let machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("first_mod", Requirement::Required, &[Side::Client]),
                entry("second_mod", Requirement::Optional, &[Side::Client]),
            ],
        )
        .expect("a legal plan");

        let surface = ModSurface::of(&machine);
        assert_eq!(surface.side(), Side::Client);
        assert_eq!(
            surface
                .entries()
                .iter()
                .map(|entry| entry.mod_id.as_str())
                .collect::<Vec<_>>(),
            ["first_mod", "second_mod"]
        );
        assert!(!surface.has_failure());
        assert_eq!(surface.actionable().count(), 0);
    }

    #[test]
    fn a_mod_that_does_not_load_on_this_side_is_listed_and_marked() {
        let machine = LifecycleMachine::new(
            Side::DedicatedServer,
            vec![entry("client_only", Requirement::Required, &[Side::Client])],
        )
        .expect("a legal plan");

        let surface = ModSurface::of(&machine);
        let listed = &surface.entries()[0];
        // It is still shown: a player who installed it should see it, and
        // hiding it would read as the mod being absent.
        assert_eq!(listed.mod_id.as_str(), "client_only");
        assert!(!listed.loads_here);
    }

    #[test]
    fn only_a_failed_required_mod_is_something_a_player_has_to_act_on() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("required_mod", Requirement::Required, &[Side::Client]),
                entry("optional_mod", Requirement::Optional, &[Side::Client]),
            ],
        )
        .expect("a legal plan");
        for name in ["required_mod", "optional_mod"] {
            machine
                .fail(&id(name), attribution(name))
                .expect("a fresh mod can fail");
        }

        let surface = ModSurface::of(&machine);
        assert!(surface.has_failure());
        assert_eq!(
            surface
                .actionable()
                .map(|entry| entry.mod_id.as_str())
                .collect::<Vec<_>>(),
            ["required_mod"]
        );

        // The optional failure is still reported, just not as work to do.
        let optional = &surface.entries()[1];
        assert!(optional.failure.is_some());
        assert!(!optional.is_actionable());
    }

    #[test]
    fn a_failure_travels_as_attribution_rather_than_as_a_flattened_sentence() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![entry("a_mod", Requirement::Required, &[Side::Client])],
        )
        .expect("a legal plan");
        machine
            .fail(&id("a_mod"), attribution("a_mod"))
            .expect("can fail");

        let surface = ModSurface::of(&machine);
        let failure = surface.entries()[0]
            .failure
            .as_ref()
            .expect("the mod failed");
        assert_eq!(failure.kind, Some(FailureKind::MissingRequiredDependency));
        assert_eq!(failure.state, LifecycleState::Resolved);
    }

    #[test]
    fn the_same_package_set_always_produces_the_same_surface() {
        let build = || {
            LifecycleMachine::new(
                Side::Client,
                vec![
                    entry("first_mod", Requirement::Required, &[Side::Client]),
                    entry("second_mod", Requirement::Required, &[Side::Client]),
                ],
            )
            .expect("a legal plan")
        };
        assert_eq!(ModSurface::of(&build()), ModSurface::of(&build()));
    }
}
