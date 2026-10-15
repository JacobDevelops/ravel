//! Settings documents, edits, and JNI crossings.
//! Fields are length-prefixed in UTF-16 code units to match Java strings.
//! The store lock is independent of the runtime lock and released before JNI or disk I/O.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::PoisonError;
use std::sync::atomic::{AtomicUsize, Ordering};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};
use ravel_config::{
    LayeredConfiguration, ResolvedSetting, SettingKey, ValueDocument, ValueKind, ValueSource, file,
};
use ravel_core::{ModId, Side};
use ravel_observability::redact;
use serde_json::Value;

use crate::guard::{HookError, text};
use crate::wire::{Reader, VERSION as WIRE_VERSION, field};

static DOCUMENT_CROSSINGS: AtomicUsize = AtomicUsize::new(0);
static SAVE_CROSSINGS: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Store {
    side: Side,
    configurations: BTreeMap<ModId, LayeredConfiguration>,
    game_directory: Result<PathBuf, String>,
}

impl Store {
    pub(crate) fn new(
        side: Side,
        configurations: Vec<LayeredConfiguration>,
        game_directory: Result<PathBuf, String>,
    ) -> Self {
        Self {
            side,
            configurations: configurations
                .into_iter()
                .map(|configuration| (configuration.owner().clone(), configuration))
                .collect(),
            game_directory,
        }
    }
}

pub(crate) fn configurable() -> BTreeSet<ModId> {
    let Some(launch) = crate::runtime::installed().and_then(|runtime| runtime.launch.as_ref())
    else {
        return BTreeSet::new();
    };
    launch
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .configurable()
}

fn document(owner: &str) -> Option<String> {
    let launch = crate::runtime::installed()?.launch.as_ref()?;
    launch
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .document(owner)
}

impl Store {
    fn configurable(&self) -> BTreeSet<ModId> {
        self.configurations
            .iter()
            .filter(|(_, configuration)| !configuration.settings_on(self.side).is_empty())
            .map(|(owner, _)| owner.clone())
            .collect()
    }

    fn document(&self, owner: &str) -> Option<String> {
        let owner = owner.parse::<ModId>().ok()?;
        let configuration = self.configurations.get(&owner)?;
        let rows = configuration.settings_on(self.side);

        let mut out = String::with_capacity(rows.len() * 256 + 64);
        field(&mut out, WIRE_VERSION);
        field(&mut out, owner.as_str());
        field(&mut out, side_name(self.side));
        field(&mut out, &rows.len().to_string());
        for resolved in &rows {
            row(&mut out, configuration, resolved);
        }
        Some(out)
    }
}

fn row(out: &mut String, configuration: &LayeredConfiguration, resolved: &ResolvedSetting<'_>) {
    let setting = resolved.setting;
    let schema = &setting.value_schema;

    field(out, setting.key.as_str());
    field(
        out,
        setting
            .title
            .as_deref()
            .unwrap_or_else(|| setting.key.as_str()),
    );
    field(out, setting.description.as_deref().unwrap_or(""));
    field(out, widget(resolved));
    field(out, schema.kind.map_or("", ValueKind::as_str));
    field(out, &text_of(resolved.value));
    field(out, source_name(resolved.source));
    field(out, &resolved.editable.to_string());
    field(out, &setting.restart_required.to_string());
    field(out, &pending(configuration, resolved).to_string());
    field(out, &bound(schema.minimum));
    field(out, &bound(schema.maximum));

    let choices = schema.permitted.as_deref().unwrap_or_default();
    field(out, &choices.len().to_string());
    for choice in choices {
        field(out, &text_of(choice));
    }
}

fn bound(value: Option<f64>) -> String {
    value.map(|at| at.to_string()).unwrap_or_default()
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other @ (Value::Null
        | Value::Bool(_)
        | Value::Number(_)
        | Value::Array(_)
        | Value::Object(_)) => other.to_string(),
    }
}

// Preserve invalid typed input as text so schema validation reports the refusal.
fn value_of(kind: Option<ValueKind>, typed: &str) -> Value {
    let parsed = match kind {
        Some(ValueKind::Boolean) => typed.parse::<bool>().ok().map(Value::Bool),
        Some(ValueKind::Integer | ValueKind::Number) => serde_json::from_str::<Value>(typed)
            .ok()
            .filter(Value::is_number),
        Some(ValueKind::String) | None => None,
    };
    parsed.unwrap_or_else(|| Value::String(typed.to_owned()))
}

fn widget(resolved: &ResolvedSetting<'_>) -> &'static str {
    let schema = &resolved.setting.value_schema;
    if schema.permitted.is_some() {
        return "choice";
    }
    match schema.kind {
        Some(ValueKind::Boolean) => "toggle",
        Some(ValueKind::Integer | ValueKind::Number)
            if schema.minimum.is_some() && schema.maximum.is_some() =>
        {
            "slider"
        }
        Some(ValueKind::Integer | ValueKind::Number | ValueKind::String) | None => "text",
    }
}

fn pending(configuration: &LayeredConfiguration, resolved: &ResolvedSetting<'_>) -> bool {
    configuration
        .player_document()
        .values
        .get(&resolved.setting.key)
        .is_some_and(|written| written != resolved.value)
}

const fn source_name(source: ValueSource) -> &'static str {
    match source {
        ValueSource::Default => "default",
        ValueSource::Pack => "pack",
        ValueSource::Player => "player",
    }
}

const fn side_name(side: Side) -> &'static str {
    match side {
        Side::Client => "client",
        Side::IntegratedServer => "integrated-server",
        Side::DedicatedServer => "dedicated-server",
    }
}

fn save(payload: &str) -> String {
    // Release the store lock before disk I/O.
    let applied = crate::runtime::installed()
        .and_then(|runtime| runtime.launch.as_ref())
        .and_then(|launch| {
            launch
                .settings
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .apply(&mut Reader::new(payload))
        });
    answer(applied)
}

struct Applied {
    refusals: usize,
    refused: String,
    document: ValueDocument,
    directory: Result<PathBuf, String>,
}

impl Store {
    fn apply(&mut self, reader: &mut Reader) -> Option<Applied> {
        if reader.field()? != WIRE_VERSION {
            return None;
        }
        let owner = reader.field()?.parse::<ModId>().ok()?;
        let count = reader.count()?;

        // Parse all edits before mutation to reject truncated payloads atomically.
        let mut edits = Vec::new();
        for _ in 0..count {
            edits.push((reader.field()?, reader.field()?));
        }
        if !reader.finished() {
            return None;
        }

        let side = self.side;
        let directory = self.game_directory.clone();
        let configuration = self.configurations.get_mut(&owner)?;

        let mut refused = String::new();
        let mut refusals = 0_usize;
        for (key, typed) in edits {
            let Ok(parsed) = key.parse::<SettingKey>() else {
                refusals += 1;
                entry(&mut refused, &key, "that is not a setting name");
                continue;
            };
            let kind = configuration
                .declaration()
                .setting(&parsed)
                .and_then(|setting| setting.value_schema.kind);
            if let Err(error) = configuration.set(&parsed, value_of(kind, &typed), side) {
                refusals += 1;
                entry(&mut refused, parsed.as_str(), &error.to_string());
            }
        }

        Some(Applied {
            refusals,
            refused,
            document: configuration.player_document(),
            directory,
        })
    }
}

fn answer(applied: Option<Applied>) -> String {
    let Some(Applied {
        mut refusals,
        mut refused,
        document,
        directory,
    }) = applied
    else {
        return refusal("", "the settings the screen sent could not be read");
    };

    if let Err(reason) = persist(directory.as_deref(), &document) {
        refusals += 1;
        entry(&mut refused, "", redact(&reason).text());
    }

    let mut out = String::with_capacity(refused.len() + 8);
    field(&mut out, &refusals.to_string());
    out.push_str(&refused);
    out
}

// Persistence failure leaves edits active in memory but absent from the next launch.
fn persist(directory: Result<&Path, &String>, document: &ValueDocument) -> Result<(), String> {
    let directory = directory.map_err(Clone::clone)?;
    crate::game_directory::admit(directory)?;
    file::write(directory, document).map_err(|error| error.to_string())
}

fn refusal(key: &str, reason: &str) -> String {
    let mut out = String::new();
    field(&mut out, "1");
    entry(&mut out, key, reason);
    out
}

fn entry(out: &mut String, key: &str, reason: &str) {
    field(out, key);
    field(out, reason);
}

/// Returns one owner's settings document.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSettingsDocument")]
pub fn ravel_settings_document<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    owner: JString<'local>,
) -> JString<'local> {
    DOCUMENT_CROSSINGS.fetch_add(1, Ordering::Relaxed);
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let owner = text(env, &owner)?;
            match document(&owner) {
                Some(document) => Ok(env.new_string(document)?),
                None => Ok(JString::default()),
            }
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Applies settings edits and returns refusals.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSaveSettings")]
pub fn ravel_save_settings<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    payload: JString<'local>,
) -> JString<'local> {
    SAVE_CROSSINGS.fetch_add(1, Ordering::Relaxed);
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let answer = save(&text(env, &payload)?);
            Ok(env.new_string(answer)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Reports document and save crossings for process-harness verification.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSettingsCrossings")]
pub fn ravel_settings_crossings<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let crossings = format!(
                "{}:{}",
                DOCUMENT_CROSSINGS.load(Ordering::Relaxed),
                SAVE_CROSSINGS.load(Ordering::Relaxed)
            );
            Ok(env.new_string(crossings)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests;
