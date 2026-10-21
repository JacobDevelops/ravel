use crate::evidence::model::{NonemptyRows, NonemptyString};
use serde::{Deserialize, de::IgnoredAny};
use serde_json::Number;

use ravel_core::Sha256Digest;

#[derive(Deserialize)]
pub(super) struct Inventory<'a> {
    #[serde(rename = "workPackage")]
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(rename = "protocol")]
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    #[serde(rename = "result")]
    #[serde(borrow)]
    pub(super) result: NonemptyString<'a>,
    #[serde(rename = "boundArtifacts")]
    #[serde(borrow)]
    pub(super) bound_artifacts: NonemptyRows<Artifact<'a>>,
    #[serde(rename = "machineryAmendments")]
    #[serde(borrow)]
    pub(super) machinery_amendments: Vec<Amendment<'a>>,
    #[serde(rename = "disclosure")]
    #[serde(borrow)]
    _disclosure: Disclosure<'a>,
    #[serde(rename = "primitiveScan")]
    _primitive_scan: PrimitiveScan,
    #[serde(rename = "artifacts")]
    _artifacts: ArtifactTotals,
    #[serde(rename = "graph")]
    _graph: Graph,
    #[serde(rename = "rights")]
    _rights: Rights,
    #[serde(rename = "surfaces")]
    _surfaces: Surfaces,
    #[serde(rename = "plans")]
    _plans: Plans,
    #[serde(rename = "reads")]
    _reads: AccessCounts,
}
#[derive(Deserialize)]
pub(super) struct Disclosure<'a> {
    #[serde(rename = "readObservation")]
    #[serde(borrow)]
    _read_observation: Observation<'a>,
    #[serde(rename = "judgedClaims")]
    #[serde(borrow)]
    _judged_claims: NonemptyRows<Judgment<'a>>,
}
#[derive(Deserialize)]
pub(super) struct Observation<'a> {
    #[serde(rename = "bound")]
    #[serde(borrow)]
    _bound: NonemptyString<'a>,
}
#[derive(Deserialize)]
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
pub(super) struct Artifact<'a> {
    #[serde(rename = "name")]
    #[serde(borrow)]
    pub(super) name: NonemptyString<'a>,
    #[serde(rename = "holds")]
    #[serde(borrow)]
    _holds: NonemptyString<'a>,
    #[serde(rename = "sha256")]
    _sha256: Sha256Digest,
    #[serde(rename = "class")]
    pub(super) class: ArtifactClass,
}
#[derive(Deserialize)]
pub(super) struct Amendment<'a> {
    #[serde(rename = "milestone")]
    #[serde(borrow)]
    _milestone: NonemptyString<'a>,
    #[serde(rename = "reason")]
    #[serde(borrow)]
    _reason: NonemptyString<'a>,
    #[serde(rename = "figures")]
    _figures: Vec<IgnoredAny>,
    #[serde(rename = "entries")]
    #[serde(borrow)]
    pub(super) entries: NonemptyRows<NonemptyString<'a>>,
}
#[derive(Deserialize)]
pub(super) struct PrimitiveScan {
    #[serde(rename = "class")]
    _class: Machinery,
}
#[derive(Deserialize)]
pub(super) struct ArtifactTotals {
    #[serde(rename = "total")]
    _total: Number,
    #[serde(rename = "catalogued")]
    _catalogued: Number,
    #[serde(rename = "uncatalogued")]
    _uncatalogued: Number,
    #[serde(rename = "planned")]
    _planned: Number,
}
#[derive(Deserialize)]
pub(super) struct Graph {
    #[serde(rename = "nodes")]
    _nodes: Number,
    #[serde(rename = "resolvedNodes")]
    _resolved_nodes: Number,
    #[serde(rename = "blockedNodes")]
    _blocked_nodes: Number,
    #[serde(rename = "edges")]
    _edges: Number,
    #[serde(rename = "resolvedEdges")]
    _resolved_edges: Number,
    #[serde(rename = "blockedEdges")]
    _blocked_edges: Number,
    #[serde(rename = "clusters")]
    _clusters: Number,
    #[serde(rename = "largestCluster")]
    _largest_cluster: Number,
}
#[derive(Deserialize)]
pub(super) struct Rights {
    #[serde(rename = "sourceUse")]
    _source_use: SourceUse,
    #[serde(rename = "publishable")]
    _publishable: Number,
}
#[derive(Deserialize)]
pub(super) struct SourceUse {
    #[serde(rename = "permitted")]
    _permitted: Number,
    #[serde(rename = "conditional")]
    _conditional: Number,
    #[serde(rename = "prohibited")]
    _prohibited: Number,
    #[serde(rename = "unresolved")]
    _unresolved: Number,
}
#[derive(Deserialize)]
pub(super) struct Surfaces {
    #[serde(rename = "classFiles")]
    _class_files: Number,
    #[serde(rename = "models")]
    _models: Number,
    #[serde(rename = "textures")]
    _textures: Number,
    #[serde(rename = "recipes")]
    _recipes: Number,
    #[serde(rename = "catalogued")]
    _catalogued: Number,
    #[serde(rename = "unclassified")]
    _unclassified: Number,
    #[serde(rename = "nestedJars")]
    _nested_jars: Number,
}
#[derive(Deserialize)]
pub(super) struct Plans {
    #[serde(rename = "planned")]
    _planned: Number,
    #[serde(rename = "readingSource")]
    _reading_source: Number,
    #[serde(rename = "notReadingSource")]
    _not_reading_source: Number,
    #[serde(rename = "dependencyEdges")]
    _dependency_edges: Number,
    #[serde(rename = "canonicalIds")]
    _canonical_ids: Number,
}
#[derive(Deserialize)]
pub(super) struct AccessCounts {
    #[serde(rename = "producersWatched")]
    _producers_watched: Number,
    #[serde(rename = "archiveReads")]
    _archive_reads: Number,
    #[serde(rename = "entryBodyReads")]
    _entry_body_reads: Number,
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
