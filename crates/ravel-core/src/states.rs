use serde::{Deserialize, Serialize};

/// One state of the mod lifecycle defined by `SPEC.md` §14.
///
/// This is the lifecycle vocabulary and its transition table, not the state
/// machine that walks them. The loader owns sequencing, ordering, and the
/// invariants that depend on more than two states; a machine consumes
/// [`LifecycleState::can_transition_to`] and [`LifecycleState::is_terminal`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LifecycleState {
    /// The package was found; no mod code has executed.
    Discovered,
    /// Metadata, target, integrity, and schema were accepted.
    Validated,
    /// The dependency graph and load order were accepted.
    Resolved,
    /// The mod's code and resources were made available.
    Loaded,
    /// The mod instance and its declared services were created.
    Constructed,
    /// Static registries and resources were committed.
    Registered,
    /// Runtime hooks and events are active.
    Running,
    /// Saves are flushing and callbacks are quiescing.
    Stopping,
    /// Shutdown completed.
    Stopped,
    /// Terminal failure carrying structured attribution.
    Failed,
}

impl LifecycleState {
    /// Every state, in the order `SPEC.md` §14 lists them.
    pub const ALL: [Self; 10] = [
        Self::Discovered,
        Self::Validated,
        Self::Resolved,
        Self::Loaded,
        Self::Constructed,
        Self::Registered,
        Self::Running,
        Self::Stopping,
        Self::Stopped,
        Self::Failed,
    ];

    /// Returns whether the lifecycle ends here.
    ///
    /// [`Self::Stopped`] and [`Self::Failed`] are the two ends, and neither has
    /// a legal outgoing transition.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }

    /// Returns whether moving from this state to `next` is legal.
    ///
    /// The successful path advances one state at a time in the `SPEC.md` §14
    /// order, and [`Self::Failed`] is reachable from every non-terminal state
    /// because a failure can occur in any phase, including while stopping.
    /// Nothing leaves a terminal state, and no state transitions to itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use ravel_core::LifecycleState;
    ///
    /// assert!(LifecycleState::Discovered.can_transition_to(LifecycleState::Validated));
    /// assert!(LifecycleState::Stopping.can_transition_to(LifecycleState::Failed));
    /// assert!(!LifecycleState::Discovered.can_transition_to(LifecycleState::Running));
    /// assert!(!LifecycleState::Failed.can_transition_to(LifecycleState::Stopped));
    /// ```
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Discovered, Self::Validated)
                | (Self::Validated, Self::Resolved)
                | (Self::Resolved, Self::Loaded)
                | (Self::Loaded, Self::Constructed)
                | (Self::Constructed, Self::Registered)
                | (Self::Registered, Self::Running)
                | (Self::Running, Self::Stopping)
                | (Self::Stopping, Self::Stopped)
                | (
                    Self::Discovered
                        | Self::Validated
                        | Self::Resolved
                        | Self::Loaded
                        | Self::Constructed
                        | Self::Registered
                        | Self::Running
                        | Self::Stopping,
                    Self::Failed,
                )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::LifecycleState;

    const SUCCESS_PATH: [LifecycleState; 9] = [
        LifecycleState::Discovered,
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
        LifecycleState::Registered,
        LifecycleState::Running,
        LifecycleState::Stopping,
        LifecycleState::Stopped,
    ];

    fn is_expected_transition(from: LifecycleState, to: LifecycleState) -> bool {
        if from.is_terminal() {
            return false;
        }
        if to == LifecycleState::Failed {
            return true;
        }
        SUCCESS_PATH
            .windows(2)
            .any(|pair| pair[0] == from && pair[1] == to)
    }

    #[test]
    fn the_spec_lists_ten_states_and_two_of_them_end_the_lifecycle() {
        assert_eq!(LifecycleState::ALL.len(), 10);
        assert_eq!(
            LifecycleState::ALL
                .into_iter()
                .filter(|state| state.is_terminal())
                .collect::<Vec<_>>(),
            vec![LifecycleState::Stopped, LifecycleState::Failed]
        );
    }

    #[test]
    fn every_ordered_pair_of_states_matches_the_spec_transition_table() {
        let mut legal = 0_usize;
        for from in LifecycleState::ALL {
            for to in LifecycleState::ALL {
                let expected = is_expected_transition(from, to);
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "{from:?} -> {to:?} should be {}",
                    if expected { "legal" } else { "illegal" }
                );
                legal += usize::from(expected);
            }
        }
        assert_eq!(legal, 16, "eight successful steps and eight failure edges");
    }

    #[test]
    fn the_successful_path_runs_end_to_end_without_touching_failed() {
        for pair in SUCCESS_PATH.windows(2) {
            assert!(pair[0].can_transition_to(pair[1]), "{pair:?}");
        }
        assert!(!SUCCESS_PATH.contains(&LifecycleState::Failed));
    }

    #[test]
    fn no_state_repeats_or_skips_itself_forward() {
        for state in LifecycleState::ALL {
            assert!(!state.can_transition_to(state), "{state:?} repeats itself");
        }
        assert!(!LifecycleState::Discovered.can_transition_to(LifecycleState::Stopped));
        assert!(!LifecycleState::Running.can_transition_to(LifecycleState::Registered));
    }

    #[test]
    fn every_state_round_trips_as_a_kebab_case_name() {
        for state in LifecycleState::ALL {
            let encoded = serde_json::to_string(&state).expect("state should serialize");
            assert_eq!(
                serde_json::from_str::<LifecycleState>(&encoded).expect("state should deserialize"),
                state
            );
        }
        assert_eq!(
            serde_json::to_string(&LifecycleState::Constructed).expect("state should serialize"),
            "\"constructed\""
        );
    }
}
