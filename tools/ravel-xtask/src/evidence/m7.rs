//! M7 closure rules.

use std::path::Path;

use serde_json::Value;

use crate::json::{
    Figure, Rule, array, closed_gates, native_attestation, nonempty, number, read_json,
    work_packages,
};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_milestone_figures, verify_record_arithmetic,
};
use crate::evidence::manifest::require_manifest_rule;

fn library_record(v: &Value) -> bool {
    v["milestone"] == "M7"
        && v["workPackage"] == "M7-W5"
        && v["protocol"] == "m7-library-v1"
        && v["result"] == "pass"
        && v["entries"].is_object()
        && ["total", "accepted", "blocked", "unstarted"]
            .iter()
            .all(|key| v["entries"][*key].is_number())
        && (v["correction"].is_null()
            || (v["correction"].is_object()
                && v["correction"]["from"].is_number()
                && v["correction"]["total"].is_number()
                && nonempty(v, "/correction/reason")))
        && v["waves"].is_array()
        && !array(v, "/waves").is_empty()
        && array(v, "/waves").iter().all(|wave| {
            wave["index"].is_number()
                && wave["entries"].is_number()
                && wave["dependsOn"].is_array()
                && array(wave, "/dependsOn").iter().all(Value::is_number)
        })
        && [
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
        ]
        .iter()
        .all(|path| v.pointer(path).is_some_and(Value::is_number))
}

const M7_RECORDS: &[(&str, Rule, &str)] = &[(
    "library-summary.json",
    library_record,
    "a library summary carrying the burn-up ledger as numbers, the waves with the waves each \
     depends on, any evidence correction with the figure it replaced and its reason, and the \
     dependency, registry, provenance, routing and suite figures M7-W5 audits",
)];

const M7_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |v| {
            [
                "/entries/accepted",
                "/entries/blocked",
                "/entries/unstarted",
            ]
            .iter()
            .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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
            number(v, "/entries/total") == Some(324.0)
                || (v["correction"].is_object()
                    && number(v, "/correction/total")
                        .zip(number(v, "/entries/total"))
                        .is_some_and(|(a, b)| a.total_cmp(&b).is_eq()))
        },
        "M7-W5 audits 324 of 324 or an evidence-corrected total, and a different figure with no \
         correction beside it is neither",
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
            array(v, "/waves")
                .iter()
                .try_fold(0.0, |sum, row| Some(sum + number(row, "/entries")?))
                .zip(number(v, "/entries/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the waves the work was ordered by carry the whole library between them",
    ),
    (
        |v| {
            array(v, "/waves").iter().enumerate().all(|(index, wave)| {
                array(v, "/waves")[..index]
                    .iter()
                    .all(|other| other["index"].as_f64() != wave["index"].as_f64())
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
            ["/routing/absorbed", "/routing/remaining"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
                .zip(number(v, "/entries/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "M7-W1's routing counts, per entry, how much Ravel already ships and how much remains",
    ),
    (
        |v| {
            ["/routing/runnableToday", "/routing/blocked"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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
        [
            "/entries/accepted",
            "/entries/blocked",
            "/entries/unstarted",
        ]
        .iter()
        .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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

pub(crate) fn verify_m7_manifest(root: &Path, manifest_path: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        |v| {
            work_packages(
                v,
                &[
                    "M7-W0", "M7-W1", "M7-W2", "M7-W3", "M7-W4", "M7-W5", "M7-W6",
                ],
            )
        },
        "M7 manifest must include every work package from M7-W0 through M7-W6 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| {
            v["benchmarks"]["protocolVersion"] == "m7-library-v1"
                && array(v, "/benchmarks/results").len() == 1
                && v["benchmarks"]["results"][0]["name"] == "library-summary.json"
        },
        "M7 manifest must record its library summary under the m7-library-v1 protocol, so that \
         the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| closed_gates(v, "M7-W6", 6),
        "M7 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        native_attestation,
        "M7 closure requires a native Linux attestation for the recorded library run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| !array(v, "/deviations").is_empty() && !array(v, "/risks").is_empty(),
        "M7 closes a library of 324 counterparts, and an empty finding set satisfies every rule \
         phrased as a refusal",
        violations,
    );

    verify_closure_records(root, "M7", "evidence/m7", M7_RECORDS, violations);
}

fn verify_m7_verified_fixture_total(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m7/library-summary.json");
    let inventory = root.join("evidence/m5/inventory-summary.json");
    if !record.is_file() || !inventory.is_file() {
        return;
    }
    let (record, inventory) = match (read_json(&record), read_json(&inventory)) {
        (Ok(record), Ok(inventory)) => (record, inventory),
        (record, inventory) => {
            violations.extend(record.err());
            violations.extend(inventory.err());
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

const M7_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m7/manifest.json",
    record: "evidence/m7/library-summary.json",
    subject: "library",
    figures: M7_CARRIED_FIGURES,
};

pub(crate) fn verify_m7_integrity(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    verify_record_arithmetic(
        root,
        "evidence/m7/library-summary.json",
        M7_ARITHMETIC,
        violations,
    );
    verify_m7_verified_fixture_total(root, violations);
    verify_milestone_figures(root, manifest, &M7_FIGURES, violations);
}

#[cfg(test)]
mod tests;
