use ravel_config::{ConfigurationDeclaration, LayeredConfiguration, ValueDocument};
use ravel_core::Side;
use serde_json::json;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{EditRequest, Identity, ModId, Reader, Store, Target, field, file, persist_and_answer};

fn identity(encoded: &str) -> Identity {
    let encoded = if encoded.contains(':') {
        encoded.to_owned()
    } else {
        format!("process:{encoded}")
    };
    Identity::parse(&encoded).expect("a typed settings identity")
}

fn document_of(launch: Option<&crate::runtime::LaunchContext>, encoded: &str) -> Option<String> {
    super::document_of(launch, &identity(encoded))
}

fn save_with(launch: Option<&crate::runtime::LaunchContext>, payload: &str) -> String {
    super::save_in(launch, launch, payload)
}

fn selected_launch<'a>(
    process: Option<&'a crate::runtime::LaunchContext>,
    integrated: Option<&'a crate::runtime::LaunchContext>,
    encoded: &str,
) -> Option<&'a crate::runtime::LaunchContext> {
    super::selected_launch(process, integrated, identity(encoded).target)
}

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
    assert_eq!(reader.field().expect("a version"), "2");
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
    std::fs::write(directory.join("options.txt"), "version:1\n").expect("a writable client marker");
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
            store: Store::new(Side::Client, configurations),
            game,
        }
    }

    fn saving_to(configurations: Vec<LayeredConfiguration>, directory: PathBuf) -> Self {
        Self {
            store: Store::new(Side::Client, configurations),
            game: directory,
        }
    }

    fn rows(&self) -> Vec<Row> {
        rows(
            &self
                .store
                .document(&identity("example_mod"))
                .expect("the owner is installed"),
        )
    }

    fn row(&self, key: &str) -> Row {
        row(&self.rows(), key)
    }

    fn save(&mut self, payload: &str) -> String {
        let applied = EditRequest::parse(payload).and_then(|request| self.store.apply(request));
        persist_and_answer(applied, &self.game)
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
    field(&mut out, "2");
    let encoded = if owner.contains(':') {
        owner.to_owned()
    } else {
        format!("process:{owner}")
    };
    field(&mut out, &encoded);
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
    let store = Store::new(Side::Client, vec![here, elsewhere]);

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
    let empty: [String; 0] = [];
    assert_eq!(row(&rows, "render.fancy").choices, empty);
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
    let written = file::read_player(&screen.game, &owner)
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
    let missing = game().join("absent");
    let mut screen = Screen::saving_to(vec![configuration], missing);

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

    let refused = refusals(&answer);
    assert_eq!(refused.len(), 1, "{answer}");
    assert_eq!(refused[0].0, "", "a whole-file failure names no key");
    assert!(refused[0].1.contains("options.txt"), "{answer}");
}

#[test]
fn a_save_without_the_client_marker_refuses_the_stored_launch_directory() {
    let mut screen = screen(None, None);
    std::fs::remove_file(screen.game.join("options.txt")).expect("remove the client marker");

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

    let refused = refusals(&answer);
    assert_eq!(refused.len(), 1, "{answer}");
    assert!(refused[0].1.contains("options.txt"), "{answer}");
    let owner: ModId = "example_mod".parse().expect("a valid owner");
    assert!(
        file::read_player(&screen.game, &owner)
            .expect("read settings")
            .is_none()
    );
}

#[test]
fn a_file_save_failure_does_not_expose_its_private_path_to_the_screen() {
    let blocked = game().join("not-a-directory");
    std::fs::write(&blocked, "blocked").expect("the blocking file should be writable");
    let configuration = LayeredConfiguration::resolve(declaration(), None, None)
        .expect("the layers should resolve");
    let mut screen = Screen::saving_to(vec![configuration], blocked.clone());

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
    std::fs::remove_file(private.join("options.txt")).expect("remove the client marker");
    let configuration = LayeredConfiguration::resolve(declaration(), None, None)
        .expect("the layers should resolve");
    let mut screen = Screen::saving_to(vec![configuration], private.clone());

    let answer = screen.save(&edits("example_mod", &[("render.distance", "8")]));

    let refused = refusals(&answer);
    assert_eq!(refused.len(), 1, "{answer}");
    assert!(
        refused[0].1.contains(
            private
                .file_name()
                .expect("a named directory")
                .to_str()
                .expect("UTF-8 name")
        ),
        "the reduced error still names the failing directory: {}",
        refused[0].1
    );
    assert!(!refused[0].1.contains(&private.display().to_string()));
}

#[test]
fn a_restart_required_edit_is_written_and_says_so_without_taking_effect() {
    let mut screen = screen(None, None);

    let answer = screen.save(&edits("example_mod", &[("render.quality", "fancy")]));

    let empty: [(String, String); 0] = [];
    assert_eq!(refusals(&answer), empty);
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

    assert_eq!(relaunched.value("render.quality"), Ok(&json!("fancy")));
    assert_eq!(relaunched.value("render.distance"), Ok(&json!(8)));
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
    for (key, reason) in refused {
        assert_eq!(
            reason,
            format!("`{key}` is not a setting this mod declares")
        );
    }
    assert_eq!(screen.row("render.distance").value, "12");
}

#[test]
fn a_payload_for_another_owner_or_one_that_does_not_read_is_refused_whole() {
    let mut screen = screen(None, None);
    let before = screen
        .store
        .document(&identity("example_mod"))
        .expect("the owner is installed");

    for payload in [
        edits("other_mod", &[("render.distance", "8")]),
        "not a payload".to_owned(),
        edits("example_mod", &[("render.distance", "8")]) + "5:extra",
    ] {
        assert_eq!(refusals(&screen.save(&payload)).len(), 1, "{payload}");
    }
    assert_eq!(
        screen.store.document(&identity("example_mod")),
        Some(before)
    );
}

#[test]
fn an_owner_this_process_does_not_hold_has_no_document_rather_than_an_empty_one() {
    let screen = screen(None, None);

    assert_eq!(screen.store.document(&identity("other_mod")), None);
    assert!(Identity::parse("process:not a mod id").is_none());
}

#[test]
fn a_screen_opened_before_anything_is_installed_reads_nothing_and_saves_nothing() {
    assert_eq!(document_of(None, "example_mod"), None);
    assert_eq!(
        refusals(&save_with(
            None,
            &edits("example_mod", &[("render.distance", "8")])
        ))
        .len(),
        1
    );
}

#[test]
fn malformed_settings_and_unknown_owners_have_specific_refusals() {
    let mut screen = screen(None, None);
    let mut version = String::new();
    field(&mut version, "unsupported");
    for (payload, expected) in [
        (version, "unsupported wire version"),
        (edits("not a mod id", &[]), "mod identifier is invalid"),
        (edits("other_mod", &[]), "no settings are declared"),
        ("1:2".to_owned(), "truncated"),
        (
            format!("{}extra", edits("example_mod", &[])),
            "trailing data",
        ),
    ] {
        let answer = screen.save(&payload);
        let reasons = refusals(&answer);
        assert_eq!(reasons.len(), 1, "{answer}");
        assert!(reasons[0].1.contains(expected), "{answer}");
    }
    let missing = save_with(None, &edits("example_mod", &[]));
    assert_eq!(
        refusals(&missing)[0].1,
        crate::guard::HookError::Unavailable.to_string()
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

#[test]
fn saving_without_a_disk_document_preserves_unedited_valid_player_values() {
    let player = document_for(
        "example_mod",
        &json!({"render.distance": 8, "render.quality": "fancy", "render.absent": 1}),
    );
    let mut screen = screen(None, Some(&player));
    assert_eq!(
        refusals(&screen.save(&edits("example_mod", &[("render.fancy", "false")]))),
        Vec::<(String, String)>::new(),
    );
    let written = file::read_player(&screen.game, &player.owner)
        .expect("read the new player file")
        .expect("a persisted document");
    assert_eq!(
        written.values.get(&"render.distance".parse().expect("key")),
        Some(&json!(8)),
    );
    assert_eq!(
        written.values.get(&"render.quality".parse().expect("key")),
        Some(&json!("fancy")),
    );
    assert_eq!(
        written.values.get(&"render.fancy".parse().expect("key")),
        Some(&json!(false)),
    );
    assert!(
        !written
            .values
            .contains_key(&"render.absent".parse().expect("key"))
    );
}

#[test]
fn a_directory_the_client_has_started_in_is_admitted() {
    let game = crate::test_support::fresh_directory("settings-directory", "started");
    std::fs::write(game.join(super::CLIENT_MARKER), "version:1\n")
        .expect("the marker should be writable");
    assert_eq!(super::admit_client_directory(&game), Ok(()));
}

fn side_launch(side: Side, directory: PathBuf) -> crate::runtime::LaunchContext {
    side_launch_with_player(side, directory, None)
}

fn side_launch_with_player(
    side: Side,
    directory: PathBuf,
    player: Option<&ValueDocument>,
) -> crate::runtime::LaunchContext {
    let declaration = ConfigurationDeclaration::parse(
        r#"{
          "schemaVersion": 1, "owner": "example_mod", "version": 1,
          "settings": [
            {"key": "client.flag", "valueSchema": {"type": "boolean"},
             "default": false, "side": "client", "restartRequired": false},
            {"key": "server.flag", "valueSchema": {"type": "boolean"},
             "default": false, "side": "integrated-server", "restartRequired": false},
            {"key": "common.flag", "valueSchema": {"type": "boolean"},
             "default": false, "side": "common", "restartRequired": false}
          ]
        }"#,
    )
    .expect("a side-specific declaration");
    crate::runtime::LaunchContext::new(
        side,
        directory,
        ravel_loader::Supplied::default(),
        vec![LayeredConfiguration::resolve(declaration, None, player).expect("valid settings")],
    )
}

#[test]
fn integrated_settings_use_typed_routing_and_never_fall_back_to_client_settings() {
    let client = side_launch(Side::Client, game());
    let server = side_launch(Side::IntegratedServer, client.game_directory.clone());
    let identity = "integrated-server:example_mod";
    let selected = selected_launch(Some(&client), Some(&server), identity);
    assert!(std::ptr::eq(
        selected.expect("the server"),
        std::ptr::from_ref(&server),
    ));
    let document = document_of(selected, identity).expect("server settings");
    let mut reader = Reader::new(&document);
    assert_eq!(reader.field().as_deref(), Some("2"));
    assert_eq!(reader.field().as_deref(), Some("example_mod"));
    assert_eq!(reader.field().as_deref(), Some("integrated-server"));
    assert_eq!(reader.count(), Some(2));
    assert_eq!(reader.field().as_deref(), Some("server.flag"));
    assert!(super::configurable(&server).contains(&"example_mod".parse().expect("mod id")));
    assert!(selected_launch(Some(&client), None, identity).is_none());
    assert!(document_of(Some(&client), identity).is_none());
    assert_eq!(
        refusals(&super::save_in(
            Some(&client),
            None,
            &edits(identity, &[("server.flag", "true")])
        ))
        .len(),
        1
    );
    assert!(std::ptr::eq(
        selected_launch(Some(&client), Some(&server), "example_mod").expect("the client"),
        std::ptr::from_ref(&client),
    ));
}

#[test]
fn saving_either_side_preserves_the_other_sides_player_values() {
    let client = side_launch(Side::Client, game());
    let server = side_launch(Side::IntegratedServer, client.game_directory.clone());
    for (identity, key, value) in [
        ("example_mod", "client.flag", "true"),
        ("integrated-server:example_mod", "server.flag", "true"),
        ("example_mod", "client.flag", "false"),
    ] {
        assert_eq!(
            refusals(&super::save_in(
                Some(&client),
                Some(&server),
                &edits(identity, &[(key, value)])
            )),
            Vec::<(String, String)>::new(),
        );
    }
    let written = file::read_player(
        &client.game_directory,
        &"example_mod".parse().expect("mod id"),
    )
    .expect("read the player file")
    .expect("a persisted document");
    assert_eq!(
        written.values.get(&"client.flag".parse().expect("key")),
        Some(&json!(false))
    );
    assert_eq!(
        written.values.get(&"server.flag".parse().expect("key")),
        Some(&json!(true))
    );
    let server_document = document_of(Some(&server), "integrated-server:example_mod")
        .expect("server remains available");
    let client_document =
        document_of(Some(&client), "example_mod").expect("client remains available");
    assert!(!server_document.contains("client.flag"));
    assert!(!client_document.contains("server.flag"));
}

#[test]
fn side_only_edits_preserve_newer_common_values_from_either_store() {
    for common_side in [Side::Client, Side::IntegratedServer] {
        let directory = game();
        let initial = document_for("example_mod", &json!({"common.flag": false}));
        file::write_player(&directory, &initial).expect("write the initial common value");
        let client = side_launch_with_player(Side::Client, directory.clone(), Some(&initial));
        let server =
            side_launch_with_player(Side::IntegratedServer, directory.clone(), Some(&initial));
        let (common_launch, common_identity, other_launch, other_identity, other_key) =
            if common_side == Side::Client {
                (
                    &client,
                    "example_mod",
                    &server,
                    "integrated-server:example_mod",
                    "server.flag",
                )
            } else {
                (
                    &server,
                    "integrated-server:example_mod",
                    &client,
                    "example_mod",
                    "client.flag",
                )
            };
        assert_eq!(
            refusals(&save_with(
                Some(common_launch),
                &edits(common_identity, &[("common.flag", "true")]),
            )),
            Vec::<(String, String)>::new(),
        );
        assert_eq!(
            refusals(&save_with(
                Some(other_launch),
                &edits(other_identity, &[(other_key, "true")]),
            )),
            Vec::<(String, String)>::new(),
        );
        let written = file::read_player(&directory, &initial.owner)
            .expect("read the shared file")
            .expect("a persisted document");
        assert_eq!(
            written.values.get(&"common.flag".parse().expect("key")),
            Some(&json!(true)),
        );
        assert_eq!(
            written.values.get(&other_key.parse().expect("key")),
            Some(&json!(true)),
        );
    }
}

#[test]
fn saving_cleans_rejected_values_but_keeps_another_sides_valid_replacement() {
    for repaired in [false, true] {
        let directory = game();
        let initial = document_for(
            "example_mod",
            &json!({
                "common.flag": "illegal",
                "client.flag": "illegal",
                "unknown.flag": true,
            }),
        );
        file::write_player(&directory, &initial).expect("write rejected player values");
        let client = side_launch_with_player(Side::Client, directory.clone(), Some(&initial));
        let server =
            side_launch_with_player(Side::IntegratedServer, directory.clone(), Some(&initial));
        if repaired {
            assert_eq!(
                refusals(&save_with(
                    Some(&client),
                    &edits("example_mod", &[("common.flag", "true")]),
                )),
                Vec::<(String, String)>::new(),
            );
        }
        assert_eq!(
            refusals(&save_with(
                Some(&server),
                &edits("integrated-server:example_mod", &[("server.flag", "true")]),
            )),
            Vec::<(String, String)>::new(),
        );
        let written = file::read_player(&directory, &initial.owner)
            .expect("read the cleaned file")
            .expect("a persisted document");
        assert_eq!(
            written.values.get(&"common.flag".parse().expect("key")),
            repaired.then_some(&json!(true)),
        );
        assert!(
            !written
                .values
                .contains_key(&"client.flag".parse().expect("key"))
        );
        assert!(
            !written
                .values
                .contains_key(&"unknown.flag".parse().expect("key"))
        );
        assert_eq!(
            written.values.get(&"server.flag".parse().expect("key")),
            Some(&json!(true)),
        );
    }
}

#[test]
fn a_directory_with_no_client_marker_is_refused_and_says_which_sign() {
    let game = crate::test_support::fresh_directory("settings-directory", "elsewhere");
    let refused = super::admit_client_directory(&game).expect_err("no marker is refused");
    assert!(refused.contains(super::CLIENT_MARKER), "{refused}");
    assert!(refused.contains(&game.display().to_string()), "{refused}");
}

#[test]
fn a_client_marker_that_is_a_directory_does_not_admit_it() {
    let game = crate::test_support::fresh_directory("settings-directory", "marker-is-a-directory");
    std::fs::create_dir(game.join(super::CLIENT_MARKER)).expect("create the directory");
    assert!(super::admit_client_directory(&game).is_err());
}

#[test]
fn settings_routes_reject_display_labels_and_unknown_targets() {
    let mut payload = String::new();
    for encoded in [
        "example_mod",
        "example_mod [integrated-server]",
        "process:example_mod [integrated-server]",
        "client:example_mod",
        "integrated-server:",
        "integrated-server:example_mod:extra",
    ] {
        assert!(Identity::parse(encoded).is_none(), "{encoded}");
        payload.clear();
        field(&mut payload, "2");
        field(&mut payload, encoded);
        field(&mut payload, "0");
        assert!(EditRequest::parse(&payload).is_err(), "{encoded}");
        let refused = refusals(&super::save_in(None, None, &payload));
        assert_eq!(refused.len(), 1);
        assert!(refused[0].1.contains("target or mod identifier is invalid"));
    }
    assert_eq!(identity("process:example_mod").target, Target::Process);
    assert_eq!(
        identity("integrated-server:example_mod").target,
        Target::IntegratedServer
    );
}

#[test]
fn route_serialization_round_trips_each_typed_target() {
    let owner = "example_mod".parse().expect("a canonical mod id");
    for target in [Target::Process, Target::IntegratedServer] {
        let encoded = Identity::encode(target, &owner);
        let decoded = Identity::parse(&encoded).expect("a serialized route");
        assert_eq!(decoded.target, target);
        assert_eq!(decoded.owner, owner);
    }
}

#[test]
fn truncated_edits_never_apply_an_earlier_valid_edit() {
    let mut screen = screen(None, None);
    let mut payload = edits(
        "example_mod",
        &[("render.distance", "8"), ("render.fancy", "false")],
    );
    payload.pop();
    assert_eq!(refusals(&screen.save(&payload)).len(), 1);
    assert_eq!(screen.row("render.distance").value, "12");
}
