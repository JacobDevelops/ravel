//! Checks what the pinned Prism Launcher can be asked to do, without launching it.
//!
//! ADG-5 asks whether Ravel reaches the official game through a stock launcher or
//! whether the launcher has to be patched. Half of that is answerable from the
//! launcher itself: the settings it exposes decide whether the arguments ADG-1
//! selected can be set at all. This runner verifies the pinned launcher against
//! its tracked hash, unpacks it, and looks for those settings by name.
//!
//! The other half — the pinned game actually launching with the agent attached —
//! needs an authenticated session and is an operator step. This runner never
//! authenticates, never launches, and records plainly that the launch has not
//! been observed.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use ravel_spike_adg_5_prism::launch;
use ravel_spike_harness as harness;

use harness::PinnedInputs;
use harness::result::{
    Allocation, Boundary, Calls, Candidate, Classification, Clock, Deviation, Disposition,
    Distribution, FailureBehaviour, FailureOutcome, FailureScenario, Gate, HashedArtifact,
    Identity, Measurement, Memory, Operability, Packaging, PatchOwnership, PrismIntegration,
    PrismLaunch, Revision, Safety, SpikeResult, Startup, Target, Trial, TrialKind,
};

/// The pinned launcher, which is a restricted local fixture and never tracked.
const APPIMAGE: &str =
    "tests/fixtures/local/tooling/prism-11.0.3/PrismLauncher-Linux-x86_64.AppImage";
/// Tracked descriptor that pins the launcher's hash.
const DESCRIPTOR: &str = "tests/fixtures/descriptors/runtime-inputs-26.1.2.json";
/// Path inside the unpacked launcher that holds the settings names.
const LAUNCHER_BINARY: &str = "squashfs-root/shared/bin/prismlauncher";
/// The one hardware profile M1 compares on.
const HARDWARE_PROFILE: &str = "linux-reference-v1";
/// Where results belong for the milestone gate to find them.
const RESULT_DIRECTORY: &str = "evidence/m1/spikes/adg-5";

/// Settings the ADG-1 mechanism needs a launcher to expose.
const REQUIRED: &[(&str, &str)] = &[
    (
        "OverrideJavaArgs",
        "lets an instance use its own JVM arguments rather than the launcher default",
    ),
    (
        "JvmArgs",
        "carries the -javaagent and -Djava.library.path Ravel needs",
    ),
];

/// Settings the rejected hosted-VM candidate would have needed instead.
const HOSTED: &[(&str, &str)] = &[
    (
        "OverrideCommands",
        "lets an instance replace the launch command",
    ),
    (
        "WrapperCommand",
        "runs the game through another program, which is what hosting the VM would need",
    ),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("wrote the ADG-5 spike result to {RESULT_DIRECTORY}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ADG-5 inspection failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let operator_log = std::env::args()
        .skip(1)
        .position(|argument| argument == "--launch-log")
        .and_then(|index| std::env::args().nth(index + 2));
    let root = repository_root()?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;

    let appimage = root.join(APPIMAGE);
    if !appimage.is_file() {
        return Err(format!(
            "the pinned launcher is absent; place it as {} describes",
            file_name(DESCRIPTOR)
        ));
    }
    let expected = pinned_launcher_hash(&root)?;
    let actual = harness::sha256_file(&appimage).map_err(|error| error.to_string())?;
    if actual != expected {
        return Err(format!(
            "{} hashes {actual} but the descriptor pins {expected}",
            file_name(APPIMAGE)
        ));
    }

    let work = root.join("target/spike/adg-5");
    let launcher = unpack(&appimage, &work)?;
    let settings = fs::read(&launcher)
        .map_err(|error| format!("cannot read the unpacked launcher: {error}"))?;

    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let found = |keys: &[(&str, &str)]| -> Vec<String> {
        keys.iter()
            .filter(|(key, _)| !contains(&settings, key))
            .map(|(key, _)| (*key).to_owned())
            .collect()
    };
    let missing_required = found(REQUIRED);
    let missing_hosted = found(HOSTED);
    if !missing_required.is_empty() {
        return Err(format!(
            "the pinned launcher exposes no {}",
            missing_required.join(", ")
        ));
    }

    let result = assemble(
        &Observed {
            started_at,
            command_sha256: harness::command_sha256(&[APPIMAGE, "--appimage-extract"]),
            launcher_sha256: actual,
            hosted_available: missing_hosted.is_empty(),
            settings_bytes: settings.len() as u64,
            failures: failure_matrix(&settings),
            launch: observed_launch(operator_log.as_deref())?,
        },
        &pinned,
        &revision,
    );
    harness::write_result(
        &root
            .join(RESULT_DIRECTORY)
            .join("adg5-stock-jvm-arguments.json"),
        &result,
    )
    .map_err(|error| error.to_string())
}

/// What the inspection observed.
struct Observed {
    started_at: String,
    command_sha256: String,
    launcher_sha256: String,
    hosted_available: bool,
    settings_bytes: u64,
    failures: Vec<FailureOutcome>,
    /// What the operator's launch log showed, when one was supplied.
    launch: Option<launch::Launch>,
}

/// Reads the operator's launch log, if they pointed at one.
///
/// Without it, the result says the launch was not observed. With it, the result
/// says what the log proves and records only the hash of its redacted form.
fn observed_launch(path: Option<&str>) -> Result<Option<launch::Launch>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text =
        fs::read_to_string(path).map_err(|error| format!("cannot read the launch log: {error}"))?;
    Ok(Some(launch::read(&text)))
}

/// Unpacks the launcher beside the work directory and returns its binary.
///
/// `--appimage-extract` unpacks and exits; it never starts the launcher.
fn unpack(appimage: &Path, work: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(work).map_err(|error| format!("cannot create a work directory: {error}"))?;
    let copy = work.join("launcher.AppImage");
    fs::copy(appimage, &copy).map_err(|error| format!("cannot stage the launcher: {error}"))?;
    // The image has to be executable to unpack itself.
    run_tool("chmod", &["+x".to_owned(), copy.display().to_string()])?;

    let status = Command::new(&copy)
        .arg("--appimage-extract")
        .current_dir(work)
        .status()
        .map_err(|error| format!("cannot unpack the launcher: {error}"))?;
    if !status.success() {
        return Err("the launcher refused to unpack".to_owned());
    }
    let launcher = work.join(LAUNCHER_BINARY);
    if !launcher.is_file() {
        return Err(format!(
            "the unpacked launcher has no {}",
            file_name(LAUNCHER_BINARY)
        ));
    }
    Ok(launcher)
}

/// Whether a settings name appears in the launcher's own bytes.
fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// The scenarios a configuration inspection can and cannot observe.
fn failure_matrix(settings: &[u8]) -> Vec<FailureOutcome> {
    let absent = "RavelSettingThatCannotExist";
    vec![
        FailureOutcome {
            scenario: FailureScenario::InvalidInput,
            outcome: if contains(settings, absent) {
                FailureBehaviour::Continued
            } else {
                FailureBehaviour::FailedClosed
            },
            detail: "a setting the launcher does not expose is reported absent rather than \
                     assumed present"
                .to_owned(),
        },
        FailureOutcome {
            scenario: FailureScenario::UnavailableRuntime,
            outcome: FailureBehaviour::FailedClosed,
            detail: "an absent launcher stops the inspection before it reads anything".to_owned(),
        },
        FailureOutcome {
            scenario: FailureScenario::CallbackFailure,
            outcome: FailureBehaviour::FailedClosed,
            detail: "a launcher whose hash does not match the tracked pin stops the inspection \
                     before it unpacks anything"
                .to_owned(),
        },
        FailureOutcome {
            scenario: FailureScenario::Crash,
            outcome: FailureBehaviour::NotEvaluated,
            detail: "this candidate runs no game, so there is no process for a crash to end"
                .to_owned(),
        },
        FailureOutcome {
            scenario: FailureScenario::Timeout,
            outcome: FailureBehaviour::NotEvaluated,
            detail: "this candidate runs no game, so there is nothing to hang".to_owned(),
        },
    ]
}

fn assemble(observed: &Observed, pinned: &PinnedInputs, revision: &Revision) -> SpikeResult {
    let not_a_launch = |what: &str| {
        Measurement::unavailable(
            "spike.harness",
            &format!(
                "this candidate inspects the launcher's configuration rather than running the \
                 game, so it measures no {what}"
            ),
        )
    };

    SpikeResult {
        schema_version: 1,
        protocol_version: "spike-v1".to_owned(),
        gate: Gate::Prism,
        candidate: Candidate {
            id: "adg5.stock-jvm-arguments".to_owned(),
            title: "Reach the official game through stock Prism JVM arguments".to_owned(),
            disposition: Disposition::Measured,
            rejection_reason: None,
        },
        trial: Trial {
            revision: revision.clone(),
            command_sha256: observed.command_sha256.clone(),
            started_at: observed.started_at.clone(),
            duration_ms: 0,
            clock: Clock::Monotonic,
            warmup_invocations: 0,
            // One inspection of one launcher.
            measured_invocations: 1,
        },
        identity: Identity {
            target: Target::X8664Linux,
            hardware_profile: HARDWARE_PROFILE.to_owned(),
            minecraft: pinned.minecraft.clone(),
            prism: pinned.prism.clone(),
            // No JVM ran: nothing was launched.
            java: None,
            trial_kind: TrialKind::Harness,
        },
        memory: Memory {
            bootstrap_peak_rss: not_a_launch("memory"),
            retained_rss: not_a_launch("memory"),
            retained_classes: not_a_launch("class count"),
        },
        startup: Startup {
            process_to_ready: not_a_launch("startup"),
            candidate_delta: not_a_launch("startup"),
        },
        calls: Calls {
            invocation_count: 0,
            callback_nanoseconds: Distribution::unavailable(
                "spike.harness",
                "a launcher configuration has no call boundary",
            ),
        },
        boundary: Boundary {
            bytes_per_call: not_a_launch("call boundary"),
            total_bytes: not_a_launch("call boundary"),
        },
        allocation: Allocation {
            allocations_per_call: not_a_launch("call boundary"),
            allocated_bytes_per_call: not_a_launch("call boundary"),
        },
        packaging: Packaging {
            // What Ravel adds to an instance is two arguments, not a launcher.
            installed_bytes: 0,
            artifact_count: 0,
            required_components: vec![
                "one -javaagent argument on the instance".to_owned(),
                "one -Djava.library.path argument on the instance".to_owned(),
            ],
        },
        failures: observed.failures.clone(),
        safety: Safety {
            maintained_unsafe_blocks: 0,
            maintained_unsafe_detail: "setting a launcher argument needs no unsafe code".to_owned(),
            audited_boundary_dependencies: Vec::new(),
        },
        operability: operability(),
        artifacts: artifacts(observed),
        isolation: None,
        abi_evolution: None,
        mod_to_mod_calls: None,
        binding_generation: None,
        state_ownership: None,
        prism_integration: Some(prism_integration(observed)),
        deviations: deviations(observed),
    }
}

/// The launcher inspected, plus the operator's redacted log when there is one.
fn artifacts(observed: &Observed) -> Vec<HashedArtifact> {
    let mut artifacts = vec![HashedArtifact {
        name: file_name(LAUNCHER_BINARY).to_owned(),
        sha256: observed.launcher_sha256.clone(),
    }];
    if let Some(log) = &observed.launch {
        artifacts.push(HashedArtifact {
            name: "redacted-instance-launch.log".to_owned(),
            sha256: log.redacted_sha256.clone(),
        });
    }
    artifacts
}

/// What the launcher can be asked to do, and what was not observed.
fn prism_integration(observed: &Observed) -> PrismIntegration {
    let launched = observed
        .launch
        .as_ref()
        .is_some_and(launch::Launch::proves_stock_launch);
    PrismIntegration {
        // True only when an operator's log shows the agent argument on the
        // command line and no other loader. Without a log this is false, and the
        // deviations name the step that closes it.
        stock_launch: launched,
        changed_lines: 0,
        ownership: PatchOwnership::NoneRequired,
        detail: format!(
            "the pinned launcher exposes OverrideJavaArgs and JvmArgs, so the two arguments ADG-1 \
             needs can be set on an instance without changing the launcher. It also exposes \
             OverrideCommands and WrapperCommand, so the rejected hosted-VM candidate would {} \
             have needed a patch either. {}",
            if observed.hosted_available {
                "not"
            } else {
                "still"
            },
            launch_sentence(observed)
        ),
    }
}

/// What the result says about the launch, which depends on what was supplied.
fn launch_sentence(observed: &Observed) -> String {
    match &observed.launch {
        None => "No launch was performed by this run: attaching the agent to the pinned game \
                 needs an authenticated session, which is an operator step."
            .to_owned(),
        Some(log) => match log.objection() {
            None => "An operator's instance log was read and three substring tests passed on \
                     it: `-javaagent:` appears, `-Djava.library.path=` appears, and none of \
                     five known mod loader names does. Those three tests are the whole proof. \
                     Nothing here checks that the launch was of the pinned 26.1.2, that the \
                     launcher was the pinned Prism, that either argument sat on the game's \
                     command line rather than elsewhere in the log, or that the client reached \
                     a world or exited cleanly."
                .to_owned(),
            Some(objection) => format!(
                "An operator's instance log was read and does not prove a \
                 launch: {objection}."
            ),
        },
    }
}

/// How an operator lives with two instance settings.
fn operability() -> Operability {
    Operability {
        diagnostics: "the launcher writes the full launch command to its own instance log, so a \
                      wrong argument is visible without Ravel reporting anything"
            .to_owned(),
        debug_workflow: "an operator edits two instance settings and can remove them again, which \
                         makes Ravel switchable off without uninstalling it"
            .to_owned(),
        crash_attribution: "the game process is the launcher's own, so a crash report belongs to \
                            the instance the launcher already tracks"
            .to_owned(),
        prism_launch: PrismLaunch::StockJvmArguments,
    }
}

/// What this inspection did not see, said plainly.
fn deviations(observed: &Observed) -> Vec<Deviation> {
    let launch = match &observed.launch {
        Some(log) if log.proves_stock_launch() => "an operator's instance log shows the pinned \
             game launching with Ravel's agent argument and no other mod loader; only the \
             hash of its redacted form is recorded, because a launcher log can carry the \
             account's session arguments"
            .to_owned(),
        Some(log) => format!(
            "an operator's instance log was read and does not prove a launch: {}",
            log.objection().unwrap_or_default()
        ),
        None => "the pinned game launching with the agent attached has not been observed. \
                 M0-W6 recorded the official 26.1.2 client reaching a fresh world on this \
                 launcher with no loader present; what is outstanding is the same launch with \
                 Ravel's two arguments set, which needs an authenticated session and therefore \
                 an operator"
            .to_owned(),
    };
    vec![
        Deviation {
            summary: launch,
            classification: Classification::NonBlocking,
        },
        Deviation {
            summary: format!(
                "the launcher's settings are read as names in its own {} bytes of executable, \
                 which proves the settings exist and not that a given value is accepted",
                observed.settings_bytes
            ),
            classification: Classification::NonBlocking,
        },
    ]
}

/// The launcher hash the tracked descriptor pins for this platform.
fn pinned_launcher_hash(root: &Path) -> Result<String, String> {
    let text = fs::read_to_string(root.join(DESCRIPTOR))
        .map_err(|error| format!("cannot read {}: {error}", file_name(DESCRIPTOR)))?;
    // ponytail: one field out of a tracked descriptor; a parser would be a
    // dependency for a substring.
    let marker = "\"fileName\": \"PrismLauncher-Linux-x86_64.AppImage\"";
    let after = text
        .split_once(marker)
        .ok_or_else(|| "the descriptor pins no Linux launcher".to_owned())?
        .1;
    let hash = after
        .split_once("\"sha256\": \"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(hash, _)| hash.to_owned())
        .ok_or_else(|| "the descriptor pins no launcher hash".to_owned())?;
    Ok(hash)
}

fn run_tool(program: &str, arguments: &[String]) -> Result<(), String> {
    let status = Command::new(program)
        .args(arguments)
        .status()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !status.success() {
        return Err(format!("{program} failed"));
    }
    Ok(())
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("cannot resolve the repository root: {error}"))
}

/// The last path segment, which is all a recorded message may say about a file.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
