use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
use ravel_core::Side;
use serde_json::json;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{ModId, Reader, Store, answer, document, field, file, save};

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

fn game() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ravel-settings-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("a writable temporary directory");
    directory
}

struct Screen {
    store: Store,
    game: PathBuf,
}

impl Screen {
    fn new(configurations: Vec<LayeredConfiguration>) -> Self {
        let game = game();
        Self {
            store: Store::new(Side::Client, configurations, Some(Ok(game.clone()))),
            game,
        }
    }

    fn saving_to(
        configurations: Vec<LayeredConfiguration>,
        directory: Result<PathBuf, String>,
    ) -> Self {
        Self {
            store: Store::new(Side::Client, configurations, Some(directory)),
            game: game(),
        }
    }

    fn rows(&self) -> Vec<Row> {
        rows(
            &self
                .store
                .document("example_mod")
                .expect("the owner is installed"),
        )
    }

    fn row(&self, key: &str) -> Row {
        row(&self.rows(), key)
    }

    fn save(&mut self, payload: &str) -> String {
        answer(self.store.apply(&mut Reader::new(payload)))
    }
}

fn screen(pack: Option<&ValueDocument>, player: Option<&ValueDocument>) -> Screen {
    let configuration = LayeredConfiguration::resolve(declaration(), pack, player)
        .expect("the layers should resolve");
    Screen::new(vec![configuration])
}

fn row(rows: &[Row], key: &str) -> Row {
    rows.iter()
        .find(|row| row.key == key)
        .unwrap_or_else(|| panic!("`{key}` should have a row"))
        .clone()
}

fn document_for(owner: &str, values: &serde_json::Value) -> ValueDocument {
    ValueDocument::parse(
        &json!({ "schemaVersion": 1, "owner": owner, "values": values }).to_string(),
    )
    .expect("the test document should parse")
}

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
fn an_owner_with_nothing_to_show_on_this_side_is_not_offered_a_screen() {
    let here = LayeredConfiguration::resolve(declaration(), None, None)
        .expect("the layers should resolve");
    let elsewhere = LayeredConfiguration::resolve(
        ConfigurationDeclaration::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "server_mod",
              "version": 1,
              "settings": [
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
        .expect("the test declaration should parse"),
        None,
        None,
    )
    .expect("the layers should resolve");
    let store = Store::new(Side::Client, vec![here, elsewhere], None);

    let offered: Vec<String> = store
        .configurable()
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(offered, ["example_mod"]);
}

#[test]
fn a_field_is_read_back_exactly_however_it_is_punctuated() {
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
    let rows = screen(None, None).rows();

    assert_eq!(row(&rows, "render.distance").widget, "slider");
    assert_eq!(row(&rows, "render.fancy").widget, "toggle");
    assert_eq!(row(&rows, "render.quality").widget, "choice");
    assert_eq!(row(&rows, "render.name").widget, "text");
}

#[test]
fn a_schema_with_no_kind_is_a_text_field_rather_than_a_row_that_is_refused() {
    let rows = screen(None, None).rows();
    let row = row(&rows, "render.name");

    assert_eq!(row.widget, "text");
    assert_eq!(row.kind, "", "the schema states no kind");
    assert_eq!(row.editable, "true");
}

#[test]
fn a_setting_with_no_title_is_labelled_by_the_key_the_declaration_names() {
    let rows = screen(None, None).rows();

    assert_eq!(row(&rows, "render.distance").label, "Render distance");
    assert_eq!(row(&rows, "render.fancy").label, "render.fancy");
}

#[test]
fn a_slider_carries_the_bounds_it_has_to_draw_between() {
    let rows = screen(None, None).rows();
    let row = row(&rows, "render.distance");

    assert_eq!(row.minimum, "2");
    assert_eq!(row.maximum, "32");
    assert_eq!(row.kind, "integer");
    assert_eq!(row.description, "How far chunks are drawn.");
    assert_eq!(row.value, "12");
}

#[test]
fn a_choice_carries_the_values_it_may_be_cycled_through() {
    let rows = screen(None, None).rows();

    assert_eq!(row(&rows, "render.quality").choices, ["fast", "fancy"]);
    assert_eq!(
        row(&rows, "render.fancy").choices,
        [] as [std::string::String; 0]
    );
}

#[test]
fn a_screen_holds_only_the_settings_the_side_it_runs_on_owns() {
    let rows = screen(None, None).rows();

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

    let row = Screen::new(vec![configuration]).row("render.distance");

    assert_eq!(row.editable, "false");
    assert_eq!(row.source, "pack");
    assert_eq!(row.value, "6");
}

#[test]
fn an_edit_the_player_makes_is_in_force_the_next_time_the_screen_opens() {
    let mut screen = screen(None, None);

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

    assert!(refusals(&answer).is_empty(), "the edit is legal");
    let row = screen.row("render.distance");
    assert_eq!(row.value, "8");
    assert_eq!(row.source, "player");
    assert_eq!(row.restart_pending, "false");
}

#[test]
fn an_edit_the_player_makes_is_on_disk_and_in_force_after_a_relaunch() {
    let mut screen = screen(None, None);

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));
    assert!(
        refusals(&answer).is_empty(),
        "the edit is legal and persists"
    );

    let owner: ModId = "example_mod".parse().expect("a valid mod id");
    let written = file::read(&screen.game, &owner)
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
    let mut screen = Screen::saving_to(
        vec![configuration],
        Err("no game directory here".to_owned()),
    );

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

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
    let mut screen = Screen::saving_to(vec![configuration], Ok(blocked.clone()));

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

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
    let mut screen = Screen::saving_to(
        vec![configuration],
        Err(format!("`{}` is not the game directory", private.display())),
    );

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

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
    let mut screen = screen(None, None);

    let answer = screen.save(&edits("example_mod", &[("render.quality", "fancy")]));

    assert_eq!(
        refusals(&answer),
        [] as [(std::string::String, std::string::String); 0]
    );
    let row = screen.row("render.quality");
    assert_eq!(row.value, "fast", "the session keeps the old value");
    assert_eq!(row.restart_pending, "true");
    assert_eq!(row.restart_required, "true");
}

#[test]
fn a_saved_edit_round_trips_through_the_document_it_is_written_back_as() {
    let mut screen = screen(None, None);

    screen.save(&edits(
        "example_mod",
        &[("render.quality", "fancy"), ("render.distance", "8")],
    ));

    let written = screen
        .store
        .configurations
        .get(&"example_mod".parse().expect("a valid id"))
        .expect("the owner is installed")
        .player_document();
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
    let mut screen = screen(None, None);

    let answer = screen.save(&edits("example_mod", &[("render.distance", "99")]));

    let refused = refusals(&answer);
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].0, "render.distance");
    assert!(!refused[0].1.is_empty(), "a refusal says why");
    assert_eq!(screen.row("render.distance").value, "12");
}

#[test]
fn text_that_is_not_the_declared_kind_is_refused_by_name_rather_than_stored_as_text() {
    let mut screen = screen(None, None);

    let answer = screen.save(&edits("example_mod", &[("render.distance", "twelve")]));

    let refused = refusals(&answer);
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].0, "render.distance");
    assert_eq!(screen.row("render.distance").value, "12");
}

#[test]
fn a_key_the_owner_does_not_declare_is_refused_by_name() {
    let mut screen = screen(None, None);

    let answer = screen.save(&edits(
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
    let mut screen = screen(None, None);
    let before = screen
        .store
        .document("example_mod")
        .expect("the owner is installed");

    for payload in [
        edits("other_mod", &[("render.distance", "8")]),
        "not a payload".to_owned(),
        edits("example_mod", &[("render.distance", "8")]) + "5:extra",
    ] {
        assert_eq!(refusals(&screen.save(&payload)).len(), 1, "{payload}");
    }
    assert_eq!(screen.store.document("example_mod"), Some(before));
}

#[test]
fn an_owner_this_process_does_not_hold_has_no_document_rather_than_an_empty_one() {
    let screen = screen(None, None);

    assert_eq!(screen.store.document("other_mod"), None);
    assert_eq!(screen.store.document("not a mod id"), None);
}

#[test]
fn a_screen_opened_before_anything_is_installed_reads_nothing_and_saves_nothing() {
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
    let rows = screen(None, Some(&player)).rows();

    assert_eq!(row(&rows, "render.distance").value, "12");
    assert_eq!(row(&rows, "render.distance").source, "default");
}
