#![forbid(unsafe_code)]

//! Client-only side and packet handling test mod.
//!
//! The client half of M4-W3's pair. `ravel-test-mod-networking` owns the
//! server-bound contract; this mod owns the client-bound one, `client_only:overlay`,
//! and it is the mod the gate's first property is about: **client-only code never
//! loads on a server side.**
//!
//! It is arranged so that property is falsifiable rather than merely stated. The
//! client-only work is a closure passed to [`ClientOnly::built_on`], and every time
//! it runs it appends a line to a log the caller kept a handle on. A test that drives
//! this mod on a server side and finds the log empty has watched the code not run; a
//! test that finds a line in it has watched the rule break. Nothing about that
//! depends on this mod's own word.
//!
//! Every type here comes from `ravel-sdk`. `Cargo.toml` beside this file lists that
//! crate and nothing else, which is a mod author's whole manifest.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::sides::Side;
//! use ravel_test_mod_client_only::{BUILT_THE_OVERLAY, ClientOnlyMod, work_log};
//!
//! // On a server side the overlay is never built, so the log stays empty.
//! let server_log = work_log();
//! let server = ClientOnlyMod::new(Side::DedicatedServer, &server_log);
//! assert!(server.work_done().is_empty());
//! assert!(!server.has_overlay());
//!
//! // On the client it is built, once, and says so.
//! let client_log = work_log();
//! let client = ClientOnlyMod::new(Side::Client, &client_log);
//! assert_eq!(client.work_done(), [BUILT_THE_OVERLAY]);
//! assert!(client.has_overlay());
//! ```

use std::sync::{Arc, Mutex, PoisonError};

use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketDelivery, PacketId, PacketReader, PacketWriter,
    WireError,
};
use ravel_sdk::sides::{ClientOnly, Side};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "client_only";

/// The version both ends of `client_only:overlay` speak.
pub const OVERLAY_VERSION: u32 = 2;

/// What the log says when the client-only overlay was built.
pub const BUILT_THE_OVERLAY: &str = "built the overlay";

/// What the log says when the overlay was redrawn from a packet.
pub const REDREW_THE_OVERLAY: &str = "redrew the overlay";

/// Where this mod records the client-only work it did.
///
/// Shared rather than returned, because the host takes ownership of a mod value and
/// a test that asked the mod afterwards would be asking a `dyn Mod` it cannot
/// downcast. A handle the caller keeps is how "the code never ran" is observable
/// from outside.
///
/// Every reader takes the lines out of a poisoned lock rather than panicking on it.
/// The point of this log is to be readable after something went wrong, and a mod
/// that panicked while holding it is exactly when a test most wants to see it.
pub type WorkLog = Arc<Mutex<Vec<&'static str>>>;

/// Builds an empty log.
#[must_use]
pub fn work_log() -> WorkLog {
    Arc::new(Mutex::new(Vec::new()))
}

/// A server telling the client what to draw.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Overlay {
    lines: Vec<String>,
}

impl Overlay {
    /// Builds an overlay of `lines`.
    #[must_use]
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }

    /// Returns the lines the server sent.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

impl Packet for Overlay {
    fn contract() -> PacketContract {
        PacketContract::new(overlay_id(), Direction::ToClient, OVERLAY_VERSION)
    }

    fn write(&self, writer: &mut PacketWriter) {
        let count =
            u32::try_from(self.lines.len()).expect("an overlay shorter than 4 GiB of lines");
        writer.u32(count);
        for line in &self.lines {
            writer.text(line);
        }
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        // The count is read from the payload, so it is a number an attacker
        // chooses. Nothing is preallocated from it and every line is read through
        // the reader, so a count larger than the buffer is
        // `WireError::Truncated` rather than a large allocation.
        let count = reader.u32()?;
        let mut lines = Vec::new();
        for _ in 0..count {
            lines.push(reader.text()?.to_owned());
        }
        Ok(Self { lines })
    }
}

/// Returns the name of the contract this mod receives under.
///
/// # Panics
///
/// Panics if [`MOD_ID`] stops being a legal mod id, which would mean this mod can
/// no longer be packaged under the identity its manifest declares.
#[must_use]
pub fn overlay_id() -> PacketId {
    PacketId::new(
        format!("{MOD_ID}:overlay")
            .parse()
            .expect("a mod id and a literal spell a legal content id"),
    )
}

/// The client-only resources this mod would hold in a running game.
///
/// A counter stands in for a screen, a texture handle, or anything else that needs
/// a client to exist. What matters is that building it is observable and that it is
/// built nowhere but the client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Overlays {
    drawn: Vec<String>,
}

impl Overlays {
    /// Returns the lines currently drawn.
    #[must_use]
    pub fn drawn(&self) -> &[String] {
        &self.drawn
    }
}

/// A mod whose work is client-only, and which loads only on the client.
///
/// The [`SideSet`](ravel_sdk::sides::SideSet) a package declares is metadata rather
/// than something a mod value carries, so [`ClientOnlyMod::SIDES`] is what this mod
/// asks for and the plan that loads it is what decides. Both halves are exercised:
/// a plan that declares the client side parks this mod on a server before any of its
/// code runs, and [`ClientOnly`] keeps the client-only work from running even in a
/// plan that declared every side.
#[derive(Clone, Debug)]
pub struct ClientOnlyMod {
    side: Side,
    overlays: ClientOnly<Overlays>,
    log: WorkLog,
}

impl ClientOnlyMod {
    /// The sides this mod asks to load on.
    pub const SIDES: [Side; 1] = [Side::Client];

    /// Builds the mod for `side`, recording its client-only work in `log`.
    ///
    /// `side` comes from the host, which composed this value for one side and is the
    /// only thing that knows which. A mod that decided for itself would be guessing
    /// the fact the gate is about.
    #[must_use]
    pub fn new(side: Side, log: &WorkLog) -> Self {
        let recorder = Arc::clone(log);
        Self {
            side,
            overlays: ClientOnly::built_on(side, move || {
                recorder
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(BUILT_THE_OVERLAY);
                Overlays::default()
            }),
            log: Arc::clone(log),
        }
    }

    /// Returns the side this mod was composed for.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns whether the client-only resources exist at all.
    #[must_use]
    pub const fn has_overlay(&self) -> bool {
        self.overlays.is_present()
    }

    /// Returns the client-only work this mod has done, in order.
    #[must_use]
    pub fn work_done(&self) -> Vec<&'static str> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Returns the lines currently drawn, when this side may have them.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on a server side, which is what
    /// [`ClientOnly::get`] answered, converted into the refusal a mod raises.
    pub fn drawn(&self) -> Result<&[String], ModError> {
        self.overlays
            .get(self.side)
            .map(Overlays::drawn)
            .map_err(|wrong| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Running,
                detail: wrong.to_string(),
            })
    }
}

#[declare_mod(id = "client_only")]
impl Mod for ClientOnlyMod {
    /// Accepts every step. Nothing here needs setting up in a phase.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    /// Redraws the overlay from one packet.
    ///
    /// Both refusals are reachable and neither is this mod's own check: the
    /// delivery's side rule refuses a client-bound packet on a server, and
    /// [`ClientOnly::get`] refuses the resources there. A mod that only had the
    /// first would draw into resources that do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when the delivery is not one [`Overlay`], and
    /// when this side holds no client-only resources to draw into.
    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        let overlay = delivery
            .decode::<Overlay>()
            .map_err(|refused| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Running,
                detail: refused.to_string(),
            })?;

        let side = self.side;
        let log = Arc::clone(&self.log);
        let overlays = self
            .overlays
            .get_mut(side)
            .map_err(|wrong| ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Running,
                detail: wrong.to_string(),
            })?;

        overlays.drawn = overlay.lines().to_vec();
        log.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(REDREW_THE_OVERLAY);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::network::{
        DeliveryRefused, Direction, Packet, PacketDelivery, WireError, decode, encode, encode_from,
    };
    use ravel_sdk::sides::{Side, SideSet};
    use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId};

    use super::{
        BUILT_THE_OVERLAY, ClientOnlyMod, MOD_ID, OVERLAY_VERSION, Overlay, REDREW_THE_OVERLAY,
        overlay_id, work_log,
    };

    fn arriving(payload: Vec<u8>, side: Side) -> PacketDelivery {
        PacketDelivery::new(overlay_id(), Direction::ToClient, side, payload)
    }

    fn overlay() -> Overlay {
        Overlay::new(vec!["copper lanterns: 3".to_owned(), String::new()])
    }

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let expected: ModId = MOD_ID.parse().expect("a legal mod id");
        let log = work_log();
        assert_eq!(ClientOnlyMod::declared_id(), &expected);
        assert_eq!(ClientOnlyMod::new(Side::Client, &log).id(), &expected);
    }

    #[test]
    fn the_sides_this_mod_asks_for_are_the_client_alone() {
        let sides = SideSet::new(ClientOnlyMod::SIDES).expect("a non-empty side set");
        assert!(sides.loads_on(Side::Client));
        for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
            assert!(!sides.loads_on(side), "{}", side.name());
        }
    }

    #[test]
    fn the_client_only_work_runs_on_the_client_and_on_no_server_side() {
        for side in Side::ALL {
            let log = work_log();
            let host = ClientOnlyMod::new(side, &log);

            if side.is_server() {
                assert!(
                    host.work_done().is_empty(),
                    "`{}` ran client-only code",
                    side.name()
                );
                assert!(!host.has_overlay(), "{}", side.name());
                assert!(host.drawn().is_err(), "{}", side.name());
            } else {
                assert_eq!(host.work_done(), [BUILT_THE_OVERLAY]);
                assert!(host.has_overlay());
                assert!(host.drawn().expect("the client holds them").is_empty());
            }
            assert_eq!(host.side(), side);
        }
    }

    #[test]
    fn a_refusal_to_reach_client_only_resources_names_this_mod_and_the_side() {
        let log = work_log();
        let host = ClientOnlyMod::new(Side::DedicatedServer, &log);

        let error = host.drawn().expect_err("a dedicated server draws nothing");
        assert_eq!(error.mod_id(), ClientOnlyMod::declared_id());
        assert_eq!(error.state(), LifecycleState::Running);
        assert!(
            matches!(&error, ModError::Refused { detail, .. }
                if detail.contains("dedicated-server")),
            "{error}"
        );
    }

    #[test]
    fn every_lifecycle_step_is_accepted_on_every_side() {
        for side in Side::ALL {
            let log = work_log();
            let mut host = ClientOnlyMod::new(side, &log);
            for state in LifecycleState::ALL {
                host.enter(state).expect("nothing is refused");
            }
        }
    }

    #[test]
    fn the_contract_is_owned_by_this_mod_and_travels_to_the_client() {
        let contract = Overlay::contract();
        assert_eq!(contract.id(), &overlay_id());
        assert_eq!(contract.id().owner(), ClientOnlyMod::declared_id());
        assert_eq!(contract.direction(), Direction::ToClient);
        assert_eq!(contract.version(), OVERLAY_VERSION);
    }

    #[test]
    fn an_overlay_round_trips_including_an_empty_line_and_no_lines_at_all() {
        for sent in [overlay(), Overlay::new(Vec::new())] {
            assert_eq!(
                decode::<Overlay>(&encode(&sent)).expect("its own bytes"),
                sent
            );
        }
    }

    #[test]
    fn only_a_server_side_may_send_this_contract() {
        for side in Side::ALL {
            let sent = encode_from(&overlay(), side);
            assert_eq!(
                sent.is_ok(),
                side.is_server(),
                "{} sent a client-bound packet",
                side.name()
            );
        }
    }

    #[test]
    fn a_packet_on_the_client_redraws_the_overlay_it_carried() {
        let log = work_log();
        let mut host = ClientOnlyMod::new(Side::Client, &log);

        host.receive_packet(&arriving(encode(&overlay()), Side::Client))
            .expect("its own contract on its own side");

        assert_eq!(
            host.drawn().expect("the client holds them"),
            overlay().lines()
        );
        assert_eq!(host.work_done(), [BUILT_THE_OVERLAY, REDREW_THE_OVERLAY]);
    }

    #[test]
    fn the_same_packet_on_a_server_side_is_refused_and_draws_nothing() {
        for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
            let log = work_log();
            let mut host = ClientOnlyMod::new(side, &log);

            let error = host
                .receive_packet(&arriving(encode(&overlay()), side))
                .expect_err("a server side is not an end of a client-bound link");
            assert_eq!(error.mod_id(), ClientOnlyMod::declared_id());
            assert!(
                host.work_done().is_empty(),
                "`{}` did client-only work",
                side.name()
            );
        }
    }

    #[test]
    fn a_truncated_or_trailing_payload_is_refused_and_nothing_is_drawn() {
        let full = encode(&overlay());
        let mut trailing = full.clone();
        trailing.push(0);

        for payload in [full[..full.len() - 2].to_vec(), trailing] {
            let log = work_log();
            let mut host = ClientOnlyMod::new(Side::Client, &log);

            host.receive_packet(&arriving(payload, Side::Client))
                .expect_err("the payload is not one overlay");
            assert_eq!(host.work_done(), [BUILT_THE_OVERLAY]);
            assert!(host.drawn().expect("the client holds them").is_empty());
        }
    }

    #[test]
    fn a_payload_written_under_another_version_is_refused_rather_than_misread() {
        let mut other = encode(&overlay());
        let offset = 4 + overlay_id().to_string().len();
        other[offset..offset + 4].copy_from_slice(&(OVERLAY_VERSION + 1).to_be_bytes());

        assert_eq!(
            arriving(other, Side::Client).decode::<Overlay>(),
            Err(DeliveryRefused::Malformed {
                id: overlay_id(),
                source: WireError::Version {
                    expected: OVERLAY_VERSION,
                    found: OVERLAY_VERSION + 1,
                },
            })
        );
    }

    #[test]
    fn a_line_count_larger_than_the_payload_is_truncation_and_not_an_allocation() {
        // The count is a number the sender chose, so this is the hostile case: a
        // header claiming four billion lines in a payload that holds none.
        let mut hostile = Vec::new();
        let name = overlay_id().to_string();
        hostile.extend_from_slice(
            &u32::try_from(name.len())
                .expect("a short name")
                .to_be_bytes(),
        );
        hostile.extend_from_slice(name.as_bytes());
        hostile.extend_from_slice(&OVERLAY_VERSION.to_be_bytes());
        hostile.extend_from_slice(&u32::MAX.to_be_bytes());

        assert!(matches!(
            decode::<Overlay>(&hostile),
            Err(WireError::Truncated { .. })
        ));
    }
}
