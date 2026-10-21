//! Settings documents, edits, and JNI crossings.
//! Fields are length-prefixed in UTF-16 code units to match Java strings.
//! The store lock is independent of the runtime lock and released before JNI or disk I/O.
//! Integrated-server identities select the world runtime; both stores merge only edited
//! player values into the shared file under one write lock, cleaning rejected values.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicUsize, Ordering};

use jni::EnvUnowned;
use jni::objects::{JClass, JString};
use ravel_config::{
    LayeredConfiguration, ResolvedSetting, SettingKey, ValueDocument, ValueKind, ValueSchema,
    ValueSource, file,
};
use ravel_core::{DiagnosticMessage, ModId, Side};
use serde_json::Value;

use crate::guard::{hand_document, text};
use crate::wire::{Reader, count, field, flag};

static DOCUMENT_CROSSINGS: AtomicUsize = AtomicUsize::new(0);
static SAVE_CROSSINGS: AtomicUsize = AtomicUsize::new(0);

const WIRE_VERSION: &str = "2";

pub(crate) struct Store {
    side: Side,
    configurations: BTreeMap<ModId, LayeredConfiguration>,
}

impl Store {
    pub(crate) fn new(side: Side, configurations: Vec<LayeredConfiguration>) -> Self {
        Self {
            side,
            configurations: configurations
                .into_iter()
                .map(|configuration| (configuration.owner().clone(), configuration))
                .collect(),
        }
    }
}

/// Human-facing label only; never used to select a runtime.
pub(crate) const INTEGRATED_LABEL_SUFFIX: &str = " [integrated-server]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Process,
    IntegratedServer,
}

impl Target {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::IntegratedServer => "integrated-server",
        }
    }
}

pub(crate) struct Identity {
    target: Target,
    owner: ModId,
}

impl Identity {
    pub(crate) fn encode(target: Target, owner: &ModId) -> String {
        format!("{}:{owner}", target.as_str())
    }

    fn parse(encoded: &str) -> Option<Self> {
        let (target, owner) = encoded.split_once(':')?;
        let target = [Target::Process, Target::IntegratedServer]
            .into_iter()
            .find(|candidate| candidate.as_str() == target)?;
        Some(Self {
            target,
            owner: owner.parse().ok()?,
        })
    }

    fn matches(&self, side: Side) -> bool {
        (self.target == Target::IntegratedServer) == (side == Side::IntegratedServer)
    }
}

pub(crate) fn configurable(launch: &crate::runtime::LaunchContext) -> BTreeSet<ModId> {
    launch
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .configurable()
}

fn selected_launch<'a>(
    process: Option<&'a crate::runtime::LaunchContext>,
    integrated: Option<&'a crate::runtime::LaunchContext>,
    target: Target,
) -> Option<&'a crate::runtime::LaunchContext> {
    match target {
        Target::Process => process,
        Target::IntegratedServer => integrated,
    }
}

fn document(encoded: &str) -> Option<String> {
    let identity = Identity::parse(encoded)?;
    let integrated = crate::runtime::integrated_runtime();
    document_of(
        selected_launch(
            crate::runtime::installed().map(|runtime| &runtime.launch),
            integrated.as_ref().map(|runtime| &runtime.launch),
            identity.target,
        ),
        &identity,
    )
}

fn document_of(
    launch: Option<&crate::runtime::LaunchContext>,
    identity: &Identity,
) -> Option<String> {
    let launch = launch?;
    launch
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .document(identity)
}

impl Store {
    fn configurable(&self) -> BTreeSet<ModId> {
        self.configurations
            .iter()
            .filter(|(_, configuration)| !configuration.settings_on(self.side).is_empty())
            .map(|(owner, _)| owner.clone())
            .collect()
    }

    fn document(&self, identity: &Identity) -> Option<String> {
        if !identity.matches(self.side) {
            return None;
        }
        let configuration = self.configurations.get(&identity.owner)?;
        let rows = configuration.settings_on(self.side);

        let mut out = String::with_capacity(rows.len() * 256 + 64);
        field(&mut out, WIRE_VERSION);
        field(&mut out, identity.owner.as_str());
        field(&mut out, self.side.as_str());
        count(&mut out, rows.len());
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
    flag(out, resolved.editable);
    flag(out, setting.restart_required);
    flag(out, pending(configuration, resolved));
    field(out, &bound(schema.minimum));
    field(out, &bound(schema.maximum));

    let choices = schema.permitted.as_deref().unwrap_or_default();
    count(out, choices.len());
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

fn save(payload: &str) -> String {
    let integrated = crate::runtime::integrated_runtime();
    save_in(
        crate::runtime::installed().map(|runtime| &runtime.launch),
        integrated.as_ref().map(|runtime| &runtime.launch),
        payload,
    )
}

fn save_in(
    process: Option<&crate::runtime::LaunchContext>,
    integrated: Option<&crate::runtime::LaunchContext>,
    payload: &str,
) -> String {
    let request = match EditRequest::parse(payload) {
        Ok(request) => request,
        Err(reason) => return refusal("", &reason),
    };
    save_with(
        selected_launch(process, integrated, request.identity.target),
        request,
    )
}

fn save_with(launch: Option<&crate::runtime::LaunchContext>, request: EditRequest) -> String {
    let Some(launch) = launch else {
        return refusal("", "the loader runtime is not installed");
    };
    // Release the store lock before disk I/O.
    let applied = launch
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .apply(request);
    persist_and_answer(applied, &launch.game_directory)
}

struct Applied {
    refusal_count: usize,
    encoded_refusals: String,
    document: ValueDocument,
    edited: BTreeSet<String>,
    rejected: BTreeMap<SettingKey, Option<ValueSchema>>,
}

struct EditRequest {
    identity: Identity,
    edits: Vec<(String, String)>,
}

impl EditRequest {
    fn parse(payload: &str) -> Result<Self, String> {
        let mut reader = Reader::new(payload);
        let version = reader.field().ok_or("the settings payload is truncated")?;
        if version != WIRE_VERSION {
            return Err(format!(
                "the screen spoke unsupported wire version `{version}`"
            ));
        }
        let encoded = reader.field().ok_or("the settings payload is truncated")?;
        let identity =
            Identity::parse(&encoded).ok_or("the settings target or mod identifier is invalid")?;
        let count = reader
            .count()
            .ok_or("the settings edit count could not be read")?;

        // Parse all edits before mutation to reject truncated payloads atomically.
        let mut edits = Vec::new();
        for _ in 0..count {
            edits.push((
                reader.field().ok_or("the settings payload is truncated")?,
                reader.field().ok_or("the settings payload is truncated")?,
            ));
        }
        if !reader.finished() {
            return Err("the settings payload contains trailing data".to_owned());
        }
        Ok(Self { identity, edits })
    }
}

impl Store {
    fn apply(&mut self, request: EditRequest) -> Result<Applied, String> {
        let EditRequest { identity, edits } = request;
        if !identity.matches(self.side) {
            return Err("the settings target does not match this runtime".to_owned());
        }
        let owner = identity.owner;
        let side = self.side;
        let configuration = self
            .configurations
            .get_mut(&owner)
            .ok_or_else(|| format!("no settings are declared for `{owner}`"))?;

        let mut refused = String::new();
        let mut refusals = 0_usize;
        let mut edited = BTreeSet::new();
        for (key, typed) in edits {
            let kind = configuration
                .declaration()
                .setting(&key)
                .and_then(|setting| setting.value_schema.kind);
            if let Err(error) = configuration.set(&key, value_of(kind, &typed), side) {
                refusals += 1;
                write_refusal(&mut refused, &key, &error.to_string());
            } else {
                edited.insert(key);
            }
        }
        Ok(Applied {
            refusal_count: refusals,
            encoded_refusals: refused,
            document: configuration.player_document(),
            edited,
            rejected: rejected_schemas(configuration),
        })
    }
}

fn rejected_schemas(
    configuration: &LayeredConfiguration,
) -> BTreeMap<SettingKey, Option<ValueSchema>> {
    configuration
        .rejected()
        .iter()
        .map(|rejected| {
            let schema = configuration
                .declaration()
                .setting(rejected.key.as_str())
                .map(|setting| setting.value_schema.clone());
            (rejected.key.clone(), schema)
        })
        .collect()
}

fn persist_and_answer(applied: Result<Applied, String>, directory: &Path) -> String {
    let Applied {
        mut refusal_count,
        mut encoded_refusals,
        document,
        edited,
        rejected,
    } = match applied {
        Ok(applied) => applied,
        Err(reason) => return refusal("", &reason),
    };

    if let Err(reason) = persist(directory, document, &edited, &rejected) {
        refusal_count += 1;
        write_refusal(
            &mut encoded_refusals,
            "",
            DiagnosticMessage::redacted(&reason).text(),
        );
    }

    let mut out = String::with_capacity(encoded_refusals.len() + 8);
    count(&mut out, refusal_count);
    out.push_str(&encoded_refusals);
    out
}

// Persistence failure leaves edits active in memory but absent from the next launch.
fn persist(
    directory: &Path,
    mut document: ValueDocument,
    edited: &BTreeSet<String>,
    rejected: &BTreeMap<SettingKey, Option<ValueSchema>>,
) -> Result<(), String> {
    static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _writing = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    admit_client_directory(directory)?;
    if let Some(mut latest) =
        file::read(directory, &document.owner).map_err(|error| error.to_string())?
    {
        document
            .values
            .retain(|key, _| edited.contains(key.as_str()));
        // Preserve newer values, including valid replacements for launch-time rejections.
        latest.values.retain(|key, value| {
            rejected.get(key).is_none_or(|schema| {
                schema
                    .as_ref()
                    .is_some_and(|schema| schema.check(value).is_ok())
            })
        });
        latest.values.extend(document.values);
        document.values = latest.values;
    }
    file::write(directory, &document).map_err(|error| error.to_string())
}

const CLIENT_MARKER: &str = "options.txt";

fn admit_client_directory(directory: &Path) -> Result<(), String> {
    if directory.join(CLIENT_MARKER).is_file() {
        return Ok(());
    }
    Err(format!(
        "`{}` holds no `{CLIENT_MARKER}` and so cannot be confirmed as the client game directory",
        directory.display()
    ))
}

fn refusal(key: &str, reason: &str) -> String {
    let mut out = String::new();
    count(&mut out, 1);
    write_refusal(&mut out, key, reason);
    out
}

fn write_refusal(out: &mut String, key: &str, reason: &str) {
    field(out, key);
    field(out, reason);
}

/// Returns one owner's settings document.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSettingsDocument")]
pub fn ravel_settings_document<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
    owner: JString<'local>,
) -> JString<'local> {
    DOCUMENT_CROSSINGS.fetch_add(1, Ordering::Relaxed);
    hand_document(env, |env| Ok(document(&text(env, &owner)?)))
}

/// Applies settings edits and returns refusals.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSaveSettings")]
pub fn ravel_save_settings<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
    payload: JString<'local>,
) -> JString<'local> {
    SAVE_CROSSINGS.fetch_add(1, Ordering::Relaxed);
    hand_document(env, |env| Ok(Some(save(&text(env, &payload)?))))
}

/// Reports document and save crossings for process-harness verification.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSettingsCrossings")]
pub fn ravel_settings_crossings<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    hand_document(env, |_| {
        Ok(Some(format!(
            "{}:{}",
            DOCUMENT_CROSSINGS.load(Ordering::Relaxed),
            SAVE_CROSSINGS.load(Ordering::Relaxed)
        )))
    })
}

#[cfg(test)]
mod tests;
