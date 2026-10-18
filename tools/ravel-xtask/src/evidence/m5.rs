//! M5 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Figure, Rule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation,
};
use crate::json::{array, nonempty, number, sum};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::rules::{
    require, require_fields, require_nonempty_array, require_rows, unique_by,
};

fn inventory_record(v: &Value) -> Result<(), String> {
    require(v["workPackage"] == "M5-W8", "/workPackage must be M5-W8")?;
    require(
        v["protocol"] == "m5-inventory-v1",
        "/protocol must be m5-inventory-v1",
    )?;
    require(
        matches!(v["result"].as_str(), Some("pass" | "provisional")),
        "/result must be pass, provisional",
    )?;
    inventory_disclosure(v)?;
    inventory_numbers(v)?;
    inventory_artifacts(v)?;
    inventory_amendments(v)
}

fn inventory_disclosure(v: &Value) -> Result<(), String> {
    require(
        nonempty(v, "/disclosure/readObservation/bound"),
        "/disclosure/readObservation/bound",
    )?;
    require_rows(v, "/disclosure/judgedClaims", |row| {
        require(nonempty(row, "/claim"), "/claim")?;
        require(row["basis"] == "judged", "/basis must be judged")?;
        require(nonempty(row, "/decidedBy"), "/decidedBy")?;

        Ok(())
    })?;
    Ok(())
}

fn inventory_artifacts(v: &Value) -> Result<(), String> {
    let artifacts = array(v, "/boundArtifacts");
    require_rows(v, "/boundArtifacts", |row| {
        require(nonempty(row, "/name"), "/name")?;
        require(nonempty(row, "/holds"), "/holds")?;
        require(
            row["sha256"].as_str().is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }),
            "/sha256 must be a lowercase SHA-256 digest",
        )?;
        require(
            matches!(row["class"].as_str(), Some("material" | "machinery")),
            "/class must be material or machinery",
        )?;

        Ok(())
    })?;
    require(
        unique_by(artifacts, |left, right| left["name"] == right["name"]),
        "/boundArtifacts unique name",
    )
}

fn inventory_amendments(v: &Value) -> Result<(), String> {
    require(
        v["primitiveScan"]["class"] == "machinery",
        "/primitiveScan/class must be machinery",
    )?;
    require(
        v["machineryAmendments"].is_array(),
        "/machineryAmendments must be an array",
    )?;
    for (index, row) in array(v, "/machineryAmendments").iter().enumerate() {
        inventory_amendment(v, row)
            .map_err(|requirement| format!("/machineryAmendments/{index}{requirement}"))?;
    }
    Ok(())
}

fn inventory_amendment(v: &Value, row: &Value) -> Result<(), String> {
    require(nonempty(row, "/milestone"), "/milestone")?;
    require(nonempty(row, "/reason"), "/reason")?;
    require(row["figures"].is_array(), "/figures must be an array")?;
    for (index, entry) in require_nonempty_array(row, "/entries")?.iter().enumerate() {
        if !array(v, "/boundArtifacts")
            .iter()
            .any(|artifact| artifact["class"] == "machinery" && artifact.get("name") == Some(entry))
        {
            return Err(format!("/entries/{index} must name bound machinery"));
        }
    }
    Ok(())
}

fn inventory_numbers(v: &Value) -> Result<(), String> {
    require_fields(
        v,
        &[
            "/artifacts/total",
            "/artifacts/catalogued",
            "/artifacts/uncatalogued",
            "/artifacts/planned",
            "/graph/nodes",
            "/graph/resolvedNodes",
            "/graph/blockedNodes",
            "/graph/edges",
            "/graph/resolvedEdges",
            "/graph/blockedEdges",
            "/graph/clusters",
            "/graph/largestCluster",
            "/rights/sourceUse/permitted",
            "/rights/sourceUse/conditional",
            "/rights/sourceUse/prohibited",
            "/rights/sourceUse/unresolved",
            "/surfaces/classFiles",
            "/surfaces/models",
            "/surfaces/textures",
            "/surfaces/recipes",
            "/surfaces/catalogued",
            "/surfaces/unclassified",
            "/surfaces/nestedJars",
            "/plans/planned",
            "/plans/readingSource",
            "/plans/notReadingSource",
            "/plans/dependencyEdges",
            "/plans/canonicalIds",
            "/reads/producersWatched",
            "/reads/archiveReads",
            "/reads/entryBodyReads",
            "/rights/publishable",
        ],
        Value::is_number,
    )
}

const M5_RECORDS: &[RecordDefinition] = &[RecordDefinition {
    name: "inventory-summary.json",
    rule: inventory_record,
    description: "an inventory summary carrying every aggregate as a number, a digest over every \
 registry file it binds with the class that digest freezes, its trail of machinery \
 amendments, and its own disclosure of what the read observation watches and \
 which of its claims rest on judgement",
    required: false,
}];

const M5_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M5-W1", "M5-W2", "M5-W3", "M5-W4", "M5-W5", "M5-W6", "M5-W7", "M5-W8",
                ],
            )
        },
        "M5 manifest must include every work package from M5-W1 through M5-W8 exactly once",
    ),
    (
        |value| has_exact_benchmark_results(value, "m5-inventory-v1", &["inventory-summary.json"]),
        "M5 manifest must record its inventory summary under the m5-inventory-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
    ),
    (
        |value| all_gates_closed(value, "M5-W8", 7),
        "M5 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
    ),
    (
        |value| has_native_attestation(value, "official-atm11"),
        "M5 closure requires a native Linux attestation for the recorded inventory run",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M5 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
    ),
];

pub(crate) fn verify_m5_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    verify_manifest_closure(manifest, M5_MANIFEST_RULES, violations);

    verify_closure_records(Milestone::M5, records, M5_RECORDS, violations);

    if records
        .get("inventory-summary.json")
        .is_some_and(|value| value["result"] == "provisional")
    {
        violations.push(
            "M5 closure requires final figures: evidence/m5/inventory-summary.json records itself \
             as provisional, which is a true statement about an inventory under repair and not \
             something a milestone can close over"
                .to_owned(),
        );
    }
}

pub(crate) const M5_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |v| {
            sum(v, &["/artifacts/catalogued", "/artifacts/uncatalogued"])
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every artifact is either catalogued or not",
    ),
    (
        |v| {
            (number(v, "/artifacts/planned")
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(a, b)| a.total_cmp(&b).is_eq()))
                && (number(v, "/artifacts/uncatalogued") == Some(0.0))
        },
        "M5-W8's gate row asks that every artifact carry a complete plan",
    ),
    (
        |v| {
            number(v, "/plans/planned")
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(a, b)| a.total_cmp(&b).is_eq())
        },
        "M5-W8's gate row asks for a plan per catalogued artifact",
    ),
    (
        |v| {
            sum(v, &["/plans/readingSource", "/plans/notReadingSource"])
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "a plan either reads source or does not",
    ),
    (
        |v| {
            number(v, "/plans/canonicalIds")
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(a, b)| a.total_cmp(&b).is_eq())
        },
        "M5-W6 assigns one canonical ID per entry",
    ),
    (
        |v| {
            sum(
                v,
                &[
                    "/rights/sourceUse/permitted",
                    "/rights/sourceUse/conditional",
                    "/rights/sourceUse/prohibited",
                    "/rights/sourceUse/unresolved",
                ],
            )
            .zip(number(v, "/artifacts/total"))
            .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every triaged artifact carries exactly one source-use verdict",
    ),
    (
        |v| {
            sum(v, &["/surfaces/catalogued", "/surfaces/unclassified"])
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every acquired artifact is catalogued or is counted as unclassified",
    ),
    (
        |v| {
            sum(v, &["/graph/resolvedNodes", "/graph/blockedNodes"])
                .zip(number(v, "/graph/nodes"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every node resolves or carries a blocking classification",
    ),
    (
        |v| {
            sum(v, &["/graph/resolvedEdges", "/graph/blockedEdges"])
                .zip(number(v, "/graph/edges"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every edge resolves or carries a blocking classification",
    ),
    (
        |v| {
            (number(v, "/graph/clusters").is_some_and(|n| n > 0.0))
                && (number(v, "/graph/largestCluster").is_some_and(|n| n > 0.0))
                && (number(v, "/graph/largestCluster")
                    .zip(number(v, "/graph/nodes"))
                    .is_some_and(|(a, b)| a <= b))
        },
        "a cluster is a set of nodes, so the largest cannot exceed the graph",
    ),
    (
        |v| {
            (number(v, "/reads/producersWatched").is_some_and(|n| n > 0.0))
                && (number(v, "/reads/archiveReads").is_some_and(|n| n > 0.0))
        },
        "an observation that classified nothing observed nothing",
    ),
    (
        |v| number(v, "/reads/entryBodyReads") == Some(0.0),
        "reading an entry body is what the metadata-only method excludes",
    ),
];

const M5_CARRIED_FIGURES: &[(&str, Figure)] = &[
    ("/environment/settings/inventory/artifacts", |v| {
        sum(v, &["/artifacts/catalogued", "/artifacts/uncatalogued"])
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/planned", |v| {
        v.pointer("/artifacts/planned").cloned()
    }),
    ("/environment/settings/inventory/graphNodes", |v| {
        sum(v, &["/graph/resolvedNodes", "/graph/blockedNodes"])
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/graphEdges", |v| {
        sum(v, &["/graph/resolvedEdges", "/graph/blockedEdges"])
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/clusters", |v| {
        v.pointer("/graph/clusters").cloned()
    }),
    ("/environment/settings/inventory/sourceProhibited", |v| {
        v.pointer("/rights/sourceUse/prohibited").cloned()
    }),
    ("/environment/settings/inventory/sourceUnresolved", |v| {
        v.pointer("/rights/sourceUse/unresolved").cloned()
    }),
    ("/environment/settings/inventory/publishable", |v| {
        v.pointer("/rights/publishable").cloned()
    }),
    ("/environment/settings/inventory/readingSource", |v| {
        v.pointer("/plans/readingSource").cloned()
    }),
    ("/environment/settings/inventory/dependencyEdges", |v| {
        v.pointer("/plans/dependencyEdges").cloned()
    }),
    ("/environment/settings/inventory/archiveReads", |v| {
        v.pointer("/reads/archiveReads").cloned()
    }),
    ("/environment/settings/inventory/entryBodyReads", |v| {
        v.pointer("/reads/entryBodyReads").cloned()
    }),
    ("/environment/settings/inventory/digestsBound", |v| {
        v.pointer("/boundArtifacts")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
];

pub(crate) const M5_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m5/manifest.json",
    record: "evidence/m5/inventory-summary.json",
    subject: "inventory",
    figures: M5_CARRIED_FIGURES,
    arithmetic: M5_ARITHMETIC,
};

pub(crate) fn verify_m5_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M5, M5_RECORDS, violations);
    if let Some(record) = records.carried(&M5_FIGURES) {
        verify_milestone_figures(record, manifest, &M5_FIGURES, violations);
    }
    records
}

#[cfg(test)]
mod tests;
