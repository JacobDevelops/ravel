//! Typed library summaries and protocol-specific fixture bindings.

use ravel_core::Sha256Digest;
use serde::Deserialize;
use serde_json::Value;

use crate::evidence::{
    model::{NonemptyRows, NonemptyString, Verdict, admit},
    rules::require,
};

pub(super) struct LibrarySummary<'a> {
    pub(super) record: Record<'a>,
    pub(super) protocol: Protocol<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Record<'a> {
    #[serde(borrow)]
    work_package: NonemptyString<'a>,
    result: Verdict,
    protocol: ProtocolKind,
    pub(super) entries: Entries,
    pub(super) waves: NonemptyRows<Wave>,
    pub(super) dependencies: Resolution,
    pub(super) registries: Resolution,
    pub(super) provenance: Provenance,
    pub(super) routing: Routing,
    pub(super) suites: Suites,
}

pub(super) enum Protocol<'a> {
    Historical { correction: Option<Correction<'a>> },
    Workload { fixture: Fixture<'a> },
}

#[derive(Deserialize)]
enum ProtocolKind {
    #[serde(rename = "m7-library-v1")]
    Historical,
    #[serde(rename = "m7-library-v2")]
    Workload,
}

#[derive(Deserialize)]
struct HistoricalBinding<'a> {
    #[serde(rename = "fixture")]
    _fixture: Option<()>,
    #[serde(borrow)]
    correction: Option<Correction<'a>>,
}

#[derive(Deserialize)]
struct WorkloadBinding<'a> {
    #[serde(borrow)]
    fixture: Fixture<'a>,
    #[serde(rename = "correction")]
    _correction: Option<()>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Fixture<'a> {
    #[serde(borrow)]
    pub(super) id: NonemptyString<'a>,
    pub(super) artifact_set: ArtifactSet,
}

#[derive(Deserialize)]
pub(super) struct ArtifactSet {
    pub(super) count: u64,
    pub(super) sha256: Sha256Digest,
}

#[derive(Deserialize)]
pub(super) struct Correction<'a> {
    pub(super) from: u64,
    pub(super) total: u64,
    #[serde(borrow, rename = "reason")]
    _reason: NonemptyString<'a>,
}

#[derive(Deserialize)]
pub(super) struct Entries {
    pub(super) total: u64,
    pub(super) accepted: u64,
    pub(super) blocked: u64,
    pub(super) unstarted: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Wave {
    pub(super) index: u64,
    pub(super) entries: u64,
    pub(super) depends_on: Vec<u64>,
}

#[derive(Deserialize)]
pub(super) struct Resolution {
    #[serde(rename = "resolved")]
    _resolved: u64,
    pub(super) unresolved: u64,
}

#[derive(Deserialize)]
pub(super) struct Provenance {
    pub(super) present: u64,
    pub(super) missing: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Routing {
    pub(super) absorbed: u64,
    pub(super) remaining: u64,
    pub(super) runnable_today: u64,
    pub(super) blocked: u64,
}

#[derive(Deserialize)]
pub(super) struct Suites {
    pub(super) packages: Suite,
    pub(super) clusters: Suite,
}

#[derive(Deserialize)]
pub(super) struct Suite {
    pub(super) total: u64,
    pub(super) failed: u64,
}

impl<'a> LibrarySummary<'a> {
    pub(super) fn read(value: &'a Value) -> Result<Self, String> {
        let admitted: Result<Self, String> = (|| {
            let record: Record<'a> = admit(value)?;
            let protocol = match record.protocol {
                ProtocolKind::Historical => {
                    let HistoricalBinding { correction, .. } = admit(value)?;
                    Protocol::Historical { correction }
                }
                ProtocolKind::Workload => {
                    let WorkloadBinding { fixture, .. } = admit(value)?;
                    Protocol::Workload { fixture }
                }
            };
            Ok(Self { record, protocol })
        })();
        admitted.map_err(|error| format!("invalid library summary: {error}"))
    }

    pub(super) fn entry_total(&self) -> u64 {
        self.record.entries.total
    }

    pub(super) fn verify(&self) -> Result<(), String> {
        crate::evidence::model::require_identity(
            self.record.work_package.0,
            "M7-W5",
            "/workPackage",
        )?;
        require(self.record.result == Verdict::Pass, "/result must be pass")?;
        if matches!(self.protocol, Protocol::Workload { .. }) {
            require(self.entry_total() > 0, "/entries/total must be positive")?;
        }
        Ok(())
    }
}
