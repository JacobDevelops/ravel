use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A mod execution role; client and integrated server may share a process.
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
    /// Every execution side in declaration order.
    pub const ALL: [Self; 3] = [Self::Client, Self::IntegratedServer, Self::DedicatedServer];

    /// Returns whether this side runs server logic.
    #[must_use]
    pub const fn is_server(self) -> bool {
        matches!(self, Self::IntegratedServer | Self::DedicatedServer)
    }

    /// Returns the side's canonical serialized name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::IntegratedServer => "integrated-server",
            Self::DedicatedServer => "dedicated-server",
        }
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
    /// Returns [`EmptySideSet`] when no side is declared.
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

/// A value built only on the client and inaccessible from server sides.
///
/// # Examples
///
/// ```
/// use ravel_core::{ClientOnly, Side};
///
/// let mut ran = false;
/// let absent = ClientOnly::built_on(Side::DedicatedServer, || {
///     ran = true;
///     "what a client-bound packet last carried"
/// });
/// assert!(!ran);
/// assert!(!absent.is_present());
/// assert!(absent.get(Side::DedicatedServer).is_err());
///
/// let present = ClientOnly::built_on(Side::Client, || "what a client-bound packet last carried");
/// assert_eq!(
///     present.get(Side::Client),
///     Ok(&"what a client-bound packet last carried")
/// );
/// assert!(present.get(Side::IntegratedServer).is_err());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientOnly<T> {
    value: Option<T>,
}

impl<T> ClientOnly<T> {
    /// Builds the value on the client side, and runs `build` nowhere else.
    pub fn built_on(side: Side, build: impl FnOnce() -> T) -> Self {
        Self {
            value: if side.is_server() {
                None
            } else {
                Some(build())
            },
        }
    }

    /// Returns the value, when `side` is a side that may hold it.
    ///
    /// # Errors
    ///
    /// Returns [`WrongSide`] on server sides or when no value was built.
    pub fn get(&self, side: Side) -> Result<&T, WrongSide> {
        if side.is_server() {
            return Err(WrongSide { side });
        }
        self.value.as_ref().ok_or(WrongSide { side })
    }

    /// Returns the value mutably, under the same rule as [`ClientOnly::get`].
    ///
    /// # Errors
    ///
    /// The same as [`ClientOnly::get`].
    pub fn get_mut(&mut self, side: Side) -> Result<&mut T, WrongSide> {
        if side.is_server() {
            return Err(WrongSide { side });
        }
        self.value.as_mut().ok_or(WrongSide { side })
    }

    /// Returns whether the value was built.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.value.is_some()
    }
}

/// A side asked for a client-only value it may not hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrongSide {
    side: Side,
}

impl WrongSide {
    /// The side that asked.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }
}

impl fmt::Display for WrongSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the `{}` side holds no client-only value; client-only code runs on the client alone",
            self.side.name()
        )
    }
}

impl std::error::Error for WrongSide {}

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

    use super::{ClientOnly, EmptySideSet, Side, SideSet};

    #[test]
    fn client_only_code_does_not_run_on_any_server_side() {
        for side in Side::ALL {
            let mut ran = 0_u32;
            let held = ClientOnly::built_on(side, || {
                ran += 1;
                "what a client-bound packet last carried"
            });

            assert_eq!(
                ran,
                u32::from(!side.is_server()),
                "`{}` ran the client-only closure",
                side.name()
            );
            assert_eq!(held.is_present(), !side.is_server(), "{}", side.name());
        }
    }

    #[test]
    fn a_client_only_value_is_readable_on_the_client_and_nowhere_else() {
        let mut held = ClientOnly::built_on(Side::Client, || 7_u32);
        assert_eq!(held.get(Side::Client), Ok(&7));
        *held.get_mut(Side::Client).expect("the client may hold it") += 1;
        assert_eq!(held.get(Side::Client), Ok(&8));

        for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
            let refusal = held.get(side).expect_err(side.name());
            assert_eq!(refusal.side(), side);
            assert!(refusal.to_string().contains(side.name()), "{refusal}");
            assert!(held.get_mut(side).is_err(), "{}", side.name());
        }
    }

    #[test]
    fn a_value_that_was_never_built_is_refused_on_the_client_too() {
        let absent = ClientOnly::<u32>::built_on(Side::DedicatedServer, || 7);
        assert!(!absent.is_present());
        assert_eq!(
            absent.get(Side::Client).map(|_| ()),
            Err(super::WrongSide { side: Side::Client })
        );
    }

    #[test]
    fn every_side_names_itself_the_way_it_serializes() {
        for side in Side::ALL {
            let encoded = serde_json::to_string(&side).expect("a side serializes");
            assert_eq!(encoded, format!("\"{}\"", side.name()));
        }
    }

    #[test]
    fn the_variant_list_serde_reads_is_exactly_the_one_side_all_holds() {
        // Serde enumerates variants independently of the hand-written `ALL`.
        let error = serde_json::from_str::<Side>("\"not-a-side\"")
            .expect_err("no side is spelled that way");
        let text = error.to_string();
        let named = text
            .split_once("expected one of ")
            .expect("serde names the variants it knows")
            .1;
        let named = named.split(" at line").next().unwrap_or(named);

        assert_eq!(
            named,
            Side::ALL
                .iter()
                .map(|side| format!("`{}`", side.name()))
                .collect::<Vec<_>>()
                .join(", "),
            "`Side::ALL` and the enum disagree about which sides exist"
        );
    }

    #[test]
    fn only_the_client_is_not_a_server_side() {
        assert!(!Side::Client.is_server());
        assert!(Side::IntegratedServer.is_server());
        assert!(Side::DedicatedServer.is_server());
        assert_eq!(
            Side::ALL
                .into_iter()
                .filter(|side| !side.is_server())
                .collect::<Vec<_>>(),
            [Side::Client],
            "exactly one side is not a server side"
        );
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
