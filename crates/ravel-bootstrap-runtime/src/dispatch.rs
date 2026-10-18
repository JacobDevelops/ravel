//! Delivers hooks through the installed runtime, the sole publication authority.

use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};

use crate::guard::HookError;

/// Whether the calling thread has a runtime to observe an invocation.
#[inline]
pub(crate) fn is_observed() -> bool {
    crate::runtime::with_selected(|_| ()).is_some()
}

/// Shows an invocation to the runtime selected for this thread.
pub(crate) fn deliver(target: &HookTarget, values: &[HookValue<'_>]) -> Result<(), HookError> {
    crate::runtime::with_selected(|runtime| match runtime.observe(target, values) {
        HookOutcome::Proceed => Ok(()),
        HookOutcome::Veto(reason) => Err(HookError::Vetoed {
            hook: target.id().as_str(),
            reason,
        }),
    })
    .unwrap_or(Ok(()))
}
