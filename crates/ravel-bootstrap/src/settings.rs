//! The settings screen's two crossings, and the configurations they read.
//!
//! ADG-6 declares configuration in data, draws the screen in Java, and budgets
//! "two boundary crossings per screen session, both batched": opening reads one
//! owner's declaration and current values, saving hands back what the player
//! changed. This module is both of them and the state they read.
//!
//! # Why this is not held with the mods
//!
//! [`crate::LoaderRuntime`] holds the machine and the mod set behind its own
//! [`Mutex`], and the paragraph that makes that lock safe is an argument about
//! *mod code*: a mod cannot re-enter the runtime because it has no way to call
//! into the JVM at all. A screen crossing is a different caller entirely, and
//! nothing in that argument covers it. Holding configurations behind the same
//! lock would mean a screen opening during a tick contends with the tick, and it
//! would extend an invariant onto a caller it was never argued for.
//!
//! So the store is separate, and the two locks are never held at the same time.
//! The lock here is taken to build a string and released before the string
//! crosses, which is what keeps the screen from holding anything while it draws.
//!
//! # The wire
//!
//! Every field is `<length>:<that many UTF-16 code units>`, and a payload is a
//! sequence of fields in a fixed order. Counts are fields that read back as
//! numbers.
//!
//! This is length-prefixed rather than delimited or JSON, and the reason is the
//! reader on the other side. A setting's title, description and value are
//! free-form text a mod author writes, so any delimiter can appear inside one and
//! a delimited format would need escaping. JSON would too, and the screen cannot
//! borrow the game's parser for it: the screen compiles against stubs generated
//! from the pinned client jar, that jar carries the game's own classes rather
//! than the libraries they compile against, and it has no `com.google.gson`
//! entry — so JSON would mean shipping a hand-written parser inside the screen
//! jar and being right about escapes and surrogate pairs forever. A length says
//! where a field ends, so nothing inside it can be misread and there is nothing
//! to escape at all. UTF-16 is the unit because that is what a Java `String`
//! counts in and what crosses the JNI boundary.
//!
//! Values travel as text and are typed on this side from the owner's own
//! declaration. The screen never decides what type a value is: it sends what the
//! player produced, and the setting's `valueSchema` says what that means. Text
//! that will not read as the declared kind stays text, which the schema then
//! refuses — and the refusal is reported rather than swallowed.
//!
//! # What crosses
//!
//! Data, in both directions, and nothing else. There is no handle in the payload
//! and no game object on either side of it, so ADG-4 has nothing to say here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};
use ravel_config::{
    LayeredConfiguration, ResolvedSetting, SettingKey, ValueDocument, ValueKind, ValueSource, file,
};
use ravel_core::{ModId, Side};
use ravel_observability::redact;
use serde_json::Value;

use crate::game_directory;
use crate::guard::{HookError, text};
use crate::wire::{VERSION as WIRE_VERSION, field};

/// One process's resolved configurations, or nothing before they are installed.
static STORE: Mutex<Option<Store>> = Mutex::new(None);

/// What the screen crossings read.
struct Store {
    /// The side this process runs as, which decides what a screen may show.
    side: Side,
    configurations: BTreeMap<ModId, LayeredConfiguration>,
    /// A directory stated by a test, or `None` for production resolution at save.
    ///
    /// Premain installs this store before the client creates `options.txt`. The
    /// marker exists by the time a settings screen can close, so production must
    /// not cache that expected early refusal.
    game_directory: Option<Result<PathBuf, String>>,
}

/// Installs the configurations a screen may read and edit.
///
/// `side` is the process's own, and it decides what a screen holds rather than
/// what it greys out: ADG-6 says a `dedicated-server` setting has no client
/// screen, so on a client such a setting is absent rather than shown disabled.
///
/// Resolution is the caller's. Reaching a [`LayeredConfiguration`] means reading
/// the pack's override document and the player's own off disk, which is the
/// pipeline's work; this crate is the JVM entry and receives configurations
/// already resolved, the same way it receives a machine already driven.
pub fn install(side: Side, configurations: Vec<LayeredConfiguration>) {
    install_in(side, configurations, None);
}

/// Installs configurations against a stated game directory.
///
/// Separate from [`install`] so the directory is a value rather than a syscall
/// at the point it is used, which is what lets a test state one.
fn install_in(
    side: Side,
    configurations: Vec<LayeredConfiguration>,
    game_directory: Option<Result<PathBuf, String>>,
) {
    let installed = configurations
        .into_iter()
        .map(|configuration| (configuration.owner().clone(), configuration))
        .collect();
    *lock() = Some(Store {
        side,
        configurations: installed,
        game_directory,
    });
}

/// Takes the store, recovering it from a previous panic.
///
/// A panic here can only have unwound out of building a document or applying an
/// edit. Neither leaves a [`LayeredConfiguration`] half-written: rendering only
/// reads, and [`LayeredConfiguration::set`] validates before it writes anything.
/// Refusing the lock afterwards would turn one bad payload into a settings
/// screen that is blank for the rest of the run.
fn lock() -> std::sync::MutexGuard<'static, Option<Store>> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reads a payload one length-prefixed field at a time.
///
/// The units are UTF-16 because that is what the string crossing the boundary
/// counts in. A length that runs past the end reads as nothing rather than as
/// much as there is: a payload claiming a field longer than itself is one this
/// side did not produce, and reading it as far as it goes would be inventing
/// data. Nothing here allocates from a length before checking it against what is
/// actually there.
struct Reader {
    units: Vec<u16>,
    at: usize,
}

impl Reader {
    fn new(payload: &str) -> Self {
        Self {
            units: payload.encode_utf16().collect(),
            at: 0,
        }
    }

    fn field(&mut self) -> Option<String> {
        let separator = self.at
            + self.units[self.at..]
                .iter()
                .position(|unit| *unit == u16::from(b':'))?;
        let length: usize = String::from_utf16(&self.units[self.at..separator])
            .ok()?
            .parse()
            .ok()?;
        let start = separator + 1;
        let end = start.checked_add(length)?;
        if end > self.units.len() {
            return None;
        }
        self.at = end;
        String::from_utf16(&self.units[start..end]).ok()
    }

    fn count(&mut self) -> Option<usize> {
        self.field()?.parse().ok()
    }

    /// Whether the payload ended exactly where its fields did.
    const fn finished(&self) -> bool {
        self.at == self.units.len()
    }
}

/// Renders one owner's settings as the document the screen draws from.
///
/// Returns `None` for an owner this process has no configuration for, which is
/// also what a screen opened before anything was installed sees. A screen with
/// no document draws no rows; that is a mod with nothing to configure, not a
/// failure worth throwing into the game.
fn document(owner: &str) -> Option<String> {
    let owner = owner.parse::<ModId>().ok()?;
    let store = lock();
    let store = store.as_ref()?;
    let configuration = store.configurations.get(&owner)?;
    let rows = configuration.settings_on(store.side);

    let mut out = String::with_capacity(rows.len() * 256 + 64);
    field(&mut out, WIRE_VERSION);
    field(&mut out, owner.as_str());
    field(&mut out, side_name(store.side));
    field(&mut out, &rows.len().to_string());
    for resolved in &rows {
        row(&mut out, configuration, resolved);
    }
    Some(out)
}

/// Writes one resolved setting as the row a widget is built from.
fn row(out: &mut String, configuration: &LayeredConfiguration, resolved: &ResolvedSetting<'_>) {
    let setting = resolved.setting;
    let schema = &setting.value_schema;

    field(out, setting.key.as_str());
    // A setting with no title shows its key. `ravel-config` makes the title
    // optional, defines no fallback and no localisation contract, and inventing
    // one in a payload would put that contract in the wrong crate.
    field(
        out,
        setting
            .title
            .as_deref()
            .unwrap_or_else(|| setting.key.as_str()),
    );
    field(out, setting.description.as_deref().unwrap_or(""));
    field(out, widget(resolved));
    field(out, schema.kind.map_or("", ValueKind::name));
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

/// A bound as the screen reads it, or nothing when the schema states none.
fn bound(value: Option<f64>) -> String {
    value.map(|at| at.to_string()).unwrap_or_default()
}

/// One value as the screen shows and returns it.
///
/// A string is itself. Everything else is written the way JSON writes it, which
/// is what [`value_of`] reads back.
fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// One returned value, typed by what the owner declared rather than by the
/// screen.
///
/// Text that will not read as the declared kind stays text. The schema then
/// refuses it, which is the outcome ADG-6 asks for — the edit fails and the
/// previous value stands — and the refusal is reported rather than swallowed.
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

/// Names the widget one setting is drawn with.
///
/// A closed choice is a choice however it is typed. A bounded number is the only
/// thing a slider can honestly draw, because a slider with no end has none to
/// draw to. Everything else is a text field, including a schema that states no
/// kind at all: it constrains nothing, so the widest widget is the honest one,
/// and the edit path already refuses an illegal value and leaves the previous
/// one standing.
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
        _ => "text",
    }
}

/// Whether this setting has an edit written and waiting for the next launch.
///
/// `ravel-config` reports the value in force per setting and a restart flag for
/// the whole owner, so a per-row answer is the player's written document read
/// against the value the screen is about to show. An edit that is in force
/// agrees with it; one waiting for a launch does not.
fn pending(configuration: &LayeredConfiguration, resolved: &ResolvedSetting<'_>) -> bool {
    configuration
        .player_document()
        .values
        .get(&resolved.setting.key)
        .is_some_and(|written| written != resolved.value)
}

/// Names the layer a value came from.
const fn source_name(source: ValueSource) -> &'static str {
    match source {
        ValueSource::Default => "default",
        ValueSource::Pack => "pack",
        ValueSource::Player => "player",
    }
}

/// Names the side a screen is running on.
const fn side_name(side: Side) -> &'static str {
    match side {
        Side::Client => "client",
        Side::IntegratedServer => "integrated-server",
        Side::DedicatedServer => "dedicated-server",
    }
}

/// Records what the player changed, and answers what was refused.
///
/// Every refusal ADG-6 names leaves the previous value in place, and the screen
/// has to be able to *say* so rather than let a player watch an edit disappear
/// between one open and the next. So the answer carries the key and the reason
/// for each one, and an empty answer means every edit landed.
///
/// A payload for an owner this process does not hold is dropped whole, and so is
/// a malformed one: a screen that sends either has a bug that applying it would
/// not fix. Both answer with one refusal naming the payload rather than silently
/// doing nothing.
fn save(payload: &str) -> String {
    apply(&mut Reader::new(payload))
        .unwrap_or_else(|| refusal("", "the settings the screen sent could not be read"))
}

/// Applies one save payload, or reports that it could not be read.
fn apply(reader: &mut Reader) -> Option<String> {
    if reader.field()? != WIRE_VERSION {
        return None;
    }
    let owner = reader.field()?.parse::<ModId>().ok()?;
    let count = reader.count()?;

    // Read every edit before applying any, so a payload that runs out halfway
    // leaves nothing half-applied.
    let mut edits = Vec::new();
    for _ in 0..count {
        edits.push((reader.field()?, reader.field()?));
    }
    if !reader.finished() {
        return None;
    }

    let mut guard = lock();
    let store = guard.as_mut()?;
    let side = store.side;
    let configuration = store.configurations.get_mut(&owner)?;

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

    // Persist outside the lock. The store is taken to apply edits and released
    // before anything touches the disk, so a screen closing never holds it
    // across a syscall and the claim this module opens with stays true.
    let document = configuration.player_document().clone();
    let directory = store.game_directory.clone();
    drop(guard);

    let directory = directory.unwrap_or_else(game_directory::resolve);
    if let Err(reason) = persist(directory.as_deref(), &document) {
        refusals += 1;
        entry(&mut refused, "", redact(&reason).text());
    }

    let mut out = String::with_capacity(refused.len() + 8);
    field(&mut out, &refusals.to_string());
    out.push_str(&refused);
    Some(out)
}

/// Writes the player's document where the next launch will read it.
///
/// This runs after the edits are applied in memory, so a failure here means the
/// player's change is in force for this run and gone at the next one. That is
/// worth a refusal rather than silence: it is the same thing a refused value is,
/// an edit the player will not find where they left it, and ADG-6's whole reason
/// for answering the save crossing is that the screen can say so.
fn persist(directory: Result<&Path, &String>, document: &ValueDocument) -> Result<(), String> {
    let directory = directory.map_err(Clone::clone)?;
    file::write(directory, document).map_err(|error| error.to_string())
}

/// One refusal, as the only entry in an answer.
fn refusal(key: &str, reason: &str) -> String {
    let mut out = String::new();
    field(&mut out, "1");
    entry(&mut out, key, reason);
    out
}

/// Writes one refused key and the reason it was refused.
fn entry(out: &mut String, key: &str, reason: &str) {
    field(out, key);
    field(out, reason);
}

/// Hands the screen one owner's settings.
///
/// # Panics
///
/// Does not panic. The guard around the body turns a panic into a thrown
/// `RuntimeException` rather than an unwind into the JVM.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSettingsDocument")]
pub fn ravel_settings_document<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    owner: JString<'local>,
) -> JString<'local> {
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

/// Takes back what the player changed and answers what was refused.
///
/// # Panics
///
/// Does not panic, for the same reason as [`ravel_settings_document`].
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSaveSettings")]
pub fn ravel_save_settings<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    payload: JString<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            let answer = save(&text(env, &payload)?);
            Ok(env.new_string(answer)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
    use ravel_core::Side;
    use serde_json::json;

    use super::{ModId, Reader, STORE, document, field, file, install_in, save};

    /// One declaration covering every widget the payload can name.
    fn declaration() -> ConfigurationDeclaration {
        ConfigurationDeclaration::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "example_mod",
              "version": 1,
              "settings": [
                {
                  "key": "render.distance",
                  "title": "Render distance",
                  "description": "How far chunks are drawn.",
                  "valueSchema": { "type": "integer", "minimum": 2, "maximum": 32 },
                  "default": 12,
                  "side": "client",
                  "restartRequired": false
                },
                {
                  "key": "render.fancy",
                  "valueSchema": { "type": "boolean" },
                  "default": true,
                  "side": "client",
                  "restartRequired": false
                },
                {
                  "key": "render.quality",
                  "valueSchema": { "enum": ["fast", "fancy"] },
                  "default": "fast",
                  "side": "client",
                  "restartRequired": true
                },
                {
                  "key": "render.name",
                  "valueSchema": {},
                  "default": "anything",
                  "side": "client",
                  "restartRequired": false
                },
                {
                  "key": "server.motd",
                  "valueSchema": { "type": "string" },
                  "default": "hello",
                  "side": "dedicated-server",
                  "restartRequired": false
                }
              ]
            }"#,
        )
        .expect("the test declaration should parse")
    }

    /// One row as the screen reads it back.
    #[derive(Clone, Debug)]
    struct Row {
        key: String,
        label: String,
        description: String,
        widget: String,
        kind: String,
        value: String,
        source: String,
        editable: String,
        restart_required: String,
        restart_pending: String,
        minimum: String,
        maximum: String,
        choices: Vec<String>,
    }

    /// Reads a document back the way the screen does, checking it ends cleanly.
    fn rows(document: &str) -> Vec<Row> {
        let mut reader = Reader::new(document);
        assert_eq!(reader.field().expect("a version"), "1");
        assert_eq!(reader.field().expect("an owner"), "example_mod");
        assert_eq!(reader.field().expect("a side"), "client");

        let count = reader.count().expect("a row count");
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            let mut row = Row {
                key: reader.field().expect("a key"),
                label: reader.field().expect("a label"),
                description: reader.field().expect("a description"),
                widget: reader.field().expect("a widget"),
                kind: reader.field().expect("a kind"),
                value: reader.field().expect("a value"),
                source: reader.field().expect("a source"),
                editable: reader.field().expect("an editable flag"),
                restart_required: reader.field().expect("a restart flag"),
                restart_pending: reader.field().expect("a pending flag"),
                minimum: reader.field().expect("a minimum"),
                maximum: reader.field().expect("a maximum"),
                choices: Vec::new(),
            };
            let choices = reader.count().expect("a choice count");
            for _ in 0..choices {
                row.choices.push(reader.field().expect("a choice"));
            }
            rows.push(row);
        }
        assert!(reader.finished(), "the document ends where its fields do");
        rows
    }

    /// A directory this test may treat as the game's.
    ///
    /// One per process, which is one per test: the gate runs each test in its
    /// own process, so a saved file cannot be another test's.
    fn game() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!("ravel-settings-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a writable temporary directory");
        directory
    }

    /// Installs against that directory, so a save writes somewhere a test owns.
    fn install_here(configurations: Vec<LayeredConfiguration>) {
        install_in(Side::Client, configurations, Some(Ok(game())));
    }

    fn installed(pack: Option<&ValueDocument>, player: Option<&ValueDocument>) -> Vec<Row> {
        let configuration = LayeredConfiguration::resolve(declaration(), pack, player)
            .expect("the layers should resolve");
        install_here(vec![configuration]);
        rows(&document("example_mod").expect("the owner is installed"))
    }

    fn row(rows: &[Row], key: &str) -> Row {
        rows.iter()
            .find(|row| row.key == key)
            .unwrap_or_else(|| panic!("`{key}` should have a row"))
            .clone()
    }

    fn reread(key: &str) -> Row {
        row(
            &rows(&document("example_mod").expect("the owner is installed")),
            key,
        )
    }

    fn document_for(owner: &str, values: &serde_json::Value) -> ValueDocument {
        ValueDocument::parse(
            &json!({ "schemaVersion": 1, "owner": owner, "values": values }).to_string(),
        )
        .expect("the test document should parse")
    }

    /// Builds a save payload the way the screen does.
    fn edits(owner: &str, values: &[(&str, &str)]) -> String {
        let mut out = String::new();
        field(&mut out, "1");
        field(&mut out, owner);
        field(&mut out, &values.len().to_string());
        for (key, value) in values {
            field(&mut out, key);
            field(&mut out, value);
        }
        out
    }

    /// Reads a save answer back as the refusals it carries.
    fn refusals(answer: &str) -> Vec<(String, String)> {
        let mut reader = Reader::new(answer);
        let count = reader.count().expect("a refusal count");
        let mut refused = Vec::with_capacity(count);
        for _ in 0..count {
            refused.push((
                reader.field().expect("a key"),
                reader.field().expect("a reason"),
            ));
        }
        assert!(reader.finished(), "the answer ends where its fields do");
        refused
    }

    #[test]
    fn a_field_is_read_back_exactly_however_it_is_punctuated() {
        // This is the whole reason the wire is length-prefixed. Every one of
        // these would need escaping in a delimited format or in JSON, and a
        // length says where the field ends whatever is inside it.
        for awkward in [
            "",
            "a:b",
            "12:not a length",
            "\"quoted\"",
            "line\nbreak",
            "back\\slash",
            "emoji 🧵 and 日本語",
        ] {
            let mut written = String::new();
            field(&mut written, awkward);
            let mut reader = Reader::new(&written);

            assert_eq!(reader.field().as_deref(), Some(awkward));
            assert!(reader.finished());
        }
    }

    #[test]
    fn a_length_that_runs_past_the_payload_reads_as_nothing_rather_than_what_is_there() {
        for broken in ["9:short", "not-a-length:x", "5", "", ":x"] {
            assert_eq!(Reader::new(broken).field(), None, "{broken}");
        }
    }

    #[test]
    fn every_schema_reaches_the_widget_that_can_draw_it() {
        let rows = installed(None, None);

        assert_eq!(row(&rows, "render.distance").widget, "slider");
        assert_eq!(row(&rows, "render.fancy").widget, "toggle");
        assert_eq!(row(&rows, "render.quality").widget, "choice");
        assert_eq!(row(&rows, "render.name").widget, "text");
    }

    #[test]
    fn a_schema_with_no_kind_is_a_text_field_rather_than_a_row_that_is_refused() {
        let rows = installed(None, None);
        let row = row(&rows, "render.name");

        assert_eq!(row.widget, "text");
        assert_eq!(row.kind, "", "the schema states no kind");
        assert_eq!(row.editable, "true");
    }

    #[test]
    fn a_setting_with_no_title_is_labelled_by_the_key_the_declaration_names() {
        let rows = installed(None, None);

        assert_eq!(row(&rows, "render.distance").label, "Render distance");
        assert_eq!(row(&rows, "render.fancy").label, "render.fancy");
    }

    #[test]
    fn a_slider_carries_the_bounds_it_has_to_draw_between() {
        let rows = installed(None, None);
        let row = row(&rows, "render.distance");

        assert_eq!(row.minimum, "2");
        assert_eq!(row.maximum, "32");
        assert_eq!(row.kind, "integer");
        assert_eq!(row.description, "How far chunks are drawn.");
        assert_eq!(row.value, "12");
    }

    #[test]
    fn a_choice_carries_the_values_it_may_be_cycled_through() {
        let rows = installed(None, None);

        assert_eq!(row(&rows, "render.quality").choices, ["fast", "fancy"]);
        assert!(row(&rows, "render.fancy").choices.is_empty());
    }

    #[test]
    fn a_screen_holds_only_the_settings_the_side_it_runs_on_owns() {
        let rows = installed(None, None);

        assert_eq!(rows.len(), 4, "the server setting has no client row");
        assert!(!rows.iter().any(|row| row.key == "server.motd"));
    }

    #[test]
    fn a_setting_a_pack_took_the_last_word_on_is_an_uneditable_row_like_any_other() {
        let declaration = ConfigurationDeclaration::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "example_mod",
              "version": 1,
              "settings": [
                {
                  "key": "render.distance",
                  "valueSchema": { "type": "integer" },
                  "default": 12,
                  "side": "client",
                  "restartRequired": false,
                  "packOverride": "locked"
                }
              ]
            }"#,
        )
        .expect("the locked declaration should parse");
        let pack = document_for("example_mod", &json!({ "render.distance": 6 }));
        let configuration = LayeredConfiguration::resolve(declaration, Some(&pack), None)
            .expect("the layers should resolve");
        install_here(vec![configuration]);

        let row = reread("render.distance");

        // The row carries no flag of its own for this. It is an ordinary row
        // that cannot be edited, and whether a pack may fix a client setting is
        // a question a differently-drawn row would answer by implementation.
        assert_eq!(row.editable, "false");
        assert_eq!(row.source, "pack");
        assert_eq!(row.value, "6");
    }

    #[test]
    fn an_edit_the_player_makes_is_in_force_the_next_time_the_screen_opens() {
        installed(None, None);

        let answer = save(&edits("example_mod", &[("render.distance", "8")]));

        assert!(refusals(&answer).is_empty(), "the edit is legal");
        let row = reread("render.distance");
        assert_eq!(row.value, "8");
        assert_eq!(row.source, "player");
        assert_eq!(row.restart_pending, "false");
    }

    #[test]
    fn an_edit_the_player_makes_is_on_disk_and_in_force_after_a_relaunch() {
        installed(None, None);

        let answer = save(&edits("example_mod", &[("render.distance", "8")]));
        assert!(
            refusals(&answer).is_empty(),
            "the edit is legal and persists"
        );

        // The file is what the next launch reads, so read it the way the next
        // launch would rather than trusting the store this one still holds.
        let owner: ModId = "example_mod".parse().expect("a valid mod id");
        let written = file::read(&game(), &owner)
            .expect("the file should be readable")
            .expect("a saved edit leaves a file");
        let relaunched = LayeredConfiguration::resolve(declaration(), None, Some(&written))
            .expect("the written document should resolve as the player layer");

        assert_eq!(
            relaunched
                .settings_on(Side::Client)
                .iter()
                .find(|resolved| resolved.setting.key.as_str() == "render.distance")
                .map(|resolved| resolved.value.clone()),
            Some(json!(8))
        );
    }

    #[test]
    fn a_save_that_cannot_reach_a_game_directory_says_so_rather_than_looking_saved() {
        let configuration = LayeredConfiguration::resolve(declaration(), None, None)
            .expect("the layers should resolve");
        install_in(
            Side::Client,
            vec![configuration],
            Some(Err("no game directory here".to_owned())),
        );

        let answer = save(&edits("example_mod", &[("render.distance", "8")]));

        let refused = refusals(&answer);
        assert_eq!(refused.len(), 1, "{answer}");
        assert_eq!(refused[0].0, "", "a whole-file failure names no key");
        assert_eq!(refused[0].1, "no game directory here");
    }

    #[test]
    fn a_file_save_failure_does_not_expose_its_private_path_to_the_screen() {
        let blocked = game().join("not-a-directory");
        std::fs::write(&blocked, "blocked").expect("the blocking file should be writable");
        let configuration = LayeredConfiguration::resolve(declaration(), None, None)
            .expect("the layers should resolve");
        install_in(Side::Client, vec![configuration], Some(Ok(blocked.clone())));

        let answer = save(&edits("example_mod", &[("render.distance", "8")]));

        let refused = refusals(&answer);
        assert_eq!(refused.len(), 1, "{answer}");
        assert!(!refused[0].1.is_empty(), "a refusal still says what failed");
        assert!(
            !refused[0].1.contains(&blocked.display().to_string()),
            "the screen received a private path: {}",
            refused[0].1
        );
    }

    #[test]
    fn a_game_directory_failure_is_redacted_before_the_screen_can_read_it() {
        let private = game();
        let configuration = LayeredConfiguration::resolve(declaration(), None, None)
            .expect("the layers should resolve");
        install_in(
            Side::Client,
            vec![configuration],
            Some(Err(format!(
                "`{}` is not the game directory",
                private.display()
            ))),
        );

        let answer = save(&edits("example_mod", &[("render.distance", "8")]));

        let refused = refusals(&answer);
        assert_eq!(refused.len(), 1, "{answer}");
        assert!(
            refused[0]
                .1
                .contains(private.file_name().unwrap().to_str().unwrap()),
            "the reduced error still names the failing directory: {}",
            refused[0].1
        );
        assert!(!refused[0].1.contains(&private.display().to_string()));
    }

    #[test]
    fn a_restart_required_edit_is_written_and_says_so_without_taking_effect() {
        installed(None, None);

        let answer = save(&edits("example_mod", &[("render.quality", "fancy")]));

        assert!(refusals(&answer).is_empty());
        let row = reread("render.quality");
        assert_eq!(row.value, "fast", "the session keeps the old value");
        assert_eq!(row.restart_pending, "true");
        assert_eq!(row.restart_required, "true");
    }

    #[test]
    fn a_saved_edit_round_trips_through_the_document_it_is_written_back_as() {
        installed(None, None);

        save(&edits(
            "example_mod",
            &[("render.quality", "fancy"), ("render.distance", "8")],
        ));

        // The round trip ADG-6 asks for: what the player's layer is written out
        // as, resolved again, is what the next launch shows — with the
        // restart-required edit now in force.
        let written = {
            let store = super::lock();
            store
                .as_ref()
                .expect("installed")
                .configurations
                .get(&"example_mod".parse().expect("a valid id"))
                .expect("the owner is installed")
                .player_document()
        };
        let relaunched = LayeredConfiguration::resolve(declaration(), None, Some(&written))
            .expect("the written document should resolve");

        assert_eq!(
            relaunched.value(&"render.quality".parse().expect("a valid key")),
            Some(&json!("fancy"))
        );
        assert_eq!(
            relaunched.value(&"render.distance".parse().expect("a valid key")),
            Some(&json!(8))
        );
        assert!(!relaunched.restart_pending());
    }

    #[test]
    fn a_value_the_schema_refuses_is_reported_and_leaves_the_one_before_it_standing() {
        installed(None, None);

        let answer = save(&edits("example_mod", &[("render.distance", "99")]));

        let refused = refusals(&answer);
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].0, "render.distance");
        assert!(!refused[0].1.is_empty(), "a refusal says why");
        assert_eq!(reread("render.distance").value, "12");
    }

    #[test]
    fn text_that_is_not_the_declared_kind_is_refused_by_name_rather_than_stored_as_text() {
        installed(None, None);

        let answer = save(&edits("example_mod", &[("render.distance", "twelve")]));

        let refused = refusals(&answer);
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].0, "render.distance");
        assert_eq!(reread("render.distance").value, "12");
    }

    #[test]
    fn a_key_the_owner_does_not_declare_is_refused_by_name() {
        installed(None, None);

        let answer = save(&edits(
            "example_mod",
            &[("render.absent", "1"), ("not a key", "1")],
        ));

        let refused = refusals(&answer);
        assert_eq!(refused.len(), 2);
        assert_eq!(refused[0].0, "render.absent");
        assert_eq!(refused[1].0, "not a key");
    }

    #[test]
    fn a_payload_for_another_owner_or_one_that_does_not_read_is_refused_whole() {
        installed(None, None);
        let before = document("example_mod").expect("the owner is installed");

        for payload in [
            edits("other_mod", &[("render.distance", "8")]),
            "not a payload".to_owned(),
            edits("example_mod", &[("render.distance", "8")]) + "5:extra",
        ] {
            assert_eq!(refusals(&save(&payload)).len(), 1, "{payload}");
        }
        assert_eq!(document("example_mod"), Some(before));
    }

    #[test]
    fn an_owner_this_process_does_not_hold_has_no_document_rather_than_an_empty_one() {
        installed(None, None);

        assert_eq!(document("other_mod"), None);
        assert_eq!(document("not a mod id"), None);
    }

    #[test]
    fn a_screen_opened_before_anything_is_installed_reads_nothing_and_saves_nothing() {
        *STORE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;

        assert_eq!(document("example_mod"), None);
        assert_eq!(
            refusals(&save(&edits("example_mod", &[("render.distance", "8")]))).len(),
            1
        );
    }

    #[test]
    fn a_player_value_the_schema_refuses_falls_back_to_the_layer_below() {
        let player = document_for(
            "example_mod",
            &json!({ "render.distance": 99, "render.absent": 1 }),
        );
        let rows = installed(None, Some(&player));

        assert_eq!(row(&rows, "render.distance").value, "12");
        assert_eq!(row(&rows, "render.distance").source, "default");
    }
}
