//! Decodes the distribution side reported by the Java agent at entry-point loading.
//!
//! Premain class-path probes cannot see classes loaded later by a server bundler.
//! An integrated server is not a distribution and has no code here.

use std::fmt;

use ravel_core::Side;

// These codes must match dev.ravel.jvm.Agent.
const CLIENT: i32 = 0;

const DEDICATED_SERVER: i32 = 1;

/// An unrecognized distribution code from the Java agent.
#[derive(Debug)]
pub struct SideError {
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

/// Decodes the distribution side reported by the Java agent.
///
/// # Errors
///
/// Returns [`SideError`] for an unknown code.
pub fn side_from_agent_code(code: i32) -> Result<Side, SideError> {
    match code {
        CLIENT => Ok(Side::Client),
        DEDICATED_SERVER => Ok(Side::DedicatedServer),
        unknown => Err(SideError { code: unknown }),
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::Side;

    use super::{CLIENT, DEDICATED_SERVER, side_from_agent_code};

    #[test]
    fn code_names_exactly_one_side() {
        assert_eq!(
            side_from_agent_code(CLIENT).expect("a known code"),
            Side::Client
        );
        assert_eq!(
            side_from_agent_code(DEDICATED_SERVER).expect("a known code"),
            Side::DedicatedServer
        );
    }

    #[test]
    fn no_code_names_the_integrated_server() {
        for code in [CLIENT, DEDICATED_SERVER] {
            assert_ne!(
                side_from_agent_code(code).expect("a known code"),
                Side::IntegratedServer,
                "a distribution entry point cannot name the server inside a client"
            );
        }
    }

    #[test]
    fn a_code_no_host_knows_refuses_rather_than_guessing() {
        for unknown in [-1, 2, i32::MAX] {
            let refused = side_from_agent_code(unknown).expect_err("no side is determinable");
            assert!(refused.to_string().contains(&unknown.to_string()));
        }
    }
}
