//! The mod list and dependency diagnostics crossing.
//!
//! `ROADMAP.md`'s M3-W5 row asks for a mod list and diagnostics UI over the
//! model `ravel-loader` already holds, ordered by resolved load order, and for
//! valid and invalid graphs to render deterministic, actionable messages. Ravel
//! declares in data and draws in Java, and M3-W5's mod list uses
//! the same mechanism as the settings screen rather than a second one. So this
//! is one read crossing carrying [`ModSurface`], on the framing the settings
//! screen already uses, and no rendering at all.
//!
//! # A notice is carried beside the rows
//!
//! The pre-world report holds what the pipeline told a player without refusing
//! them, and until this crossing carried it nothing read it after it was
//! recorded. It rides here because the mods screen's attention summary is
//! already the surface a player checks when something looks wrong, and a second
//! screen for one line would be a second place to forget to look.
//!
//! # The surface is read live rather than snapshotted
//!
//! The store holds the runtime, not a copy of its answer, and every open asks it
//! afresh. A diagnostics screen that showed a set as it stood at launch would go
//! on saying every mod is fine after one of them failed — and a check that is
//! guaranteed to look healthy at the moment something breaks is not a weak
//! diagnostic, it is a misleading one. Building the surface takes the runtime's
//! lock briefly to walk the machine and release it; the payload is assembled
//! after the lock is gone, so nothing is held while the screen draws.
//!
//! # Attribution crosses as attribution
//!
//! [`FailureAttribution`] is structured on purpose, and `surface.rs` says why: a
//! renderer applies its own redaction policy rather than receiving text that has
//! already been flattened past the point of redacting it. So the fields cross as
//! fields — kind, surface, the dependency chain in order, the state it failed
//! in — and the screen composes whatever sentence it shows. Nothing here writes
//! a sentence for it.
//!
//! What this module *does* own is the redaction, because it is the report
//! surface: `dependency-rules.md` records that every failure in the workspace
//! reports its own text as [`Redaction::Raw`] and the report surface redacts it
//! on the way in. Showing a player the raw text would publish exactly the
//! launcher tokens and filesystem paths `SPEC.md` §9.1 forbids, so the detail
//! runs through [`Redactor`] here and the payload carries only what came back.
//!
//! A notice is the exception, and it is not one: `PreWorldReport::note` is the
//! report surface a notice already crossed, and it redacts there. The policy is
//! not idempotent — a marker standing where a credential's value stood is
//! itself the word after a credential's name, so a second pass marks the marker
//! — so this carries what the report holds.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};
use ravel_core::{DiagnosticMessage, FailureAttribution, FailureSurface, ModId, Side};
use ravel_loader::{ModEntry, Requirement};
use ravel_observability::Redactor;

use crate::guard::HookError;
use crate::runtime::LoaderRuntime;
use crate::wire::{MODS_VERSION, count, field, flag};

/// The installed runtime, or nothing before one is installed.
///
/// The payload is the runtime's own method, so what a screen reads is whichever
/// runtime it was handed. This static is the one a launch installs into.
static RUNTIME: Mutex<Option<Arc<LoaderRuntime>>> = Mutex::new(None);

/// Makes the running mod set readable by the mod list screen.
///
/// Called wherever the runtime is installed rather than by a second caller, so
/// there is no launch in which hooks reach the mod set but the screen does not.
pub(crate) fn install(runtime: Arc<LoaderRuntime>) {
    *lock() = Some(runtime);
}

/// Takes the store, recovering it from a previous panic.
///
/// Nothing here mutates the runtime, so a poisoned lock cannot mean a half-built
/// one. Refusing it afterwards would mean one bad open leaves a player with no
/// way to see why their mods are broken, which is the screen this is.
fn lock() -> std::sync::MutexGuard<'static, Option<Arc<LoaderRuntime>>> {
    RUNTIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The whole mod list, as one payload.
///
/// Returns nothing before a set is installed, which is a launch that has not got
/// far enough to have a mod list rather than a failure.
fn document() -> Option<String> {
    // Asked before the runtime is taken, so the two stores' locks are never held
    // at once and `settings.rs`'s opening claim stays true.
    let configurable = crate::settings::configurable();
    // Bound rather than passed inline so the guard drops here: the store's lock
    // is not held while the surface is built.
    let runtime = lock().clone();
    document_of(runtime.as_deref(), &configurable)
}

/// The mod list a screen reads from one runtime, or nothing without one.
///
/// Separate from [`document`] so the absent case is a value a caller passes
/// rather than the state of the process's own store. A test that had to empty
/// that store to see the case would be asserting against every other test in
/// the process, and the plain runner shares one.
fn document_of(runtime: Option<&LoaderRuntime>, configurable: &BTreeSet<ModId>) -> Option<String> {
    Some(runtime?.mod_document(configurable))
}

impl LoaderRuntime {
    /// The whole mod list this runtime holds, as one payload.
    pub(crate) fn mod_document(&self, configurable: &BTreeSet<ModId>) -> String {
        // One acquisition, so the rows and the notice describe the same instant.
        let (surface, notices) = self.mod_view();

        let mut out = String::new();
        field(&mut out, MODS_VERSION);
        field(&mut out, side_name(surface.side()));
        count(&mut out, surface.actionable().count());
        count(&mut out, surface.entries().len());
        for (index, entry) in surface.entries().iter().enumerate() {
            row(&mut out, index, entry, configurable.contains(&entry.mod_id));
        }
        // After the rows rather than beside the counts, because a notice is not a
        // mod and the screen composes its own line for one.
        count(&mut out, notices.len());
        for notice in &notices {
            field(&mut out, notice.text());
        }
        out
    }
}

/// Writes one mod's row.
///
/// The position is the index into the surface's own entries, which `surface.rs`
/// guarantees to be the resolved load order — so it is derived from the order
/// rather than stated beside it, and the two cannot disagree. It is numbered
/// from one because it is read by a player rather than by a program.
///
/// Every row carries one, not only the ones a screen might move. A player
/// comparing two rows needs both numbers, and numbering some would read as those
/// being the special ones.
fn row(out: &mut String, index: usize, entry: &ModEntry, configurable: bool) {
    count(out, index + 1);
    field(out, entry.mod_id.as_str());
    field(out, entry.state.name());
    field(out, requirement_name(entry.requirement));
    flag(out, entry.loads_here);
    flag(out, entry.is_actionable());
    flag(out, configurable);
    match &entry.failure {
        // A count rather than a flag, so the screen reads the block the same way
        // it reads every other repeated group on this wire.
        None => count(out, 0),
        Some(failure) => {
            count(out, 1);
            attribution(out, failure);
        }
    }
}

/// Writes one failure's structured attribution.
fn attribution(out: &mut String, failure: &FailureAttribution) {
    field(out, failure.kind.map_or("", |kind| kind.name()));
    let (surface_kind, surface_name) = surface_of(failure.surface.as_ref());
    field(out, surface_kind);
    field(out, surface_name);
    field(out, failure.state.name());
    field(
        out,
        failure
            .version
            .as_ref()
            .map_or_else(String::new, ToString::to_string)
            .as_str(),
    );
    // The environment — Ravel and Minecraft versions, platform, architecture —
    // is deliberately not carried. It is identical for every row, so sending it
    // per failure would repeat one fact per mod, and a mod list is not where a
    // player reads it. A bug report is, and that is a report file rather than
    // this screen. Carry it here the day a screen has somewhere to put it.
    count(out, failure.dependency_chain.len());
    for link in &failure.dependency_chain {
        field(out, link.as_str());
    }
    field(out, publishable(&failure.message).text());
}

/// Runs the workspace's redaction policy over a failure's detail.
///
/// Always runs it, rather than running it only over text tagged
/// [`ravel_core::Redaction::Raw`]. Trusting a tag would mean the one place that
/// publishes text to a player decides what to redact by asking whether somebody
/// upstream says they already did, and the policy removes paths and credentials
/// that are not there a second time.
fn publishable(message: &DiagnosticMessage) -> DiagnosticMessage {
    DiagnosticMessage::redacted(&Redactor, message.text())
}

/// Names a failure surface as a kind and the identifier it carries.
///
/// Two fields rather than one joined string: the kind is a fixed word the screen
/// can translate and the name is a mod-facing identifier it cannot, and joining
/// them here would be composing the sentence the screen is supposed to compose.
const fn surface_of(surface: Option<&FailureSurface>) -> (&'static str, &str) {
    match surface {
        None => ("", ""),
        Some(FailureSurface::Hook(name)) => ("hook", name.as_str()),
        Some(FailureSurface::Event(name)) => ("event", name.as_str()),
        Some(FailureSurface::Registry(name)) => ("registry", name.as_str()),
        Some(FailureSurface::Packet(name)) => ("packet", name.as_str()),
    }
}

/// Names whether the set fails closed without a mod.
const fn requirement_name(requirement: Requirement) -> &'static str {
    match requirement {
        Requirement::Required => "required",
        Requirement::Optional => "optional",
    }
}

/// Names the side a set is running on.
const fn side_name(side: Side) -> &'static str {
    match side {
        Side::Client => "client",
        Side::IntegratedServer => "integrated-server",
        Side::DedicatedServer => "dedicated-server",
    }
}

/// Hands the screen the whole installed mod set.
///
/// # Panics
///
/// Does not panic. The guard around the body turns a panic into a thrown
/// `RuntimeException` rather than an unwind into the JVM.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelModSurface")]
pub fn ravel_mod_surface<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            match document() {
                Some(document) => Ok(env.new_string(document)?),
                None => Ok(JString::default()),
            }
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use ravel_core::{
        DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, FailureSurface,
        LifecycleState, ModId, Side, SideSet,
    };
    use ravel_host_api::HostMod;
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};

    use std::collections::BTreeSet;

    use super::document_of;
    use crate::runtime::LoaderRuntime;

    /// Reads a payload back into its fields, the way the screen does.
    fn fields(payload: &str) -> Vec<String> {
        let units: Vec<u16> = payload.encode_utf16().collect();
        let mut read = Vec::new();
        let mut at = 0;
        while at < units.len() {
            let separator = at
                + units[at..]
                    .iter()
                    .position(|unit| *unit == u16::from(b':'))
                    .expect("every field states a length");
            let length: usize = String::from_utf16(&units[at..separator])
                .expect("a length is text")
                .parse()
                .expect("a length is a number");
            let start = separator + 1;
            read.push(String::from_utf16(&units[start..start + length]).expect("a field is text"));
            at = start + length;
        }
        read
    }

    fn mod_id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    /// The owners a screen could open, as a test states them.
    fn configurable(names: &[&str]) -> BTreeSet<ModId> {
        names.iter().copied().map(mod_id).collect()
    }

    fn entry(name: &str, requirement: Requirement) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: mod_id(name),
            sides: SideSet::new([Side::Client, Side::IntegratedServer]).expect("two sides"),
            requirement,
            saved_data_versions: std::collections::BTreeSet::new(),
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

    /// A runtime holding that machine as its running set.
    ///
    /// The runtime is the test's own rather than the process's, so what one test
    /// reads back is the set it built and no other.
    fn running(machine: LifecycleMachine) -> LoaderRuntime {
        LoaderRuntime::new(machine, Vec::<Box<dyn HostMod>>::new())
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::Client, entries).expect("a legal plan")
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
        assert_eq!(read[6], LifecycleState::Discovered.name());
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
    fn every_row_is_numbered_and_the_numbers_are_the_resolved_load_order() {
        let runtime = running(machine(vec![
            entry("base_mod", Requirement::Required),
            entry("second_mod", Requirement::Optional),
            entry("third_mod", Requirement::Required),
        ]));

        let read = fields(&runtime.mod_document(&configurable(&[])));

        // Eight fields per unfailed row, after the four the payload opens with.
        let positions: Vec<&str> = (0..3).map(|row| read[4 + row * 8].as_str()).collect();
        let names: Vec<&str> = (0..3).map(|row| read[5 + row * 8].as_str()).collect();

        assert_eq!(positions, ["1", "2", "3"], "every row is numbered");
        assert_eq!(
            names,
            ["base_mod", "second_mod", "third_mod"],
            "and the numbers follow the resolved load order rather than a second one"
        );
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
        plan.fail(
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
        assert_eq!(read[12], FailureKind::DependencyCycle.name());
        assert_eq!(read[13], "hook");
        assert_eq!(read[14], "client.tick");
        assert_eq!(read[15], LifecycleState::Constructed.name());
        assert_eq!(read[16], "1.2.3");
        assert_eq!(read[17], "2", "the chain crosses in order");
        assert_eq!(read[18], "base_mod");
        assert_eq!(read[19], "broken_mod");
    }

    #[test]
    fn a_failed_optional_mod_is_reported_without_asking_the_player_to_fix_it() {
        let mut plan = machine(vec![entry("broken_mod", Requirement::Optional)]);
        plan.fail(
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
        broken.message = DiagnosticMessage::raw(
            "could not read /home/player/.minecraft/mods/thing.rmod, token=ghp_secretvalue",
        );
        let mut plan = machine(vec![entry("broken_mod", Requirement::Required)]);
        plan.fail(&mod_id("broken_mod"), broken)
            .expect("a mod the machine holds can fail");

        let read = fields(&running(plan).mod_document(&configurable(&[])));
        let detail = read[20].clone();

        assert!(!detail.contains("/home/player"), "{detail}");
        assert!(!detail.contains("ghp_secretvalue"), "{detail}");
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
    fn noticed(machine: LifecycleMachine, notice: &str) -> LoaderRuntime {
        let mut report = crate::runtime::pre_world_report();
        report.note(notice);
        LoaderRuntime::profiling(
            machine,
            Vec::<Box<dyn HostMod>>::new(),
            ravel_observability::Profiler::off(),
            report,
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
            "could not read /home/player/.minecraft/options.txt, token=ghp_secretvalue",
        );

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert_eq!(read[12], "1");
        assert!(!read[13].contains("/home/player"), "{}", read[13]);
        assert!(!read[13].contains("ghp_secretvalue"), "{}", read[13]);
    }

    /// Redacted once, which is the only number of times that reads as English.
    ///
    /// The policy is not idempotent: a marker standing where a credential's
    /// value stood is itself the word after a credential's name, so a second
    /// pass marks the marker and the player reads nested brackets.
    #[test]
    fn a_notice_carrying_a_credential_is_marked_once_and_not_twice() {
        let runtime = noticed(
            machine(vec![entry("base_mod", Requirement::Required)]),
            "the launcher passed token hunter2 to the game",
        );

        let read = fields(&runtime.mod_document(&configurable(&[])));

        assert!(!read[13].contains("hunter2"), "{}", read[13]);
        assert_eq!(
            read[13], "the launcher passed token [redacted] to the game",
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

        // Fail the mod after the screen has already read the set once.
        runtime.fail_while_running(
            &mod_id("broken_mod"),
            attribution(LifecycleState::Loaded, Some(FailureKind::SideMismatch)),
        );

        let after = fields(&runtime.mod_document(&configurable(&[])));
        assert_eq!(after[9], "true", "the second open sees the failure");
    }
}
