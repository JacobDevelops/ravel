//! The panic guard between the JVM's stack frame and Ravel's own code.
//!
//! ADG-1 left two things about this frame unmeasured and assigned both to
//! M2-W5: what a panic guard costs, and what the veto-by-throwing path looks
//! like. This module is the answer to the second and the shape the first will be
//! measured against.
//!
//! A Rust panic that unwinds out of an `extern "system"` function aborts the
//! process, so the plumbing between the exported frame and any code that can
//! panic has to be inside a `catch_unwind`. `jni` provides exactly one:
//! [`EnvUnowned::with_env`] wraps its closure, and the [`EnvOutcome`] it returns
//! turns both an error and a caught panic into a thrown Java exception under the
//! policy chosen at [`jni::EnvOutcome::resolve`]. That is also the veto path:
//! an observer that refuses becomes `Err`, which becomes a `RuntimeException` on
//! the game's own thread, which unwinds the target method the agent rewrote. The
//! injected call site needs no branch and no return value to honour it.
//!
//! The guard is not paid on every crossing. When no observer is registered
//! there is nothing between the exported frame and the return that can panic —
//! one relaxed atomic add for the count and one relaxed load asking whether
//! anything is listening — so the fast path skips the attach guard and the
//! `catch_unwind` entirely. What that costs relative to ADG-1's unguarded
//! 10.02 ns is M2-W8's measurement; the shape is fixed here.

use std::fmt;

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::JString;
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean};
use jni::{Env, EnvUnowned};
use ravel_minecraft::{HookTarget, HookValue};

use crate::{dispatch, report};

/// Anything that stops a hook invocation from completing normally.
#[derive(Debug)]
pub enum HookError {
    /// An observer refused the call, and the reason is thrown into the JVM.
    Vetoed {
        /// The reviewed hook that was refused.
        hook: &'static str,
        /// Why it was refused, as the observer said it.
        reason: String,
    },
    /// The observer registry could not be read, so no decision can be trusted.
    Unavailable,
    /// The JVM refused a call the hook had to make to read its arguments.
    Jvm(jni::errors::Error),
}

impl fmt::Display for HookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vetoed { hook, reason } => write!(formatter, "{hook} was refused: {reason}"),
            Self::Unavailable => formatter.write_str("the hook registry is unavailable"),
            Self::Jvm(error) => write!(formatter, "the JVM refused a hook argument: {error}"),
        }
    }
}

impl std::error::Error for HookError {}

impl From<jni::errors::Error> for HookError {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jvm(error)
    }
}

/// Runs one hook invocation under the panic guard.
///
/// `collect` turns the JVM's arguments into what an observer is shown, and is
/// only called when something is listening: copying a string out of the JVM is
/// the one place that costs anything, so it is not paid when no observer would
/// see the result.
///
/// `'handle` is the exported frame's own borrow of the references the JVM passed
/// it, and it is shorter than `'local`, which `'local: 'handle` states.
///
/// **That split buys compilability, not safety, and nothing here is what stops a
/// handle escaping.** It exists so the closure may return values borrowed from
/// the frame at all; one lifetime would force the borrow to last as long as the
/// JNI frame and would not compile. A caller is free to instantiate `'handle` as
/// `'static`, and the signature does not stop it.
///
/// What actually confines a handle is two things, neither of them in this file:
///
/// - [`dispatch`]'s registry holds `Box<dyn HookObserver>`, which is `'static`,
///   so an observer has no field that could name a shorter lifetime to store a
///   handle into.
/// - `HookObserver::observe` takes `values: &[HookValue<'_>]` with the lifetime
///   *elided*, so it is late-bound and every call gets a fresh region the
///   implementor cannot name, unify with `&self`, or return.
///
/// Both are load-bearing and neither is obvious from reading this function.
/// Changing `observe` to `fn observe<'a>(&'a self, …, values: &'a [HookValue<'a>])`
/// would tie the values to the borrow of the observer and break the scheme while
/// leaving every lifetime in this signature exactly as it is, which is why the
/// invariant is written down at `observe` as well as here.
pub(crate) fn enter<'local, 'handle, Collect>(
    mut env: EnvUnowned<'local>,
    target: &'static HookTarget,
    collect: Collect,
) where
    'local: 'handle,
    Collect: FnOnce(&mut Env<'local>) -> Result<Vec<HookValue<'handle>>, HookError>,
{
    report::record_invocation();
    if !dispatch::is_observed() {
        return;
    }

    let outcome = env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let values = collect(env)?;
        dispatch::deliver(target, &values)
    });
    let () = outcome.resolve::<ThrowRuntimeExAndDefault>();
}

/// Runs one hook invocation for a target whose prologue may refuse it.
///
/// Same guard, same registry, same collection: the only difference is what a
/// refusal becomes. ADG-1's Phase C amendment says an opted-in prologue answers
/// a boolean and the injected call site returns from the host method when it is
/// true, so a veto is reported as that verdict instead of being thrown. Throwing
/// would unwind into game code Ravel does not author, which is the whole reason
/// the amendment exists.
///
/// Everything else still throws. A collection error or a panic resolves through
/// [`ThrowRuntimeExAndDefault`], whose default for a boolean is false, so the
/// pending exception is what the JVM acts on rather than a verdict this frame
/// never reached.
///
/// The lifetimes and what confines a handle are exactly as documented on
/// [`enter`].
pub(crate) fn enter_refusable<'local, 'handle, Collect>(
    mut env: EnvUnowned<'local>,
    target: &'static HookTarget,
    collect: Collect,
) -> jboolean
where
    'local: 'handle,
    Collect: FnOnce(&mut Env<'local>) -> Result<Vec<HookValue<'handle>>, HookError>,
{
    report::record_invocation();
    if !dispatch::is_observed() {
        return JNI_FALSE;
    }

    let outcome = env.with_env(|env: &mut Env<'local>| -> Result<jboolean, HookError> {
        let values = collect(env)?;
        match dispatch::deliver(target, &values) {
            Ok(()) => Ok(JNI_FALSE),
            Err(HookError::Vetoed { .. }) => Ok(JNI_TRUE),
            Err(other) => Err(other),
        }
    });
    outcome.resolve::<ThrowRuntimeExAndDefault>()
}

/// Copies a `java.lang.String` argument out of the JVM.
///
/// # Errors
///
/// Returns an error when the JVM refuses to hand over the characters.
pub(crate) fn text(env: &mut Env<'_>, value: &JString<'_>) -> Result<String, HookError> {
    Ok(value.try_to_string(env)?)
}

#[cfg(test)]
mod tests {
    use ravel_minecraft::{HookValue, JavaHandle};

    use super::HookError;

    /// One generated export, written out verbatim and compiled here.
    ///
    /// This is the emitted form rather than an approximation of it: the
    /// `jni_mangle` attribute that supplies the ABI and the exported name, the
    /// `'local` lifetime, one `JObject` per reference the target declares, and a
    /// handle borrowed from each. It matches a real inventory entry —
    /// `registry.register` resolves to `MappedRegistry::register`, whose
    /// descriptor declares a `ResourceKey`, an `Object` and a `RegistrationInfo`
    /// — so the three parameters are the three that target actually carries.
    ///
    /// Nothing calls it: an `EnvUnowned` is a live JNI attachment and there is no
    /// way to build one without a JVM. Compiling is the whole test. Its value is
    /// that a change to `enter`'s lifetimes, to `JavaHandle::new`, or to what
    /// `jni_mangle` does to a signature fails here, in source someone can read,
    /// instead of only inside a generated file.
    #[::jni::jni_mangle("dev.ravel.jvm.Hook", "ravelGeneratedShapeProbe")]
    pub fn generated_export<'local>(
        env: ::jni::EnvUnowned<'local>,
        _class: ::jni::objects::JClass<'local>,
        argument0: ::jni::objects::JObject<'local>,
        argument1: ::jni::objects::JObject<'local>,
        argument2: ::jni::objects::JObject<'local>,
    ) {
        crate::guard::enter(
            env,
            &::ravel_minecraft_adapter::pinned::REGISTRY_REGISTER,
            |_env| {
                Ok(::std::vec![
                    HookValue::Handle(JavaHandle::new(
                        "com/example/resources/ResourceKey",
                        &argument0,
                    )),
                    HookValue::Handle(JavaHandle::new("java/lang/Object", &argument1)),
                    HookValue::Handle(JavaHandle::new(
                        "com/example/core/RegistrationInfo",
                        &argument2,
                    )),
                ])
            },
        );
    }

    #[test]
    fn the_generated_export_shape_type_checks_against_the_guard() {
        // Compiling is the test. Naming the function is what keeps it from being
        // dead code; there is nothing about a function pointer's value worth
        // asserting, and an assertion here would only look like one.
        let _ = generated_export;
    }

    #[test]
    fn every_refusal_names_the_hook_or_says_why_it_could_not_decide() {
        let vetoed = HookError::Vetoed {
            hook: "chat.allowed-character",
            reason: "the character is not printable".to_owned(),
        };

        assert_eq!(
            vetoed.to_string(),
            "chat.allowed-character was refused: the character is not printable"
        );
        assert_eq!(
            HookError::Unavailable.to_string(),
            "the hook registry is unavailable"
        );
    }

    #[test]
    fn a_jvm_error_travels_as_a_hook_error_so_the_guard_can_throw_it() {
        let error: HookError = jni::errors::Error::JavaException.into();

        assert!(error.to_string().starts_with("the JVM refused"));
        assert!(matches!(error, HookError::Jvm(_)));
    }
}
