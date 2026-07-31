//! Records what one real Prism launch of the pinned client proved about Ravel.
//!
//! M2-W5's row asks for the official 26.1.2 process reaching Ravel without
//! another loader. A synthetic JVM harness cannot answer it: it proves the
//! agent, the transformer and the JNI binding work, and says nothing about the
//! client. This reads the launcher's own log from a launch an operator ran with
//! Ravel's two ADG-5 arguments attached, and writes what it proves.
//!
//! Nothing the account owns survives. The log is redacted through the same
//! module the ADG-5 runner uses, only the digest of the redacted form is
//! recorded, and no line of the log reaches the result. The log itself is the
//! operator's to delete, and it lives outside the repository.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ravel_spike_adg_5_prism::launch;
use ravel_spike_harness as harness;
use serde_json::{Value, json};

/// Where the record belongs for the M2 gate to find it.
const RESULT: &str = "evidence/m2/prism-launch.json";
/// The premain agent the launch attached.
const AGENT_JAR: &str = "target/ravel-jvm/ravel-agent.jar";
/// The native library the rewritten call sites bind to.
const NATIVE_LIBRARY: &str = "target/release/libravel_bootstrap.so";

/// What the JVM prints when Ravel's own class loads the native library.
///
/// The warning is the runtime's, not Ravel's, and it names the calling class, so
/// it is evidence that survives whatever the game does to `System.out`.
const HOOK_LOADED_LIBRARY: &str = "System::loadLibrary has been called by dev.ravel.jvm.Hook";

/// What the two arguments on the command line do and do not prove.
///
/// The count matters: Ravel is two JVM arguments, the launch configuration that
/// produced this record was three stock settings, and only one of the three is
/// what the conclusion rests on.
const LAUNCH_DETAIL: &str = "ADG-5 decided Ravel reaches the game through two stock JVM arguments \
                             and no launcher change, and both appear on the command the launcher \
                             built. The conclusion rests on the -javaagent argument alone: \
                             removing it switches Ravel off without uninstalling anything, while \
                             the launcher appends its own -Djava.library.path after the \
                             instance's and the last one wins, so the instance's copy is inert. \
                             The instance carried a third stock setting beyond Ravel's two \
                             arguments, a wrapper command that only tees the process's output so \
                             the agent's shutdown report could be captured. Both are recorded as \
                             deviations below.";

/// What the instance directory shows Ravel installed, which is nothing.
const INSTANCE_DETAIL: &str = "The instance's own component list and mod directories, read after \
                               the launch. Ravel installs neither a component nor a file here: it \
                               is two JVM arguments. The launched instance carried a third stock \
                               setting as well, the wrapper command that captured the agent's \
                               shutdown report, which is a capture aid rather than part of Ravel.";

/// Component identifiers that mean a Java mod loader is installed.
const LOADER_COMPONENTS: &[&str] = &[
    "net.neoforged",
    "net.minecraftforge",
    "net.fabricmc",
    "org.quiltmc",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("wrote the M2 launch record to {RESULT}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the launch record could not be written: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let (log_path, instance) = arguments()?;
    let root = repository_root()?;

    let text = fs::read_to_string(&log_path)
        .map_err(|error| format!("cannot read the launch log: {error}"))?;
    let observed = launch::read(&text);
    let report = Report::read(&text);
    let in_process = text.contains(HOOK_LOADED_LIBRARY);
    let instance = Instance::scan(Path::new(&instance))?;
    drop(text);

    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;

    let record = json!({
        "schemaVersion": 1,
        "record": "m2-prism-launch",
        "milestone": "M2",
        "workPackages": ["M2-W5", "M2-W8"],
        "summary": "The pinned official client launched from stock Prism with Ravel's two \
                    ADG-5 arguments attached, rewrote its target classes inside the real game \
                    process, and reached native code, with no other mod loader installed.",
        "identity": {
            "platform": "linux",
            "architecture": "x86_64",
            "hardwareProfile": "linux-reference-v1",
            "minecraft": pinned.minecraft,
            "prism": pinned.prism,
            "java": pinned.java,
        },
        "trial": {
            "startedAt": started_at,
            "revision": {
                "id": revision.id,
                "changeId": revision.change_id,
                "dirty": revision.dirty,
            },
        },
        "launch": {
            "agentArgument": observed.agent,
            "libraryPathArgument": observed.library_path,
            "otherLoaderNamed": observed.other_loader,
            "provesStockLaunch": observed.proves_stock_launch(),
            "objection": observed.objection(),
            "attestation": attested_launcher(),
            "detail": LAUNCH_DETAIL,
        },
        "ravel": {
            "reachedGameProcess": in_process || report.reached_ravel(),
            "hookClassInitialized": in_process,
            "evidence": "The JVM's own restricted-method warning names dev.ravel.jvm.Hook as the \
                         caller of System.loadLibrary, and only that class's static initializer \
                         calls it, so Ravel's Java class ran and its native library loaded inside \
                         the official game process.",
            "rewrite": report.as_json(),
        },
        "instance": instance.as_json(),
        "artifacts": artifacts(&root, &observed)?,
        "deviations": [
            {
                "summary": "The launcher puts its own -Djava.library.path=<instance>/natives after \
                            the instance's JvmArgs and the last one wins, so the appended library \
                            directory ADG-5 describes has no effect, and the launcher deletes that \
                            natives directory when a launch ends. The native library was placed \
                            there immediately before this launch. Ravel is still switched off by \
                            removing the -javaagent argument, because nothing loads the library \
                            without it, and choosing the fix is an ADG-5 amendment rather than \
                            M2's to make.",
                "classification": "non-blocking",
            },
            {
                "summary": "The agent reports what it rewrote from a JVM shutdown hook, and the \
                            launcher shows that stream in a console window it writes to no file \
                            and closes with the game. This launch used a stock WrapperCommand \
                            that only tees the process's own output to a file. It changes no \
                            launch argument, adds no component, and starts nothing else.",
                "classification": "non-blocking",
            },
        ],
        "redaction": {
            "recordsOnlyDigest": true,
            "removed": [
                "the account's session arguments, replaced by value before anything was hashed",
            ],
            "detail": "No line of the launcher's log reaches this record. The log is written \
                       outside the repository, redacted in memory, hashed, and deleted; the \
                       digest is the only thing that survives, which is how M0 and M1 recorded \
                       their launches.",
        },
    });

    let mut text = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("the record does not serialize: {error}"))?;
    text.push('\n');
    fs::write(root.join(RESULT), text).map_err(|error| format!("cannot write {RESULT}: {error}"))
}

/// The two claims about the launcher itself, which nothing here measures.
///
/// What this runner observes is the launcher's log and the instance directory;
/// the rest of the record is either repository fact — the pinned versions, the
/// revision, the digests of files hashed here — or a constant naming the machine
/// this runs on. None of that is a claim about the launcher. These two are: the
/// launcher binary is never opened, so a record presenting them as derived would
/// be claiming a measurement it never took. `identity.prism` names the same
/// unverified build, and is a pin rather than an observation of what ran.
fn attested_launcher() -> Value {
    json!({
        "derived": false,
        "stockLauncher": true,
        "launcherChangedLines": 0,
        "detail": "An operator states that the launcher was the pinned stock build and that no \
                   line of it was changed for this launch. This runner reads the launcher's log, \
                   not the launcher, so these two are attested rather than measured, and the \
                   milestone attestation is where a person stands behind them.",
    })
}

/// What the agent's shutdown line said this launch did.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Report {
    /// Classes the transformer rewrote.
    classes: u64,
    /// Methods the transformer rewrote.
    methods: u64,
    /// Hook invocations that reached native code, or `-1` when the library
    /// never loaded.
    invocations: i64,
    /// Whether the line was found at all.
    reported: bool,
}

impl Report {
    /// Reads the one line the agent prints from its shutdown hook.
    ///
    /// The line is the whole point of the launch: ADG-5's own launch proved the
    /// JVM accepted both arguments and the client ran, and could not prove the
    /// rewrite happened because nothing reported it.
    fn read(log: &str) -> Self {
        log.lines()
            .find_map(|line| {
                let after = line.split_once("[ravel] rewrote ")?.1;
                let (classes, after) = after.split_once(" classes and ")?;
                let (methods, after) = after.split_once(" methods; ")?;
                let (invocations, _) = after.split_once(" hook invocations")?;
                Some(Self {
                    classes: classes.trim().parse().ok()?,
                    methods: methods.trim().parse().ok()?,
                    invocations: invocations.trim().parse().ok()?,
                    reported: true,
                })
            })
            .unwrap_or_default()
    }

    /// Whether Ravel demonstrably ran inside the game process.
    const fn reached_ravel(self) -> bool {
        self.reported && self.classes > 0 && self.methods > 0
    }

    fn as_json(self) -> Value {
        json!({
            "reported": self.reported,
            "classesRewritten": self.classes,
            "methodsRewritten": self.methods,
            "nativeInvocations": self.invocations,
            "reachedRavel": self.reached_ravel(),
            "reachedNativeCode": self.invocations > 0,
            "detail": "The agent counts what it rewrote and asks the native library how many \
                       hook invocations reached it, then prints one line from a shutdown hook. \
                       A negative invocation count means the library never loaded, and a zero \
                       rewrite count means the transformer matched nothing.",
        })
    }
}

/// What the launched instance had installed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Instance {
    /// Every version component the instance declares, as `uid` and version.
    components: Vec<(String, String)>,
    /// Files sitting in the instance's `mods` directory.
    mod_files: usize,
    /// Files sitting in the instance's `coremods` directory.
    core_mod_files: usize,
}

impl Instance {
    /// Reads the instance's component list and mod directories.
    ///
    /// This is the other half of M2-W5's criterion: the launch has to be of the
    /// official game *without another loader*, and a launch log alone cannot
    /// prove a directory is empty.
    fn scan(directory: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(directory.join("mmc-pack.json"))
            .map_err(|error| format!("cannot read the instance component list: {error}"))?;
        let document: Value = serde_json::from_str(&text)
            .map_err(|error| format!("the instance component list is not JSON: {error}"))?;
        let components = document["components"]
            .as_array()
            .ok_or("the instance component list has no components")?
            .iter()
            .map(|component| {
                (
                    component["uid"].as_str().unwrap_or_default().to_owned(),
                    component["version"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        Ok(Self {
            components,
            mod_files: count(&directory.join("minecraft/mods")),
            core_mod_files: count(&directory.join("minecraft/coremods")),
        })
    }

    /// Components that are another Java mod loader.
    fn loader_components(&self) -> Vec<&str> {
        self.components
            .iter()
            .map(|(uid, _)| uid.as_str())
            .filter(|uid| {
                LOADER_COMPONENTS
                    .iter()
                    .any(|loader| uid.starts_with(loader))
            })
            .collect()
    }

    /// Whether the instance is the official game and nothing else.
    fn without_another_loader(&self) -> bool {
        self.loader_components().is_empty() && self.mod_files == 0 && self.core_mod_files == 0
    }

    fn as_json(&self) -> Value {
        json!({
            "components": self
                .components
                .iter()
                .map(|(uid, version)| json!({"uid": uid, "version": version}))
                .collect::<Vec<_>>(),
            "modFiles": self.mod_files,
            "coreModFiles": self.core_mod_files,
            "otherLoaderComponents": self.loader_components(),
            "withoutAnotherLoader": self.without_another_loader(),
            "detail": INSTANCE_DETAIL,
        })
    }
}

/// Files in a directory, or zero when it does not exist.
fn count(directory: &Path) -> usize {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .count()
}

/// The digests a reader can recompute, plus the one they cannot.
fn artifacts(root: &Path, observed: &launch::Launch) -> Result<Value, String> {
    let mut artifacts = vec![json!({
        "name": "redacted-instance-launch.log",
        "sha256": observed.redacted_sha256,
        "reproducible": false,
    })];
    for (name, relative) in [
        ("ravel-agent.jar", AGENT_JAR),
        ("libravel_bootstrap.so", NATIVE_LIBRARY),
    ] {
        let digest = harness::sha256_file(&root.join(relative))
            .map_err(|error| format!("cannot hash {name}: {error}"))?;
        artifacts.push(json!({
            "name": name,
            "sha256": digest,
            "reproducible": name == "ravel-agent.jar",
        }));
    }
    Ok(Value::Array(artifacts))
}

/// Reads the two paths this runner needs.
fn arguments() -> Result<(String, String), String> {
    let mut log = None;
    let mut instance = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--launch-log" => log = arguments.next(),
            "--instance" => instance = arguments.next(),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    match (log, instance) {
        (Some(log), Some(instance)) => Ok((log, instance)),
        _ => Err("usage: m2-launch-record --launch-log <path> --instance <directory>".to_owned()),
    }
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("cannot resolve the repository root: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};

    use super::repository_root;
    use super::{INSTANCE_DETAIL, Instance, LAUNCH_DETAIL, RESULT, Report, attested_launcher};

    #[test]
    fn the_recorded_launch_says_what_this_runner_would_say_now() {
        // The log this record was built from is deleted by design, so the record
        // cannot be regenerated. What it can still be held to is its own
        // generator's prose and its attested pair: a conclusion edited into the
        // file and not into this runner fails here.
        let root = repository_root().expect("the repository root should resolve");
        let text = fs::read_to_string(root.join(RESULT)).expect("the record should be readable");
        let record: Value = serde_json::from_str(&text).expect("the record should be JSON");

        assert_eq!(record["launch"]["detail"], json!(LAUNCH_DETAIL));
        assert_eq!(record["launch"]["attestation"], attested_launcher());
        assert_eq!(record["instance"]["detail"], json!(INSTANCE_DETAIL));
        // The attested pair is the operator's word and says so; nothing else in
        // the record is allowed to read as measured when it is not.
        assert_eq!(record["launch"]["attestation"]["derived"], json!(false));
    }

    #[test]
    fn the_agents_shutdown_line_is_read_whatever_the_launcher_prefixed_it_with() {
        let report = Report::read(
            "Game: some other line\n\
             Game: [ravel] rewrote 2 classes and 6 methods; 41 hook invocations reached native \
             code\n",
        );

        assert!(report.reported);
        assert_eq!(report.classes, 2);
        assert_eq!(report.methods, 6);
        assert_eq!(report.invocations, 41);
        assert!(report.reached_ravel());
    }

    #[test]
    fn a_launch_that_never_reported_proves_nothing_about_the_rewrite() {
        let report = Report::read("Game: the client started and stopped\n");

        assert!(!report.reported);
        assert!(!report.reached_ravel());
    }

    #[test]
    fn a_library_that_never_loaded_is_read_as_the_negative_count_it_prints() {
        let report = Report::read(
            "[ravel] rewrote 1 classes and 4 methods; -1 hook invocations reached \
                          native code\n",
        );

        assert!(report.reported);
        assert_eq!(report.invocations, -1);
        // The rewrite still happened; only the native side did not answer.
        assert!(report.reached_ravel());
    }

    #[test]
    fn an_instance_with_only_the_game_carries_no_other_loader() {
        let instance = Instance {
            components: vec![
                ("org.lwjgl3".to_owned(), "3.4.1".to_owned()),
                ("net.minecraft".to_owned(), "0.0.0".to_owned()),
            ],
            mod_files: 0,
            core_mod_files: 0,
        };

        assert!(instance.loader_components().is_empty());
        assert!(instance.without_another_loader());
    }

    #[test]
    fn a_loader_component_or_a_mod_file_refuses_the_claim() {
        let with_loader = Instance {
            components: vec![("net.neoforged".to_owned(), "0.0.0".to_owned())],
            mod_files: 0,
            core_mod_files: 0,
        };
        let with_mod = Instance {
            components: vec![("net.minecraft".to_owned(), "0.0.0".to_owned())],
            mod_files: 1,
            core_mod_files: 0,
        };

        assert_eq!(with_loader.loader_components(), ["net.neoforged"]);
        assert!(!with_loader.without_another_loader());
        assert!(!with_mod.without_another_loader());
    }
}
