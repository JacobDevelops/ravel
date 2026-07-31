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
/// `collect` turns the JVM's arguments into owned values, and is only called
/// when something is listening: ADG-4 says a hook carries values, and copying a
/// string out of the JVM is the one place that costs anything, so it is not paid
/// when no observer would see the result.
pub(crate) fn enter<'local, Collect>(
    mut env: EnvUnowned<'local>,
    target: &'static HookTarget,
    collect: Collect,
) where
    Collect: FnOnce(&mut Env<'local>) -> Result<Vec<HookValue>, HookError>,
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
    use super::HookError;

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
