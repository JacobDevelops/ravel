//! Who a hook invocation is shown to.
//!
//! This is a registry of safe Rust trait objects and nothing more. How a mod's
//! code arrives in the process is ADG-2's question, ADG-2 is deferred to M3, and
//! nothing here names a binary format, a dynamic library, or a foreign calling
//! convention.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};

use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};

use crate::guard::HookError;

/// Whether anything is listening.
///
/// Kept beside the registry rather than inside it so the hot path can ask
/// without taking a lock, and so the answer is one relaxed load in the common
/// case where nothing is registered.
static OBSERVED: AtomicBool = AtomicBool::new(false);

type Registry = RwLock<Vec<Box<dyn HookObserver>>>;

fn registry() -> &'static Registry {
    static OBSERVERS: OnceLock<Registry> = OnceLock::new();
    OBSERVERS.get_or_init(|| RwLock::new(Vec::new()))
}

/// Registers something to be shown every hook invocation from now on.
///
/// # Errors
///
/// Returns an error when the registry cannot be written, which happens only
/// after a previous holder panicked while inside it.
pub fn register(observer: Box<dyn HookObserver>) -> Result<(), HookError> {
    let mut observers = registry().write().map_err(|_| HookError::Unavailable)?;
    observers.push(observer);
    OBSERVED.store(true, Ordering::Release);
    Ok(())
}

/// Whether any observer would see an invocation.
#[inline]
pub(crate) fn is_observed() -> bool {
    OBSERVED.load(Ordering::Acquire)
}

/// Shows one invocation to every observer, stopping at the first refusal.
pub(crate) fn deliver(target: &HookTarget, values: &[HookValue<'_>]) -> Result<(), HookError> {
    let observers = registry().read().map_err(|_| HookError::Unavailable)?;
    for observer in observers.iter() {
        if let HookOutcome::Veto(reason) = observer.observe(target, values) {
            return Err(HookError::Vetoed {
                hook: target.id().as_str(),
                reason,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};
    use ravel_minecraft_adapter::pinned::TARGETS;

    use super::{deliver, is_observed, register};

    struct Watching;

    impl HookObserver for Watching {
        fn observe(&self, _target: &HookTarget, _values: &[HookValue<'_>]) -> HookOutcome {
            HookOutcome::Proceed
        }
    }

    struct Refusing;

    impl HookObserver for Refusing {
        fn observe(&self, target: &HookTarget, _values: &[HookValue<'_>]) -> HookOutcome {
            HookOutcome::Veto(format!("{} is not allowed here", target.member()))
        }
    }

    #[test]
    fn nothing_is_observed_until_something_registers() {
        assert!(!is_observed(), "the registry starts empty");

        register(Box::new(Watching)).expect("the registry should accept an observer");

        assert!(is_observed());
        assert!(deliver(&TARGETS[0], &[HookValue::Int(65)]).is_ok());
    }

    #[test]
    fn the_first_refusal_stops_the_call_and_names_the_hook() {
        register(Box::new(Watching)).expect("registers");
        register(Box::new(Refusing)).expect("registers");

        let error = deliver(&TARGETS[0], &[HookValue::Int(65)])
            .expect_err("a refusing observer must stop the call");

        assert!(
            error.to_string().starts_with(TARGETS[0].id().as_str()),
            "{error}"
        );
        assert!(error.to_string().contains("is not allowed here"), "{error}");
    }
}
