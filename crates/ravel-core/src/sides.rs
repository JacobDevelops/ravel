use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// One side a mod can execute on (`SPEC.md` §3.4).
///
/// A single-player session runs a client and an integrated server in one
/// process, so a side is a role within a process rather than a process kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    /// The rendering and input side of the Minecraft client.
    Client,
    /// The server hosted inside a single-player client process.
    IntegratedServer,
    /// A standalone server process.
    DedicatedServer,
}

impl Side {
    /// Every side, in the order `SPEC.md` §3.4 lists them.
    pub const ALL: [Self; 3] = [Self::Client, Self::IntegratedServer, Self::DedicatedServer];

    /// Returns whether this side runs server logic.
    ///
    /// `SPEC.md` §14 requires that client-only code never loads on a server
    /// side, and the client is the only non-server side, so this is the
    /// predicate that invariant tests against.
    #[must_use]
    pub const fn is_server(self) -> bool {
        matches!(self, Self::IntegratedServer | Self::DedicatedServer)
    }
}

/// The non-empty set of sides a mod declares.
///
/// # Examples
///
/// ```
/// use ravel_core::{EmptySideSet, Side, SideSet};
///
/// let client_only = SideSet::new([Side::Client])?;
/// assert!(client_only.loads_on(Side::Client));
/// assert!(!client_only.loads_on(Side::DedicatedServer));
///
/// assert_eq!(SideSet::new([]), Err(EmptySideSet));
/// # Ok::<(), EmptySideSet>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(into = "BTreeSet<Side>", try_from = "BTreeSet<Side>")]
pub struct SideSet(BTreeSet<Side>);

impl SideSet {
    /// Builds a side declaration, discarding duplicates.
    ///
    /// # Errors
    ///
    /// Returns [`EmptySideSet`] when no side is declared. A mod that runs on no
    /// side can never load, so an empty declaration is a metadata error rather
    /// than a mod that happens to be inactive everywhere.
    pub fn new(sides: impl IntoIterator<Item = Side>) -> Result<Self, EmptySideSet> {
        let sides: BTreeSet<Side> = sides.into_iter().collect();
        if sides.is_empty() {
            return Err(EmptySideSet);
        }
        Ok(Self(sides))
    }

    /// Returns whether a mod carrying this declaration loads on `side`.
    #[must_use]
    pub fn loads_on(&self, side: Side) -> bool {
        self.0.contains(&side)
    }

    /// Returns the declared sides in [`Side::ALL`] order.
    pub fn sides(&self) -> impl Iterator<Item = Side> + '_ {
        self.0.iter().copied()
    }
}

impl From<SideSet> for BTreeSet<Side> {
    fn from(sides: SideSet) -> Self {
        sides.0
    }
}

impl TryFrom<BTreeSet<Side>> for SideSet {
    type Error = EmptySideSet;

    fn try_from(sides: BTreeSet<Side>) -> Result<Self, Self::Error> {
        Self::new(sides)
    }
}

/// A side declaration named no side at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptySideSet;

impl fmt::Display for EmptySideSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a mod must declare at least one side")
    }
}

impl std::error::Error for EmptySideSet {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{EmptySideSet, Side, SideSet};

    #[test]
    fn only_the_client_is_not_a_server_side() {
        assert!(!Side::Client.is_server());
        assert!(Side::IntegratedServer.is_server());
        assert!(Side::DedicatedServer.is_server());
        assert_eq!(Side::ALL.iter().filter(|side| side.is_server()).count(), 2);
    }

    #[test]
    fn a_declaration_answers_every_side_it_was_given() {
        let sides = SideSet::new([Side::Client, Side::IntegratedServer]).expect("two sides");
        assert!(sides.loads_on(Side::Client));
        assert!(sides.loads_on(Side::IntegratedServer));
        assert!(!sides.loads_on(Side::DedicatedServer));
        assert_eq!(
            sides.sides().collect::<Vec<_>>(),
            vec![Side::Client, Side::IntegratedServer]
        );
    }

    #[test]
    fn duplicate_declarations_collapse_and_order_follows_the_spec_listing() {
        let sides = SideSet::new([
            Side::DedicatedServer,
            Side::Client,
            Side::DedicatedServer,
            Side::IntegratedServer,
        ])
        .expect("three sides");
        assert_eq!(sides.sides().collect::<Vec<_>>(), Side::ALL.to_vec());
    }

    #[test]
    fn an_empty_declaration_is_refused_wherever_it_arrives() {
        assert_eq!(SideSet::new([]), Err(EmptySideSet));
        assert_eq!(SideSet::try_from(BTreeSet::new()), Err(EmptySideSet));
        assert_eq!(
            EmptySideSet.to_string(),
            "a mod must declare at least one side"
        );

        let error = serde_json::from_str::<SideSet>("[]").expect_err("empty declaration");
        assert!(error.to_string().contains("at least one side"), "{error}");
    }

    #[test]
    fn a_declaration_round_trips_as_a_sequence_of_kebab_case_sides() {
        let sides = SideSet::new([Side::Client, Side::DedicatedServer]).expect("two sides");
        let encoded = serde_json::to_string(&sides).expect("declaration should serialize");
        assert_eq!(encoded, "[\"client\",\"dedicated-server\"]");
        assert_eq!(
            serde_json::from_str::<SideSet>(&encoded).expect("declaration should deserialize"),
            sides
        );
        assert_eq!(
            BTreeSet::from(sides),
            BTreeSet::from([Side::Client, Side::DedicatedServer])
        );
    }

    #[test]
    fn every_side_round_trips_on_its_own() {
        for side in Side::ALL {
            let encoded = serde_json::to_string(&side).expect("side should serialize");
            assert_eq!(
                serde_json::from_str::<Side>(&encoded).expect("side should deserialize"),
                side
            );
        }
        assert_eq!(
            serde_json::to_string(&Side::IntegratedServer).expect("side should serialize"),
            "\"integrated-server\""
        );
    }
}
