//! Live mod-list and dependency diagnostics encoded for the Java screen.
//!
//! Failure details are redacted here; notices were already redacted by the report.
//! Client screens append labelled integrated-server rows and notices without
//! exposing that runtime's settings through the client store.

use std::collections::BTreeSet;
use std::fmt::Write;

use jni::EnvUnowned;
use jni::objects::{JClass, JString};
use ravel_core::{DiagnosticMessage, FailureAttribution, FailureSurface, ModId};
use ravel_loader::{ModEntry, Requirement};

use crate::guard::hand_document;
use crate::runtime::LoaderRuntime;
use crate::wire::{MODS_VERSION, count, field, flag};

/// Encodes the live mod list, or returns nothing before runtime installation.
fn document() -> Option<String> {
    let configurable = crate::settings::configurable();
    let integrated = crate::runtime::integrated_runtime();
    document_with_integrated(
        crate::runtime::installed().map(AsRef::as_ref),
        integrated.as_deref(),
        &configurable,
    )
}

/// The mod list a screen reads from one runtime, or nothing without one.
fn document_of(runtime: Option<&LoaderRuntime>, configurable: &BTreeSet<ModId>) -> Option<String> {
    Some(runtime?.mod_document(configurable))
}

/// Keeps the client settings identity while adding independently attributed server rows.
fn document_with_integrated(
    runtime: Option<&LoaderRuntime>,
    integrated: Option<&LoaderRuntime>,
    configurable: &BTreeSet<ModId>,
) -> Option<String> {
    let runtime = runtime?;
    let Some(integrated) = integrated else {
        return document_of(Some(runtime), configurable);
    };
    let (client, client_notices) = runtime.mod_view();
    let (server, server_notices) = integrated.mod_view();
    let mut out = String::new();
    field(&mut out, MODS_VERSION);
    field(&mut out, client.side().as_str());
    count(
        &mut out,
        client.actionable().count() + server.actionable().count(),
    );
    count(&mut out, client.entries().len() + server.entries().len());
    for (index, entry) in client.entries().iter().enumerate() {
        row(&mut out, index, entry, configurable.contains(&entry.mod_id));
    }
    let mut label = String::new();
    for (index, entry) in server.entries().iter().enumerate() {
        // The wire's id field is also the display label. Server rows never open
        // client settings, and are not collapsed into an identically named client mod.
        label.clear();
        write!(label, "{} [integrated-server]", entry.mod_id)
            .expect("writing into a String cannot fail");
        labelled_row(
            &mut out,
            client.entries().len() + index,
            entry,
            false,
            &label,
        );
    }
    count(&mut out, client_notices.len() + server_notices.len());
    for notice in &client_notices {
        field(&mut out, notice.text());
    }
    for notice in &server_notices {
        label.clear();
        write!(label, "[integrated-server] {}", notice.text())
            .expect("writing into a String cannot fail");
        field(&mut out, &label);
    }
    Some(out)
}

impl LoaderRuntime {
    /// The whole mod list this runtime holds, as one payload.
    pub(crate) fn mod_document(&self, configurable: &BTreeSet<ModId>) -> String {
        // One acquisition, so the rows and the notice describe the same instant.
        let (surface, notices) = self.mod_view();

        let mut out = String::new();
        field(&mut out, MODS_VERSION);
        field(&mut out, surface.side().as_str());
        count(&mut out, surface.actionable().count());
        count(&mut out, surface.entries().len());
        for (index, entry) in surface.entries().iter().enumerate() {
            row(&mut out, index, entry, configurable.contains(&entry.mod_id));
        }
        count(&mut out, notices.len());
        for notice in &notices {
            field(&mut out, notice.text());
        }
        out
    }
}

/// Writes one row with its one-based position in resolved load order.
fn row(out: &mut String, index: usize, entry: &ModEntry, configurable: bool) {
    labelled_row(out, index, entry, configurable, entry.mod_id.as_str());
}

/// Encodes the unchanged row schema with an explicit display label.
fn labelled_row(out: &mut String, index: usize, entry: &ModEntry, configurable: bool, label: &str) {
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
            // Registry keys contain only validated ASCII, so bytes equal UTF-16 units.
            let units = key.kind().as_str().len()
                + key.id().namespace().as_str().len()
                + key.id().path().len()
                + 2;
            write!(out, "{units}:{key}").expect("writing into a String cannot fail");
        }
        Some(FailureSurface::Packet(name)) => {
            field(out, "packet");
            field(out, name);
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

    use super::document_of;
    use crate::runtime::{DocumentFixture, LoaderRuntime};
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
        LoaderRuntime::new(fixture.machine, fixture.mods, crate::runtime::test_launch())
    }

    fn machine(entries: Vec<LifecycleEntry>) -> DocumentFixture {
        DocumentFixture::new(Side::Client, entries)
    }

    #[test]
    fn a_screen_opened_before_a_set_is_installed_reads_nothing() {
        assert_eq!(document_of(None, &configurable(&[])), None);
    }

    #[test]
    fn a_valid_graph_lists_every_mod_in_resolved_load_order() {
        let runtime = running(machine(vec![
            entry("base_mod", Requirement::Required),
            entry("second_mod", Requirement::Optional),
        ]));

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert_eq!(read[0], "3", "the mod list's own version opens the payload");
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
        assert_eq!(read[12], "2", "and the next row states the next position");
        assert_eq!(read[13], "second_mod");
        assert_eq!(read[15], "optional");
    }

    #[test]
    fn the_same_set_produces_the_same_payload_every_time() {
        let runtime = running(machine(vec![
            entry("base_mod", Requirement::Required),
            entry("second_mod", Requirement::Required),
        ]));

        assert_eq!(
            runtime.mod_document(&configurable(&[])),
            runtime.mod_document(&configurable(&[]))
        );
    }

    #[test]
    fn a_failed_required_mod_is_actionable_and_carries_its_attribution_in_pieces() {
        let mut plan = machine(vec![entry("broken_mod", Requirement::Required)]);
        plan.machine
            .fail(
                &mod_id("broken_mod"),
                attribution(
                    LifecycleState::Constructed,
                    Some(FailureKind::DependencyCycle),
                ),
            )
            .expect("a mod the machine holds can fail");

        let read = fields(&running(plan).mod_document(&configurable(&[])));

        assert_eq!(read[2], "1", "one entry asks the player to act");
        assert_eq!(read[9], "true");
        assert_eq!(read[11], "1", "a failure block follows");
        assert_eq!(read[12], FailureKind::DependencyCycle.as_str());
        assert_eq!(read[13], "hook");
        assert_eq!(read[14], "client.tick");
        assert_eq!(read[15], LifecycleState::Constructed.as_str());
        assert_eq!(read[16], "1.2.3");
        assert_eq!(read[17], "2", "the chain crosses in order");
        assert_eq!(read[18], "base_mod");
        assert_eq!(read[19], "broken_mod");
    }

    #[test]
    fn a_typed_registry_surface_keeps_its_complete_wire_key() {
        let mut plan = machine(vec![entry("broken_mod", Requirement::Required)]);
        let mut failure = attribution(LifecycleState::Running, None);
        failure.surface = Some(FailureSurface::Registry(
            "command:broken_mod:go".parse().expect("valid registry key"),
        ));
        plan.machine
            .fail(&mod_id("broken_mod"), failure)
            .expect("the owner is tracked");

        let read = fields(&running(plan).mod_document(&configurable(&[])));

        assert_eq!(read[13], "registry");
        assert_eq!(read[14], "command:broken_mod:go");
        assert_eq!(
            read[15], "running",
            "the key length preserves field framing"
        );
    }

    #[test]
    fn a_failed_optional_mod_is_reported_without_asking_the_player_to_fix_it() {
        let mut plan = machine(vec![entry("broken_mod", Requirement::Optional)]);
        plan.machine
            .fail(
                &mod_id("broken_mod"),
                attribution(LifecycleState::Loaded, None),
            )
            .expect("a mod the machine holds can fail");

        let read = fields(&running(plan).mod_document(&configurable(&[])));

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
        let mut plan = machine(vec![entry("broken_mod", Requirement::Required)]);
        plan.machine
            .fail(&mod_id("broken_mod"), broken)
            .expect("a mod the machine holds can fail");

        let read = fields(&running(plan).mod_document(&configurable(&[])));
        let detail = read[20].clone();

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

        let read = fields(&runtime.mod_document(&configurable(&["second_mod"])));

        assert_eq!(read[5], "base_mod");
        assert_eq!(read[10], "false", "it declares nothing this side can show");
        assert_eq!(read[13], "second_mod");
        assert_eq!(read[18], "true", "and this one does");
    }

    /// A runtime whose pre-world report already carries one notice.
    fn noticed(fixture: DocumentFixture, notice: &str) -> LoaderRuntime {
        let mut report = crate::runtime::pre_world_report();
        report.note(notice);
        LoaderRuntime::with_observers(
            fixture.machine,
            fixture.mods,
            ravel_observability::Profiler::off(),
            report,
            crate::runtime::test_launch(),
            std::collections::BTreeMap::new(),
        )
    }

    #[test]
    fn a_notice_the_pipeline_recorded_reaches_the_screen_that_shows_attention() {
        let runtime = noticed(
            machine(vec![entry("base_mod", Requirement::Required)]),
            "Ravel wrote the resource pack its mods supply and could not switch it on.",
        );

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert_eq!(read[12], "1", "one notice follows the rows");
        assert_eq!(
            read[13], "Ravel wrote the resource pack its mods supply and could not switch it on.",
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

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert_eq!(read[12], "1");
        assert!(!read[13].contains("/home/player"), "{}", read[13]);
        assert!(
            !read[13].contains(concat!("ghp_", "secretvalue")),
            "{}",
            read[13]
        );
    }

    #[test]
    fn a_notice_carrying_a_credential_is_marked_once_and_not_twice() {
        let runtime = noticed(
            machine(vec![entry("base_mod", Requirement::Required)]),
            concat!("the launcher passed token=", "hunter2 to the game"),
        );

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert!(!read[13].contains("hunter2"), "{}", read[13]);
        assert_eq!(
            read[13], "the launcher passed token=[redacted] to the game",
            "one marker, not a marker over a marker"
        );
    }

    #[test]
    fn a_set_with_nothing_to_say_carries_no_notices_at_all() {
        let runtime = running(machine(vec![entry("base_mod", Requirement::Required)]));

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert_eq!(read[12], "0", "the block is present and empty");
        assert_eq!(read.len(), 13, "and nothing follows it");
    }

    #[test]
    fn the_screen_reads_the_set_as_it_stands_rather_than_as_it_launched() {
        let runtime = running(machine(vec![entry("broken_mod", Requirement::Required)]));
        let before = fields(&runtime.mod_document(&configurable(&[])));
        assert_eq!(before[9], "false");

        runtime.fail_while_running(
            &mod_id("broken_mod"),
            attribution(LifecycleState::Loaded, Some(FailureKind::SideMismatch)),
        );

        let after = fields(&runtime.mod_document(&configurable(&[])));
        assert_eq!(after[9], "true", "the second open sees the failure");
    }

    #[test]
    fn integrated_failure_stays_distinct_and_cannot_open_client_settings() {
        let client = running(machine(vec![entry("broken_mod", Requirement::Required)]));
        let mut plan = DocumentFixture::new(
            Side::IntegratedServer,
            vec![entry("broken_mod", Requirement::Required)],
        );
        let mut failure = attribution(LifecycleState::Running, None);
        failure.surface = Some(FailureSurface::Hook("server.tick".to_owned()));
        plan.machine
            .fail(&mod_id("broken_mod"), failure)
            .expect("tracked mod");
        let server = noticed(plan, "server composition could not register its command");
        let read = fields(
            &super::document_with_integrated(
                Some(&client),
                Some(&server),
                &configurable(&["broken_mod"]),
            )
            .expect("client installed"),
        );

        assert_eq!(&read[..4], &["3", "client", "1", "2"]);
        assert_eq!(read[5], "broken_mod");
        assert_eq!(read[9], "false", "client did not fail");
        assert_eq!(read[10], "true", "client settings retain their identity");
        assert_eq!(read[13], "broken_mod [integrated-server]");
        assert_eq!(read[17], "true", "server failure is actionable");
        assert_eq!(read[18], "false", "server must not open client settings");
        assert_eq!(read[22], "server.tick");
        assert_eq!(read[29], "1");
        assert_eq!(
            read[30],
            "[integrated-server] server composition could not register its command",
        );
    }

    #[test]
    fn absent_integrated_runtime_leaves_mod_document_unchanged() {
        let client = running(machine(vec![entry("base_mod", Requirement::Required)]));
        let settings = configurable(&["base_mod"]);
        assert_eq!(
            super::document_with_integrated(Some(&client), None, &settings),
            document_of(Some(&client), &settings),
        );
        assert_eq!(
            super::document_with_integrated(None, Some(&client), &settings),
            None,
        );
    }
}
