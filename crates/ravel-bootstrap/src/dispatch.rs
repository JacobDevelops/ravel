//! Who a hook invocation is shown to.
//!
//! This is a registry of safe Rust trait objects and nothing more. Mods arrive
//! through static source composition; this registry names no binary format,
//! dynamic library, or foreign calling convention.

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};

use crate::guard::HookError;

/// Everything one process shows its hook invocations to.
///
/// A registry is a value rather than a set of statics so that what is
/// registered belongs to whoever holds it. The process has one, installed at
/// premain and read by the guard; a test holds its own and is unaffected by
/// what any other observer registered.
struct Observers {
    /// Whether anything is listening.
    ///
    /// Kept beside the registry rather than inside it so the hot path can ask
    /// without taking a lock, and so the answer is one relaxed load in the
    /// common case where nothing is registered.
    observed: AtomicBool,
    registry: RwLock<Vec<Box<dyn HookObserver>>>,
}

impl Observers {
    const fn new() -> Self {
        Self {
            observed: AtomicBool::new(false),
            registry: RwLock::new(Vec::new()),
        }
    }

    fn register(&self, observer: Box<dyn HookObserver>) -> Result<(), HookError> {
        let mut observers = self.registry.write().map_err(|_| HookError::Unavailable)?;
        observers.push(observer);
        self.observed.store(true, Ordering::Release);
        Ok(())
    }

    #[inline]
    fn is_observed(&self) -> bool {
        self.observed.load(Ordering::Acquire)
    }

    fn deliver(&self, target: &HookTarget, values: &[HookValue<'_>]) -> Result<(), HookError> {
        let observers = self.registry.read().map_err(|_| HookError::Unavailable)?;
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
}

/// The one registry a launch installs into and the guard delivers through.
static OBSERVERS: Observers = Observers::new();

/// Registers something to be shown every hook invocation from now on.
///
/// # Errors
///
/// Returns an error when the registry cannot be written, which happens only
/// after a previous holder panicked while inside it.
pub fn register(observer: Box<dyn HookObserver>) -> Result<(), HookError> {
    OBSERVERS.register(observer)
}

/// Whether any observer would see an invocation.
#[inline]
pub(crate) fn is_observed() -> bool {
    OBSERVERS.is_observed()
}

/// Shows one invocation to every observer, stopping at the first refusal.
pub(crate) fn deliver(target: &HookTarget, values: &[HookValue<'_>]) -> Result<(), HookError> {
    OBSERVERS.deliver(target, values)
}

#[cfg(test)]
mod tests {
    use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};
    use ravel_minecraft_adapter::pinned::TARGETS;

    use super::Observers;

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
        let observers = Observers::new();

        assert!(!observers.is_observed(), "the registry starts empty");

        observers
            .register(Box::new(Watching))
            .expect("the registry should accept an observer");

        assert!(observers.is_observed());
        assert!(
            observers
                .deliver(&TARGETS[0], &[HookValue::Int(65)])
                .is_ok()
        );
    }

    #[test]
    fn the_first_refusal_stops_the_call_and_names_the_hook() {
        let observers = Observers::new();
        observers.register(Box::new(Watching)).expect("registers");
        observers.register(Box::new(Refusing)).expect("registers");

        let error = observers
            .deliver(&TARGETS[0], &[HookValue::Int(65)])
            .expect_err("a refusing observer must stop the call");

        assert!(
            error.to_string().starts_with(TARGETS[0].id().as_str()),
            "{error}"
        );
        assert!(error.to_string().contains("is not allowed here"), "{error}");
    }
}
