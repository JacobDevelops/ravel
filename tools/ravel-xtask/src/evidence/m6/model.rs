use crate::evidence::model::{NonemptyRows, NonemptyString};
use serde::{Deserialize, de::IgnoredAny};
use serde_json::Number;

#[derive(Deserialize)]
pub(super) struct CounterpartRecord<'a> {
    #[serde(rename = "workPackage")]
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(rename = "protocol")]
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    #[serde(rename = "result")]
    #[serde(borrow)]
    pub(super) result: NonemptyString<'a>,
    #[serde(rename = "wave")]
    #[serde(borrow)]
    pub(super) wave: Wave<'a>,
    #[serde(rename = "counterparts")]
    #[serde(borrow)]
    pub(super) counterparts: NonemptyRows<Counterpart<'a>>,
    #[serde(rename = "cluster")]
    #[serde(borrow)]
    _cluster: Cluster<'a>,
    #[serde(rename = "processValidation")]
    #[serde(borrow)]
    _process_validation: ProcessValidation<'a>,
    #[serde(rename = "rewriteIntegrity")]
    #[serde(borrow)]
    _rewrite_integrity: RewriteIntegrity<'a>,
    #[serde(rename = "totals")]
    _totals: Totals,
}
#[derive(Deserialize)]
pub(super) struct Wave<'a> {
    #[serde(rename = "frozen")]
    pub(super) frozen: bool,
    #[serde(rename = "entries")]
    #[serde(borrow)]
    _entries: NonemptyRows<WaveEntry<'a>>,
}
#[derive(Deserialize)]
pub(super) struct WaveEntry<'a> {
    #[serde(rename = "id")]
    #[serde(borrow)]
    _id: NonemptyString<'a>,
    #[serde(rename = "reason")]
    #[serde(borrow)]
    _reason: NonemptyString<'a>,
    #[serde(rename = "dependencies")]
    _dependencies: Vec<IgnoredAny>,
}
#[derive(Deserialize)]
pub(super) struct Counterpart<'a> {
    #[serde(rename = "id")]
    #[serde(borrow)]
    pub(super) id: NonemptyString<'a>,
    #[serde(rename = "entry")]
    #[serde(borrow)]
    _entry: NonemptyString<'a>,
    #[serde(rename = "specification")]
    _specification: Specification,
    #[serde(rename = "observableAreas")]
    _observable_areas: Number,
    #[serde(rename = "declarableOnly")]
    _declarable_only: Number,
    #[serde(rename = "exercised")]
    _exercised: Number,
    #[serde(rename = "provenance")]
    _provenance: Provenance,
    #[serde(rename = "suites")]
    _suites: Suites,
}
#[derive(Deserialize)]
pub(super) struct Specification {
    #[serde(rename = "route")]
    _route: Route,
    #[serde(rename = "accepted")]
    _accepted: bool,
    #[serde(rename = "structureReviewed")]
    _structure_reviewed: bool,
    #[serde(rename = "carriesUpstreamIdentifier")]
    _carries_upstream_identifier: bool,
}
#[derive(Deserialize)]
pub(super) struct Provenance {
    #[serde(rename = "complete")]
    _complete: bool,
}
#[derive(Deserialize)]
pub(super) struct Suites {
    #[serde(rename = "total")]
    _total: Number,
    #[serde(rename = "failed")]
    _failed: Number,
}
#[derive(Deserialize)]
pub(super) struct Cluster<'a> {
    #[serde(rename = "scenario")]
    #[serde(borrow)]
    _scenario: NonemptyString<'a>,
    #[serde(rename = "steps")]
    #[serde(borrow)]
    _steps: NonemptyRows<Step<'a>>,
}
#[derive(Deserialize)]
pub(super) struct Step<'a> {
    #[serde(rename = "name")]
    #[serde(borrow)]
    _name: NonemptyString<'a>,
    #[serde(rename = "result")]
    #[serde(borrow)]
    _result: NonemptyString<'a>,
}
#[derive(Deserialize)]
pub(super) struct ProcessValidation<'a> {
    #[serde(rename = "template")]
    #[serde(borrow)]
    _template: NonemptyString<'a>,
    #[serde(rename = "checklist")]
    #[serde(borrow)]
    _checklist: NonemptyString<'a>,
    #[serde(rename = "evidenceSchema")]
    #[serde(borrow)]
    _evidence_schema: NonemptyString<'a>,
    #[serde(rename = "secondCounterpart")]
    #[serde(borrow)]
    _second_counterpart: NonemptyString<'a>,
}
#[derive(Deserialize)]
pub(super) struct RewriteIntegrity<'a> {
    #[serde(rename = "enforcing")]
    _enforcing: bool,
    #[serde(rename = "lexicon")]
    #[serde(borrow)]
    _lexicon: NonemptyString<'a>,
    #[serde(rename = "identifierSet")]
    #[serde(borrow)]
    _identifier_set: NonemptyString<'a>,
    #[serde(rename = "filesScanned")]
    _files_scanned: Number,
    #[serde(rename = "artifactsScanned")]
    _artifacts_scanned: Number,
    #[serde(rename = "violations")]
    _violations: Number,
    #[serde(rename = "specificationViolations")]
    _specification_violations: Number,
    #[serde(rename = "mutationProof")]
    _mutation_proof: MutationProof,
}
#[derive(Deserialize)]
pub(super) struct MutationProof {
    #[serde(rename = "sourceInjection")]
    _source_injection: ExitStatus,
    #[serde(rename = "artifactInjection")]
    _artifact_injection: ExitStatus,
    #[serde(rename = "cleanRun")]
    _clean_run: ExitStatus,
}
#[derive(Deserialize)]
pub(super) struct ExitStatus {
    #[serde(rename = "exitStatus")]
    _exit_status: Number,
}
#[derive(Deserialize)]
pub(super) struct Totals {
    #[serde(rename = "counterparts")]
    _counterparts: Number,
    #[serde(rename = "observableAreas")]
    _observable_areas: Number,
    #[serde(rename = "declarableOnly")]
    _declarable_only: Number,
    #[serde(rename = "exercised")]
    _exercised: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Route {
    Source,
    Observed,
}
