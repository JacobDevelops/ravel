//! Times the production pre-world loader pipeline through `ravel-testkit`.

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

/// Discarded warmup runs.
const WARMUP_RUNS: usize = 500;

/// Samples per metric.
const MEASURED_RUNS: usize = 5_000;

/// Package-set sizes; larger sets copy the no-op package under distinct identities.
pub const SCALING_SIZES: [usize; 4] = [1, 16, 64, 256];

/// No-op behavior under a supplied identity for scaling measurements.
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

/// Batch cheap operations to amortize monotonic-clock overhead.
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
    /// Verifying artifact and provenance digests.
    pub integrity: Timing,
    /// Resolving validated manifests into a load plan.
    pub resolve: Timing,
    /// Validation, resolution, and ordered lifecycle entry for the no-op package.
    pub whole: Timing,
    /// One `HostMod::enter` call through the trait object the loader drives.
    pub mod_call: Timing,
    /// What the pipeline costs as the package set grows.
    pub scaling: Vec<ScalingPoint>,
    /// Mod calls one mod receives in one process run.
    pub mod_calls_per_run: usize,
}

impl LoaderOverhead {
    /// Whole-pipeline p99 minus individually measured stage p99s, clamped to zero.
    #[must_use]
    pub fn lifecycle_residual(&self) -> f64 {
        let stages = self.parse.percentiles.p99
            + self.integrity.percentiles.p99
            + self.resolve.percentiles.p99;
        (self.whole.percentiles.p99 - stages).max(0.0)
    }

    /// Marginal per-package cost from the scaling curve's endpoints.
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
/// Returns a message when the package cannot be read or synthetic sets cannot be written.
pub fn measure(root: &Path, work: &Path) -> Result<LoaderOverhead, String> {
    let package = DiscoveredPackage::read(root.join(PACKAGE_DIRECTORY))
        .map_err(|error| format!("the no-op package is unreadable: {error}"))?;
    let host = host(&package)?;
    let manifest = PackageManifest::parse(&package.document)
        .map_err(|error| format!("the no-op manifest does not parse: {error}"))?;
    let manifests = [manifest.clone()];
    let package_root = root.join(PACKAGE_DIRECTORY);

    // Keep measured results observable to the optimizer.
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
    // Hide the receiver to prevent devirtualization and folded counter increments.
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

/// Copies the no-op package with distinct ids, preserving artifact and provenance digests.
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
    fs::create_dir_all(to.join("src"))
        .map_err(|error| format!("cannot create a synthetic package directory: {error}"))?;
    for relative in ["Cargo.toml", "provenance.json", "src/lib.rs"] {
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

/// Host versions from the package's game declaration and this crate's version.
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
    Ok(Host { ravel, minecraft })
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
