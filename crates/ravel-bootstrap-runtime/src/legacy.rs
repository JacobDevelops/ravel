//! Checked legacy adapter crossings, separate from raw hook observations.
//!
//! Java selects successful reloads and owning-thread packet/tick delivery. Rust
//! retains lifecycle, side, ownership and callback-failure admission. Commands
//! require vanilla permission level two before Java calls the execution crossing.

use std::sync::PoisonError;

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean};
use jni::{Env, EnvUnowned};
use ravel_core::{ContentId, ContentKind};
use ravel_network::PacketId;

use crate::guard::{HookError, text};
use crate::runtime::LoaderRuntime;
use crate::wire::{VERSION, count, field};

const MAX_PACKET_BYTES: usize = 32_767;
const MAX_CHANNEL_BYTES: usize = 20;

fn refused(reason: String) -> HookError {
    HookError::Vetoed {
        hook: "legacy.semantic",
        reason,
    }
}

fn runtime() -> Result<std::sync::Arc<LoaderRuntime>, HookError> {
    crate::runtime::selected().ok_or(HookError::Unavailable)
}

fn commands(runtime: &LoaderRuntime) -> String {
    let keys: Vec<_> = runtime
        .inner
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .machine
        .claimed_keys()
        .filter(|key| key.kind() == ContentKind::Command)
        .cloned()
        .collect();
    let mut output = String::new();
    field(&mut output, VERSION);
    count(&mut output, keys.len());
    for key in keys {
        let literal = key.id().to_string();
        field(&mut output, &literal);
        field(&mut output, "2");
        field(&mut output, &literal);
    }
    output
}

/// Lists the admitted commands and their explicit vanilla permission policy.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyCommands")]
pub fn ravel_legacy_commands<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let runtime = runtime()?;
            runtime.claim_command_thread().map_err(refused)?;
            Ok(env.new_string(commands(&runtime))?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Executes a Java-authorized command, returning a publishable refusal or empty text.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyCommand")]
pub fn ravel_legacy_command<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    typed: JString<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let typed = text(env, &typed)?;
            let answer = runtime()?.legacy_command(&typed).err().unwrap_or_default();
            Ok(env.new_string(ravel_core::DiagnosticMessage::redacted(&answer).text())?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Delivers only the three semantic events reviewed for the legacy adapter.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyEvent")]
pub fn ravel_legacy_event<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    kind: JString<'local>,
) {
    env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let kind = text(env, &kind)?;
        if !matches!(
            kind.as_str(),
            "client-tick" | "server-tick" | "resource-reloaded"
        ) {
            return Err(refused("unknown legacy semantic event".to_owned()));
        }
        let delivery = runtime().and_then(|runtime| runtime.legacy_event(&kind).map_err(refused));
        if let Err(error) = delivery {
            eprintln!(
                "[ravel] legacy event refused: {}",
                ravel_core::DiagnosticMessage::redacted(&error.to_string()).text()
            );
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Copies a bounded vanilla custom payload into the existing native packet router.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyPacket")]
pub fn ravel_legacy_packet<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    channel: JString<'local>,
    bytes: JByteArray<'local>,
    to_server: jboolean,
) {
    env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let channel = text(env, &channel)?;
        if channel.len() > MAX_CHANNEL_BYTES
            || bytes.is_null()
            || bytes.len(env)? > MAX_PACKET_BYTES
        {
            eprintln!("[ravel] legacy packet dropped: invalid channel or payload bounds");
            return Ok(());
        }
        let Ok(id) = channel.parse::<ContentId>() else {
            eprintln!("[ravel] legacy packet dropped: invalid channel identity");
            return Ok(());
        };
        let Some(runtime) = crate::runtime::selected() else {
            eprintln!("[ravel] legacy packet dropped: no selected runtime");
            return Ok(());
        };
        let payload = env.convert_byte_array(&bytes)?;
        // Network input refusals must not unwind vanilla's dispatch loop. Trusted
        // callback failures have already been attributed to their owner by Rust.
        if let Err(reason) =
            runtime.legacy_packet(PacketId::new(id), payload, to_server != JNI_FALSE)
        {
            eprintln!(
                "[ravel] legacy packet dropped: {}",
                ravel_core::DiagnosticMessage::redacted(&reason).text()
            );
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Checks namespace ownership before Java touches an incoming packet buffer.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyPacketOwner")]
pub fn ravel_legacy_packet_owner<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    channel: JString<'local>,
) -> jboolean {
    env.with_env(|env: &mut Env<'local>| -> Result<jboolean, HookError> {
        let channel = text(env, &channel)?;
        let owned = channel.len() <= MAX_CHANNEL_BYTES
            && channel.parse::<ContentId>().is_ok_and(|id| {
                crate::runtime::selected().is_some_and(|runtime| {
                    let inner = runtime.inner.lock().unwrap_or_else(PoisonError::into_inner);
                    inner.machine.save_access().is_ok()
                        && inner.machine.loads(id.namespace())
                        && inner.machine.state(id.namespace()).is_ok_and(|state| {
                            matches!(
                                state,
                                ravel_core::LifecycleState::Running
                                    | ravel_core::LifecycleState::Stopping,
                            )
                        })
                })
            });
        Ok(if owned { jni::sys::JNI_TRUE } else { JNI_FALSE })
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

fn resource_paths(runtime: &LoaderRuntime) -> String {
    let paths: std::collections::BTreeSet<_> = runtime
        .launch
        .supplied
        .set()
        .declared()
        .iter()
        .map(|resource| resource.path.as_str())
        .collect();
    let mut output = String::new();
    field(&mut output, VERSION);
    count(&mut output, paths.len());
    for path in paths {
        field(&mut output, path);
    }
    output
}

/// Lists only the resource paths admitted into this composition.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyResourcePaths")]
pub fn ravel_legacy_resource_paths<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            Ok(env.new_string(resource_paths(runtime()?.as_ref()))?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Reads a winning admitted resource; never opens a caller-supplied filesystem path.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyResource")]
pub fn ravel_legacy_resource<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) -> JByteArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JByteArray<'local>, HookError> {
            let runtime = runtime()?;
            let path = text(env, &path)?;
            if path.len() > 4096 {
                return Ok(JByteArray::null());
            }
            let Ok(path) = path.try_into() else {
                return Ok(JByteArray::null());
            };
            match runtime.launch.supplied.bytes(&path) {
                Some(bytes) => Ok(env.byte_array_from_slice(bytes)?),
                None => Ok(JByteArray::null()),
            }
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
