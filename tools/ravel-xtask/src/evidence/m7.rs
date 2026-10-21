//! M7 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::model::count_sum;
use crate::evidence::rules::{
    Rule, all_gates_closed, has_exact_work_packages, has_native_attestation,
};
use crate::json::{array, read_json};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};

mod model;
use model::{LibrarySummary, Protocol};

fn legacy_fixture_total(root: &Path) -> Result<u64, String> {
    let path = "tests/fixtures/descriptors/atm10-tts-2.0.2.json";
    let descriptor = read_json(root, &root.join(path))?;
    descriptor
        .pointer("/manifest/expectedReferenceCount")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{path}: missing whole-number /manifest/expectedReferenceCount"))
}

fn verify_library_record(value: &Value) -> Result<(), String> {
    LibrarySummary::read(value)?.verify()
}

const M7_RECORDS: &[RecordDefinition] = &[RecordDefinition {
    name: "library-summary.json",
    rule: verify_library_record,
    description: "a library summary carrying the burn-up ledger as numbers, the waves with the waves each \
 depends on, any evidence correction with the figure it replaced and its reason, and the \
 dependency, registry, provenance, routing and suite figures M7-W5 audits",
    required: false,
}];

type LibraryRule = for<'a> fn(&LibrarySummary<'a>) -> bool;
type LibraryFigure = for<'a> fn(&LibrarySummary<'a>) -> Option<Value>;

const M7_ARITHMETIC: &[(LibraryRule, &str)] = &[
    (
        |summary| {
            let entries = &summary.record.entries;
            count_sum([entries.accepted, entries.blocked, entries.unstarted]) == Some(entries.total)
        },
        "M7-W4's burn-up ledger asks that the accepted, blocked and unstarted totals equal the verified fixture total",
    ),
    (
        |summary| summary.record.entries.blocked == 0 && summary.record.entries.unstarted == 0,
        "a wave cannot close with failing or unclassified entries, so the library cannot close with any",
    ),
    (
        |summary| summary.record.entries.accepted == summary.entry_total(),
        "M7-W5's audit is the whole fixture, and a counterpart absent from it is not present",
    ),
    (
        |summary| summary.record.dependencies.unresolved == 0,
        "M7-W5's gate row asks for zero unresolved dependencies",
    ),
    (
        |summary| summary.record.registries.unresolved == 0,
        "M7-W5's gate row asks for zero unresolved registries",
    ),
    (
        |summary| {
            summary.record.provenance.present == summary.entry_total()
                && summary.record.provenance.missing == 0
        },
        "M7-W5's gate row asks that all provenance be present",
    ),
    (
        |summary| {
            count_sum(summary.record.waves.0.iter().map(|wave| wave.entries))
                == Some(summary.entry_total())
        },
        "the waves the work was ordered by carry the whole library between them",
    ),
    (
        |summary| {
            let waves = &summary.record.waves.0;
            waves.iter().enumerate().all(|(index, wave)| {
                waves
                    .iter()
                    .skip(index + 1)
                    .all(|other| wave.index != other.index)
            })
        },
        "two waves sharing an index are one wave counted twice, and order is what a wave is for",
    ),
    (
        |summary| {
            summary.record.waves.0.iter().all(|wave| {
                wave.depends_on.iter().all(|dependency| {
                    summary
                        .record
                        .waves
                        .0
                        .iter()
                        .any(|other| other.index == *dependency)
                })
            })
        },
        "M7-W1's graph closure proves no wave depends on an unavailable wave",
    ),
    (
        |summary| {
            summary.record.waves.0.iter().all(|wave| {
                wave.depends_on
                    .iter()
                    .all(|dependency| *dependency < wave.index)
            })
        },
        "M7-W1's graph closure proves no wave depends on a later wave",
    ),
    (
        |summary| {
            summary
                .record
                .routing
                .absorbed
                .checked_add(summary.record.routing.remaining)
                == Some(summary.entry_total())
        },
        "M7-W1's routing counts, per entry, how much Ravel already ships and how much remains",
    ),
    (
        |summary| {
            summary
                .record
                .routing
                .runnable_today
                .checked_add(summary.record.routing.blocked)
                == Some(summary.entry_total())
        },
        "an entry is runnable today or it is blocked, and the two account for the library",
    ),
    (
        |summary| summary.record.routing.blocked == 0,
        "a library whose entries cannot be run today is not the complete library M7 delivers",
    ),
    (
        |summary| {
            summary.record.suites.packages.total > 0 && summary.record.suites.packages.failed == 0
        },
        "M7-W6's gate row asks that all package suites pass, and a library running none passed none",
    ),
    (
        |summary| {
            summary.record.suites.clusters.total > 0 && summary.record.suites.clusters.failed == 0
        },
        "M7-W6's gate row asks that all cluster suites pass, and a library running none passed none",
    ),
];

const M7_CARRIED_FIGURES: &[(&str, LibraryFigure)] = &[
    ("/environment/settings/library/entries", |summary| {
        let entries = &summary.record.entries;
        count_sum([entries.accepted, entries.blocked, entries.unstarted]).map(Value::from)
    }),
    ("/environment/settings/library/accepted", |summary| {
        Some(Value::from(summary.record.entries.accepted))
    }),
    ("/environment/settings/library/blocked", |summary| {
        Some(Value::from(summary.record.entries.blocked))
    }),
    ("/environment/settings/library/unstarted", |summary| {
        Some(Value::from(summary.record.entries.unstarted))
    }),
    ("/environment/settings/library/waves", |summary| {
        Some(Value::from(summary.record.waves.0.len()))
    }),
    (
        "/environment/settings/library/dependenciesUnresolved",
        |summary| Some(Value::from(summary.record.dependencies.unresolved)),
    ),
    (
        "/environment/settings/library/registriesUnresolved",
        |summary| Some(Value::from(summary.record.registries.unresolved)),
    ),
    (
        "/environment/settings/library/provenancePresent",
        |summary| Some(Value::from(summary.record.provenance.present)),
    ),
    ("/environment/settings/library/absorbed", |summary| {
        Some(Value::from(summary.record.routing.absorbed))
    }),
    ("/environment/settings/library/packageSuites", |summary| {
        Some(Value::from(summary.record.suites.packages.total))
    }),
    ("/environment/settings/library/clusterSuites", |summary| {
        Some(Value::from(summary.record.suites.clusters.total))
    }),
];

const M7_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M7-W0", "M7-W1", "M7-W2", "M7-W3", "M7-W4", "M7-W5", "M7-W6",
                ],
            )
        },
        "M7 manifest must include every work package from M7-W0 through M7-W6 exactly once",
    ),
    (
        |value| {
            matches!(
                value["benchmarks"]["protocolVersion"].as_str(),
                Some("m7-library-v1" | "m7-library-v2")
            ) && array(value, "/benchmarks/results").len() == 1
                && value["benchmarks"]["results"][0]["name"] == "library-summary.json"
        },
        "M7 manifest must record its library summary under the m7-library-v1 or m7-library-v2 \
         protocol, so that the record carrying every figure is covered by a recomputed digest",
    ),
    (
        |value| all_gates_closed(value, "M7-W6", 6),
        "M7 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
    ),
    (
        |value| has_native_attestation(value, "official-atm11"),
        "M7 closure requires a native Linux attestation for the recorded library run",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M7 closes the selected workload library, and an empty finding set satisfies every rule \
         phrased as a refusal",
    ),
];

pub(crate) fn verify_m7_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    verify_manifest_closure(manifest, M7_MANIFEST_RULES, violations);

    verify_closure_records(Milestone::M7, records, M7_RECORDS, violations);
    if records
        .get("library-summary.json")
        .is_some_and(|record| record["protocol"] != manifest["benchmarks"]["protocolVersion"])
    {
        violations.push("M7 manifest and library summary must use the same protocol".to_owned());
    }
}

fn verify_m7_verified_fixture_total(root: &Path, record: &Value, violations: &mut Vec<String>) {
    match LibrarySummary::read(record).and_then(|summary| {
        summary.verify()?;
        Ok(summary)
    }) {
        Ok(summary) => match &summary.protocol {
            Protocol::Historical { correction } => {
                verify_m7_historical_total(
                    root,
                    summary.entry_total(),
                    correction.as_ref(),
                    violations,
                );
            }
            Protocol::Workload { fixture } => {
                verify_m7_workload_total(root, record, fixture, summary.entry_total(), violations);
            }
        },
        Err(error) => violations.push(format!("evidence/m7/library-summary.json: {error}")),
    }
}

fn verify_m7_historical_total(
    root: &Path,
    total: u64,
    correction: Option<&model::Correction<'_>>,
    violations: &mut Vec<String>,
) {
    let expected = match legacy_fixture_total(root) {
        Ok(expected) => expected,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let audited = total;
    if audited != expected && correction.is_none_or(|correction| correction.total != audited) {
        let recorded = total;
        violations.push(format!(
            "evidence/m7/library-summary.json audits {recorded} entries while \
             tests/fixtures/descriptors/atm10-tts-2.0.2.json pins {expected}; \
             M7-W5 must audit the pinned historical total or an explicit correction"
        ));
    }
    let inventory = root.join("evidence/m5/inventory-summary.json");
    // Historical M5 evidence is optional for isolated M7 closure records.
    if !inventory.is_file() {
        return;
    }
    let inventory = match read_json(root, &inventory) {
        Ok(inventory) => inventory,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(verified) = inventory
        .pointer("/artifacts/total")
        .and_then(Value::as_u64)
    else {
        violations.push(
            "evidence/m7/library-summary.json `.entries.total` and \
             evidence/m5/inventory-summary.json `.artifacts.total` cannot both be read, so \
             nothing holds M7's denominator to the inventory that measured it"
                .to_owned(),
        );
        return;
    };
    if audited == verified {
        return;
    }
    let corrects_the_inventory = correction
        .is_some_and(|correction| correction.from == verified && correction.total == audited);
    if !corrects_the_inventory {
        violations.push(format!(
            "evidence/m7/library-summary.json audits {audited} entries while \
             evidence/m5/inventory-summary.json verified {verified}; M7-W5 audits the verified \
             fixture total or an evidence correction that names the figure it replaced"
        ));
    }
}

fn verify_m7_workload_total(
    root: &Path,
    record: &Value,
    fixture: &model::Fixture<'_>,
    total: u64,
    violations: &mut Vec<String>,
) {
    let id = fixture.id.0;
    if !(!id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
        && !id.starts_with('.'))
    {
        violations.push("M7 fixture must name a safe workload inventory id".to_owned());
        return;
    }
    let path = root.join(format!("registry/inventory/{id}/graph.json"));
    let inventory = match read_json(root, &path) {
        Ok(inventory) => inventory,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let binding = &fixture.artifact_set;
    let count = inventory
        .pointer("/fixture/artifactSet/count")
        .and_then(Value::as_u64);
    let digest = inventory
        .pointer("/fixture/artifactSet/sha256")
        .and_then(Value::as_str);
    let digest = digest.and_then(|digest| ravel_core::Sha256Digest::try_from(digest).ok());
    for (valid, requirement) in [
        (inventory["fixture"]["id"] == fixture.id.0, "fixture.id"),
        (
            count.is_some_and(|count| count > 0),
            "fixture.artifactSet.count must be positive",
        ),
        (
            digest.is_some(),
            "fixture.artifactSet.sha256 must be a SHA-256 digest",
        ),
        (
            count == Some(binding.count)
                && digest == Some(binding.sha256)
                && record["fixture"]["artifactSet"] == inventory["fixture"]["artifactSet"],
            "fixture.artifactSet",
        ),
        (Some(total) == count, "entries.total"),
    ] {
        if !valid {
            violations.push(format!(
                "M7 fixture binding and entries.total must match {id}'s verified artifact-set count and digest: failed {requirement}"
            ));
        }
    }
}

const M7_FIGURES: CarriedFigures = CarriedFigures {
    record: "library-summary.json",
    subject: "library",
    verify: |milestone, record, manifest, carried, violations| match LibrarySummary::read(record) {
        Ok(record) => crate::evidence::closure::verify_typed_figures(
            milestone,
            &record,
            manifest,
            carried,
            M7_CARRIED_FIGURES,
            M7_ARITHMETIC,
            violations,
        ),
        Err(error) => violations.push(format!(
            "{}/{}: {error}",
            milestone.directory(),
            carried.record
        )),
    },
};

pub(crate) fn verify_m7_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M7, M7_RECORDS, violations);
    if let Some(record) = records.carried(&M7_FIGURES) {
        verify_m7_verified_fixture_total(root, record, violations);
        if verify_library_record(record).is_ok() {
            verify_milestone_figures(Milestone::M7, record, manifest, &M7_FIGURES, violations);
        }
    }
    records
}

#[cfg(test)]
mod tests;
