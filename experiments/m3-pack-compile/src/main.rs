//! Measures what compiling a pack-sized set of realistic mods costs.
//!
//! ADG-2 decided that a mod is independently shipped and interchangeable and
//! left the mechanism open: a per-platform binary, or source that a bundled
//! toolchain compiles on the player's machine. Its own record names the
//! deciding measurement — "compile time for a realistic pack-sized mod set,
//! cold and cached" — and says M2-W8's 880 ms is a floor from generated
//! one-struct crates rather than a prediction.
//!
//! This replaces that floor. It generates mod crates with bodies, at a baseline
//! and four pack sizes, and records cold compile time, warm rebuild time after
//! one mod changes, peak resident memory during the build, and what the
//! toolchain and the build cache cost on disk.
//!
//! It measures and selects nothing. Every crate it generates is ordinary safe
//! Rust behind the same `HostMod` contract the tracked test mods use, and
//! nothing here loads a library, starts a guest runtime, spawns a mod process,
//! or exports a foreign symbol.

#![forbid(unsafe_code)]

mod generate;
mod measure;
mod variants;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use ravel_spike_harness as harness;
use serde_json::{Value, json};

use generate::{FEATURES_PER_MOD, Workspace};
use measure::BuildRun;

/// Where the record belongs, unless a caller names another path.
const RESULT: &str = "evidence/m3/pack-compile.json";
/// The hardware profile every Ravel Linux measurement is compared on.
const HARDWARE_PROFILE: &str = "tests/performance/hardware/linux-reference.toml";

/// Pack sizes the curve is taken at.
///
/// Zero is the baseline: a host that links no mod, which is the shared cost of
/// the loader and its dependencies and is subtracted to attribute the rest to
/// the mods. The other four are the sizes M2-W8's loader scaling curve used, so
/// the two records line up point for point, and 256 is inside the scale of
/// ATM11 0.2.1, the pinned M0 modpack fixture.
const PACK_SIZES: [usize; 5] = [0, 1, 16, 64, 256];

/// Builds timed per pack size, cold and warm alike.
///
/// Three, because M2-W8's build figures were single unrepeated samples and its
/// own record said so. Every sample is written to the record beside its median,
/// so the spread is readable rather than summarized away.
const REPEATS: usize = 3;

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("wrote {path}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the pack compile measurement failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let root = repository_root()?;
    let (result, mode) = arguments()?;
    if mode == Mode::Variants {
        return variants::run(&root, &result);
    }
    let work = root.join("target/m3-pack-compile");
    let started = Instant::now();
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;
    let pinned = harness::pinned_inputs(&root).map_err(|error| error.to_string())?;
    let profile =
        harness::sha256_file(&root.join(HARDWARE_PROFILE)).map_err(|error| error.to_string())?;

    let mut points = Vec::with_capacity(PACK_SIZES.len());
    let mut shape = None;
    for size in PACK_SIZES {
        let workspace = generate::generate(&root, &work, generate::PackSpec::plain(size))?;
        if size > 0 {
            shape = Some(workspace.clone());
        }
        points.push(measure_point(&workspace)?);
    }
    let shape = shape.ok_or("no pack size generated a mod crate")?;

    let toolchain = measure::toolchain_root()?;
    let record = json!({
        "schemaVersion": 1,
        "protocol": "m3-pack-compile-v1",
        "record": "m3-adg-2-pack-compile",
        "milestone": "M3",
        "gate": "ADG-2",
        "summary": "What compiling a pack-sized set of mods with real bodies costs on Linux — \
                    cold, warm after one mod changes, in peak memory, and on disk — which is the \
                    measurement ADG-2 named as deciding between a shipped per-platform binary and \
                    source compiled by a bundled toolchain.",
        "identity": {
            "target": "x86_64-linux",
            "hardwareProfile": "linux-reference-v1",
            "hardwareProfileSha256": profile,
            "minecraft": pinned.minecraft,
            "buildProfile": "release",
            "clock": "monotonic",
            "toolchain": toolchain_name(&toolchain),
            "rustc": rustc_version()?,
            "buildJobs": std::thread::available_parallelism().map_or(0, std::num::NonZero::get),
        },
        "trial": {
            "startedAt": started_at,
            "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "revision": {
                "id": revision.id,
                "changeId": revision.change_id,
                "dirty": revision.dirty,
            },
            "commandSha256": harness::command_sha256(&[
                "cargo",
                "run",
                "--release",
                "-p",
                "ravel-spike-m3-pack-compile",
            ]),
        },
        "syntheticMod": synthetic_mod_record(&shape),
        "compile": compile_record(&points),
        "disk": disk_record(&points, &toolchain),
        "whatThisDoesNotSay": caveats(),
    });

    let mut text = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("the record does not serialize: {error}"))?;
    text.push('\n');
    let path = root.join(&result);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("cannot create {result}: {error}"))?;
    }
    fs::write(&path, text).map_err(|error| format!("cannot write {result}: {error}"))?;
    Ok(result)
}

/// One pack size, built [`REPEATS`] times cold and [`REPEATS`] times warm.
struct Point {
    packs: usize,
    cold: Vec<BuildRun>,
    warm: Vec<BuildRun>,
    binary_bytes: u64,
    cache_bytes: u64,
    source_bytes: u64,
}

impl Point {
    /// The cold build time a reader should quote, in milliseconds.
    fn cold_median_ms(&self) -> u64 {
        measure::median_ms(&self.cold).unwrap_or(0)
    }

    /// The warm rebuild time a reader should quote, in milliseconds.
    fn warm_median_ms(&self) -> Option<u64> {
        measure::median_ms(&self.warm)
    }
}

fn measure_point(workspace: &Workspace) -> Result<Point, String> {
    let mut cold = Vec::with_capacity(REPEATS);
    for _ in 0..REPEATS {
        cold.push(measure::cold(&workspace.root, None)?);
    }
    let binary_bytes = measure::file_bytes(&workspace.binary());
    let cache_bytes = measure::directory_bytes(&workspace.cache());

    let mut warm = Vec::new();
    if workspace.mods > 0 {
        let source = workspace.first_mod_source();
        for _ in 0..REPEATS {
            warm.push(measure::warm(&workspace.root, &source, None)?);
        }
    }

    Ok(Point {
        packs: workspace.mods,
        cold,
        warm,
        binary_bytes,
        cache_bytes,
        source_bytes: workspace.bytes_per_mod * workspace.mods as u64,
    })
}

/// What the generated mod is, so a reader can judge what the numbers cover.
fn synthetic_mod_record(shape: &Workspace) -> Value {
    json!({
        "featuresPerMod": FEATURES_PER_MOD,
        "sourceLines": shape.lines_per_mod,
        "sourceBytes": shape.bytes_per_mod,
        "dependencies": ["ravel-core", "ravel-host-api", "serde (derive)", "serde_json"],
        "shape": "One configuration type deserialized from JSON at construction, one graded enum \
                  with four variants and three match-based methods, one internally tagged result \
                  enum, a generic `Rule` trait with an associated input type, two generic \
                  functions over that trait, and one feature block per feature — each a \
                  serde-derived input struct, a `Rule` implementation with branching logic, three \
                  registry claims, and a driver. The `HostMod` implementation registers every \
                  claim and runs every feature on entering `Running`.",
        "derivedTypes": 3 + FEATURES_PER_MOD,
        "monomorphizations": 2 * FEATURES_PER_MOD,
        "registryClaimsPerMod": 3 * FEATURES_PER_MOD,
        "representativeness": "Sized against `test-mods/registries`, the largest tracked test \
                               mod, whose body is about 150 lines before its tests; `sourceLines` \
                               above says what this one came to. It was scaled by derives and \
                               generic instantiations rather than by lines, because that is what \
                               Rust charges for. It is still not a real mod: a large Minecraft \
                               mod is thousands of lines with dozens of dependencies, and nothing \
                               measured here has a dependency outside serde. Read every figure as \
                               a floor — a far higher one than M2-W8's one-struct crates, and \
                               still a floor.",
    })
}

/// The curve itself.
fn compile_record(points: &[Point]) -> Value {
    let baseline = points
        .iter()
        .find(|point| point.packs == 0)
        .map_or(0, Point::cold_median_ms);
    let largest = points.iter().map(|point| point.packs).max().unwrap_or(0);
    json!({
        "method": "Three cold release builds per pack size against a cleared build directory, \
                   then three warm rebuilds after appending a line to one mod's source. Cargo \
                   builds across every core the machine has, so a per-mod figure is amortized \
                   throughput on a sixteen-thread host rather than what one mod costs to compile.",
        "baselineMilliseconds": baseline,
        "baselineDetail": "A host linking no mod, which is the shared cost of the loader crates, \
                           serde, and the link itself. Subtracted from each cold build so the \
                           remainder is attributable to the mods.",
        "repeats": REPEATS,
        "unit": "milliseconds",
        "points": points
            .iter()
            .map(|point| point_record(point, baseline))
            .collect::<Vec<_>>(),
        "marginalColdMillisecondsPerMod": marginal(points, baseline),
        "marginalMethod": format!(
            "a secant between the one-mod and {largest}-mod cold medians, each already less the \
             baseline, so it is what one more mod adds across the whole measured range rather \
             than a fitted slope."
        ),
    })
}

fn point_record(point: &Point, baseline: u64) -> Value {
    let attributable = point.cold_median_ms().saturating_sub(baseline);
    json!({
        "packSize": point.packs,
        "coldMilliseconds": point.cold_median_ms(),
        "coldSamplesMilliseconds": measure::samples(&point.cold),
        "coldMillisecondsLessBaseline": attributable,
        "coldMillisecondsPerMod": divide(attributable, point.packs),
        "coldPeakTreeRssBytes": measure::peak_tree_rss(&point.cold),
        "coldPeakProcessRssBytes": measure::peak_process_rss(&point.cold),
        "coldRssSamples": point.cold.iter().map(|run| run.samples).sum::<u64>(),
        "warmMilliseconds": point.warm_median_ms(),
        "warmSamplesMilliseconds": measure::samples(&point.warm),
        "warmPeakTreeRssBytes": measure::peak_tree_rss(&point.warm),
        "binaryBytes": point.binary_bytes,
        "cacheBytes": point.cache_bytes,
        "modSourceBytes": point.source_bytes,
    })
}

/// What one more mod adds to a cold build, from the two ends of the curve.
fn marginal(points: &[Point], baseline: u64) -> Option<f64> {
    let first = points.iter().find(|point| point.packs == 1)?;
    let last = points.iter().max_by_key(|point| point.packs)?;
    let span = last
        .packs
        .checked_sub(first.packs)
        .filter(|span| *span > 0)?;
    let grew = last
        .cold_median_ms()
        .saturating_sub(baseline)
        .saturating_sub(first.cold_median_ms().saturating_sub(baseline));
    Some(round(
        as_f64(grew) / as_f64(u64::try_from(span).unwrap_or(u64::MAX)),
    ))
}

/// What a compile-on-install mechanism would have to ship and keep.
fn disk_record(points: &[Point], toolchain: &Path) -> Value {
    let largest = points.iter().max_by_key(|point| point.packs);
    json!({
        "toolchainBytes": measure::directory_bytes(toolchain),
        "toolchainDetail": "The `rustc --print=sysroot` tree: the compiler, its standard library \
                            for this target, and cargo. Apparent size summed per path, so files \
                            the store hard-links are counted once each rather than deduplicated, \
                            and a shipped toolchain would compress. It excludes the linker and \
                            the C runtime, which come from the system.",
        "largestPackCacheBytes": largest.map_or(0, |point| point.cache_bytes),
        "largestPackBinaryBytes": largest.map_or(0, |point| point.binary_bytes),
        "cacheDetail": "The generated workspace's whole `target` directory after the cold build, \
                        which is what a player's machine keeps so the next warm rebuild is warm. \
                        A release build with debug info off still keeps every crate's rlib and \
                        every incremental fingerprint.",
    })
}

/// What this measurement does not say, said before someone reads it as saying it.
fn caveats() -> Value {
    json!([
        "The mods are generated, so they are alike. A real pack's mods differ in size, in \
         dependencies, and in how much they share, and 256 copies of one shape compile more \
         evenly than 256 different ones would.",
        "No generated mod depends on another. A real pack has a dependency graph, and cargo \
         cannot start a dependent crate until its dependency finishes, so a real set of the same \
         total size parallelizes worse than this one.",
        "Nothing depends on anything outside serde and the two Ravel crates. Real mods pull real \
         dependency trees, and those are compiled once per pack rather than once per mod — a cost \
         this measurement omits entirely rather than underestimates.",
        "Each figure is the median of three runs on one machine on one afternoon. Every sample is \
         recorded beside it, so the spread is readable, but three runs is not a distribution and \
         nothing here is a percentile.",
        "Peak memory is a 25-millisecond sampled sum over the cargo process tree, so a spike \
         shorter than the interval is missed, and the true peak is at or above what is recorded.",
        "Cargo compiled across every core this machine has. A player with four cores pays more \
         wall-clock time for the same work, and the per-mod figures are throughput on a \
         sixteen-thread host rather than the cost of one mod.",
        "The build ran against a warm cargo registry and a warm page cache, and downloaded \
         nothing. A first install would also fetch and unpack the dependency crates.",
        "The warm rebuild changes one mod and rebuilds. That is updating a mod, not adding one: \
         adding a mod also compiles a crate that was never compiled here and relinks, which is \
         between this figure and the cold one and is not measured.",
        "This is release profile with cargo's defaults. A mechanism that shipped source could \
         choose a cheaper profile, and a cheaper profile is a slower game.",
        "Nothing here selects an ADG-2 mechanism. It measures what one of the two open mechanisms \
         would cost, and says nothing about what the other one costs to maintain.",
    ])
}

/// What this run is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    /// Measure the base curve and write the record.
    Curve,
    /// Measure the variants and splice them into an existing record.
    ///
    /// A separate mode because rerunning the curve would move numbers that are
    /// already recorded, and a variant is meant to sit beside them rather than
    /// replace them.
    Variants,
}

/// Where the record goes and what to put in it.
fn arguments() -> Result<(String, Mode), String> {
    let mut result = RESULT.to_owned();
    let mut mode = Mode::Curve;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                result = arguments
                    .next()
                    .ok_or_else(|| "--output needs a repository-relative path".to_owned())?;
            }
            "--variants" => mode = Mode::Variants,
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok((result, mode))
}

/// The toolchain's directory name, which identifies it without a machine path.
fn toolchain_name(toolchain: &Path) -> String {
    toolchain.file_name().map_or_else(
        || "unknown".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn rustc_version() -> Result<String, String> {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|error| format!("cannot run rustc: {error}"))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Milliseconds per mod, or `None` when there is no mod to divide by.
fn divide(total: u64, count: usize) -> Option<f64> {
    if count == 0 {
        return None;
    }
    Some(round(as_f64(total) / as_f64(count as u64)))
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn as_f64(value: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "millisecond counts and pack sizes stay far below f64 integer precision limits"
    )]
    let widened = value as f64;
    widened
}

fn repository_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("cannot resolve the repository root: {error}"))
}
