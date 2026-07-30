//! Helper process that runs the shared mod body for the process candidate.
//!
//! It answers one frame at a time and never holds a standard-stream lock across
//! a call, because a mod-to-mod call re-enters the same streams to reach the
//! host.

#![forbid(unsafe_code)]

use std::io::{stdin, stdout};
use std::process::ExitCode;

use ravel_spike_adg_2_mod_execution as protocol;
use ravel_spike_adg2_guest as guest;

fn main() -> ExitCode {
    match serve() {
        // The host closing the pipe is how a trial ends, not a failure.
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("worker failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn serve() -> Result<(), String> {
    // A mod-to-mod call reaches the host over the same pipe, so the mod body's
    // host function is bound to a request rather than to a local function.
    guest::ravel_mod_bind_peer(request_peer);

    while let Some((opcode, payload)) =
        protocol::read_frame(&mut stdin()).map_err(|error| format!("cannot read: {error}"))?
    {
        let (tag, value) = match opcode {
            protocol::OPCODE_CALL => (protocol::TAG_RESULT, guest::call(payload)),
            protocol::OPCODE_PEER => (protocol::TAG_RESULT, guest::peer(payload)),
            protocol::OPCODE_ABI_VERSION => (protocol::TAG_RESULT, u64::from(guest::ABI_VERSION)),
            protocol::OPCODE_UNKNOWN_ENTRY => (protocol::TAG_REFUSED, 0),
            protocol::OPCODE_CRASH => {
                guest::ravel_mod_crash();
                return Err("the mod survived a crash request".to_owned());
            }
            unknown => return Err(format!("unknown opcode {unknown}")),
        };
        protocol::write_frame(&mut stdout(), tag, value)
            .map_err(|error| format!("cannot reply: {error}"))?;
    }
    Ok(())
}

/// Asks the host to run the peer call and blocks for its answer.
///
/// The mod cannot reach another mod except through the host, and here that costs
/// a second round trip.
extern "C" fn request_peer(payload: u64) -> u64 {
    if protocol::write_frame(&mut stdout(), protocol::TAG_PEER_REQUEST, payload).is_err() {
        return guest::REJECTED;
    }
    match protocol::read_frame(&mut stdin()) {
        Ok(Some((_, answer))) => answer,
        Ok(None) | Err(_) => guest::REJECTED,
    }
}
