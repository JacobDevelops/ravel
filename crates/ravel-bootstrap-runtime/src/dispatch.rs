//! Delivers hooks through the installed runtime, the sole publication authority.

use ravel_minecraft::{HookTarget, HookValue};

/// Whether the calling thread has a runtime to observe an invocation.
#[inline]
pub(crate) fn is_observed() -> bool {
    crate::runtime::with_selected(|_| ()).is_some()
}

/// Shows an invocation to the runtime selected for this thread.
pub(crate) fn deliver(target: &HookTarget, values: &[HookValue<'_>]) {
    crate::runtime::with_selected(|runtime| runtime.observe(target, values));
}
