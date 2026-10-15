#![forbid(unsafe_code)]

//! Server-only side test mod.
//!
//! The mirror of `ravel-test-mod-client-only`, and it exists for one reason: the
//! shipped composition could prove only half of what the side rule claims. A
//! client-only member showed a declaration keeping code off a server, and nothing
//! in a launched game showed the other direction — so "the running side decides"
//! was, end to end, one example wide. A set with a member on each side is what
//! makes the rule symmetric rather than a single case that happens to pass.
//!
//! The property is falsifiable the same way its counterpart's is. The server-only
//! work is a closure run only where this mod loads, and every time it runs it
//! appends a line to a log the caller kept a handle on. A test that drives this
//! mod on the client and finds the log empty has watched the code not run; a test
//! that finds a line in it has watched the rule break. Nothing about that depends
//! on this mod's own word.
//!
//! Every type here comes from `ravel-sdk`. `Cargo.toml` beside this file lists
//! that crate and nothing else, which is a mod author's whole manifest.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::sides::Side;
//! use ravel_test_mod_server_only::{OPENED_THE_LEDGER, ServerOnlyMod, work_log};
//!
//! // On the client the ledger is never opened, so the log stays empty.
//! let client_log = work_log();
//! let client = ServerOnlyMod::new(Side::Client, &client_log);
//! assert!(client.work_done().is_empty());
//! assert!(!client.has_ledger());
//!
//! // On a server side it is opened, once, and says so.
//! let server_log = work_log();
//! let server = ServerOnlyMod::new(Side::DedicatedServer, &server_log);
//! assert_eq!(server.work_done(), [OPENED_THE_LEDGER]);
//! assert!(server.has_ledger());
//! ```

use std::sync::{Arc, Mutex, PoisonError};

use ravel_sdk::sides::Side;
use ravel_sdk::{LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "server_only";

/// What the log says when the server-only ledger was opened.
pub const OPENED_THE_LEDGER: &str = "opened the ledger";

/// What the log says when a tick was recorded into it.
pub const RECORDED_A_TICK: &str = "recorded a tick";

/// Where this mod records the server-only work it did.
///
/// Shared rather than returned, for the reason its counterpart gives: the host
/// takes ownership of a mod value, so a test that asked the mod afterwards would
/// be asking a `dyn Mod` it cannot downcast. A handle the caller keeps is how
/// "the code never ran" is observable from outside.
///
/// Every reader takes the lines out of a poisoned lock rather than panicking on
/// it, because this log is most worth reading after something went wrong.
pub type WorkLog = Arc<Mutex<Vec<&'static str>>>;

/// Builds an empty log.
#[must_use]
pub fn work_log() -> WorkLog {
    Arc::new(Mutex::new(Vec::new()))
}

/// The authoritative state a server side holds and a client does not.
///
/// A count of the ticks this side has been told about. It stands for anything a
/// server owns outright: Ravel gives a mod no access to the world, so what
/// matters here is that the state exists nowhere but a server and that building
/// it is observable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Ledger {
    ticks: u64,
}

impl Ledger {
    /// Returns how many ticks this ledger has recorded.
    #[must_use]
    pub const fn ticks(&self) -> u64 {
        self.ticks
    }
}

/// A mod whose work is server-only, and which loads only on a server side.
///
/// The sides a package declares are metadata rather than something a mod value
/// carries, so [`ServerOnlyMod::SIDES`] is what this mod asks for and the plan
/// that loads it is what decides. Both halves are exercised: a plan that declares
/// the server sides parks this mod on a client before any of its code runs, and
/// the guard below keeps the server-only work from running even in a plan that
/// declared every side.
#[derive(Clone, Debug)]
pub struct ServerOnlyMod {
    side: Side,
    ledger: Option<Ledger>,
    log: WorkLog,
}

impl ServerOnlyMod {
    /// The sides this mod asks to load on.
    pub const SIDES: [Side; 2] = [Side::IntegratedServer, Side::DedicatedServer];

    /// Builds the mod for `side`, recording its server-only work in `log`.
    ///
    /// `side` comes from the host, which composed this value for one side and is
    /// the only thing that knows which. A mod that decided for itself would be
    /// guessing the fact the gate is about.
    #[must_use]
    pub fn new(side: Side, log: &WorkLog) -> Self {
        let ledger = side.is_server().then(|| {
            log.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(OPENED_THE_LEDGER);
            Ledger::default()
        });
        Self {
            side,
            ledger,
            log: Arc::clone(log),
        }
    }

    /// Returns the side this mod was composed for.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns whether the server-only state exists at all.
    #[must_use]
    pub const fn has_ledger(&self) -> bool {
        self.ledger.is_some()
    }

    /// Returns the server-only work this mod has done, in order.
    #[must_use]
    pub fn work_done(&self) -> Vec<&'static str> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Returns the ledger, when this side may have one.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on the client, which holds no ledger to
    /// return and is named in the refusal.
    pub fn ledger(&self) -> Result<&Ledger, ModError> {
        self.ledger.as_ref().ok_or_else(|| {
            ModError::refused(format!(
                "`{}` holds no server-only state",
                self.side.as_str()
            ))
        })
    }

    /// Records one tick against the ledger.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on the client, where there is nothing to
    /// record against.
    pub fn record_tick(&mut self) -> Result<(), ModError> {
        let log = Arc::clone(&self.log);
        let ledger = self.ledger.as_mut().ok_or_else(|| {
            ModError::refused(format!(
                "`{}` holds no server-only state",
                self.side.as_str()
            ))
        })?;
        ledger.ticks += 1;
        log.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(RECORDED_A_TICK);
        Ok(())
    }
}

#[declare_mod(id = "server_only")]
impl Mod for ServerOnlyMod {
    /// Accepts every step. Nothing here needs setting up in a phase.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::sides::{Side, SideSet};
    use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId};

    use super::{MOD_ID, OPENED_THE_LEDGER, RECORDED_A_TICK, ServerOnlyMod, work_log};

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let expected: ModId = MOD_ID.parse().expect("a legal mod id");
        let log = work_log();
        assert_eq!(ServerOnlyMod::declared_id(), &expected);
        assert_eq!(
            ServerOnlyMod::new(Side::DedicatedServer, &log).id(),
            &expected
        );
    }

    #[test]
    fn the_sides_this_mod_asks_for_are_the_server_sides_and_not_the_client() {
        let sides = SideSet::new(ServerOnlyMod::SIDES).expect("a non-empty side set");
        assert!(!sides.loads_on(Side::Client));
        for side in Side::ALL.into_iter().filter(|side| side.is_server()) {
            assert!(sides.loads_on(side), "{}", side.as_str());
        }
    }

    #[test]
    fn the_server_only_work_runs_on_every_server_side_and_not_on_the_client() {
        for side in Side::ALL {
            let log = work_log();
            let host = ServerOnlyMod::new(side, &log);

            if side.is_server() {
                assert_eq!(host.work_done(), [OPENED_THE_LEDGER], "{}", side.as_str());
                assert!(host.has_ledger(), "{}", side.as_str());
                assert_eq!(host.ledger().expect("a server holds one").ticks(), 0);
            } else {
                assert!(
                    host.work_done().is_empty(),
                    "`{}` ran server-only code",
                    side.as_str()
                );
                assert!(!host.has_ledger(), "{}", side.as_str());
                assert!(host.ledger().is_err(), "{}", side.as_str());
            }
            assert_eq!(host.side(), side);
        }
    }

    #[test]
    fn a_refusal_to_reach_server_only_state_names_this_mod_and_the_side() {
        let log = work_log();
        let host = ServerOnlyMod::new(Side::Client, &log);

        let error = host.ledger().expect_err("the client holds no ledger");
        assert!(
            matches!(&error, ModError::Refused { detail, .. } if detail.contains("client")),
            "{error}"
        );
    }

    #[test]
    fn a_tick_is_recorded_on_a_server_side_and_refused_on_the_client() {
        for side in Side::ALL {
            let log = work_log();
            let mut host = ServerOnlyMod::new(side, &log);

            let recorded = host.record_tick();

            assert_eq!(recorded.is_ok(), side.is_server(), "{}", side.as_str());
            if side.is_server() {
                assert_eq!(host.ledger().expect("a server holds one").ticks(), 1);
                assert_eq!(host.work_done(), [OPENED_THE_LEDGER, RECORDED_A_TICK]);
            } else {
                assert!(
                    host.work_done().is_empty(),
                    "`{}` did server-only work",
                    side.as_str()
                );
            }
        }
    }

    #[test]
    fn every_lifecycle_step_is_accepted_on_every_side() {
        for side in Side::ALL {
            let log = work_log();
            let mut host = ServerOnlyMod::new(side, &log);
            for state in LifecycleState::ALL {
                host.enter(state).expect("nothing is refused");
            }
        }
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(|side| ServerOnlyMod::new(side, &work_log()));
