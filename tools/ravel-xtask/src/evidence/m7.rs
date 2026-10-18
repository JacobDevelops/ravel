//! M7 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Figure, Rule, all_gates_closed, has_exact_work_packages, has_native_attestation,
};
use crate::json::{array, column_sum, nonempty, number, read_json, sum};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::rules::{require, require_fields, require_rows, unique_by};

fn legacy_fixture_total() -> Result<f64, String> {
    static TOTAL: std::sync::LazyLock<Result<f64, String>> = std::sync::LazyLock::new(|| {
        parse_legacy_fixture_total(include_str!(
            "../../../../tests/fixtures/descriptors/atm10-tts-2.0.2.json"
        ))
    });
    TOTAL.clone()
}

fn parse_legacy_fixture_total(source: &str) -> Result<f64, String> {
    let descriptor: Value = serde_json::from_str(source)
        .map_err(|error| format!("cannot parse pinned historical fixture descriptor: {error}"))?;
    number(&descriptor, "/manifest/expectedReferenceCount").ok_or_else(|| {
        "pinned historical fixture descriptor is missing numeric /manifest/expectedReferenceCount"
            .to_owned()
    })
}

fn library_protocol(v: &Value) -> Result<(), String> {
    match v["protocol"].as_str() {
        Some("m7-library-v1") => require(v["fixture"].is_null(), "/fixture must be null for v1"),
        Some("m7-library-v2") => {
            require(
                v["fixture"].is_object(),
                "/fixture must be an object for v2",
            )?;
            require(nonempty(v, "/fixture/id"), "/fixture/id")?;
            require(
                v["fixture"]["artifactSet"].is_object(),
                "/fixture/artifactSet",
            )?;
            require(v["correction"].is_null(), "/correction must be null for v2")?;
            require_fields(
                v,
                &[
                    "/entries/total",
                    "/entries/accepted",
                    "/entries/blocked",
                    "/entries/unstarted",
                ],
                |value| value.as_u64().is_some(),
            )?;
            require(
                v["entries"]["total"]
                    .as_u64()
                    .is_some_and(|total| total > 0),
                "v2 entries.total must be positive",
            )
        }
        _ => Err("/protocol must be m7-library-v1 or m7-library-v2".to_owned()),
    }
}

fn library_record(v: &Value) -> Result<(), String> {
    require(v["workPackage"] == "M7-W5", "/workPackage must be M7-W5")?;
    library_protocol(v)?;
    require(v["result"] == "pass", "/result must be pass")?;
    require(v["entries"].is_object(), "/entries")?;
    require_fields(
        v,
        &[
            "/entries/total",
            "/entries/accepted",
            "/entries/blocked",
            "/entries/unstarted",
        ],
        Value::is_number,
    )?;
    library_waves_and_correction(v)?;
    library_counts(v)
}

fn library_waves_and_correction(v: &Value) -> Result<(), String> {
    if !v["correction"].is_null() {
        require(
            v["correction"].is_object(),
            "/correction must be an object or null",
        )?;
        require_fields(
            v,
            &["/correction/from", "/correction/total"],
            Value::is_number,
        )?;
        require(nonempty(v, "/correction/reason"), "/correction/reason")?;
    }
    require_rows(v, "/waves", |wave| {
        require_fields(wave, &["/index", "/entries"], Value::is_number)?;
        require(wave["dependsOn"].is_array(), "/dependsOn must be an array")?;
        require(
            array(wave, "/dependsOn").iter().all(Value::is_number),
            "/dependsOn must contain numbers",
        )?;

        Ok(())
    })?;
    Ok(())
}

fn library_counts(v: &Value) -> Result<(), String> {
    require_fields(
        v,
        &[
            "/dependencies/resolved",
            "/dependencies/unresolved",
            "/registries/resolved",
            "/registries/unresolved",
            "/provenance/present",
            "/provenance/missing",
            "/routing/absorbed",
            "/routing/remaining",
            "/routing/runnableToday",
            "/routing/blocked",
            "/suites/packages/total",
            "/suites/packages/failed",
            "/suites/clusters/total",
            "/suites/clusters/failed",
        ],
        Value::is_number,
    )?;
    Ok(())
}

const M7_RECORDS: &[RecordDefinition] = &[RecordDefinition {
    name: "library-summary.json",
    rule: library_record,
    description: "a library summary carrying the burn-up ledger as numbers, the waves with the waves each \
 depends on, any evidence correction with the figure it replaced and its reason, and the \
 dependency, registry, provenance, routing and suite figures M7-W5 audits",
    required: false,
}];

const M7_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |v| {
            sum(
                v,
                &[
                    "/entries/accepted",
                    "/entries/blocked",
                    "/entries/unstarted",
                ],
            )
            .zip(number(v, "/entries/total"))
            .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "M7-W4's burn-up ledger asks that the accepted, blocked and unstarted totals equal the \
         verified fixture total",
    ),
    (
        |v| {
            (number(v, "/entries/blocked") == Some(0.0))
                && (number(v, "/entries/unstarted") == Some(0.0))
        },
        "a wave cannot close with failing or unclassified entries, so the library cannot close \
         with any",
    ),
    (
        |v| {
            number(v, "/entries/accepted")
                .zip(number(v, "/entries/total"))
                .is_some_and(|(a, b)| a.total_cmp(&b).is_eq())
        },
        "M7-W5's audit is the whole fixture, and a counterpart absent from it is not present",
    ),
    (
        |v| {
            if v["protocol"] == "m7-library-v2" {
                true
            } else {
                legacy_fixture_total().map_or(true, |expected| {
                    number(v, "/entries/total")
                        .is_some_and(|total| total.total_cmp(&expected).is_eq())
                }) || (v["correction"].is_object()
                    && number(v, "/correction/total")
                        .zip(number(v, "/entries/total"))
                        .is_some_and(|(a, b)| a.total_cmp(&b).is_eq()))
            }
        },
        "M7-W5 audits the pinned historical reference total or an explicit correction under v1, \
         or a positive whole-number total bound to the verified workload inventory under v2",
    ),
    (
        |v| number(v, "/dependencies/unresolved") == Some(0.0),
        "M7-W5's gate row asks for zero unresolved dependencies",
    ),
    (
        |v| number(v, "/registries/unresolved") == Some(0.0),
        "M7-W5's gate row asks for zero unresolved registries",
    ),
    (
        |v| {
            (number(v, "/provenance/present")
                .zip(number(v, "/entries/total"))
                .is_some_and(|(a, b)| a.total_cmp(&b).is_eq()))
                && (number(v, "/provenance/missing") == Some(0.0))
        },
        "M7-W5's gate row asks that all provenance be present",
    ),
    (
        |v| {
            column_sum(v, "/waves", "/entries")
                .zip(number(v, "/entries/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the waves the work was ordered by carry the whole library between them",
    ),
    (
        |v| {
            unique_by(array(v, "/waves"), |left, right| {
                left["index"].as_f64() == right["index"].as_f64()
            })
        },
        "two waves sharing an index are one wave counted twice, and order is what a wave is for",
    ),
    (
        |v| {
            array(v, "/waves").iter().all(|wave| {
                array(wave, "/dependsOn").iter().all(|dependency| {
                    array(v, "/waves").iter().any(|other| {
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
        |v| {
            array(v, "/waves").iter().all(|wave| {
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
        |v| {
            sum(v, &["/routing/absorbed", "/routing/remaining"])
                .zip(number(v, "/entries/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "M7-W1's routing counts, per entry, how much Ravel already ships and how much remains",
    ),
    (
        |v| {
            sum(v, &["/routing/runnableToday", "/routing/blocked"])
                .zip(number(v, "/entries/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "an entry is runnable today or it is blocked, and the two account for the library",
    ),
    (
        |v| number(v, "/routing/blocked") == Some(0.0),
        "a library whose entries cannot be run today is not the complete library M7 delivers",
    ),
    (
        |v| {
            (number(v, "/suites/packages/total").is_some_and(|n| n > 0.0))
                && (number(v, "/suites/packages/failed") == Some(0.0))
        },
        "M7-W6's gate row asks that all package suites pass, and a library running none passed \
         none",
    ),
    (
        |v| {
            (number(v, "/suites/clusters/total").is_some_and(|n| n > 0.0))
                && (number(v, "/suites/clusters/failed") == Some(0.0))
        },
        "M7-W6's gate row asks that all cluster suites pass, and a library running none passed \
         none",
    ),
];

const M7_CARRIED_FIGURES: &[(&str, Figure)] = &[
    ("/environment/settings/library/entries", |v| {
        sum(
            v,
            &[
                "/entries/accepted",
                "/entries/blocked",
                "/entries/unstarted",
            ],
        )
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
    }),
    ("/environment/settings/library/accepted", |v| {
        v.pointer("/entries/accepted").cloned()
    }),
    ("/environment/settings/library/blocked", |v| {
        v.pointer("/entries/blocked").cloned()
    }),
    ("/environment/settings/library/unstarted", |v| {
        v.pointer("/entries/unstarted").cloned()
    }),
    ("/environment/settings/library/waves", |v| {
        v.pointer("/waves")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
    (
        "/environment/settings/library/dependenciesUnresolved",
        |v| v.pointer("/dependencies/unresolved").cloned(),
    ),
    ("/environment/settings/library/registriesUnresolved", |v| {
        v.pointer("/registries/unresolved").cloned()
    }),
    ("/environment/settings/library/provenancePresent", |v| {
        v.pointer("/provenance/present").cloned()
    }),
    ("/environment/settings/library/absorbed", |v| {
        v.pointer("/routing/absorbed").cloned()
    }),
    ("/environment/settings/library/packageSuites", |v| {
        v.pointer("/suites/packages/total").cloned()
    }),
    ("/environment/settings/library/clusterSuites", |v| {
        v.pointer("/suites/clusters/total").cloned()
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
    if record["protocol"] == "m7-library-v2" {
        if let Err(requirement) = library_protocol(record) {
            violations.push(format!(
                "evidence/m7/{}: failed {requirement}",
                M7_FIGURES.record
            ));
            return;
        }
        verify_m7_workload_total(root, record, violations);
        return;
    }
    if !record["fixture"].is_null() {
        violations.push("M7 v1 requires a null fixture".to_owned());
        return;
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
        record.pointer("/entries/total").and_then(Value::as_f64),
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
    if audited.total_cmp(&verified).is_eq() {
        return;
    }
    let corrects_the_inventory = record["correction"].is_object()
        && record["correction"]["from"].as_f64() == Some(verified)
        && record["correction"]["total"].as_f64() == Some(audited);
    if !corrects_the_inventory {
        violations.push(format!(
            "evidence/m7/library-summary.json audits {audited} entries while \
             evidence/m5/inventory-summary.json verified {verified}; M7-W5 audits the verified \
             fixture total or an evidence correction that names the figure it replaced"
        ));
    }
}

fn verify_m7_workload_total(root: &Path, record: &Value, violations: &mut Vec<String>) {
    let Some(id) = record["fixture"]["id"].as_str().filter(|id| {
        !id.is_empty()
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
            && !id.starts_with('.')
    }) else {
        violations.push("M7 fixture must name a safe workload inventory id".to_owned());
        return;
    };
    let path = root.join(format!("registry/inventory/{id}/graph.json"));
    let inventory = match read_json(root, &path) {
        Ok(inventory) => inventory,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let binding = &record["fixture"];
    let count = inventory
        .pointer("/fixture/artifactSet/count")
        .and_then(Value::as_u64);
    let digest = inventory
        .pointer("/fixture/artifactSet/sha256")
        .and_then(Value::as_str);
    let valid_digest = digest
        .is_some_and(|digest| ravel_package::Sha256Digest::try_from(digest.to_owned()).is_ok());
    for (valid, requirement) in [
        (inventory["fixture"]["id"] == binding["id"], "fixture.id"),
        (
            count.is_some_and(|count| count > 0),
            "fixture.artifactSet.count must be positive",
        ),
        (
            valid_digest,
            "fixture.artifactSet.sha256 must be a SHA-256 digest",
        ),
        (
            inventory["fixture"]["artifactSet"] == binding["artifactSet"],
            "fixture.artifactSet",
        ),
        (
            record.pointer("/entries/total").and_then(Value::as_u64) == count,
            "entries.total",
        ),
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
        if record["protocol"] != "m7-library-v2"
            && let Err(error) = legacy_fixture_total()
        {
            violations.push(error);
        }
        verify_m7_verified_fixture_total(root, record, violations);
        verify_milestone_figures(Milestone::M7, record, manifest, &M7_FIGURES, violations);
    }
    records
}

#[cfg(test)]
mod tests;
