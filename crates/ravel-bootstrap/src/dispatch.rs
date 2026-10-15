//! Process-wide hook observer registration and delivery.

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use ravel_minecraft::{HookObserver, HookOutcome, HookTarget, HookValue};

use crate::guard::HookError;

struct Observers {
    // The empty-registry fast path must not acquire the lock.
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
        self.register_with(observer, || Ok(()))
    }

    fn register_with(
        &self,
        observer: Box<dyn HookObserver>,
        publish: impl FnOnce() -> Result<(), HookError>,
    ) -> Result<(), HookError> {
        let mut registry = self.registry.write().map_err(|_| HookError::Unavailable)?;
        publish()?;
        registry.push(observer);
        drop(registry);
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
        drop(observers);
        Ok(())
    }
}

static OBSERVERS: Observers = Observers::new();

/// Registers an observer for subsequent hook invocations.
///
/// # Errors
///
/// Returns an error if the registry's write lock is poisoned.
pub fn register(observer: Box<dyn HookObserver>) -> Result<(), HookError> {
    OBSERVERS.register(observer)
}

/// Publishes runtime state only after the observer registry is writable.
pub(crate) fn register_with(
    observer: Box<dyn HookObserver>,
    publish: impl FnOnce() -> Result<(), HookError>,
) -> Result<(), HookError> {
    OBSERVERS.register_with(observer, publish)
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

    #[test]
    fn a_poisoned_registry_does_not_publish_runtime_state() {
        let observers = Observers::new();
        let poisoned = std::panic::catch_unwind(|| {
            let _guard = observers.registry.write().expect("initially writable");
            panic!("poison the registry");
        });
        assert!(poisoned.is_err());
        let mut published = false;
        let result = observers.register_with(Box::new(Watching), || {
            published = true;
            Ok(())
        });
        assert!(matches!(result, Err(crate::guard::HookError::Unavailable)));
        assert!(!published);
        assert!(!observers.is_observed());
    }

    #[test]
    fn a_refused_publication_does_not_register_an_observer() {
        let observers = Observers::new();
        let result = observers.register_with(Box::new(Watching), || {
            Err(crate::guard::HookError::AlreadyInstalled)
        });
        assert!(matches!(
            result,
            Err(crate::guard::HookError::AlreadyInstalled)
        ));
        assert!(!observers.is_observed());
        assert!(
            observers
                .registry
                .read()
                .expect("writable registry")
                .is_empty()
        );
    }
}
