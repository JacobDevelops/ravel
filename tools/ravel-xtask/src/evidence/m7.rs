//! M7 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Figure, Rule, all_gates_closed, has_exact_work_packages, has_native_attestation,
};
use crate::json::{array, column_sum, number, read_json, same, sum};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::rules::unique_by;

mod model;
use model::{LibrarySummary, Protocol};

fn legacy_fixture_total(root: &Path) -> Result<f64, String> {
    let path = "tests/fixtures/descriptors/atm10-tts-2.0.2.json";
    let descriptor = read_json(root, &root.join(path))?;
    number(&descriptor, "/manifest/expectedReferenceCount")
        .ok_or_else(|| format!("{path}: missing numeric /manifest/expectedReferenceCount"))
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

const M7_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |value| {
            same(
                sum(
                    value,
                    &[
                        "/entries/accepted",
                        "/entries/blocked",
                        "/entries/unstarted",
                    ],
                ),
                number(value, "/entries/total"),
            )
        },
        "M7-W4's burn-up ledger asks that the accepted, blocked and unstarted totals equal the \
         verified fixture total",
    ),
    (
        |value| {
            (number(value, "/entries/blocked") == Some(0.0))
                && (number(value, "/entries/unstarted") == Some(0.0))
        },
        "a wave cannot close with failing or unclassified entries, so the library cannot close \
         with any",
    ),
    (
        |value| {
            same(
                number(value, "/entries/accepted"),
                number(value, "/entries/total"),
            )
        },
        "M7-W5's audit is the whole fixture, and a counterpart absent from it is not present",
    ),
    (
        |value| number(value, "/dependencies/unresolved") == Some(0.0),
        "M7-W5's gate row asks for zero unresolved dependencies",
    ),
    (
        |value| number(value, "/registries/unresolved") == Some(0.0),
        "M7-W5's gate row asks for zero unresolved registries",
    ),
    (
        |value| {
            (same(
                number(value, "/provenance/present"),
                number(value, "/entries/total"),
            )) && (number(value, "/provenance/missing") == Some(0.0))
        },
        "M7-W5's gate row asks that all provenance be present",
    ),
    (
        |value| {
            same(
                column_sum(value, "/waves", "/entries"),
                number(value, "/entries/total"),
            )
        },
        "the waves the work was ordered by carry the whole library between them",
    ),
    (
        |value| {
            unique_by(array(value, "/waves"), |left, right| {
                left["index"].as_f64() == right["index"].as_f64()
            })
        },
        "two waves sharing an index are one wave counted twice, and order is what a wave is for",
    ),
    (
        |value| {
            array(value, "/waves").iter().all(|wave| {
                array(wave, "/dependsOn").iter().all(|dependency| {
                    array(value, "/waves").iter().any(|other| {
                        other["index"]
                            .as_f64()
                            .is_some_and(|index| Some(index) == dependency.as_f64())
                    })
                })
            })
        },
        "M7-W1's graph closure proves no wave depends on an unavailable wave",
    ),
    (
        |value| {
            array(value, "/waves").iter().all(|wave| {
                array(wave, "/dependsOn").iter().all(|dependency| {
                    dependency
                        .as_f64()
                        .zip(number(wave, "/index"))
                        .is_some_and(|(dependency, index)| dependency < index)
                })
            })
        },
        "M7-W1's graph closure proves no wave depends on a later wave",
    ),
    (
        |value| {
            same(
                sum(value, &["/routing/absorbed", "/routing/remaining"]),
                number(value, "/entries/total"),
            )
        },
        "M7-W1's routing counts, per entry, how much Ravel already ships and how much remains",
    ),
    (
        |value| {
            same(
                sum(value, &["/routing/runnableToday", "/routing/blocked"]),
                number(value, "/entries/total"),
            )
        },
        "an entry is runnable today or it is blocked, and the two account for the library",
    ),
    (
        |value| number(value, "/routing/blocked") == Some(0.0),
        "a library whose entries cannot be run today is not the complete library M7 delivers",
    ),
    (
        |value| {
            (number(value, "/suites/packages/total").is_some_and(|n| n > 0.0))
                && (number(value, "/suites/packages/failed") == Some(0.0))
        },
        "M7-W6's gate row asks that all package suites pass, and a library running none passed \
         none",
    ),
    (
        |value| {
            (number(value, "/suites/clusters/total").is_some_and(|n| n > 0.0))
                && (number(value, "/suites/clusters/failed") == Some(0.0))
        },
        "M7-W6's gate row asks that all cluster suites pass, and a library running none passed \
         none",
    ),
];

const M7_CARRIED_FIGURES: &[(&str, Figure)] = &[
    ("/environment/settings/library/entries", |value| {
        sum(
            value,
            &[
                "/entries/accepted",
                "/entries/blocked",
                "/entries/unstarted",
            ],
        )
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
    }),
    ("/environment/settings/library/accepted", |value| {
        value.pointer("/entries/accepted").cloned()
    }),
    ("/environment/settings/library/blocked", |value| {
        value.pointer("/entries/blocked").cloned()
    }),
    ("/environment/settings/library/unstarted", |value| {
        value.pointer("/entries/unstarted").cloned()
    }),
    ("/environment/settings/library/waves", |value| {
        value
            .pointer("/waves")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
    (
        "/environment/settings/library/dependenciesUnresolved",
        |value| value.pointer("/dependencies/unresolved").cloned(),
    ),
    (
        "/environment/settings/library/registriesUnresolved",
        |value| value.pointer("/registries/unresolved").cloned(),
    ),
    ("/environment/settings/library/provenancePresent", |value| {
        value.pointer("/provenance/present").cloned()
    }),
    ("/environment/settings/library/absorbed", |value| {
        value.pointer("/routing/absorbed").cloned()
    }),
    ("/environment/settings/library/packageSuites", |value| {
        value.pointer("/suites/packages/total").cloned()
    }),
    ("/environment/settings/library/clusterSuites", |value| {
        value.pointer("/suites/clusters/total").cloned()
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
    total: &serde_json::Number,
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
    let audited = total.as_f64();
    if !same(audited, Some(expected))
        && !correction.is_some_and(|correction| same(correction.total.as_f64(), audited))
    {
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
    let (Some(audited), Some(verified)) = (
        audited,
        inventory
            .pointer("/artifacts/total")
            .and_then(Value::as_f64),
    ) else {
        violations.push(
            "evidence/m7/library-summary.json `.entries.total` and \
             evidence/m5/inventory-summary.json `.artifacts.total` cannot both be read, so \
             nothing holds M7's denominator to the inventory that measured it"
                .to_owned(),
        );
        return;
    };
    if same(Some(audited), Some(verified)) {
        return;
    }
    let corrects_the_inventory = correction.is_some_and(|correction| {
        same(correction.from.as_f64(), Some(verified))
            && same(correction.total.as_f64(), Some(audited))
    });
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
    total: &serde_json::Number,
    violations: &mut Vec<String>,
) {
    let id = fixture.id;
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
        (inventory["fixture"]["id"] == fixture.id, "fixture.id"),
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
        (total.as_u64() == count, "entries.total"),
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
    figures: M7_CARRIED_FIGURES,
    arithmetic: M7_ARITHMETIC,
};

pub(crate) fn verify_m7_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M7, M7_RECORDS, violations);
    if let Some(record) = records.carried(&M7_FIGURES) {
        verify_m7_verified_fixture_total(root, record, violations);
        verify_milestone_figures(Milestone::M7, record, manifest, &M7_FIGURES, violations);
    }
    records
}

#[cfg(test)]
mod tests;
