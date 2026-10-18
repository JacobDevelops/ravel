//! Guards JNI hook delivery against Rust panics and translates failures to JVM exceptions.
//!
//! Without a selected runtime, the fast path counts the invocation without entering JNI.

use std::fmt;

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::JString;
use jni::{Env, EnvUnowned};
use ravel_minecraft::{HookTarget, HookValue};

use crate::{dispatch, invocations};

/// A semantic refusal, unavailable runtime, or JNI argument error.
#[derive(Debug)]
pub enum HookError {
    /// A semantic crossing refused, thrown into the JVM.
    Refused {
        /// The refused hook's identifier.
        hook: &'static str,
        /// The semantic crossing's reason.
        reason: String,
    },
    /// No runtime is installed for the calling thread.
    Unavailable,
    /// Reading a hook argument failed.
    Jvm(jni::errors::Error),
}

impl fmt::Display for HookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused { hook, reason } => write!(formatter, "{hook} was refused: {reason}"),
            Self::Unavailable => formatter.write_str("no runtime is installed for this thread"),
            Self::Jvm(error) => write!(formatter, "the JVM refused a hook argument: {error}"),
        }
    }
}

impl std::error::Error for HookError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jvm(error) => Some(error),
            Self::Refused { .. } | Self::Unavailable => None,
        }
    }
}

impl From<jni::errors::Error> for HookError {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jvm(error)
    }
}

/// Delivers a hook, converting panics and errors to Java exceptions.
///
/// Collection is skipped without a selected runtime. The split lifetimes permit
/// frame-borrowed values; handle confinement follows `LoaderRuntime::observe`'s
/// independent values lifetime.
pub(crate) fn enter<'local, 'handle, Collect>(
    mut env: EnvUnowned<'local>,
    target: &'static HookTarget,
    collect: Collect,
) where
    'local: 'handle,
    Collect: FnOnce(&mut Env<'local>) -> Result<Vec<HookValue<'handle>>, HookError>,
{
    invocations::record_invocation();
    if !dispatch::is_observed() {
        return;
    }

    let outcome = env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let values = collect(env)?;
        dispatch::deliver(target, &values);
        Ok(())
    });
    let () = outcome.resolve::<ThrowRuntimeExAndDefault>();
}

/// Hands Java the document `produce` returns, or null when there is none.
///
/// Panics and errors become Java `RuntimeException`s rather than unwinding into the JVM.
pub(crate) fn hand_document<'local>(
    mut env: EnvUnowned<'local>,
    produce: impl FnOnce(&Env<'local>) -> Result<Option<String>, HookError>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            match produce(env)? {
                Some(document) => Ok(env.new_string(document)?),
                None => Ok(JString::default()),
            }
        },
    )
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Copies a `java.lang.String` argument out of the JVM.
///
/// # Errors
///
/// Returns an error when the JVM refuses to hand over the characters.
pub(crate) fn text(env: &Env<'_>, value: &JString<'_>) -> Result<String, HookError> {
    Ok(value.try_to_string(env)?)
}

#[cfg(test)]
pub(crate) mod tests {
    use ravel_minecraft::{HookId, HookKind, HookTarget, HookValue, JavaHandle};

    use super::HookError;

    static GENERATED_SHAPE_TARGET: HookTarget = HookTarget::new(
        HookId::new("generated.shape-probe"),
        "com/example/Generated",
        "probe",
        "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V",
        HookKind::StaticMethod,
    );

    /// Compile-only witness for generated JNI exports with frame-borrowed handles.
    #[::jni::jni_mangle("dev.ravel.jvm.Hook", "ravelGeneratedShapeProbe")]
    pub fn generated_export<'local>(
        env: ::jni::EnvUnowned<'local>,
        _class: ::jni::objects::JClass<'local>,
        argument0: ::jni::objects::JObject<'local>,
        argument1: ::jni::objects::JObject<'local>,
        argument2: ::jni::objects::JObject<'local>,
    ) {
        crate::guard::enter(env, &GENERATED_SHAPE_TARGET, |_env| {
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
        });
    }

    #[test]
    fn the_generated_export_shape_type_checks_against_the_guard() {
        // A live JVM is required to call this export; compilation checks its shape.
        let _ = generated_export;
    }

    #[test]
    fn every_refusal_names_the_hook_or_says_why_it_could_not_decide() {
        let refused = HookError::Refused {
            hook: "chat.allowed-character",
            reason: "the character is not printable".to_owned(),
        };

        assert_eq!(
            refused.to_string(),
            "chat.allowed-character was refused: the character is not printable"
        );
        assert_eq!(
            HookError::Unavailable.to_string(),
            "no runtime is installed for this thread"
        );
    }

    #[test]
    fn a_jvm_error_travels_as_a_hook_error_so_the_guard_can_throw_it() {
        let error: HookError = jni::errors::Error::JavaException.into();

        assert!(error.to_string().starts_with("the JVM refused"));
        let source = std::error::Error::source(&error).expect("the JVM cause survives");
        assert!(source.downcast_ref::<jni::errors::Error>().is_some());
        assert!(matches!(error, HookError::Jvm(_)));
    }
}
