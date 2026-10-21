use crate::evidence::model::{NonemptyRows, NonemptyString, Verdict};
use serde::{Deserialize, de::IgnoredAny};

use ravel_core::Sha256Digest;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Inventory<'a> {
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    pub(super) result: Verdict,
    #[serde(borrow)]
    pub(super) bound_artifacts: NonemptyRows<Artifact<'a>>,
    #[serde(borrow)]
    pub(super) machinery_amendments: Vec<Amendment<'a>>,
    #[serde(rename = "disclosure")]
    #[serde(borrow)]
    _disclosure: Disclosure<'a>,
    #[serde(rename = "primitiveScan")]
    _primitive_scan: PrimitiveScan,
    pub(super) artifacts: ArtifactTotals,
    pub(super) graph: Graph,
    pub(super) rights: Rights,
    pub(super) surfaces: Surfaces,
    pub(super) plans: Plans,
    pub(super) reads: AccessCounts,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Disclosure<'a> {
    #[serde(rename = "readObservation")]
    #[serde(borrow)]
    _read_observation: Observation<'a>,
    #[serde(rename = "judgedClaims")]
    #[serde(borrow)]
    _judged_claims: NonemptyRows<Judgment<'a>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Observation<'a> {
    #[serde(rename = "bound")]
    #[serde(borrow)]
    _bound: NonemptyString<'a>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Judgment<'a> {
    #[serde(rename = "claim")]
    #[serde(borrow)]
    _claim: NonemptyString<'a>,
    #[serde(rename = "decidedBy")]
    #[serde(borrow)]
    _decided_by: NonemptyString<'a>,
    #[serde(rename = "basis")]
    _basis: Judged,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Artifact<'a> {
    #[serde(borrow)]
    pub(super) name: NonemptyString<'a>,
    #[serde(rename = "holds")]
    #[serde(borrow)]
    _holds: NonemptyString<'a>,
    #[serde(rename = "sha256")]
    _sha256: Sha256Digest,
    pub(super) class: ArtifactClass,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Amendment<'a> {
    #[serde(rename = "milestone")]
    #[serde(borrow)]
    _milestone: NonemptyString<'a>,
    #[serde(rename = "reason")]
    #[serde(borrow)]
    _reason: NonemptyString<'a>,
    #[serde(rename = "figures")]
    _figures: Vec<IgnoredAny>,
    #[serde(borrow)]
    pub(super) entries: NonemptyRows<NonemptyString<'a>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrimitiveScan {
    #[serde(rename = "class")]
    _class: Machinery,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArtifactTotals {
    pub(super) total: u64,
    pub(super) catalogued: u64,
    pub(super) uncatalogued: u64,
    pub(super) planned: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Graph {
    pub(super) nodes: u64,
    pub(super) resolved_nodes: u64,
    pub(super) blocked_nodes: u64,
    pub(super) edges: u64,
    pub(super) resolved_edges: u64,
    pub(super) blocked_edges: u64,
    pub(super) clusters: u64,
    pub(super) largest_cluster: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Rights {
    pub(super) source_use: SourceUse,
    pub(super) publishable: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceUse {
    pub(super) permitted: u64,
    pub(super) conditional: u64,
    pub(super) prohibited: u64,
    pub(super) unresolved: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Surfaces {
    #[serde(rename = "classFiles")]
    _class_files: u64,
    #[serde(rename = "models")]
    _models: u64,
    #[serde(rename = "textures")]
    _textures: u64,
    #[serde(rename = "recipes")]
    _recipes: u64,
    pub(super) catalogued: u64,
    pub(super) unclassified: u64,
    #[serde(rename = "nestedJars")]
    _nested_jars: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Plans {
    pub(super) planned: u64,
    pub(super) reading_source: u64,
    pub(super) not_reading_source: u64,
    pub(super) dependency_edges: u64,
    pub(super) canonical_ids: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccessCounts {
    pub(super) producers_watched: u64,
    pub(super) archive_reads: u64,
    pub(super) entry_body_reads: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ArtifactClass {
    Material,
    Machinery,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Machinery {
    Machinery,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Judged {
    Judged,
}
