//! Live mod-list and dependency diagnostics encoded for the Java screen.
//!
//! Failure details are redacted here; notices were already redacted by the report.
//! Client screens append labelled integrated-server rows and notices. A separate
//! typed settings identity routes requests independently of those display labels.

use std::collections::BTreeSet;

use jni::EnvUnowned;
use jni::objects::{JClass, JString};
use ravel_core::{DiagnosticMessage, FailureAttribution, FailureSurface, ModId};
use ravel_loader::{ModEntry, Requirement};

use crate::guard::hand_document;
use crate::runtime::LoaderRuntime;
use crate::settings::{Identity, Target};
use crate::wire::{MODS_VERSION, count, field, flag, integrated_note, integrated_row};

/// Encodes the live mod list, or returns nothing before runtime installation.
fn document() -> Option<String> {
    let configurable = crate::runtime::installed()
        .map(|runtime| crate::settings::configurable(&runtime.launch))
        .unwrap_or_default();
    let integrated = crate::runtime::integrated_runtime();
    document_with_integrated(
        crate::runtime::installed().map(AsRef::as_ref),
        integrated.as_deref(),
        &configurable,
    )
}

/// Keeps the client settings identity while adding independently attributed server rows.
fn document_with_integrated(
    runtime: Option<&LoaderRuntime>,
    integrated: Option<&LoaderRuntime>,
    configurable: &BTreeSet<ModId>,
) -> Option<String> {
    let runtime = runtime?;
    // Each snapshot keeps its rows and notices at the same instant.
    let (client, client_notices) = runtime.mod_view();
    let server_settings = integrated
        .map(|runtime| crate::settings::configurable(&runtime.launch))
        .unwrap_or_default();
    let integrated = integrated.map(LoaderRuntime::mod_view);
    let server = integrated.as_ref().map(|(surface, _)| surface);
    let server_notices = integrated
        .as_ref()
        .map_or(&[][..], |(_, notices)| notices.as_slice());
    let mut out = String::new();
    field(&mut out, MODS_VERSION);
    field(&mut out, client.side().as_str());
    count(
        &mut out,
        client.actionable().count() + server.map_or(0, |surface| surface.actionable().count()),
    );
    count(
        &mut out,
        client.entries().len() + server.map_or(0, |surface| surface.entries().len()),
    );
    for (index, entry) in client.entries().iter().enumerate() {
        labelled_row(
            &mut out,
            index,
            entry,
            configurable.contains(&entry.mod_id),
            entry.mod_id.as_str(),
            Target::Process,
        );
    }
    let mut label = String::new();
    for (index, entry) in server
        .into_iter()
        .flat_map(ravel_loader::ModSurface::entries)
        .enumerate()
    {
        // The label is presentation only; the row carries a separate settings target.
        label.clear();
        integrated_row(&mut label, &entry.mod_id);
        labelled_row(
            &mut out,
            client.entries().len() + index,
            entry,
            server_settings.contains(&entry.mod_id),
            &label,
            Target::IntegratedServer,
        );
    }
    count(&mut out, client_notices.len() + server_notices.len());
    for notice in &client_notices {
        field(&mut out, notice.text());
    }
    for notice in server_notices {
        label.clear();
        integrated_note(&mut label, &notice.text());
        field(&mut out, &label);
    }
    Some(out)
}

/// Encodes one row with independent presentation and settings routing fields.
fn labelled_row(
    out: &mut String,
    index: usize,
    entry: &ModEntry,
    configurable: bool,
    label: &str,
    target: Target,
) {
    count(out, index + 1);
    field(out, label);
    field(out, entry.state.as_str());
    field(out, requirement_name(entry.requirement));
    flag(out, entry.loads_here);
    flag(out, entry.is_actionable());
    flag(out, configurable);
    match &entry.failure {
        None => count(out, 0),
        Some(failure) => {
            count(out, 1);
            attribution(out, failure);
        }
    }
    field(out, &Identity::encode(target, &entry.mod_id));
}

/// Writes one failure's structured attribution.
fn attribution(out: &mut String, failure: &FailureAttribution) {
    field(out, failure.kind.map_or("", |kind| kind.as_str()));
    match failure.surface.as_ref() {
        None => {
            field(out, "");
            field(out, "");
        }
        Some(FailureSurface::Hook(name)) => {
            field(out, "hook");
            field(out, name);
        }
        Some(FailureSurface::Event(name)) => {
            field(out, "event");
            field(out, name);
        }
        Some(FailureSurface::Registry(key)) => {
            field(out, "registry");
            field(out, &key.to_string());
        }
        Some(FailureSurface::Packet(name)) => {
            field(out, "packet");
            field(out, &name.to_string());
        }
    }
    field(out, failure.state.as_str());
    field(
        out,
        failure
            .version
            .as_ref()
            .map_or_else(String::new, ToString::to_string)
            .as_str(),
    );
    count(out, failure.dependency_chain.len());
    for link in &failure.dependency_chain {
        field(out, link.as_str());
    }
    field(out, publishable(&failure.message).text());
}

/// Redacts failure details at the publishing boundary, regardless of their tag.
fn publishable(message: &DiagnosticMessage) -> DiagnosticMessage {
    DiagnosticMessage::redacted(message.text())
}

/// Names whether the set fails closed without a mod.
const fn requirement_name(requirement: Requirement) -> &'static str {
    match requirement {
        Requirement::Required => "required",
        Requirement::Optional => "optional",
    }
}

/// Hands the screen the whole installed mod set.
///
/// # Panics
///
/// Panics become Java `RuntimeException`s rather than unwinding into the JVM.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelModSurface")]
pub fn ravel_mod_surface<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    hand_document(env, |_| Ok(document()))
}

#[cfg(test)]
mod tests {
    use ravel_core::{
        DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, FailureSurface,
        LifecycleState, ModId, Side, SideSet,
    };
    use ravel_loader::{LifecycleEntry, Requirement};

    use std::collections::BTreeSet;

    use super::document_with_integrated;
    use crate::runtime::LoaderRuntime;
    use crate::test_support::DocumentFixture;
    use crate::wire::fields;

    fn mod_id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    fn configurable(names: &[&str]) -> BTreeSet<ModId> {
        names.iter().copied().map(mod_id).collect()
    }

    fn entry(name: &str, requirement: Requirement) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: mod_id(name),
            sides: SideSet::new([Side::Client, Side::IntegratedServer]).expect("two sides"),
            requirement,
            saved_data_versions: BTreeSet::new(),
        }
    }

    fn attribution(state: LifecycleState, kind: Option<FailureKind>) -> FailureAttribution {
        FailureAttribution {
            mod_id: Some(mod_id("broken_mod")),
            version: Some("1.2.3".parse().expect("a valid version")),
            dependency_chain: vec![mod_id("base_mod"), mod_id("broken_mod")],
            state,
            kind,
            surface: Some(FailureSurface::Hook("client.tick".to_owned())),
            environment: FailureEnvironment {
                ravel: "0.0.0".parse().expect("a valid version"),
                minecraft: "the-pinned-version".to_owned(),
                platform: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
            },
            message: DiagnosticMessage::raw("failed"),
        }
    }

    fn running(fixture: DocumentFixture) -> LoaderRuntime {
        LoaderRuntime::new(
            fixture.machine,
            fixture.mods,
            crate::test_support::test_launch(),
        )
    }

    fn machine(entries: Vec<LifecycleEntry>) -> DocumentFixture {
        DocumentFixture::new(Side::Client, entries)
    }

    fn refuse_start(plan: &mut DocumentFixture, state: LifecycleState) {
        plan.mods = vec![Box::new(crate::test_support::RefusingStart {
            owner: mod_id("broken_mod"),
            state,
            detail: "failed",
        })];
        let required = plan
            .machine
            .entry(&mod_id("broken_mod"))
            .expect("tracked owner")
            .requirement
            == Requirement::Required;
        let outcome = ravel_loader::start_all(
            &mut ravel_loader::LoadedMods::new(&mut plan.machine, &mut plan.mods)
                .expect("complete hosts"),
            &ravel_minecraft_adapter::environment(),
        );
        assert_eq!(
            outcome.is_err(),
            required,
            "required failures refuse startup"
        );
    }

    #[test]
    fn a_screen_opened_before_a_set_is_installed_reads_nothing() {
        assert_eq!(
            document_with_integrated(None, None, &configurable(&[])),
            None
        );
    }

    #[test]
    fn a_valid_graph_lists_every_mod_in_resolved_load_order() {
        let runtime = running(machine(vec![
            entry("base_mod", Requirement::Required),
            entry("second_mod", Requirement::Optional),
        ]));

        let read = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );

        assert_eq!(read[0], "4", "the mod list's own version opens the payload");
        assert_eq!(read[1], "client");
        assert_eq!(read[2], "0", "a valid graph asks the player to do nothing");
        assert_eq!(read[3], "2");
        assert_eq!(read[4], "1", "the first row states its load position");
        assert_eq!(read[5], "base_mod");
        assert_eq!(read[6], LifecycleState::Discovered.as_str());
        assert_eq!(read[7], "required");
        assert_eq!(read[8], "true", "it loads on the side the set runs on");
        assert_eq!(read[9], "false", "nothing failed, so nothing is actionable");
        assert_eq!(
            read[10], "false",
            "it declares no settings a screen can open"
        );
        assert_eq!(read[11], "0", "and no failure block follows");
        assert_eq!(
            read[12], "process:base_mod",
            "routing is not a display label"
        );
        assert_eq!(read[13], "2", "and the next row states the next position");
        assert_eq!(read[14], "second_mod");
        assert_eq!(read[16], "optional");
    }

    #[test]
    fn a_spoofed_display_suffix_cannot_select_the_settings_runtime() {
        let runtime = running(machine(vec![entry("base_mod", Requirement::Required)]));
        let (surface, _) = runtime.mod_view();
        let mut out = String::new();
        super::labelled_row(
            &mut out,
            1,
            &surface.entries()[0],
            true,
            "renamed [integrated-server]",
            super::Target::Process,
        );
        let read = fields(&out);
        assert_eq!(read[1], "renamed [integrated-server]");
        assert_eq!(read.last().map(String::as_str), Some("process:base_mod"));
    }

    #[test]
    fn the_same_set_produces_the_same_payload_every_time() {
        let runtime = running(machine(vec![
            entry("base_mod", Requirement::Required),
            entry("second_mod", Requirement::Required),
        ]));

        assert_eq!(
            document_with_integrated(Some(&runtime), None, &configurable(&[])),
            document_with_integrated(Some(&runtime), None, &configurable(&[]))
        );
    }

    #[test]
    fn a_failed_required_mod_is_actionable_and_carries_its_callback_attribution() {
        let mut plan = machine(vec![entry("broken_mod", Requirement::Required)]);
        refuse_start(&mut plan, LifecycleState::Constructed);
        let read = fields(
            &document_with_integrated(Some(&running(plan)), None, &configurable(&[]))
                .expect("runtime installed"),
        );
        assert_eq!(read[2], "1", "one entry asks the player to act");
        assert_eq!(read[9], "true");
        assert_eq!(read[11], "1", "a failure block follows");
        assert_eq!(read[12], "", "a callback failure is not a graph failure");
        assert_eq!(read[15], LifecycleState::Constructed.as_str());
        assert_eq!(
            read[18],
            "`broken_mod` refused to enter `constructed`: failed"
        );
    }

    #[test]
    fn a_failure_attribution_keeps_its_kind_version_and_dependency_chain() {
        let mut payload = String::new();
        super::attribution(
            &mut payload,
            &attribution(
                LifecycleState::Constructed,
                Some(FailureKind::DependencyCycle),
            ),
        );
        assert_eq!(
            fields(&payload),
            vec![
                FailureKind::DependencyCycle.as_str(),
                "hook",
                "client.tick",
                "constructed",
                "1.2.3",
                "2",
                "base_mod",
                "broken_mod",
                "failed",
            ],
        );
    }

    #[test]
    fn a_typed_registry_surface_keeps_its_complete_wire_key() {
        let mut payload = String::new();
        let mut failure = attribution(LifecycleState::Running, None);
        failure.surface = Some(FailureSurface::Registry(
            "command:broken_mod:go".parse().expect("valid registry key"),
        ));
        super::attribution(&mut payload, &failure);
        let read = fields(&payload);

        assert_eq!(read[1], "registry");
        assert_eq!(read[2], "command:broken_mod:go");
        assert_eq!(read[3], "running", "the key length preserves field framing");
    }

    #[test]
    fn a_failed_optional_mod_is_reported_without_asking_the_player_to_fix_it() {
        let mut plan = machine(vec![entry("broken_mod", Requirement::Optional)]);
        refuse_start(&mut plan, LifecycleState::Loaded);

        let read = fields(
            &document_with_integrated(Some(&running(plan)), None, &configurable(&[]))
                .expect("runtime installed"),
        );

        assert_eq!(read[2], "0", "the plan tolerates its absence");
        assert_eq!(read[9], "false", "so it is reportable and not actionable");
        assert_eq!(read[11], "1", "but it is still reported");
        assert_eq!(read[12], "", "a failure outside the graph names no kind");
    }

    #[test]
    fn a_failure_detail_is_redacted_before_a_player_can_read_it() {
        let mut broken = attribution(LifecycleState::Loaded, None);
        broken.message = DiagnosticMessage::raw(concat!(
            "could not read /home/player/.minecraft/mods/thing.rmod, token=",
            "ghp_",
            "secretvalue"
        ));
        let mut payload = String::new();
        super::attribution(&mut payload, &broken);
        let read = fields(&payload);
        let detail = read[8].clone();

        assert!(!detail.contains("/home/player"), "{detail}");
        assert!(!detail.contains(concat!("ghp_", "secretvalue")), "{detail}");
        assert!(detail.contains("thing.rmod"), "the file it names survives");
    }

    #[test]
    fn a_row_says_whether_the_screen_has_settings_to_open_for_it() {
        let runtime = running(machine(vec![
            entry("base_mod", Requirement::Required),
            entry("second_mod", Requirement::Required),
        ]));

        let read = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&["second_mod"]))
                .expect("runtime installed"),
        );

        assert_eq!(read[5], "base_mod");
        assert_eq!(read[10], "false", "it declares nothing this side can show");
        assert_eq!(read[14], "second_mod");
        assert_eq!(read[19], "true", "and this one does");
    }

    /// A runtime whose pre-world report already carries one notice.
    fn noticed(fixture: DocumentFixture, notice: &str) -> LoaderRuntime {
        let mut report =
            ravel_observability::PreWorldReport::new(ravel_minecraft_adapter::environment());
        report.note(notice);
        LoaderRuntime::with_observers(
            fixture.machine,
            fixture.mods,
            ravel_observability::Profiler::off(),
            report,
            crate::test_support::test_launch(),
            std::collections::BTreeMap::new(),
        )
    }

    #[test]
    fn a_notice_the_pipeline_recorded_reaches_the_screen_that_shows_attention() {
        let runtime = noticed(
            machine(vec![entry("base_mod", Requirement::Required)]),
            "Ravel wrote the resource pack its mods supply and could not switch it on.",
        );

        let read = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );

        assert_eq!(read[13], "1", "one notice follows the rows");
        assert_eq!(
            read[14], "Ravel wrote the resource pack its mods supply and could not switch it on.",
            "and it crosses whole"
        );
    }

    #[test]
    fn a_notice_is_redacted_before_a_player_can_read_it() {
        let runtime = noticed(
            machine(vec![entry("base_mod", Requirement::Required)]),
            concat!(
                "could not read /home/player/.minecraft/options.txt, token=",
                "ghp_",
                "secretvalue"
            ),
        );

        let read = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );

        assert_eq!(read[13], "1");
        assert!(!read[14].contains("/home/player"), "{}", read[14]);
        assert!(
            !read[14].contains(concat!("ghp_", "secretvalue")),
            "{}",
            read[14]
        );
    }

    #[test]
    fn a_notice_carrying_a_credential_is_marked_once_and_not_twice() {
        let runtime = noticed(
            machine(vec![entry("base_mod", Requirement::Required)]),
            concat!("the launcher passed token=", "hunter2 to the game"),
        );

        let read = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );

        assert!(!read[14].contains("hunter2"), "{}", read[14]);
        assert_eq!(
            read[14], "the launcher passed token=[redacted] to the game",
            "one marker, not a marker over a marker"
        );
    }

    #[test]
    fn a_set_with_nothing_to_say_carries_no_notices_at_all() {
        let runtime = running(machine(vec![entry("base_mod", Requirement::Required)]));

        let read = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );

        assert_eq!(read[13], "0", "the block is present and empty");
        assert_eq!(read.len(), 14, "and nothing follows it");
    }

    struct RefusingObserver(ModId);

    impl ravel_host_api::Mod for RefusingObserver {
        fn id(&self) -> &ModId {
            &self.0
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }

        fn observe(
            &mut self,
            _: ravel_host_api::GameEvent,
        ) -> Result<(), ravel_host_api::ModError> {
            Err(ravel_host_api::ModError::refused("the observer broke"))
        }
    }

    #[test]
    fn the_screen_reads_the_set_as_it_stands_rather_than_as_it_launched() {
        let mut fixture = machine(vec![entry("broken_mod", Requirement::Required)]);
        crate::test_support::advance_through(&mut fixture.machine, LifecycleState::Running);
        let runtime = LoaderRuntime::new(
            fixture.machine,
            vec![Box::new(RefusingObserver(mod_id("broken_mod")))],
            crate::test_support::test_launch(),
        );
        let before = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );
        assert_eq!(before[9], "false");

        assert!(
            runtime
                .legacy_event(ravel_host_api::GameEvent::ClientTick)
                .is_err()
        );

        let after = fields(
            &document_with_integrated(Some(&runtime), None, &configurable(&[]))
                .expect("runtime installed"),
        );
        assert_eq!(after[9], "true", "the second open sees the failure");
    }

    #[test]
    fn integrated_failure_stays_distinct_and_cannot_open_client_settings() {
        let client = running(machine(vec![entry("broken_mod", Requirement::Required)]));
        let mut plan = DocumentFixture::new(
            Side::IntegratedServer,
            vec![entry("broken_mod", Requirement::Required)],
        );
        refuse_start(&mut plan, LifecycleState::Running);
        let server = noticed(plan, "server composition could not register its command");
        let declaration = ravel_config::ConfigurationDeclaration::parse(
            r#"{"schemaVersion":1,"owner":"broken_mod","version":1,"settings":[
              {"key":"server.flag","valueSchema":{"type":"boolean"},"default":false,
               "side":"integrated-server","restartRequired":false}
            ]}"#,
        )
        .expect("server declaration");
        *server
            .launch
            .settings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = crate::settings::Store::new(
            Side::IntegratedServer,
            vec![
                ravel_config::LayeredConfiguration::resolve(declaration, None, None)
                    .expect("server settings"),
            ],
        );
        let read = fields(
            &document_with_integrated(Some(&client), Some(&server), &configurable(&["broken_mod"]))
                .expect("client installed"),
        );

        assert_eq!(&read[..4], &["4", "client", "1", "2"]);
        assert_eq!(read[5], "broken_mod");
        assert_eq!(read[9], "false", "client did not fail");
        assert_eq!(read[10], "true", "client settings retain their identity");
        assert_eq!(read[12], "process:broken_mod");
        assert_eq!(read[14], "broken_mod [integrated-server]");
        assert_eq!(read[18], "true", "server failure is actionable");
        assert_eq!(
            read[19], "true",
            "server settings retain their own identity"
        );
        assert_eq!(read[23], "", "a startup callback has no hook surface");
        assert_eq!(read[24], "running");
        assert_eq!(read[28], "integrated-server:broken_mod");
        assert_eq!(read[29], "1");
        assert_eq!(
            read[30],
            "[integrated-server] server composition could not register its command",
        );
    }

    #[test]
    fn an_integrated_runtime_alone_cannot_supply_a_mod_document() {
        let client = running(machine(vec![entry("base_mod", Requirement::Required)]));
        let settings = configurable(&["base_mod"]);
        assert_eq!(
            document_with_integrated(None, Some(&client), &settings),
            None,
        );
    }
}
