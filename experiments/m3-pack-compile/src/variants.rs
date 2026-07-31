//! Measures the three variants that change the answer, beside the base curve.
//!
//! The base curve builds independent mod crates on every core the machine has.
//! Three things about that are not what a player gets, and each is measured
//! here rather than argued about:
//!
//! - **Four cores.** The reference machine has sixteen threads. A player's does
//!   not, and peak memory during a build is set by how many `rustc` processes
//!   run at once, so the core count moves both numbers.
//! - **A dependency graph.** Real mods sit on libraries that sit on libraries.
//!   Cargo cannot start a crate before its dependencies finish, so a deep graph
//!   serializes work that the independent pack ran in parallel.
//! - **Whole-pack optimization.** Mods shipped and versioned separately but
//!   optimized together is the variant that buys runtime performance. What it
//!   costs is a link that cannot be incremental.
//!
//! These are spliced into the existing record rather than written over it: the
//! curve's numbers stay exactly as they were measured.

use std::fs;
use std::path::Path;
use std::time::Instant;

use ravel_spike_harness as harness;
use serde_json::{Value, json};

use crate::generate::{self, PackSpec, SPINE_DEPTH, Shape, Workspace};
use crate::measure::{
    self, BuildRun, fastest_ms as fastest, peak_process_rss, peak_tree_rss as peak_rss, samples,
};

/// Pack sizes the variants are measured at.
///
/// The two large points of the base curve. Below 64 the differences these
/// variants create are smaller than the run-to-run spread, so measuring them
/// there would produce numbers without a signal in them.
const VARIANT_SIZES: [usize; 2] = [64, 256];

/// Builds per figure.
///
/// Two rather than the curve's three, because a fat-link build of 256 crates
/// costs about two minutes and the full matrix is already forty of them. The
/// quoted figure is the faster of the two runs and both are recorded: build
/// noise only ever adds time, so the minimum is the closest estimate of the
/// cost, and it is stated rather than dressed up as a median.
const VARIANT_REPEATS: usize = 2;

/// One thing being varied against the base curve.
struct Variant {
    /// Identifier the record uses.
    id: &'static str,
    /// How the mods depend on one another.
    shape: Shape,
    /// Whether the pack is optimized as one unit.
    lto: bool,
    /// Cargo's job limit, or `None` for every core.
    jobs: Option<usize>,
    /// What the variant is for, in the record's own words.
    question: &'static str,
    /// Which cache this variant's warm figure is warm against.
    ///
    /// Present only where that differs from the baseline's, because a reader who
    /// assumes it does not will read two different measurements as one.
    warm_path_detail: Option<&'static str>,
}

/// The matrix. The control is first, and exists so the others are compared
/// against a run under the same protocol rather than against the curve's
/// median-of-three.
const VARIANTS: [Variant; 4] = [
    Variant {
        id: "independent-j16",
        shape: Shape::Independent,
        lto: false,
        jobs: None,
        question: "The control. The same pack the curve measured, rerun under this section's \
                   two-run protocol so the other three are compared against a like measurement \
                   rather than against the curve's median of three.",
        warm_path_detail: None,
    },
    Variant {
        id: "independent-j4",
        shape: Shape::Independent,
        lto: false,
        jobs: Some(4),
        question: "What a four-core player pays, in time and in peak memory. Peak memory during \
                   a build is set by how many rustc processes run at once, so this is the number \
                   that decides whether a build is safe on a small machine.",
        warm_path_detail: None,
    },
    Variant {
        id: "chain-j16",
        shape: Shape::Chain,
        lto: false,
        jobs: None,
        question: "What a dependency graph costs. Cargo cannot start a crate until its \
                   dependencies finish, so a deep graph converts parallel work into serial work \
                   and the cost lands on machines with more cores, not fewer.",
        warm_path_detail: None,
    },
    Variant {
        id: "lto-fat-j16",
        shape: Shape::Independent,
        lto: true,
        jobs: None,
        question: "What optimizing the pack as one unit costs, and whether the warm path \
                   survives it. Mods stay separately shipped and versioned; only the \
                   optimization is shared.",
        warm_path_detail: Some(
            "This variant's warm figure is warm with respect to an LTO cache: the workspace is \
             built with `lto = \"fat\"` from the start, so the rebuild reuses an LTO build \
             directory and never an ordinary one. It is a different path from the baseline warm \
             figure, not the same path made slower, and the two must not be read as one \
             measurement degrading. Cargo cannot reuse a non-LTO cache for an LTO build, so no \
             comparison between them is a before-and-after.",
        ),
    },
];

/// What one variant cost at one pack size.
struct Measured {
    packs: usize,
    cold: Vec<BuildRun>,
    warm_first: Vec<BuildRun>,
    warm_last: Vec<BuildRun>,
    binary_bytes: u64,
    cache_bytes: u64,
}

/// Measures every variant at every size and splices the result into `result`.
///
/// # Errors
///
/// Returns a message when the record is missing, unreadable, not an object, or
/// when a generated pack will not build.
pub fn run(root: &Path, result: &str) -> Result<String, String> {
    let path = root.join(result);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {result}: {error}; run the curve first"))?;
    let mut record: Value =
        serde_json::from_str(&text).map_err(|error| format!("cannot parse {result}: {error}"))?;
    if !record.is_object() {
        return Err(format!("{result} is not a record"));
    }

    let work = root.join("target/m3-pack-compile");
    let started = Instant::now();
    let started_at = harness::utc_timestamp().map_err(|error| error.to_string())?;
    let revision = harness::revision().map_err(|error| error.to_string())?;

    let mut measured = Vec::new();
    for variant in &VARIANTS {
        let mut sizes = Vec::new();
        for size in VARIANT_SIZES {
            let workspace = generate::generate(
                root,
                &work,
                PackSpec {
                    mods: size,
                    shape: variant.shape,
                    lto: variant.lto,
                    label: variant.id,
                },
            )?;
            sizes.push(measure(&workspace, variant)?);
        }
        measured.push((variant, sizes));
    }

    record["variants"] = json!({
        "method": "Each variant changes exactly one thing about the base curve and is measured \
                   at 64 and 256 mods. Every figure is the faster of two builds, with both \
                   recorded; the curve above is a median of three, so compare a variant against \
                   the `independent-j16` control here rather than against the curve directly.",
        "repeats": VARIANT_REPEATS,
        "sizes": VARIANT_SIZES,
        "unit": "milliseconds",
        "warmTouchPoints": "Two warm rebuilds are measured per variant: one changing `mod-000` \
                            and one changing the last mod. Under the independent shape those are \
                            two ordinary mods and the pair is a repeat. Under the chain shape \
                            `mod-000` is the root of the spine, so everything depends on it \
                            transitively, and the last mod is a leaf that nothing depends on — \
                            the worst and best cases a player can hit.",
        "chainShape": format!(
            "`mod-000` through `mod-{:03}` form a linear spine, each depending on the one before \
             it. Every mod off the spine depends on the spine's tip and its midpoint. The \
             critical path is therefore {} crates deep, and no two spine crates can compile at \
             the same time. Each dependency is called, not merely declared, so the edge is real \
             in the type graph.",
            SPINE_DEPTH - 1,
            SPINE_DEPTH + 1,
        ),
        "spineDepth": SPINE_DEPTH,
        "ltoShape": "`lto = \"fat\"` with `codegen-units = 1` on the release profile of the \
                     generated workspace, so every mod crate's bitcode is merged and optimized \
                     as one unit at link time.",
        "trial": {
            "startedAt": started_at,
            "durationMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "revision": {
                "id": revision.id,
                "changeId": revision.change_id,
                "dirty": revision.dirty,
            },
            "detail": "Measured in a later run than the curve above and spliced in, so the \
                       curve's numbers are the ones it recorded rather than a rerun of them.",
        },
        "results": measured
            .iter()
            .map(|(variant, sizes)| {
                let finding = if variant.lto {
                    link_time_finding(&measured)
                } else {
                    None
                };
                variant_record(variant, sizes, finding.as_deref())
            })
            .collect::<Vec<_>>(),
        "whatThisDoesNotSay": caveats(),
    });

    let mut written = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("the record does not serialize: {error}"))?;
    written.push('\n');
    fs::write(&path, written).map_err(|error| format!("cannot write {result}: {error}"))?;
    Ok(result.to_owned())
}

/// Builds one variant at one size, cold and warm at both touch points.
fn measure(workspace: &Workspace, variant: &Variant) -> Result<Measured, String> {
    let mut cold = Vec::with_capacity(VARIANT_REPEATS);
    for _ in 0..VARIANT_REPEATS {
        cold.push(measure::cold(&workspace.root, variant.jobs)?);
    }
    let binary_bytes = measure::file_bytes(&workspace.binary());
    let cache_bytes = measure::directory_bytes(&workspace.cache());

    let first = workspace.first_mod_source();
    let last = workspace.last_mod_source();
    let mut warm_first = Vec::with_capacity(VARIANT_REPEATS);
    let mut warm_last = Vec::with_capacity(VARIANT_REPEATS);
    for _ in 0..VARIANT_REPEATS {
        warm_first.push(measure::warm(&workspace.root, &first, variant.jobs)?);
    }
    for _ in 0..VARIANT_REPEATS {
        warm_last.push(measure::warm(&workspace.root, &last, variant.jobs)?);
    }

    Ok(Measured {
        packs: workspace.mods,
        cold,
        warm_first,
        warm_last,
        binary_bytes,
        cache_bytes,
    })
}

fn variant_record(variant: &Variant, sizes: &[Measured], finding: Option<&str>) -> Value {
    let mut record = json!({
        "id": variant.id,
        "shape": variant.shape.as_str(),
        "lto": variant.lto,
        "jobs": variant.jobs,
        "jobsDetail": variant.jobs.map_or_else(
            || "every hardware thread the machine has".to_owned(),
            |jobs| format!("cargo --jobs {jobs}"),
        ),
        "question": variant.question,
        "points": sizes.iter().map(point).collect::<Vec<_>>(),
    });
    if let Some(detail) = variant.warm_path_detail {
        record["warmPathDetail"] = json!(detail);
    }
    if let Some(finding) = finding {
        record["finding"] = json!(finding);
    }
    record
}

/// States what the link-time variant found, from the numbers this run produced.
///
/// The claim is fixed and the evidence is computed. Writing this run's seconds
/// into the sentence would make a rerun on other hardware quote figures its own
/// `points` contradict, which is the failure the record is supposed to prevent
/// rather than commit.
fn link_time_finding(results: &[(&Variant, Vec<Measured>)]) -> Option<String> {
    let link_time = results.iter().find(|(variant, _)| variant.lto)?;
    let chain = results
        .iter()
        .find(|(variant, _)| variant.shape == Shape::Chain)?;

    let mut pairs = Vec::new();
    for measured in &link_time.1 {
        pairs.push(format!(
            "{} s and {} s at {} mods",
            seconds(fastest(&measured.warm_first)?),
            seconds(fastest(&measured.warm_last)?),
            measured.packs,
        ));
    }
    // The largest chain point, because the contrast is clearest where there is
    // most work for a dependency edge to hold up.
    let contrast = chain.1.iter().max_by_key(|measured| measured.packs)?;

    Some(format!(
        "Changing one mod forces the whole pack to be reoptimized. The evidence is that changing \
         the first mod and changing the last mod cost the same — {} — and under every other \
         variant those two differ, sharply so under the chain shape, where at {} mods the same \
         pair is {} s against {} s. A fat link has no incremental path: which crate changed does \
         not narrow what has to be re-optimized, so the pack is rebuilt as one unit every time.",
        pairs.join(", "),
        contrast.packs,
        seconds(fastest(&contrast.warm_first)?),
        seconds(fastest(&contrast.warm_last)?),
    ))
}

/// Milliseconds as seconds, to two decimals.
fn seconds(milliseconds: u64) -> String {
    #[expect(
        clippy::cast_precision_loss,
        reason = "build times stay far below f64 integer precision limits"
    )]
    let value = milliseconds as f64 / 1000.0;
    format!("{value:.2}")
}

fn point(measured: &Measured) -> Value {
    json!({
        "packSize": measured.packs,
        "coldMilliseconds": fastest(&measured.cold),
        "coldSamplesMilliseconds": samples(&measured.cold),
        "coldPeakTreeRssBytes": peak_rss(&measured.cold),
        "coldPeakProcessRssBytes": peak_process_rss(&measured.cold),
        "warmFirstModMilliseconds": fastest(&measured.warm_first),
        "warmFirstModSamplesMilliseconds": samples(&measured.warm_first),
        "warmFirstModPeakTreeRssBytes": peak_rss(&measured.warm_first),
        "warmLastModMilliseconds": fastest(&measured.warm_last),
        "warmLastModSamplesMilliseconds": samples(&measured.warm_last),
        "warmLastModPeakTreeRssBytes": peak_rss(&measured.warm_last),
        "binaryBytes": measured.binary_bytes,
        "cacheBytes": measured.cache_bytes,
    })
}

/// What this section does not say.
fn caveats() -> Value {
    json!([
        "The four-core figure limits cargo's job count on a sixteen-thread machine. It is not a \
         four-core machine: the cores are the same fast cores, the cache is the whole chip's, and \
         memory bandwidth is not contended the way a smaller part would contend it. Read it as \
         the parallelism a player has, not as the machine.",
        "The chain shape is one graph, not a survey of graphs. Its depth is the whole finding, \
         and a real pack's depth is unknown — a shallower graph costs less than this and a \
         deeper one costs more, in proportion to the serial section.",
        "Every mod off the spine depends on the same two spine crates, so the fan-out is uniform. \
         A real pack has hub libraries of very different popularity.",
        "The link-time figures are fat LTO with one codegen unit, which is the expensive end. \
         Thin LTO would cost less and buy less, and is not measured.",
        "Peak memory stays a 25-millisecond sampled sum over the process tree, so the true peak \
         is at or above what is recorded. That understates the link-time variant most, because \
         its peak is one short-lived process rather than a long plateau.",
        "The disk figures here are for the variant's own workspace after its last cold build. \
         The warm rebuilds that follow grow the cache further and that growth is not recorded.",
    ])
}
