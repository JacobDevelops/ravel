use crate::evidence::model::{NonemptyRows, NonemptyString, Verdict};
use serde::{Deserialize, de::IgnoredAny};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CounterpartRecord<'a> {
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    pub(super) result: Verdict,
    #[serde(borrow)]
    pub(super) wave: Wave<'a>,
    #[serde(borrow)]
    pub(super) counterparts: NonemptyRows<Counterpart<'a>>,
    #[serde(borrow)]
    pub(super) cluster: Cluster<'a>,
    #[serde(borrow)]
    pub(super) process_validation: ProcessValidation<'a>,
    #[serde(borrow)]
    pub(super) rewrite_integrity: RewriteIntegrity<'a>,
    pub(super) totals: Totals,
}

#[derive(Deserialize)]
pub(super) struct Wave<'a> {
    pub(super) frozen: bool,
    #[serde(borrow)]
    pub(super) entries: NonemptyRows<WaveEntry<'a>>,
}

#[derive(Deserialize)]
pub(super) struct WaveEntry<'a> {
    #[serde(borrow)]
    pub(super) id: NonemptyString<'a>,
    #[serde(rename = "reason", borrow)]
    _reason: NonemptyString<'a>,
    #[serde(rename = "dependencies")]
    _dependencies: Vec<IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Counterpart<'a> {
    #[serde(borrow)]
    pub(super) id: NonemptyString<'a>,
    #[serde(borrow)]
    pub(super) entry: NonemptyString<'a>,
    pub(super) specification: Specification,
    pub(super) observable_areas: u64,
    pub(super) declarable_only: u64,
    pub(super) exercised: u64,
    pub(super) provenance: Provenance,
    pub(super) suites: Suites,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Specification {
    #[serde(rename = "route")]
    _route: Route,
    pub(super) accepted: bool,
    pub(super) structure_reviewed: bool,
    pub(super) carries_upstream_identifier: bool,
}

#[derive(Deserialize)]
pub(super) struct Provenance {
    pub(super) complete: bool,
}

#[derive(Deserialize)]
pub(super) struct Suites {
    pub(super) total: u64,
    pub(super) failed: u64,
}

#[derive(Deserialize)]
pub(super) struct Cluster<'a> {
    #[serde(rename = "scenario", borrow)]
    _scenario: NonemptyString<'a>,
    #[serde(borrow)]
    pub(super) steps: NonemptyRows<Step<'a>>,
}

#[derive(Deserialize)]
pub(super) struct Step<'a> {
    #[serde(borrow)]
    pub(super) name: NonemptyString<'a>,
    pub(super) result: Verdict,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProcessValidation<'a> {
    #[serde(rename = "template", borrow)]
    _template: NonemptyString<'a>,
    #[serde(rename = "checklist", borrow)]
    _checklist: NonemptyString<'a>,
    #[serde(rename = "evidenceSchema", borrow)]
    _evidence_schema: NonemptyString<'a>,
    #[serde(borrow)]
    pub(super) second_counterpart: NonemptyString<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RewriteIntegrity<'a> {
    pub(super) enforcing: bool,
    #[serde(rename = "lexicon", borrow)]
    _lexicon: NonemptyString<'a>,
    #[serde(rename = "identifierSet", borrow)]
    _identifier_set: NonemptyString<'a>,
    pub(super) files_scanned: u64,
    pub(super) artifacts_scanned: u64,
    pub(super) violations: u64,
    pub(super) specification_violations: u64,
    pub(super) mutation_proof: MutationProof,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MutationProof {
    pub(super) source_injection: ExitStatus,
    pub(super) artifact_injection: ExitStatus,
    pub(super) clean_run: ExitStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExitStatus {
    pub(super) exit_status: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Totals {
    pub(super) counterparts: u64,
    pub(super) observable_areas: u64,
    pub(super) declarable_only: u64,
    pub(super) exercised: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Route {
    Source,
    Observed,
}
