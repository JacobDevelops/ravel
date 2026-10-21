//! Typed admission for library summaries. Underscored figures are consumed by shared JSON arithmetic.

use ravel_core::Sha256Digest;
use serde::Deserialize;
use serde_json::{Number, Value};

use crate::evidence::{
    model::{NonemptyRows, admit},
    rules::require,
};

pub(super) struct LibrarySummary<'a> {
    record: Record<'a>,
    pub(super) protocol: Protocol<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record<'a> {
    work_package: &'a str,
    result: &'a str,
    protocol: ProtocolKind,
    entries: Entries,
    #[serde(rename = "waves")]
    _waves: NonemptyRows<Wave>,
    #[serde(rename = "dependencies")]
    _dependencies: Resolution,
    #[serde(rename = "registries")]
    _registries: Resolution,
    #[serde(rename = "provenance")]
    _provenance: Provenance,
    #[serde(rename = "routing")]
    _routing: Routing,
    #[serde(rename = "suites")]
    _suites: Suites,
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
    pub(super) id: &'a str,
    pub(super) artifact_set: ArtifactSet,
}

#[derive(Deserialize)]
pub(super) struct ArtifactSet {
    pub(super) count: u64,
    pub(super) sha256: Sha256Digest,
}

#[derive(Deserialize)]
pub(super) struct Correction<'a> {
    #[serde(rename = "from")]
    pub(super) from: Number,
    #[serde(rename = "total")]
    pub(super) total: Number,
    reason: &'a str,
}

#[derive(Deserialize)]
struct Entries {
    total: Number,
    accepted: Number,
    blocked: Number,
    unstarted: Number,
}

#[derive(Deserialize)]
struct Wave {
    #[serde(rename = "index")]
    _index: Number,
    #[serde(rename = "entries")]
    _entries: Number,
    #[serde(rename = "dependsOn")]
    _depends_on: Vec<Number>,
}

#[derive(Deserialize)]
struct Resolution {
    #[serde(rename = "resolved")]
    _resolved: Number,
    #[serde(rename = "unresolved")]
    _unresolved: Number,
}

#[derive(Deserialize)]
struct Provenance {
    #[serde(rename = "present")]
    _present: Number,
    #[serde(rename = "missing")]
    _missing: Number,
}

#[derive(Deserialize)]
struct Routing {
    #[serde(rename = "absorbed")]
    _absorbed: Number,
    #[serde(rename = "remaining")]
    _remaining: Number,
    #[serde(rename = "runnableToday")]
    _runnable_today: Number,
    #[serde(rename = "blocked")]
    _blocked: Number,
}

#[derive(Deserialize)]
struct Suites {
    #[serde(rename = "packages")]
    _packages: Suite,
    #[serde(rename = "clusters")]
    _clusters: Suite,
}

#[derive(Deserialize)]
struct Suite {
    #[serde(rename = "total")]
    _total: Number,
    #[serde(rename = "failed")]
    _failed: Number,
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

    pub(super) fn entry_total(&self) -> &Number {
        &self.record.entries.total
    }

    pub(super) fn verify(&self) -> Result<(), String> {
        require(
            self.record.work_package == "M7-W5",
            "/workPackage must be M7-W5",
        )?;
        require(self.record.result == "pass", "/result must be pass")?;
        match &self.protocol {
            Protocol::Historical { correction } => {
                if let Some(correction) = correction {
                    require(
                        !correction.reason.is_empty(),
                        "/correction/reason must be nonempty",
                    )?;
                }
            }
            Protocol::Workload { fixture } => {
                require(!fixture.id.is_empty(), "/fixture/id must be nonempty")?;
                require(
                    self.record.entries.total.as_u64().is_some_and(|n| n > 0),
                    "/entries/total must be positive",
                )?;
                for (count, requirement) in [
                    (
                        &self.record.entries.accepted,
                        "/entries/accepted must be a whole number",
                    ),
                    (
                        &self.record.entries.blocked,
                        "/entries/blocked must be a whole number",
                    ),
                    (
                        &self.record.entries.unstarted,
                        "/entries/unstarted must be a whole number",
                    ),
                ] {
                    require(count.as_u64().is_some(), requirement)?;
                }
            }
        }
        Ok(())
    }
}
