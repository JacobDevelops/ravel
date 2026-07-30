//! The one mod body every ADG-2 execution candidate runs.
//!
//! The body is ordinary safe Rust. Only its binding to a host differs: an
//! exported symbol a dynamic loader resolves, a WebAssembly export, a direct
//! Rust call when the mod is linked in, or a command a helper process decodes.
//! Anything the four candidates measure differently is therefore the binding.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Host contract version this mod was built against.
pub const ABI_VERSION: u32 = 1;

/// Payload the host sends to test invalid input.
pub const INVALID_PAYLOAD: u64 = 0;
/// Payload that makes the mod body itself fail.
pub const FAILING_PAYLOAD: u64 = 1;
/// Value the body returns for input it rejects.
pub const REJECTED: u64 = u64::MAX;
/// Smallest payload the measured window uses.
pub const FIRST_MEASURED_PAYLOAD: u64 = 2;

/// A host function the mod can call, in the models that pass one in.
pub type Peer = extern "C" fn(u64) -> u64;

static STATE: AtomicU64 = AtomicU64::new(1);
static PEER: OnceLock<Peer> = OnceLock::new();

/// The mod body: one cheap, order-dependent step the compiler cannot delete.
///
/// # Panics
///
/// Panics on [`FAILING_PAYLOAD`], which is how the failure matrix observes a mod
/// whose own body fails rather than one given bad input.
#[must_use]
pub fn call(payload: u64) -> u64 {
    if payload == INVALID_PAYLOAD {
        return REJECTED;
    }
    assert!(payload != FAILING_PAYLOAD, "the mod body failed");

    let next = STATE
        .load(Ordering::Relaxed)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(payload);
    STATE.store(next, Ordering::Relaxed);
    next
}

/// Asks the host to run the same call in another mod and returns what it said.
///
/// Mod-to-mod calls go through the host in every candidate, because the host is
/// the only party that knows which mods exist.
///
/// # Panics
///
/// Panics when no host function was bound, which is a host defect rather than a
/// measurable outcome.
#[must_use]
pub fn peer(payload: u64) -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        // Safety: the host supplies this import when it instantiates the module.
        unsafe { imports::ravel_host_peer(payload) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bound = PEER.get().expect("the host bound no peer function");
        bound(payload)
    }
}

#[cfg(target_arch = "wasm32")]
mod imports {
    #[link(wasm_import_module = "ravel")]
    unsafe extern "C" {
        pub(super) fn ravel_host_peer(payload: u64) -> u64;
    }
}

/// Reports the contract version, so a host can reject a mismatch before calling.
#[unsafe(no_mangle)]
pub extern "C" fn ravel_mod_abi_version() -> u32 {
    ABI_VERSION
}

/// Binds the host function used for mod-to-mod calls.
///
/// The WebAssembly binding takes its host function as a module import instead,
/// so this entry point exists but does nothing there.
#[unsafe(no_mangle)]
pub extern "C" fn ravel_mod_bind_peer(host_peer: Peer) {
    let _ = PEER.set(host_peer);
}

/// The measured entry point.
#[unsafe(no_mangle)]
pub extern "C" fn ravel_mod_call(payload: u64) -> u64 {
    call(payload)
}

/// The mod-to-mod entry point.
#[unsafe(no_mangle)]
pub extern "C" fn ravel_mod_peer(payload: u64) -> u64 {
    peer(payload)
}

/// Ends the mod process-fatally, so isolation can be observed rather than argued.
#[unsafe(no_mangle)]
pub extern "C" fn ravel_mod_crash() {
    std::process::abort();
}

#[cfg(test)]
mod tests {
    use super::{FAILING_PAYLOAD, FIRST_MEASURED_PAYLOAD, INVALID_PAYLOAD, REJECTED, call};

    #[test]
    fn the_body_rejects_invalid_input_and_keeps_moving_otherwise() {
        assert_eq!(call(INVALID_PAYLOAD), REJECTED);

        let first = call(FIRST_MEASURED_PAYLOAD);
        let second = call(FIRST_MEASURED_PAYLOAD);

        assert_ne!(first, second, "the body must depend on its own state");
    }

    #[test]
    fn the_body_fails_loudly_on_the_failing_payload() {
        assert!(std::panic::catch_unwind(|| call(FAILING_PAYLOAD)).is_err());
    }
}
