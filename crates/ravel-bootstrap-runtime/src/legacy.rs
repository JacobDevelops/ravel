//! Checked legacy adapter crossings, separate from raw hook observations.
//!
//! Java selects successful reloads and owning-thread packet/tick delivery. Rust
//! retains lifecycle, side, ownership and callback-failure admission. Commands
//! require vanilla permission level two before Java calls the execution crossing.

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean};
use jni::{Env, EnvUnowned};
use ravel_core::ContentId;
use ravel_network::PacketId;

use crate::guard::{HookError, hand_document, report, selected_runtime, text};
use crate::runtime::{LoaderRuntime, Undelivered};
use crate::wire::{VERSION, count, field};

const MAX_PACKET_BYTES: usize = 32_767;
const MAX_CHANNEL_BYTES: usize = 20;
const MAX_RESOURCE_PATH_BYTES: usize = 4096;
/// Vanilla operator permission imposed by the legacy adapter, not a mod declaration.
const LEGACY_COMMAND_LEVEL: &str = "2";

fn refused(reason: String) -> HookError {
    HookError::Refused {
        hook: "legacy.semantic",
        reason,
    }
}

fn admitted_channel(channel: &str) -> Result<ContentId, &'static str> {
    if channel.len() > MAX_CHANNEL_BYTES {
        return Err("channel exceeds the byte limit");
    }
    channel.parse().map_err(|_| "invalid channel identity")
}

fn admitted_packet(channel: &str, payload_len: Option<usize>) -> Result<ContentId, &'static str> {
    if payload_len.is_none_or(|len| len > MAX_PACKET_BYTES) {
        return Err("null payload or payload exceeds the byte limit");
    }
    admitted_channel(channel)
}

fn commands(runtime: &LoaderRuntime) -> String {
    let keys = runtime.command_keys();
    let mut output = String::new();
    field(&mut output, VERSION);
    count(&mut output, keys.len());
    for key in keys {
        let literal = key.id().to_string();
        field(&mut output, &literal);
        field(&mut output, LEGACY_COMMAND_LEVEL);
        field(&mut output, &literal);
    }
    output
}

fn command_answer(result: Result<(), Undelivered>) -> String {
    match result {
        Ok(()) => String::new(),
        Err(Undelivered::Declined(ravel_host_api::HostError::OperationRefused {
            detail, ..
        })) => ravel_core::redact_text(&detail),
        Err(error) => {
            report("legacy command failed", &error);
            "command failed".to_owned()
        }
    }
}

/// Lists the admitted commands and their explicit vanilla permission policy.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyCommands")]
pub fn ravel_legacy_commands<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    hand_document(env, |_| {
        let runtime = selected_runtime()?;
        runtime
            .claim_command_thread()
            .map_err(|error| refused(error.to_string()))?;
        Ok(Some(commands(&runtime)))
    })
}

/// Executes a Java-authorized command, returning a publishable refusal or empty text.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyCommand")]
pub fn ravel_legacy_command<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
    typed: JString<'local>,
) -> JString<'local> {
    hand_document(env, |env| {
        let typed = text(env, &typed)?;
        Ok(Some(command_answer(
            selected_runtime()?.deliver_legacy_command(&typed),
        )))
    })
}

/// Delivers canonical semantic events after the Java adapter's success/thread checks.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelLegacyEvent")]
pub fn ravel_legacy_event<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    kind: JString<'local>,
) {
    env.with_env(|env: &mut Env<'local>| -> Result<(), HookError> {
        let kind = text(env, &kind)?;
        let event = ravel_host_api::GameEvent::published(&kind)
            .ok_or_else(|| refused("unknown legacy semantic event".to_owned()))?;
        match selected_runtime() {
            Ok(runtime) => {
                if let Err(error) = runtime.deliver_legacy_event(event) {
                    report(error.context(), &error);
                }
            }
            Err(error) => report("legacy event refused", &error),
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
        let payload_len = if bytes.is_null() {
            None
        } else {
            Some(bytes.len(env)?)
        };
        let id = match admitted_packet(&channel, payload_len) {
            Ok(id) => id,
            Err(error) => {
                report("legacy packet dropped", &error);
                return Ok(());
            }
        };
        let runtime = match selected_runtime() {
            Ok(runtime) => runtime,
            Err(error) => {
                report("legacy packet dropped", &error);
                return Ok(());
            }
        };
        let payload = env.convert_byte_array(&bytes)?;
        // Network input refusals must not unwind vanilla's dispatch loop. Trusted
        // callback failures have already been attributed to their owner by Rust.
        if let Err(reason) =
            runtime.deliver_legacy_packet(PacketId::new(id), payload, to_server != JNI_FALSE)
        {
            report("legacy packet dropped", &reason);
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
        let owned = admitted_channel(&channel).is_ok_and(|id| {
            selected_runtime().is_ok_and(|runtime| runtime.accepts_packet_owner(id.namespace()))
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
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    hand_document(env, |_| {
        Ok(Some(resource_paths(selected_runtime()?.as_ref())))
    })
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
            let runtime = selected_runtime()?;
            let path = text(env, &path)?;
            if path.len() > MAX_RESOURCE_PATH_BYTES {
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

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::BTreeSet;

    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_data::ContentDeclaration;
    use ravel_host_api::{HostError, Mod, ModError};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, LoadedMods, Requirement};

    use super::{command_answer, commands, resource_paths};
    use crate::runtime::{LoaderRuntime, Undelivered};
    use crate::test_support::{DocumentFixture, test_launch};
    use crate::wire::fields;

    #[test]
    fn packet_bounds_match_the_java_adapter() {
        let java = include_str!("../../../jvm/bootstrap/dev/ravel/jvm/LegacyPayload.java");
        assert!(java.contains(&format!(
            "static final int MAX_BYTES = {};",
            super::MAX_PACKET_BYTES
        )));
        assert!(java.contains(&format!(
            "static final int MAX_CHANNEL_BYTES = {};",
            super::MAX_CHANNEL_BYTES
        )));
    }

    #[test]
    fn packet_admission_checks_inclusive_bounds_and_identity() {
        let channel = format!("a:{}", "b".repeat(super::MAX_CHANNEL_BYTES - 2));
        assert!(super::admitted_packet(&channel, Some(super::MAX_PACKET_BYTES)).is_ok());
        assert!(super::admitted_channel(&format!("{channel}b")).is_err());
        assert!(super::admitted_packet(&channel, Some(super::MAX_PACKET_BYTES + 1)).is_err());
        assert!(super::admitted_channel("not-an-identity").is_err());
        assert!(super::admitted_packet(&channel, None).is_err());
    }

    #[test]
    fn command_payload_pins_literal_permission_and_usage() -> Result<(), Box<dyn std::error::Error>>
    {
        let owner: ModId = "example_mod".parse()?;
        let entry = LifecycleEntry {
            mod_id: owner,
            sides: SideSet::new([Side::DedicatedServer])?,
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        };
        let mut fixture = DocumentFixture::new(Side::DedicatedServer, vec![entry]);
        fixture.start_with_claims(&[("command:example_mod:warp".parse()?, None)]);
        let runtime = LoaderRuntime::new(fixture.machine, fixture.mods, test_launch());
        assert_eq!(
            fields(&commands(&runtime)),
            ["1", "1", "example_mod:warp", "2", "example_mod:warp"]
        );
        Ok(())
    }

    struct Supplying {
        owner: ModId,
        declaration: ContentDeclaration,
    }

    impl Mod for Supplying {
        fn id(&self) -> &ModId {
            &self.owner
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn content_declaration(&self) -> Result<Option<Cow<'_, ContentDeclaration>>, ModError> {
            Ok(Some(Cow::Borrowed(&self.declaration)))
        }
    }

    #[test]
    fn resource_payload_lists_only_admitted_paths_in_sorted_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let owner: ModId = "example_mod".parse()?;
        let mut declaration = ContentDeclaration::new(owner.clone());
        let first = "assets/example_mod/lang/en_us.json";
        let last = "assets/example_mod/models/item/warp.json";
        for path in [last, first] {
            declaration.resource(path.to_owned().try_into()?, b"{}".to_vec())?;
        }
        let entry = LifecycleEntry {
            mod_id: owner.clone(),
            sides: SideSet::new([Side::Client])?,
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        };
        let mut machine = LifecycleMachine::new(Side::Client, vec![entry])?;
        let mut mods: Vec<Box<dyn Mod>> = vec![Box::new(Supplying { owner, declaration })];
        let supplied = ravel_loader::open_resources(
            &mut LoadedMods::new(&mut machine, &mut mods)?,
            &ravel_minecraft_adapter::environment(),
        )?;
        let mut launch = test_launch();
        launch.supplied = supplied;
        let runtime = LoaderRuntime::new(machine, mods, launch);
        assert_eq!(fields(&resource_paths(&runtime)), ["1", "2", first, last]);
        Ok(())
    }

    #[test]
    fn command_answers_publish_only_authored_refusal_details()
    -> Result<(), Box<dyn std::error::Error>> {
        let owner = "example_mod".parse::<ModId>()?;
        let failure = |error| Err(Undelivered::Failed(error));
        assert_eq!(command_answer(Ok(())), "");
        assert_eq!(
            command_answer(Err(Undelivered::Declined(HostError::OperationRefused {
                mod_id: owner.clone(),
                state: LifecycleState::Running,
                detail: "choose a destination".to_owned(),
            }))),
            "choose a destination"
        );
        assert_eq!(
            command_answer(failure(HostError::Panicked {
                mod_id: owner.clone(),
                state: LifecycleState::Running,
            })),
            "command failed"
        );
        assert_eq!(
            command_answer(Err(Undelivered::Refused(
                ravel_loader::DispatchRefused::Halted { by: owner }
            ))),
            "command failed"
        );
        assert_eq!(
            command_answer(Err(Undelivered::Precondition("internal thread detail"))),
            "command failed"
        );
        Ok(())
    }
}
