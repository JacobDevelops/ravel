use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{ContentId, RegistryKey};

/// A named progression step, its prerequisites, and an optional registry gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressionState {
    id: ContentId,
    prerequisites: BTreeSet<ContentId>,
    gate: Option<RegistryKey>,
}

impl ProgressionState {
    /// Returns what this step is called.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.id
    }

    /// Returns the states that must be reached before this one, in name order.
    pub fn prerequisites(&self) -> impl Iterator<Item = &ContentId> + '_ {
        self.prerequisites.iter()
    }

    /// Returns the registry entry that gates this step, when one does.
    #[must_use]
    pub const fn gate(&self) -> Option<&RegistryKey> {
        self.gate.as_ref()
    }
}

/// The progression graph; [`crate::validate`] checks gates against pack members.
///
/// # Examples
///
/// ```
/// use ravel_core::{ContentId, ModId};
/// use ravel_pack::{Progression, ProgressionError};
///
/// fn state(path: &str) -> ContentId {
///     ContentId::new("pack".parse::<ModId>().expect("a legal mod id"), path)
///         .expect("a legal content path")
/// }
///
/// let mut progression = Progression::default();
/// progression.declare(state("start"), [], None)?;
/// progression.declare(state("smelting"), [state("start")], None)?;
///
/// assert!(progression.unreachable().is_empty());
///
/// let mut stuck = Progression::default();
/// stuck.declare(state("left"), [state("right")], None)?;
/// stuck.declare(state("right"), [state("left")], None)?;
/// assert_eq!(stuck.unreachable(), vec![state("left"), state("right")]);
/// # Ok::<(), ProgressionError>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Progression {
    states: BTreeMap<ContentId, ProgressionState>,
}

impl Progression {
    /// Declares one step, its prerequisites and its gate.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate steps or self-prerequisites; missing prerequisites are checked later.
    pub fn declare(
        &mut self,
        id: ContentId,
        prerequisites: impl IntoIterator<Item = ContentId>,
        gate: Option<RegistryKey>,
    ) -> Result<(), ProgressionError> {
        let prerequisites: BTreeSet<ContentId> = prerequisites.into_iter().collect();
        if self.states.contains_key(&id) {
            return Err(ProgressionError::DuplicateState { state: id });
        }
        if prerequisites.contains(&id) {
            return Err(ProgressionError::RequiresItself { state: id });
        }
        self.states.insert(
            id.clone(),
            ProgressionState {
                id,
                prerequisites,
                gate,
            },
        );
        Ok(())
    }

    /// Returns every declared step, in name order.
    pub fn states(&self) -> impl Iterator<Item = &ProgressionState> + '_ {
        self.states.values()
    }

    /// Returns the step called `id`, when the pack declares one.
    #[must_use]
    pub fn state(&self, id: &ContentId) -> Option<&ProgressionState> {
        self.states.get(id)
    }

    /// Returns whether the pack declares any step at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Returns undeclared prerequisites in name order.
    #[must_use]
    pub fn dangling(&self) -> Vec<(ContentId, ContentId)> {
        let mut dangling = Vec::new();
        for state in self.states.values() {
            for prerequisite in &state.prerequisites {
                if !self.states.contains_key(prerequisite) {
                    dangling.push((state.id.clone(), prerequisite.clone()));
                }
            }
        }
        dangling
    }

    /// Returns steps unreachable from roots, including cycles and missing prerequisites, in name order.
    #[must_use]
    pub fn unreachable(&self) -> Vec<ContentId> {
        let mut reached: BTreeSet<&ContentId> = BTreeSet::new();
        loop {
            let ready: Vec<&ContentId> = self
                .states
                .values()
                .filter(|state| !reached.contains(&state.id))
                .filter(|state| {
                    state.prerequisites.iter().all(|prerequisite| {
                        self.states.contains_key(prerequisite) && reached.contains(prerequisite)
                    })
                })
                .map(|state| &state.id)
                .collect();
            if ready.is_empty() {
                break;
            }
            reached.extend(ready);
        }

        self.states
            .keys()
            .filter(|id| !reached.contains(id))
            .cloned()
            .collect()
    }
}

/// Why a step could not be declared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressionError {
    /// The pack already declares a step by this name.
    DuplicateState {
        /// The name declared twice.
        state: ContentId,
    },
    /// A step names itself as its own prerequisite.
    RequiresItself {
        /// The step that does.
        state: ContentId,
    },
}

impl fmt::Display for ProgressionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateState { state } => write!(
                formatter,
                "`{state}` is already a step in this progression, and one name is one step"
            ),
            Self::RequiresItself { state } => write!(
                formatter,
                "`{state}` requires itself, so nothing could ever reach it"
            ),
        }
    }
}

impl std::error::Error for ProgressionError {}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, ContentKind, ModId, RegistryKey};

    use super::{Progression, ProgressionError};

    fn state(path: &str) -> ContentId {
        ContentId::new("pack".parse::<ModId>().expect("a legal mod id"), path)
            .expect("a legal content path")
    }

    fn gate(path: &str) -> RegistryKey {
        RegistryKey::new(
            ContentKind::Item,
            ContentId::new(
                "example_mod".parse::<ModId>().expect("a legal mod id"),
                path,
            )
            .expect("a legal content path"),
        )
    }

    #[test]
    fn an_empty_progression_declares_nothing_and_strands_nobody() {
        let progression = Progression::default();
        assert!(progression.is_empty());
        assert_eq!(progression.states().count(), 0);
        assert_eq!(
            progression.dangling(),
            [] as [(ravel_core::ContentId, ravel_core::ContentId); 0]
        );
        assert_eq!(progression.unreachable(), [] as [ravel_core::ContentId; 0]);
        assert_eq!(progression.state(&state("start")), None);
    }

    #[test]
    fn a_chain_grown_from_a_root_is_reachable_whatever_order_it_was_declared_in() {
        let mut backwards = Progression::default();
        backwards
            .declare(state("third"), [state("second")], None)
            .expect("a legal step");
        backwards
            .declare(state("second"), [state("first")], None)
            .expect("a legal step");
        backwards
            .declare(state("first"), [], Some(gate("lantern")))
            .expect("a legal step");

        assert_eq!(backwards.unreachable(), [] as [ravel_core::ContentId; 0]);
        assert_eq!(
            backwards.dangling(),
            [] as [(ravel_core::ContentId, ravel_core::ContentId); 0]
        );
        assert_eq!(
            backwards
                .state(&state("first"))
                .and_then(|found| found.gate()),
            Some(&gate("lantern"))
        );
        assert_eq!(
            backwards
                .state(&state("third"))
                .expect("declared")
                .prerequisites()
                .collect::<Vec<_>>(),
            [&state("second")]
        );
    }

    #[test]
    fn a_step_standing_on_a_name_nothing_declares_is_dangling_and_unreachable() {
        let mut progression = Progression::default();
        progression
            .declare(state("root"), [], None)
            .expect("a legal step");
        progression
            .declare(state("locked"), [state("absent")], None)
            .expect("a step may name a prerequisite not yet declared");
        progression
            .declare(state("after"), [state("locked")], None)
            .expect("a legal step");

        assert_eq!(progression.dangling(), [(state("locked"), state("absent"))]);
        assert_eq!(progression.unreachable(), [state("after"), state("locked")]);
    }

    #[test]
    fn a_cycle_is_unreachable_without_being_dangling() {
        let mut progression = Progression::default();
        progression
            .declare(state("left"), [state("right")], None)
            .expect("a legal step");
        progression
            .declare(state("right"), [state("left")], None)
            .expect("a legal step");
        progression
            .declare(state("root"), [], None)
            .expect("a legal step");

        assert!(
            progression.dangling().is_empty(),
            "every name in the cycle exists"
        );
        assert_eq!(progression.unreachable(), [state("left"), state("right")]);
    }

    #[test]
    fn a_progression_with_no_root_at_all_strands_every_step() {
        let mut progression = Progression::default();
        progression
            .declare(state("one"), [state("two")], None)
            .expect("a legal step");
        progression
            .declare(state("two"), [state("three")], None)
            .expect("a legal step");
        progression
            .declare(state("three"), [state("one")], None)
            .expect("a legal step");

        assert_eq!(
            progression.unreachable(),
            [state("one"), state("three"), state("two")]
        );
    }

    #[test]
    fn a_step_declared_twice_and_a_step_requiring_itself_are_both_refused() {
        let mut progression = Progression::default();
        progression
            .declare(state("root"), [], None)
            .expect("a legal step");

        assert_eq!(
            progression.declare(state("root"), [], None),
            Err(ProgressionError::DuplicateState {
                state: state("root")
            })
        );
        assert_eq!(
            progression.declare(state("loop"), [state("loop")], None),
            Err(ProgressionError::RequiresItself {
                state: state("loop")
            })
        );
        assert_eq!(
            progression.states().count(),
            1,
            "neither refusal recorded a step"
        );
        for error in [
            ProgressionError::DuplicateState {
                state: state("root"),
            },
            ProgressionError::RequiresItself {
                state: state("loop"),
            },
        ] {
            assert!(error.to_string().contains("pack:"), "{error}");
        }
    }
}
