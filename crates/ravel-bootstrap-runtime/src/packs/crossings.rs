//! JNI crossings for game pack publication and selection.

use jni::objects::{JClass, JString};
use jni::sys::jint;
use jni::{Env, EnvUnowned};

use crate::guard::{HookError, hand_document, text};

use super::PackOutcome;

impl From<PackOutcome> for jint {
    fn from(outcome: PackOutcome) -> Self {
        match outcome {
            PackOutcome::RuntimeUnavailable | PackOutcome::Refused => 0,
            PackOutcome::Unchanged => 1,
            PackOutcome::Written => 2,
        }
    }
}

/// Writes the world data pack before pack discovery; failures become notices.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelWriteWorldDatapack")]
pub fn ravel_write_world_datapack<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    world: JString<'local>,
) {
    env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let world = text(env, &world)?;
        // This void JVM hook reports filesystem refusals through runtime notices.
        match super::write_world_datapack(std::path::Path::new(&world)) {
            PackOutcome::RuntimeUnavailable => crate::guard::report(
                "world data pack not written",
                &"the loader runtime is not installed",
            ),
            PackOutcome::Refused | PackOutcome::Unchanged | PackOutcome::Written => {}
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Stages the data pack and returns 0 for absent, 1 for unchanged, or 2 for changed.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelWriteStagedDatapack")]
pub fn ravel_write_staged_datapack<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    directory: JString<'local>,
) -> jint {
    env.with_env(|env: &mut Env<'local>| -> Result<jint, HookError> {
        let directory = text(env, &directory)?;
        Ok(jint::from(super::write_staged_datapack(
            std::path::Path::new(&directory),
        )))
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Returns the game-facing identifier of Ravel's data pack.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelPackId")]
pub fn ravel_pack_id<'local>(env: EnvUnowned<'local>, _class: JClass<'local>) -> JString<'local> {
    hand_document(env, |_| Ok(Some(crate::packs::pack_id().to_owned())))
}
