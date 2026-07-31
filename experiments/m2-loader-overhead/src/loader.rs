//! Times the pre-world loader pipeline for the no-op package.
//!
//! Every measurement here runs the production crates through
//! `ravel-testkit`'s pipeline, which is the same composition
//! `tests/conformance/tests/noop_pipeline.rs` records. Nothing is
//! reimplemented for speed, because a benchmark of a second implementation
//! would be a number about the benchmark.

use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ravel_core::{LifecycleState, ModId, Side, Version};
use ravel_host_api::{HostError, HostMod};
use ravel_package::{PackageManifest, verify_package};
use ravel_resolve::resolve;
use ravel_spike_harness::{Percentiles, percentiles};
use ravel_test_mod_noop::NoopMod;
use ravel_testkit::{DiscoveredPackage, Host, run};

/// The tracked package every primary number is measured against.
pub const PACKAGE_DIRECTORY: &str = "tests/fixtures/public/packages/noop";

/// Runs discarded before the first sample, so a measured run finds a warm
/// allocator, a warm page cache and a warm branch predictor.
const WARMUP_RUNS: usize = 500;

/// Samples taken per metric. Large enough for a 99th percentile to mean
/// something, small enough that the whole run stays under a minute.
const MEASURED_RUNS: usize = 5_000;

/// Package-set sizes the scaling curve is taken at.
///
/// One is the no-op package itself. The rest are copies of it under different
/// identities, which isolates what the loader costs per package from what any
/// particular package costs.
pub const SCALING_SIZES: [usize; 4] = [1, 16, 64, 256];

/// A mod value the loader owns, used where the set needs more than one identity.
///
/// It is the no-op mod's behaviour under a supplied identity. It loads nothing,
/// starts nothing and exports nothing: ADG-2 selects the mod execution model
/// and is deferred to M3.
struct SetMod {
    id: ModId,
    entered: usize,
}

impl HostMod for SetMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        self.entered += 1;
        Ok(())
    }
}

/// A timed metric, in nanoseconds.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Samples the percentiles were taken from.
    pub samples: usize,
    /// Nearest-rank percentiles, shared with every other Ravel measurement.
    pub percentiles: Percentiles,
}

/// Calls per sample for an operation too cheap to time one at a time.
///
/// Reading the monotonic clock twice costs about as much as a handful of
/// nanoseconds of work, so a single-call sample of a single-digit-nanosecond
/// operation measures the clock. A thousand calls per sample puts the clock
/// three orders of magnitude below the thing being measured.
const CALLS_PER_SAMPLE: usize = 1_000;

impl Timing {
    /// Times `body` `MEASURED_RUNS` times after `WARMUP_RUNS` discarded runs.
    fn of(body: impl FnMut()) -> Self {
        Self::batched(1, body)
    }

    /// Times `batch` calls of `body` per sample and reports the per-call cost.
    fn batched(batch: usize, mut body: impl FnMut()) -> Self {
        for _ in 0..WARMUP_RUNS {
            body();
        }
        let divisor = as_f64(batch.max(1));
        let mut samples = Vec::with_capacity(MEASURED_RUNS);
        for _ in 0..MEASURED_RUNS {
            let started = Instant::now();
            for _ in 0..batch {
                body();
            }
            let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            samples.push(elapsed);
        }
        let measured = percentiles(&samples).expect("a measured run produces samples");
        Self {
            samples: samples.len(),
            percentiles: Percentiles {
                median: measured.median / divisor,
                p95: measured.p95 / divisor,
                p99: measured.p99 / divisor,
            },
        }
    }

    /// The value a budget is set from.
    ///
    /// The 99th percentile rather than the median, because `ADG-4` asks for
    /// tails: a loader that is usually fast and occasionally slow is a loader
    /// that occasionally stalls a launch.
    #[must_use]
    pub const fn budget_basis(self) -> f64 {
        self.percentiles.p99
    }
}

/// What one package-set size cost.
#[derive(Clone, Copy, Debug)]
pub struct ScalingPoint {
    /// How many packages the set held.
    pub packages: usize,
    /// What the whole pipeline cost for that set.
    pub whole: Timing,
}

/// Everything the loader measurement produced.
pub struct LoaderOverhead {
    /// Parsing the manifest document.
    pub parse: Timing,
    /// Verifying declared artifacts and the provenance record against their
    /// digests.
    pub integrity: Timing,
    /// Resolving validated manifests into a load plan.
    pub resolve: Timing,
    /// The whole pipeline for the no-op package: validate, resolve, and every
    /// lifecycle state in order.
    pub whole: Timing,
    /// One `HostMod::enter` call through the trait object the loader drives.
    pub mod_call: Timing,
    /// What the pipeline costs as the package set grows.
    pub scaling: Vec<ScalingPoint>,
    /// Mod calls one mod receives in one process run.
    pub mod_calls_per_run: usize,
}

impl LoaderOverhead {
    /// The residual: what the whole pipeline costs beyond the stages measured
    /// individually, which is the lifecycle machine plus the compatibility and
    /// target checks.
    #[must_use]
    pub fn lifecycle_residual(&self) -> f64 {
        let stages = self.parse.percentiles.p99
            + self.integrity.percentiles.p99
            + self.resolve.percentiles.p99;
        (self.whole.percentiles.p99 - stages).max(0.0)
    }

    /// What one extra package adds to a run, from the two ends of the curve.
    ///
    /// Taken from the ends rather than fitted, because two points and a stated
    /// method are checkable and a regression line is not.
    #[must_use]
    pub fn marginal_per_package(&self) -> f64 {
        let first = self.scaling.first().expect("the curve has a first point");
        let last = self.scaling.last().expect("the curve has a last point");
        let span = last.packages.saturating_sub(first.packages);
        if span == 0 {
            return 0.0;
        }
        (last.whole.percentiles.p99 - first.whole.percentiles.p99) / as_f64(span)
    }
}

/// Measures the loader against the tracked no-op package.
///
/// # Errors
///
/// Returns a message when the package cannot be read or the synthetic sets
/// cannot be written.
pub fn measure(root: &Path, work: &Path) -> Result<LoaderOverhead, String> {
    let package = DiscoveredPackage::read(root.join(PACKAGE_DIRECTORY))
        .map_err(|error| format!("the no-op package is unreadable: {error}"))?;
    let host = host(&package)?;
    let manifest = PackageManifest::parse(&package.document)
        .map_err(|error| format!("the no-op manifest does not parse: {error}"))?;
    let manifests = [manifest.clone()];
    let package_root = root.join(PACKAGE_DIRECTORY);

    // Every result goes through `black_box`, because a measured call whose
    // result nothing reads is a call the optimizer may delete.
    let parse = Timing::of(|| {
        let parsed = PackageManifest::parse(black_box(&package.document));
        debug_assert!(parsed.is_ok());
        drop(black_box(parsed));
    });
    let integrity = Timing::of(|| {
        let verified = verify_package(black_box(&package_root), black_box(&manifest));
        debug_assert!(verified.is_ok());
        drop(black_box(verified));
    });
    let resolve_timing = Timing::of(|| {
        let plan = resolve(black_box(&manifests), Side::Client);
        debug_assert!(plan.is_ok());
        drop(black_box(plan));
    });
    let whole = Timing::of(|| {
        let mut noop = NoopMod::new();
        let completed = run(
            black_box(std::slice::from_ref(&package)),
            black_box(&host),
            Side::Client,
            &mut [&mut noop as &mut dyn HostMod],
        );
        debug_assert!(completed.is_ok());
        drop(black_box(completed));
    });

    let mut counter = SetMod {
        id: "noop".parse().map_err(|_| "`noop` is a legal mod id")?,
        entered: 0,
    };
    // The receiver is hidden from the optimizer as well as the result: without
    // that, a concrete value behind a `dyn` reference is devirtualized and a
    // thousand counter increments fold into one add.
    let mod_call = Timing::batched(CALLS_PER_SAMPLE, || {
        let value: &mut dyn HostMod = black_box(&mut counter);
        let entered = value.enter(black_box(LifecycleState::Running));
        debug_assert!(entered.is_ok());
        drop(black_box(entered));
    });

    let mod_calls_per_run = {
        let mut noop = NoopMod::new();
        run(
            std::slice::from_ref(&package),
            &host,
            Side::Client,
            &mut [&mut noop as &mut dyn HostMod],
        )
        .map_err(|error| format!("the no-op package did not complete the pipeline: {error}"))?;
        noop.entered().len()
    };

    let scaling = scaling_curve(&package, &host, work)?;

    Ok(LoaderOverhead {
        parse,
        integrity,
        resolve: resolve_timing,
        whole,
        mod_call,
        scaling,
        mod_calls_per_run,
    })
}

/// Times the pipeline for each set size in [`SCALING_SIZES`].
fn scaling_curve(
    package: &DiscoveredPackage,
    host: &Host,
    work: &Path,
) -> Result<Vec<ScalingPoint>, String> {
    let mut curve = Vec::with_capacity(SCALING_SIZES.len());
    for size in SCALING_SIZES {
        let packages = synthetic_set(package, work, size)?;
        let mut mods = mods_for(&packages)?;
        let whole = Timing::of(|| {
            let mut borrowed: Vec<&mut dyn HostMod> = mods
                .iter_mut()
                .map(|value| value as &mut dyn HostMod)
                .collect();
            let completed = run(black_box(&packages), host, Side::Client, &mut borrowed);
            debug_assert!(completed.is_ok());
            drop(black_box(completed));
        });
        curve.push(ScalingPoint {
            packages: size,
            whole,
        });
    }
    Ok(curve)
}

/// Writes `size` copies of the no-op package under distinct identities.
///
/// Only the manifest's `id` changes. The artifact bytes and the provenance
/// record are copied verbatim so their recorded digests stay correct, which
/// keeps integrity verification doing real work rather than being skipped.
fn synthetic_set(
    package: &DiscoveredPackage,
    work: &Path,
    size: usize,
) -> Result<Vec<DiscoveredPackage>, String> {
    let set = work.join(format!("packages/set-{size}"));
    let mut packages = Vec::with_capacity(size);
    for index in 0..size {
        let id = format!("noop-{index:03}");
        let directory = set.join(&id);
        copy_package(&package.root, &directory)?;
        let document =
            package
                .document
                .replacen("\"id\": \"noop\"", &format!("\"id\": \"{id}\""), 1);
        write(
            &directory.join(DiscoveredPackage::MANIFEST_FILE),
            document.as_bytes(),
        )?;
        packages.push(DiscoveredPackage::new(directory, document));
    }
    Ok(packages)
}

/// Copies everything the manifest's relative paths resolve against.
fn copy_package(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to.join("artifacts"))
        .map_err(|error| format!("cannot create a synthetic package directory: {error}"))?;
    for relative in ["provenance.json", "artifacts/noop.artifact"] {
        let bytes = fs::read(from.join(relative))
            .map_err(|error| format!("cannot read a package file: {error}"))?;
        write(&to.join(relative), &bytes)?;
    }
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("cannot write a synthetic package: {error}"))
}

/// One mod value per package, under the identity that package declares.
fn mods_for(packages: &[DiscoveredPackage]) -> Result<Vec<SetMod>, String> {
    packages
        .iter()
        .map(|package| {
            let manifest = PackageManifest::parse(&package.document)
                .map_err(|error| format!("a synthetic manifest does not parse: {error}"))?;
            Ok(SetMod {
                id: manifest.id,
                entered: 0,
            })
        })
        .collect()
}

/// What the packages are admitted against.
///
/// The Minecraft version comes from the package's own declaration and the Ravel
/// version from the crate this binary compiled at, so no version literal lives
/// in this file.
fn host(package: &DiscoveredPackage) -> Result<Host, String> {
    let manifest = PackageManifest::parse(&package.document)
        .map_err(|error| format!("the no-op manifest does not parse: {error}"))?;
    let minecraft = manifest
        .compatibility
        .minecraft
        .first()
        .ok_or("the package declares no Minecraft version")?
        .as_str()
        .to_owned();
    let ravel: Version = env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|_| "the crate version is not semver")?;
    Ok(Host {
        ravel,
        minecraft,
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    })
}

/// The one place a count becomes a float, so the lint exception is stated once.
#[must_use]
pub fn as_f64(value: usize) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "package counts and byte sizes stay far below f64 integer precision limits"
    )]
    let widened = value as f64;
    widened
}

/// Returns the working directory the synthetic sets are written under.
#[must_use]
pub fn work_directory(root: &Path) -> PathBuf {
    root.join("target/m2-loader-overhead")
}
