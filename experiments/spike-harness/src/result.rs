//! Serializable form of `schemas/spike-result.schema.json`.
//!
//! Candidates build these values directly so that every gate reports the same
//! fields, units, collector identity, and evidence quality.

use serde::Serialize;

/// Architecture decision gate a result belongs to.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Gate {
    #[serde(rename = "ADG-1")]
    JvmEntry,
    #[serde(rename = "ADG-2")]
    ModExecution,
    #[serde(rename = "ADG-3")]
    Bindings,
    #[serde(rename = "ADG-4")]
    StateOwnership,
    #[serde(rename = "ADG-5")]
    Prism,
}

/// Unit of a reported measurement.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Unit {
    Bytes,
    Classes,
    Milliseconds,
    Nanoseconds,
    Count,
    BytesPerCall,
    CountPerCall,
}

/// How exactly a value reflects what it claims to measure.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Quality {
    Exact,
    Estimated,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Status {
    Measured,
    Unavailable,
}

/// One numeric metric with its unit, collector, and quality, or a recorded gap.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    status: Status,
    collector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<Unit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<Quality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl Measurement {
    /// Records a directly observed value.
    #[must_use]
    pub fn exact(collector: &str, value: f64, unit: Unit) -> Self {
        Self::new(collector, value, unit, Quality::Exact)
    }

    /// Records a derived or sampled value that is not an exact observation.
    #[must_use]
    pub fn estimated(collector: &str, value: f64, unit: Unit) -> Self {
        Self::new(collector, value, unit, Quality::Estimated)
    }

    /// Records that the platform cannot expose this metric for this candidate.
    #[must_use]
    pub fn unavailable(collector: &str, reason: &str) -> Self {
        Self {
            status: Status::Unavailable,
            collector: collector.to_owned(),
            value: None,
            unit: None,
            quality: None,
            reason: Some(reason.to_owned()),
        }
    }

    fn new(collector: &str, value: f64, unit: Unit, quality: Quality) -> Self {
        Self {
            status: Status::Measured,
            collector: collector.to_owned(),
            value: Some(value),
            unit: Some(unit),
            quality: Some(quality),
            reason: None,
        }
    }
}

/// Callback-cost distribution over the measured invocation window.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    status: Status,
    collector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<Quality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    median: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p95: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    p99: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl Distribution {
    /// Summarizes observed samples through one shared percentile definition.
    ///
    /// Returns an unavailable distribution when no sample was collected, because
    /// a missing measurement must never be reported as zero.
    #[must_use]
    pub fn measured(collector: &str, samples: &[u64]) -> Self {
        let Some(percentiles) = crate::percentiles(samples) else {
            return Self::unavailable(collector, "the trial collected no callback samples");
        };
        Self {
            status: Status::Measured,
            collector: collector.to_owned(),
            quality: Some(Quality::Exact),
            sample_count: Some(samples.len()),
            median: Some(percentiles.median),
            p95: Some(percentiles.p95),
            p99: Some(percentiles.p99),
            reason: None,
        }
    }

    /// Summarizes batch totals as per-call cost.
    ///
    /// A single boundary transition costs less than the clock's resolution, so
    /// trials time a batch. Dividing inside the trial would truncate a two
    /// nanosecond call to zero, so the division happens here, once, after the
    /// percentiles are taken. Percentiles scale with the divisor, and the result
    /// is labelled estimated because a batch mean is not a single observation.
    #[must_use]
    pub fn from_batches(collector: &str, batch_nanoseconds: &[u64], batch: u64) -> Self {
        if batch == 0 {
            return Self::unavailable(collector, "the trial reported an empty batch size");
        }
        let Some(percentiles) = crate::percentiles(batch_nanoseconds) else {
            return Self::unavailable(collector, "the trial collected no callback samples");
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "batch sizes stay far below f64 integer precision limits"
        )]
        let divisor = batch as f64;
        Self {
            status: Status::Measured,
            collector: collector.to_owned(),
            quality: Some(Quality::Estimated),
            sample_count: Some(batch_nanoseconds.len()),
            median: Some(percentiles.median / divisor),
            p95: Some(percentiles.p95 / divisor),
            p99: Some(percentiles.p99 / divisor),
            reason: None,
        }
    }

    /// Records that callback cost could not be observed for this candidate.
    #[must_use]
    pub fn unavailable(collector: &str, reason: &str) -> Self {
        Self {
            status: Status::Unavailable,
            collector: collector.to_owned(),
            quality: None,
            sample_count: None,
            median: None,
            p95: None,
            p99: None,
            reason: Some(reason.to_owned()),
        }
    }
}

/// Sanitized name and content hash of a raw trial artifact.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HashedArtifact {
    pub name: String,
    pub sha256: String,
}

/// Candidate identity and whether it stayed in the comparison.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub disposition: Disposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

/// Whether a candidate is measured evidence or a recorded rejection.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    Measured,
    Rejected,
}

/// Source revision the trial exercised.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    /// A commit that existed when the trial ran.
    ///
    /// Every later rewrite abandons it, so it records what ran and is not a way
    /// to find anything. Use `change_id` to resolve the result.
    pub id: String,
    /// The change that holds this result.
    ///
    /// Stable across every rewrite, which is why it is the checkable half:
    /// `ravel:verify-spikes` recomputes it against the change that introduced
    /// the file rather than trusting what is written here.
    pub change_id: String,
    /// Whether the measured tree differed from `id`.
    ///
    /// For results this harness writes, and in a jj repository, this is normally
    /// true and says nothing bad: the
    /// working copy is a change with content, and the measured tree is `id`
    /// plus the diff of the change named by `change_id`. What makes the
    /// measurement reproducible is that the difference is exactly that diff,
    /// which is what a reviewer reads.
    pub dirty: bool,
}

/// Trial provenance shared by every candidate of every gate.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trial {
    pub revision: Revision,
    pub command_sha256: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub clock: Clock,
    pub warmup_invocations: u64,
    pub measured_invocations: u64,
}

/// The single clock every recorded interval uses.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Clock {
    Monotonic,
}

/// Pinned runtime inputs the trial ran against.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub target: Target,
    pub hardware_profile: String,
    pub minecraft: String,
    pub prism: String,
    /// Runtime the trial ran on, when it hosted a JVM at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java: Option<JavaIdentity>,
    pub trial_kind: TrialKind,
}

/// The only target M1 measures.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    #[serde(rename = "x86_64-linux")]
    X8664Linux,
}

/// Java runtime the trial used, and whether it is the pinned game runtime.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaIdentity {
    pub version: String,
    pub runtime: String,
    pub pinned: bool,
}

/// Whether a trial ran in the spike harness or the real game process.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrialKind {
    Harness,
    GameProcess,
}

/// Bootstrap and retained cost of the candidate.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub bootstrap_peak_rss: Measurement,
    pub retained_rss: Measurement,
    pub retained_classes: Measurement,
}

/// Time from process start to the candidate's ready boundary.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub process_to_ready: Measurement,
    pub candidate_delta: Measurement,
}

/// Callback invocation cost over the measured window.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calls {
    pub invocation_count: u64,
    pub callback_nanoseconds: Distribution,
}

/// Bytes crossing the candidate's boundary.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Boundary {
    pub bytes_per_call: Measurement,
    pub total_bytes: Measurement,
}

/// Allocation behaviour attributable to the boundary.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub allocations_per_call: Measurement,
    pub allocated_bytes_per_call: Measurement,
}

/// Installed footprint of the candidate's artifacts.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Packaging {
    pub installed_bytes: u64,
    pub artifact_count: u64,
    pub required_components: Vec<String>,
}

/// One required failure scenario and its observed outcome.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureOutcome {
    pub scenario: FailureScenario,
    pub outcome: FailureBehaviour,
    pub detail: String,
}

/// Failure scenarios every measured candidate must exercise.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureScenario {
    InvalidInput,
    UnavailableRuntime,
    CallbackFailure,
    Crash,
    Timeout,
}

/// How the candidate behaved in a failure scenario.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureBehaviour {
    FailedClosed,
    Continued,
    NotEvaluated,
}

/// Unsafe surface a production implementation of the candidate would need.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Safety {
    pub maintained_unsafe_blocks: u32,
    pub maintained_unsafe_detail: String,
    pub audited_boundary_dependencies: Vec<BoundaryDependency>,
}

/// Audited dependency that encapsulates a boundary operation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundaryDependency {
    pub name: String,
    pub version: String,
    pub licence: String,
}

/// Day-to-day operability of the candidate.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operability {
    pub diagnostics: String,
    pub debug_workflow: String,
    pub crash_attribution: String,
    pub prism_launch: PrismLaunch,
}

/// How the candidate reaches the pinned game through Prism.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrismLaunch {
    StockJvmArguments,
    StockWrapperCommand,
    PatchRequired,
    NotEvaluated,
}

/// Observation that is qualitative rather than numeric.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub status: NoteStatus,
    pub detail: String,
}

/// Whether a qualitative observation was actually made.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteStatus {
    Observed,
    Unavailable,
}

/// ADG-3 binding generation evidence.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingGeneration {
    pub duration_ms: u64,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub drift_detected: bool,
}

/// ADG-4 state-ownership evidence for the representative object flow.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateOwnership {
    pub model: OwnershipModel,
    pub duplicated_state_bytes: Measurement,
    pub boundary_bytes_per_flow: Measurement,
}

/// Which side holds authoritative state in a measured variant.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnershipModel {
    JavaAuthoritative,
    RustAuthoritative,
    Mirrored,
}

/// ADG-5 Prism integration evidence.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrismIntegration {
    pub stock_launch: bool,
    pub changed_lines: u32,
    pub ownership: PatchOwnership,
    pub detail: String,
}

/// Who would own a launcher change if one were required.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatchOwnership {
    PrismUpstream,
    Ravel,
    NoneRequired,
}

/// Recorded difference between the planned and executed trial.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deviation {
    pub summary: String,
    pub classification: Classification,
}

/// Whether a deviation blocks the gate.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    Blocking,
    NonBlocking,
}

/// One candidate's complete comparable evidence for one gate.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpikeResult {
    pub schema_version: u8,
    pub protocol_version: String,
    pub gate: Gate,
    pub candidate: Candidate,
    pub trial: Trial,
    pub identity: Identity,
    pub memory: Memory,
    pub startup: Startup,
    pub calls: Calls,
    pub boundary: Boundary,
    pub allocation: Allocation,
    pub packaging: Packaging,
    pub failures: Vec<FailureOutcome>,
    pub safety: Safety,
    pub operability: Operability,
    pub artifacts: Vec<HashedArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolation: Option<Note>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abi_evolution: Option<Note>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_to_mod_calls: Option<Measurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding_generation: Option<BindingGeneration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_ownership: Option<StateOwnership>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prism_integration: Option<PrismIntegration>,
    pub deviations: Vec<Deviation>,
}
