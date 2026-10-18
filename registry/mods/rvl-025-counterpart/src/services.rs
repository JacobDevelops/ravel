//! JSON adapters for the published capability surface and ordering service.

use ravel_sdk::content::ContentId;
use ravel_sdk::{JsonValue, ModId, json};

use crate::DEFAULT_DIMENSION;
use crate::capabilities::{self, Surface};
use crate::ordering::Ordering;

/// Answers the surface service, which is how a dependent reaches §15.
///
/// The arguments are another mod's JSON, so every field is read as untrusted
/// input: a call naming no action, or an action this entry does not publish,
/// is answered with a refusal rather than guessed at.
pub(super) fn answer_surface(surface: &mut Surface, arguments: &JsonValue) -> JsonValue {
    let Some(action) = arguments.get("action").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a call names no action" });
    };
    match action {
        "read_dimension" => return read_surface_dimension(surface, arguments),
        "register_instruction" => {
            return match read_instruction(arguments) {
                Ok(instruction) => service_outcome(surface.register_instruction(instruction)),
                Err(refused) => json!({ "answered": false, "refused": refused }),
            };
        }
        "run" => return run_surface_instruction(surface, arguments),
        _ => {}
    }
    let Some(owner) = arguments
        .get("owner")
        .and_then(JsonValue::as_str)
        .and_then(|owner| owner.parse::<ModId>().ok())
    else {
        return json!({ "answered": false, "refused": "a call names no legal owner" });
    };
    match action {
        "register_channel" => {
            let Some(identifier) = read_identifier(arguments) else {
                return json!({ "answered": false, "refused": "a channel names no legal identifier" });
            };
            service_outcome(
                surface.register_channel(capabilities::ChannelKind { identifier, owner }),
            )
        }
        "register_data_factory" => {
            let Some(kind) = arguments
                .get("kind")
                .and_then(JsonValue::as_str)
                .and_then(|kind| kind.parse::<ContentId>().ok())
            else {
                return json!({ "answered": false, "refused": "a factory names no legal kind" });
            };
            service_outcome(surface.register_data_factory(&owner, &kind))
        }
        "insert_rule" => {
            let Some(rule) = arguments.get("rule").and_then(JsonValue::as_str) else {
                return json!({ "answered": false, "refused": "a rule has no name" });
            };
            service_outcome(surface.insert_rule(owner.as_str(), rule))
        }
        "register_module" => register_surface_module(surface, arguments, owner),
        "publish_dimension" => publish_surface_dimension(surface, arguments, owner),
        _ => json!({ "answered": false, "refused": "this entry publishes no such action" }),
    }
}

fn read_surface_dimension(surface: &Surface, arguments: &JsonValue) -> JsonValue {
    let Some(identifier) = read_identifier(arguments) else {
        return json!({ "answered": false, "refused": "a dimension names no legal identifier" });
    };
    surface.read_dimension(&identifier).map_or_else(
        || json!({ "answered": true, "present": false }),
        |report| {
            json!({
                "answered": true,
                "owner": report.owner,
                "energy": report.energy,
                "capacity": report.capacity,
                "probes": report.probes,
            })
        },
    )
}

fn publish_surface_dimension(
    surface: &mut Surface,
    arguments: &JsonValue,
    owner: ModId,
) -> JsonValue {
    let Some(identifier) = read_identifier(arguments) else {
        return json!({ "answered": false, "refused": "a dimension names no legal identifier" });
    };
    let (Some(energy), Some(capacity), Some(probes)) = (
        optional_field(arguments, "energy", 0, JsonValue::as_i64),
        optional_field(arguments, "capacity", 0, JsonValue::as_i64),
        optional_field(arguments, "probes", 0, read_u32),
    ) else {
        return json!({ "answered": false, "refused": "a dimension field has no legal value" });
    };
    service_outcome(surface.publish_dimension(
        &identifier,
        capabilities::DimensionReport {
            owner,
            energy,
            capacity,
            probes,
        },
    ))
}

fn register_surface_module(
    surface: &mut Surface,
    arguments: &JsonValue,
    owner: ModId,
) -> JsonValue {
    let Some(name) = arguments.get("name").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a module has no name" });
    };
    let Some(data) = arguments.get("data").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a module names no data" });
    };
    let (Some(energy_per_tick), Some(takes_clicks)) = (
        optional_field(arguments, "energy_per_tick", 0, read_u32),
        optional_field(arguments, "takes_clicks", false, JsonValue::as_bool),
    ) else {
        return json!({ "answered": false, "refused": "a module field has no legal value" });
    };
    service_outcome(surface.register_module(capabilities::Module {
        owner,
        name: name.to_owned(),
        data: data.to_owned(),
        energy_per_tick,
        takes_clicks,
    }))
}

fn read_identifier(arguments: &JsonValue) -> Option<ContentId> {
    arguments.get("identifier")?.as_str()?.parse().ok()
}

fn read_u32(value: &JsonValue) -> Option<u32> {
    u32::try_from(value.as_u64()?).ok()
}

fn optional_field<'a, T>(
    arguments: &'a JsonValue,
    name: &str,
    default: T,
    read: impl FnOnce(&'a JsonValue) -> Option<T>,
) -> Option<T> {
    arguments.get(name).map_or(Some(default), read)
}

fn run_surface_instruction(surface: &Surface, arguments: &JsonValue) -> JsonValue {
    let Some(name) = arguments.get("name").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a call names no instruction" });
    };
    let supplied: Vec<capabilities::Argument> = arguments
        .get("arguments")
        .and_then(JsonValue::as_array)
        .map_or_else(Vec::new, |values| {
            values.iter().map(read_argument).collect()
        });
    let mut program = capabilities::Program::default();
    match surface.run(&mut program, name, &supplied) {
        Ok(()) => json!({
            "answered": true,
            "last": write_argument(program.last.as_ref()),
            "ticket": program.ticket,
            "delay": program.delay,
        }),
        Err(refusal) => json!({ "answered": false, "refused": refusal.to_string() }),
    }
}

/// Answers the ordering service, which is how a dependent reaches §14.
pub(super) fn answer_ordering(ordering: &mut Ordering, arguments: &JsonValue) -> JsonValue {
    let Some(action) = arguments.get("action").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a call names no action" });
    };
    match action {
        "enqueue" => {
            let Some(dimension) =
                optional_field(arguments, "dimension", DEFAULT_DIMENSION, JsonValue::as_str)
            else {
                return json!({ "answered": false, "refused": "a call names no legal dimension" });
            };
            let Some(rank) = arguments
                .get("rank")
                .and_then(JsonValue::as_u64)
                .and_then(|rank| u8::try_from(rank).ok())
            else {
                return json!({ "answered": false, "refused": "a call names no legal rank" });
            };
            let Some(who) = optional_field(arguments, "who", "", JsonValue::as_str) else {
                return json!({ "answered": false, "refused": "a call names no legal requester" });
            };
            match ordering.enqueue(dimension, rank, who) {
                Ok(entry) => json!({ "answered": true, "stamp": entry.stamp }),
                Err(refusal) => {
                    json!({ "answered": false, "refused": refusal.to_string() })
                }
            }
        }
        "stamp" => json!({ "answered": true, "stamp": ordering.stamp() }),
        "queued" => {
            let Some(dimension) =
                optional_field(arguments, "dimension", DEFAULT_DIMENSION, JsonValue::as_str)
            else {
                return json!({ "answered": false, "refused": "a call names no legal dimension" });
            };
            json!({ "answered": true, "queued": ordering.queued(dimension) })
        }
        _ => json!({ "answered": false, "refused": "this entry publishes no such action" }),
    }
}

/// Reads one declared parameter kind out of a dependent's JSON.
fn read_kind(value: Option<&JsonValue>) -> Option<capabilities::ParameterKind> {
    match value.and_then(JsonValue::as_str)? {
        "amount" => Some(capabilities::ParameterKind::Amount),
        "text" => Some(capabilities::ParameterKind::Text),
        "flag" => Some(capabilities::ParameterKind::Flag),
        _ => None,
    }
}

/// Reads one supplied argument, where anything unreadable is the absent form.
///
/// Absent is a value §15 already carries, so a body that required the
/// parameter refuses on the count rather than this read guessing a number.
fn read_argument(value: &JsonValue) -> capabilities::Argument {
    if let Some(amount) = value.as_i64() {
        return capabilities::Argument::Amount(amount);
    }
    if let Some(flag) = value.as_bool() {
        return capabilities::Argument::Flag(flag);
    }
    value
        .as_str()
        .map_or(capabilities::Argument::Absent, |text| {
            capabilities::Argument::Text(text.to_owned())
        })
}

/// Writes one produced value back out to the caller.
fn write_argument(argument: Option<&capabilities::Argument>) -> JsonValue {
    match argument {
        Some(capabilities::Argument::Amount(amount)) => json!(amount),
        Some(capabilities::Argument::Text(text)) => json!(text),
        Some(capabilities::Argument::Flag(flag)) => json!(flag),
        Some(capabilities::Argument::Absent) | None => JsonValue::Null,
    }
}

/// Reads one instruction, body and all, out of a dependent's JSON.
///
/// §15 has the dependent supply the body, so every field here is untrusted:
/// a step naming no kind, or an index that is not a number, is refused by
/// name rather than dropped, and the surface's own body check still runs on
/// what this returns.
fn read_instruction(arguments: &JsonValue) -> Result<capabilities::Instruction, &'static str> {
    let name = arguments
        .get("name")
        .and_then(JsonValue::as_str)
        .ok_or("an instruction has no name")?;
    let output = read_kind(arguments.get("output")).ok_or("an instruction names no output kind")?;
    let mut parameters = Vec::new();
    for declared in arguments
        .get("parameters")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        parameters.push(capabilities::Parameter {
            kind: read_kind(declared.get("kind")).ok_or("a parameter names no legal kind")?,
            required: declared
                .get("required")
                .and_then(JsonValue::as_bool)
                .unwrap_or_default(),
        });
    }
    let mut body = Vec::new();
    for step in arguments
        .get("body")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        let index = || {
            step.get("index")
                .and_then(JsonValue::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .ok_or("a body step names no legal index")
        };
        body.push(match step.get("step").and_then(JsonValue::as_str) {
            Some("produce") => capabilities::Effect::Produce { index: index()? },
            Some("await") => capabilities::Effect::Await { index: index()? },
            Some("constant") => capabilities::Effect::Constant(read_argument(
                step.get("value").unwrap_or(&JsonValue::Null),
            )),
            Some("suspend") => capabilities::Effect::Suspend {
                ticks: step
                    .get("ticks")
                    .and_then(JsonValue::as_u64)
                    .and_then(|ticks| u32::try_from(ticks).ok())
                    .ok_or("a suspend step names no legal tick count")?,
            },
            _ => return Err("a body step is not one this entry runs"),
        });
    }
    let categories = arguments
        .get("categories")
        .and_then(JsonValue::as_array)
        .map_or_else(Vec::new, |listed| {
            listed
                .iter()
                .filter_map(|category| category.as_str().map(str::to_owned))
                .collect()
        });
    Ok(capabilities::Instruction {
        name: name.to_owned(),
        parameters,
        output,
        categories,
        body,
    })
}

fn service_outcome<E: std::fmt::Display>(outcome: Result<(), E>) -> JsonValue {
    match outcome {
        Ok(()) => json!({ "answered": true }),
        Err(refusal) => json!({ "answered": false, "refused": refusal.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_bearing_actions_refuse_absent_wrong_typed_and_malformed_owners() {
        for action in [
            "register_channel",
            "register_data_factory",
            "insert_rule",
            "register_module",
            "publish_dimension",
        ] {
            for owner in [
                None,
                Some(json!(null)),
                Some(json!(7)),
                Some(json!("")),
                Some(json!("bad:owner")),
            ] {
                let mut arguments = json!({
                    "action": action,
                    "identifier": "dependent:test",
                    "kind": "dependent:test",
                    "name": "test",
                    "data": "amount",
                    "rule": "test",
                });
                if let Some(owner) = owner {
                    arguments["owner"] = owner;
                }
                let mut surface = Surface::new();
                let answer = answer_surface(&mut surface, &arguments);
                assert_eq!(answer["answered"], json!(false), "{arguments}");
                assert_eq!(answer["refused"], json!("a call names no legal owner"));
            }
        }
    }

    #[test]
    fn an_empty_owner_cannot_admit_a_factory_in_an_empty_namespace() {
        let mut surface = Surface::new();
        for owner in [None, Some(json!(""))] {
            let mut arguments = json!({ "action": "register_data_factory", "kind": ":x" });
            if let Some(owner) = owner {
                arguments["owner"] = owner;
            }
            assert_eq!(
                answer_surface(&mut surface, &arguments)["answered"],
                json!(false)
            );
        }
        let module = json!({
            "action": "register_module", "owner": "dependent", "name": "test", "data": ":x",
        });
        assert_eq!(
            answer_surface(&mut surface, &module)["answered"],
            json!(false)
        );
        assert_eq!(surface.modules(), []);
    }

    #[test]
    fn factories_refuse_malformed_kinds_and_foreign_namespaces() {
        let mut surface = Surface::new();
        for kind in [
            json!(null),
            json!(7),
            json!(""),
            json!(":x"),
            json!("dependent:"),
            json!("elsewhere:x"),
        ] {
            let arguments = json!({
                "action": "register_data_factory", "owner": "dependent", "kind": kind,
            });
            assert_eq!(
                answer_surface(&mut surface, &arguments)["answered"],
                json!(false)
            );
            let module = json!({
                "action": "register_module", "owner": "dependent", "name": "test", "data": kind,
            });
            assert_eq!(
                answer_surface(&mut surface, &module)["answered"],
                json!(false)
            );
            assert_eq!(surface.modules(), []);
        }
    }

    #[test]
    fn malformed_and_foreign_channel_identifiers_leave_registrations_unchanged() {
        let mut surface = Surface::new();
        for identifier in [
            json!(null),
            json!(7),
            json!(""),
            json!(":x"),
            json!("elsewhere:x"),
        ] {
            let arguments = json!({
                "action": "register_channel", "owner": "dependent", "identifier": identifier,
            });
            assert_eq!(
                answer_surface(&mut surface, &arguments)["answered"],
                json!(false)
            );
            assert_eq!(surface.channels(), []);
        }
        assert_eq!(
            surface.register_channel(capabilities::ChannelKind {
                identifier: "elsewhere:x".parse().expect("a legal identifier"),
                owner: "dependent".parse().expect("a legal owner"),
            }),
            Err(capabilities::SurfaceRefused::ForeignNamespace)
        );
        assert_eq!(surface.channels(), []);
    }

    #[test]
    fn a_foreign_dimension_owner_cannot_replace_an_existing_report() {
        let mut surface = Surface::new();
        let identifier: ContentId = "dependent:vault".parse().expect("a legal identifier");
        let report = capabilities::DimensionReport {
            owner: "dependent".parse().expect("a legal owner"),
            energy: 12,
            capacity: 100,
            probes: 3,
        };
        surface
            .publish_dimension(&identifier, report.clone())
            .expect("its namespace");
        let replacement = capabilities::DimensionReport {
            owner: "elsewhere".parse().expect("a legal owner"),
            energy: 99,
            ..report
        };
        assert_eq!(
            surface.publish_dimension(&identifier, replacement),
            Err(capabilities::SurfaceRefused::ForeignNamespace)
        );
        for bad_identifier in [
            json!(null),
            json!(""),
            json!(":x"),
            json!("elsewhere:vault"),
        ] {
            let arguments = json!({
                "action": "publish_dimension", "owner": "dependent", "identifier": bad_identifier,
            });
            assert_eq!(
                answer_surface(&mut surface, &arguments)["answered"],
                json!(false)
            );
            assert_eq!(surface.read_dimension(&identifier), Some(&report));
        }
    }

    #[test]
    fn malformed_module_scalars_do_not_register_and_absent_scalars_default() {
        let mut surface = Surface::new();
        for (field, value) in [
            ("energy_per_tick", json!(null)),
            ("energy_per_tick", json!("1")),
            ("energy_per_tick", json!(-1)),
            ("energy_per_tick", json!(u64::from(u32::MAX) + 1)),
            ("takes_clicks", json!(null)),
            ("takes_clicks", json!(1)),
        ] {
            let mut arguments = json!({
                "action": "register_module", "owner": "dependent", "name": "test", "data": "amount",
            });
            arguments[field] = value;
            assert_eq!(
                answer_surface(&mut surface, &arguments)["answered"],
                json!(false)
            );
            assert_eq!(surface.modules(), []);
        }
        let arguments = json!({
            "action": "register_module", "owner": "dependent", "name": "test", "data": "amount",
        });
        assert_eq!(
            answer_surface(&mut surface, &arguments)["answered"],
            json!(true)
        );
        assert_eq!(surface.modules()[0].energy_per_tick, 0);
        assert!(!surface.modules()[0].takes_clicks);
    }

    #[test]
    fn malformed_dimension_scalars_preserve_the_previous_report() {
        let mut surface = Surface::new();
        let arguments = json!({
            "action": "publish_dimension", "owner": "dependent", "identifier": "dependent:vault",
        });
        assert_eq!(
            answer_surface(&mut surface, &arguments)["answered"],
            json!(true)
        );
        let identifier = "dependent:vault".parse().expect("a legal identifier");
        let previous = surface
            .read_dimension(&identifier)
            .expect("published")
            .clone();
        assert_eq!(
            (previous.energy, previous.capacity, previous.probes),
            (0, 0, 0)
        );
        for (field, value) in [
            ("energy", json!(null)),
            ("energy", json!("1")),
            ("energy", json!(u64::MAX)),
            ("capacity", json!(null)),
            ("capacity", json!(false)),
            ("capacity", json!(u64::MAX)),
            ("probes", json!(null)),
            ("probes", json!(-1)),
            ("probes", json!(u64::from(u32::MAX) + 1)),
        ] {
            let mut malformed = arguments.clone();
            malformed[field] = value;
            assert_eq!(
                answer_surface(&mut surface, &malformed)["answered"],
                json!(false)
            );
            assert_eq!(surface.read_dimension(&identifier), Some(&previous));
        }
    }

    #[test]
    fn malformed_ordering_fields_are_refused_and_absent_fields_default() {
        let mut ordering = Ordering::default();
        for arguments in [
            json!({ "action": "enqueue", "rank": 1, "dimension": 7 }),
            json!({ "action": "enqueue", "rank": 1, "who": false }),
            json!({ "action": "queued", "dimension": null }),
        ] {
            let response = answer_ordering(&mut ordering, &arguments);
            assert_eq!(response["answered"], json!(false));
            assert!(response["refused"].as_str().is_some());
            assert_eq!(ordering.queued(DEFAULT_DIMENSION), 0);
        }
        let response = answer_ordering(&mut ordering, &json!({ "action": "enqueue", "rank": 1 }));
        assert_eq!(response["answered"], json!(true));
        assert_eq!(
            answer_ordering(&mut ordering, &json!({ "action": "queued" }))["queued"],
            json!(1)
        );
    }
}
