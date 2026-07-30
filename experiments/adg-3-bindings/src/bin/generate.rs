//! Generates the pinned 26.1.2 binding with one extractor, once, and reports it.
//!
//! The trial follows the harness protocol: it does its work, announces
//! readiness, prints one report, and parks so its memory can be read. A batch
//! tool has no boundary between bootstrap and work, so peak memory at the ready
//! marker is peak memory for the whole generation, which is what it costs.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use ravel_spike_adg3 as bindings;
use ravel_spike_harness::{READY_MARKER, ReportedFailure, TrialReport};

use bindings::{Adapter, Extractor, GenerateError, Inventory, Kind};

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("generation failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let root = PathBuf::from(option(arguments, "--root").unwrap_or_else(|| ".".to_owned()));
    let out = PathBuf::from(
        option(arguments, "--out").ok_or_else(|| "the generator needs --out".to_owned())?,
    );
    let mode = option(arguments, "--mode").unwrap_or_else(|| "generate".to_owned());
    let extractor = option(arguments, "--extractor")
        .as_deref()
        .and_then(Extractor::parse)
        .ok_or_else(|| "the generator needs --extractor classfile|javap".to_owned())?;

    if mode == "hang" {
        // Never reports, so the harness timeout is what ends this trial.
        loop {
            std::thread::park();
        }
    }

    let started = Instant::now();
    let adapter = bindings::adapter(&root).map_err(|error| error.to_string())?;
    let inventory = bindings::inventory(&root, &adapter).map_err(|error| error.to_string())?;
    let jar = root.join(&adapter.inputs.client_jar.path);

    let resolved =
        bindings::resolve(&jar, &inventory, extractor).map_err(|error| error.to_string())?;
    let generated = bindings::generate(&out, &adapter, &inventory, &resolved, extractor)
        .map_err(|error| error.to_string())?;

    if mode == "crash" {
        // Ends the process with the output already renamed into place, so the
        // runner can check whether a half-finished generation is visible.
        std::process::abort();
    }

    // Generating twice must produce the same bytes, or nothing downstream can
    // trust a cached binding.
    let again = bindings::generate(&out, &adapter, &inventory, &resolved, extractor)
        .map_err(|error| error.to_string())?;
    if again.sha256 != generated.sha256 {
        return Err("generating twice produced different bytes".to_owned());
    }
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let agreement = cross_check(&jar, &inventory, extractor);
    let failures = failure_matrix(&jar, &inventory, extractor);
    let drift = drift_refused(&root, &adapter);

    let mut metrics = BTreeMap::new();
    metrics.insert("generation.duration_ms".to_owned(), as_float(duration_ms));
    metrics.insert("generation.bytes".to_owned(), as_float(generated.bytes));
    metrics.insert(
        "generation.targets".to_owned(),
        as_float(resolved.len() as u64),
    );

    let mut notes = BTreeMap::new();
    notes.insert("generation.sha256".to_owned(), generated.sha256.clone());
    notes.insert("generation.drift".to_owned(), drift);
    notes.insert("extraction.agreement".to_owned(), agreement);
    notes.insert(
        "generation.files".to_owned(),
        generated
            .files
            .iter()
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>()
            .join(", "),
    );

    let ready_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    println!("{READY_MARKER}");
    stdout().flush().map_err(|error| error.to_string())?;

    let report = TrialReport {
        candidate: extractor.flag().to_owned(),
        ready_ms,
        failures,
        gate_metrics: metrics,
        gate_notes: notes,
        // A generator crosses no call boundary and hosts no runtime.
        ..TrialReport::default()
    };
    let line = serde_json::to_string(&report)
        .map_err(|error| format!("cannot serialize the report: {error}"))?;
    println!("{line}");
    stdout().flush().map_err(|error| error.to_string())?;

    let mut release = [0_u8; 1];
    if stdin().read(&mut release).map_err(|e| e.to_string())? == 0 {
        return Err("the harness closed standard input without releasing".to_owned());
    }
    Ok(())
}

/// Reads the same targets with the other extractor and compares.
///
/// Two independent readings agreeing is the only cheap evidence that either is
/// right, so disagreement is reported rather than hidden.
fn cross_check(jar: &Path, inventory: &Inventory, extractor: Extractor) -> String {
    let other = if extractor == Extractor::ClassFile {
        Extractor::Javap
    } else {
        Extractor::ClassFile
    };
    let mine = bindings::resolve(jar, inventory, extractor);
    let theirs = bindings::resolve(jar, inventory, other);
    match (mine, theirs) {
        (Ok(mine), Ok(theirs)) if mine == theirs => format!(
            "the {} and {} extractors resolved all {} targets identically",
            extractor.flag(),
            other.flag(),
            mine.len()
        ),
        (Ok(_), Ok(_)) => format!(
            "the {} and {} extractors disagreed about at least one target",
            extractor.flag(),
            other.flag()
        ),
        (_, Err(error)) => format!("the {} extractor failed: {error}", other.flag()),
        (Err(error), _) => format!("the {} extractor failed: {error}", extractor.flag()),
    }
}

/// Exercises the three scenarios a generator can observe in its own process.
///
/// Each one is a real inventory with one entry made wrong, because that is how a
/// bad inventory actually arrives: someone edits one line.
fn failure_matrix(jar: &Path, inventory: &Inventory, extractor: Extractor) -> Vec<ReportedFailure> {
    let mut unknown_descriptor = inventory.clone();
    unknown_descriptor.hooks.truncate(1);
    let mut wrong_shape = unknown_descriptor.clone();
    if let Some(hook) = unknown_descriptor.hooks.first_mut() {
        hook.descriptor = String::from("(Lnope;)V");
    }
    if let Some(hook) = wrong_shape.hooks.first_mut() {
        hook.kind = Kind::InstanceField;
    }

    vec![
        refusal(
            "invalid-input",
            bindings::resolve(jar, &unknown_descriptor, extractor),
        ),
        refusal(
            "callback-failure",
            bindings::resolve(jar, &wrong_shape, extractor),
        ),
        refusal(
            "unavailable-runtime",
            bindings::resolve(
                &jar.with_file_name("absent-client.jar"),
                inventory,
                extractor,
            ),
        ),
    ]
}

fn refusal<T>(scenario: &str, observed: Result<T, GenerateError>) -> ReportedFailure {
    let (outcome, detail) = match observed {
        Ok(_) => (
            "continued",
            "the generator accepted an input it should have refused".to_owned(),
        ),
        Err(error) => ("failed-closed", error.to_string()),
    };
    ReportedFailure {
        scenario: scenario.to_owned(),
        outcome: outcome.to_owned(),
        detail,
    }
}

/// Checks that a jar whose hash no longer matches the tracked pin is refused.
fn drift_refused(root: &Path, adapter: &Adapter) -> String {
    let mut drifted = adapter.inputs.client_jar.clone();
    drifted.sha256 = Some("0".repeat(64));
    match bindings::verify_input(root, &drifted) {
        Err(GenerateError::Drift(detail)) => format!("refused: {detail}"),
        Err(other) => format!("refused for the wrong reason: {other}"),
        Ok(()) => "accepted a jar whose hash does not match the tracked pin".to_owned(),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "generated sizes and durations stay far below f64 integer precision limits"
)]
fn as_float(value: u64) -> f64 {
    value as f64
}

fn option(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}
