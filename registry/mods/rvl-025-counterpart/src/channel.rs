//! JSON adapters for the published capability surface and ordering service.

use ravel_sdk::{JsonValue, json};

use crate::{DEFAULT_DIMENSION, Ordering, Surface, capabilities};

/// Answers the surface service, which is how a dependent reaches §15.
///
/// The arguments are another mod's JSON, so every field is read as untrusted
/// input: a call naming no action, or an action this entry does not publish,
/// is answered with a refusal rather than guessed at.
pub(super) fn answer_surface(surface: &mut Surface, arguments: &JsonValue) -> JsonValue {
    let Some(action) = arguments.get("action").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a call names no action" });
    };
    let owner = arguments
        .get("owner")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    match action {
        "register_channel" => {
            let Some(identifier) = arguments.get("identifier").and_then(JsonValue::as_str) else {
                return json!({ "answered": false, "refused": "a channel names no identifier" });
            };
            service_outcome(surface.register_channel(capabilities::ChannelKind {
                identifier: identifier.to_owned(),
                owner: owner.to_owned(),
            }))
        }
        "register_data_factory" => {
            let Some(kind) = arguments.get("kind").and_then(JsonValue::as_str) else {
                return json!({ "answered": false, "refused": "a factory names no kind" });
            };
            service_outcome(surface.register_data_factory(owner, kind))
        }
        "insert_rule" => {
            let Some(rule) = arguments.get("rule").and_then(JsonValue::as_str) else {
                return json!({ "answered": false, "refused": "a rule has no name" });
            };
            service_outcome(surface.insert_rule(owner, rule))
        }
        "read_dimension" => read_surface_dimension(surface, arguments),
        "register_module" => register_surface_module(surface, arguments, owner),
        "register_instruction" => match read_instruction(arguments) {
            Ok(instruction) => service_outcome(surface.register_instruction(instruction)),
            Err(refused) => json!({ "answered": false, "refused": refused }),
        },
        "run" => run_surface_instruction(surface, arguments),
        "publish_dimension" => publish_surface_dimension(surface, arguments, owner),
        _ => json!({ "answered": false, "refused": "this entry publishes no such action" }),
    }
}

fn read_surface_dimension(surface: &Surface, arguments: &JsonValue) -> JsonValue {
    let identifier = arguments
        .get("identifier")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    surface.read_dimension(identifier).map_or_else(
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
    owner: &str,
) -> JsonValue {
    let Some(identifier) = arguments.get("identifier").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a dimension has no identifier" });
    };
    service_outcome(
        surface.publish_dimension(
            identifier,
            capabilities::DimensionReport {
                owner: owner.to_owned(),
                energy: arguments
                    .get("energy")
                    .and_then(JsonValue::as_i64)
                    .unwrap_or_default(),
                capacity: arguments
                    .get("capacity")
                    .and_then(JsonValue::as_i64)
                    .unwrap_or_default(),
                probes: arguments
                    .get("probes")
                    .and_then(JsonValue::as_u64)
                    .and_then(|probes| u32::try_from(probes).ok())
                    .unwrap_or_default(),
            },
        ),
    )
}

fn register_surface_module(surface: &mut Surface, arguments: &JsonValue, owner: &str) -> JsonValue {
    let Some(name) = arguments.get("name").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a module has no name" });
    };
    let Some(data) = arguments.get("data").and_then(JsonValue::as_str) else {
        return json!({ "answered": false, "refused": "a module names no data" });
    };
    let energy_per_tick = arguments
        .get("energy_per_tick")
        .and_then(JsonValue::as_u64)
        .and_then(|cost| u32::try_from(cost).ok())
        .unwrap_or_default();
    service_outcome(
        surface.register_module(capabilities::Module {
            owner: owner.to_owned(),
            name: name.to_owned(),
            data: data.to_owned(),
            energy_per_tick,
            takes_clicks: arguments
                .get("takes_clicks")
                .and_then(JsonValue::as_bool)
                .unwrap_or_default(),
        }),
    )
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
            let dimension = arguments
                .get("dimension")
                .and_then(JsonValue::as_str)
                .unwrap_or(DEFAULT_DIMENSION);
            let Some(rank) = arguments
                .get("rank")
                .and_then(JsonValue::as_u64)
                .and_then(|rank| u8::try_from(rank).ok())
            else {
                return json!({ "answered": false, "refused": "a call names no legal rank" });
            };
            let who = arguments
                .get("who")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            match ordering.enqueue(dimension, rank, who) {
                Ok(entry) => json!({ "answered": true, "stamp": entry.stamp }),
                Err(refusal) => {
                    json!({ "answered": false, "refused": refusal.to_string() })
                }
            }
        }
        "stamp" => json!({ "answered": true, "stamp": ordering.stamp() }),
        "queued" => {
            let dimension = arguments
                .get("dimension")
                .and_then(JsonValue::as_str)
                .unwrap_or(DEFAULT_DIMENSION);
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
