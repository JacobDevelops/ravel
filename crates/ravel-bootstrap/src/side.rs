//! Which side this process is, told to the host rather than guessed.
//!
//! Until this module existed the composed host wrote `Side::Client` into every
//! launch, so the `sides` list a mod ships could not discriminate in production:
//! a dedicated server would have believed it was a client and started every
//! client-only mod on it. The rule was real and its input was a constant.
//!
//! # Why the answer arrives from Java
//!
//! [`game_directory`](crate::game_directory) records what the pinned Prism
//! launch carries, and it rules out the two obvious answers: the command line
//! reaches the process with no program arguments at all, and the only property
//! the game sets at startup is `java.awt.headless`. Neither names a side.
//!
//! The answer this module used to compute was a class-path probe at premain,
//! asking whether the JVM could load the client's own entry point and, failing
//! that, a class every distribution carries. A dedicated-server launch measured
//! against Mojang's published server jar showed why that cannot stand. The
//! published jar is a bundler: at premain the only class on any loader is the
//! bundler's own, and the real server jars are extracted and loaded later under a
//! loader of the bundler's making. Both probes miss, and a launch that refuses on
//! a miss refuses on every published server.
//!
//! So the side is named where the fact first exists rather than before it. The
//! agent's transformer is shown every class in every loader before it
//! initialises, and the first of the two distribution entry points to arrive
//! there says which distribution this process was started from. Initialisation
//! then runs from a prologue that transformer prepends to that entry point's own
//! `main`, so side detection runs in the game's defining class loader.
//!
//! # The code this receives
//!
//! `dev.ravel.jvm.Agent` names the side and hands the code to
//! `Hook.ravelInitialize`. The two constants below are the whole agreement
//! between the two languages, and a code neither language knows is
//! [`SideError::Undeterminable`] rather than a guessed side: every rule below
//! this point — which mods load, which packets may be sent — is a rule about a
//! side.
//!
//! # What this cannot answer
//!
//! **The integrated server is not a distribution.** [`Side::IntegratedServer`]
//! is a server running inside a client process, so no entry point produces it and
//! this never answers it. A client launch is admitted as [`Side::Client`], which
//! is the side the process *is*; a mod that declares only `integrated-server` is
//! parked there. `docs/guide/limits.md` states that.

use std::fmt;

use ravel_core::Side;

/// The code the agent reports for the client distribution.
///
/// Written in `dev.ravel.jvm.Agent` as well, and nowhere else in either
/// language. A code the agent could report and this does not know would install
/// nothing, which is why the tests below pin both.
const CLIENT: i32 = 0;

/// The code the agent reports for the dedicated server distribution.
const DEDICATED_SERVER: i32 = 1;

/// Why this process cannot say which side it is.
#[derive(Debug)]
pub struct SideError {
    /// The code the agent reported, which no host knows.
    code: i32,
}

impl fmt::Display for SideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the agent reported side {}, which names no distribution this host knows",
            self.code
        )
    }
}

impl std::error::Error for SideError {}

/// The side this process is running as, from the code the agent reported.
///
/// # Errors
///
/// Returns [`SideError`] for a code no host knows, which is a Java half and a
/// Rust half that no longer agree. A launch that cannot name its side installs
/// nothing.
pub fn side(code: i32) -> Result<Side, SideError> {
    match code {
        CLIENT => Ok(Side::Client),
        DEDICATED_SERVER => Ok(Side::DedicatedServer),
        unknown => Err(SideError { code: unknown }),
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::Side;

    use super::{CLIENT, DEDICATED_SERVER, side};

    /// Both codes, and the fact that neither answers the other's side.
    #[test]
    fn code_names_exactly_one_side() {
        assert_eq!(side(CLIENT).expect("a known code"), Side::Client);
        assert_eq!(
            side(DEDICATED_SERVER).expect("a known code"),
            Side::DedicatedServer
        );
    }

    /// The integrated server is not a distribution, so no code produces it.
    #[test]
    fn no_code_names_the_integrated_server() {
        for code in [CLIENT, DEDICATED_SERVER] {
            assert_ne!(
                side(code).expect("a known code"),
                Side::IntegratedServer,
                "a distribution entry point cannot name the server inside a client"
            );
        }
    }

    /// A code neither language knows installs nothing and says what it saw.
    #[test]
    fn a_code_no_host_knows_refuses_rather_than_guessing() {
        for unknown in [-1, 2, i32::MAX] {
            let refused = side(unknown).expect_err("no side is determinable");
            assert!(refused.to_string().contains(&unknown.to_string()));
        }
    }
}
