//! M5 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Figure, Rule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation,
};
use crate::json::{array, number, same, sum};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::model::admit;
use crate::evidence::rules::require;

mod model;

fn verify_inventory_record(value: &Value) -> Result<(), String> {
    let inventory: model::Inventory<'_> = admit(value)?;
    require(
        inventory.work_package.0 == "M5-W8",
        "/workPackage must be M5-W8",
    )?;
    require(
        inventory.protocol.0 == "m5-inventory-v1",
        "/protocol must be m5-inventory-v1",
    )?;
    require(
        matches!(inventory.result.0, "pass" | "provisional"),
        "/result must be pass, provisional",
    )?;
    let artifacts = &inventory.bound_artifacts.0;
    require(
        artifacts.iter().enumerate().all(|(index, artifact)| {
            artifacts[..index]
                .iter()
                .all(|other| other.name.0 != artifact.name.0)
        }),
        "/boundArtifacts unique name",
    )?;
    for (index, amendment) in inventory.machinery_amendments.iter().enumerate() {
        for (entry_index, entry) in amendment.entries.0.iter().enumerate() {
            if !artifacts.iter().any(|artifact| {
                matches!(artifact.class, model::ArtifactClass::Machinery)
                    && artifact.name.0 == entry.0
            }) {
                return Err(format!(
                    "/machineryAmendments/{index}/entries/{entry_index} must name bound machinery"
                ));
            }
        }
    }
    Ok(())
}

const M5_RECORDS: &[RecordDefinition] = &[RecordDefinition {
    name: "inventory-summary.json",
    rule: verify_inventory_record,
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
        |value| {
            same(
                sum(value, &["/artifacts/catalogued", "/artifacts/uncatalogued"]),
                number(value, "/artifacts/total"),
            )
        },
        "every artifact is either catalogued or not",
    ),
    (
        |value| {
            (same(
                number(value, "/artifacts/planned"),
                number(value, "/artifacts/total"),
            )) && (number(value, "/artifacts/uncatalogued") == Some(0.0))
        },
        "M5-W8's gate row asks that every artifact carry a complete plan",
    ),
    (
        |value| {
            same(
                number(value, "/plans/planned"),
                number(value, "/artifacts/total"),
            )
        },
        "M5-W8's gate row asks for a plan per catalogued artifact",
    ),
    (
        |value| {
            same(
                sum(value, &["/plans/readingSource", "/plans/notReadingSource"]),
                number(value, "/artifacts/total"),
            )
        },
        "a plan either reads source or does not",
    ),
    (
        |value| {
            same(
                number(value, "/plans/canonicalIds"),
                number(value, "/artifacts/total"),
            )
        },
        "M5-W6 assigns one canonical ID per entry",
    ),
    (
        |value| {
            same(
                sum(
                    value,
                    &[
                        "/rights/sourceUse/permitted",
                        "/rights/sourceUse/conditional",
                        "/rights/sourceUse/prohibited",
                        "/rights/sourceUse/unresolved",
                    ],
                ),
                number(value, "/artifacts/total"),
            )
        },
        "every triaged artifact carries exactly one source-use verdict",
    ),
    (
        |value| {
            same(
                sum(value, &["/surfaces/catalogued", "/surfaces/unclassified"]),
                number(value, "/artifacts/total"),
            )
        },
        "every acquired artifact is catalogued or is counted as unclassified",
    ),
    (
        |value| {
            same(
                sum(value, &["/graph/resolvedNodes", "/graph/blockedNodes"]),
                number(value, "/graph/nodes"),
            )
        },
        "every node resolves or carries a blocking classification",
    ),
    (
        |value| {
            same(
                sum(value, &["/graph/resolvedEdges", "/graph/blockedEdges"]),
                number(value, "/graph/edges"),
            )
        },
        "every edge resolves or carries a blocking classification",
    ),
    (
        |value| {
            (number(value, "/graph/clusters").is_some_and(|n| n > 0.0))
                && (number(value, "/graph/largestCluster").is_some_and(|n| n > 0.0))
                && (number(value, "/graph/largestCluster")
                    .zip(number(value, "/graph/nodes"))
                    .is_some_and(|(a, b)| a <= b))
        },
        "a cluster is a set of nodes, so the largest cannot exceed the graph",
    ),
    (
        |value| {
            (number(value, "/reads/producersWatched").is_some_and(|n| n > 0.0))
                && (number(value, "/reads/archiveReads").is_some_and(|n| n > 0.0))
        },
        "an observation that classified nothing observed nothing",
    ),
    (
        |value| number(value, "/reads/entryBodyReads") == Some(0.0),
        "reading an entry body is what the metadata-only method excludes",
    ),
];

const M5_CARRIED_FIGURES: &[(&str, Figure)] = &[
    ("/environment/settings/inventory/artifacts", |value| {
        sum(value, &["/artifacts/catalogued", "/artifacts/uncatalogued"])
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/planned", |value| {
        value.pointer("/artifacts/planned").cloned()
    }),
    ("/environment/settings/inventory/graphNodes", |value| {
        sum(value, &["/graph/resolvedNodes", "/graph/blockedNodes"])
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/graphEdges", |value| {
        sum(value, &["/graph/resolvedEdges", "/graph/blockedEdges"])
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/clusters", |value| {
        value.pointer("/graph/clusters").cloned()
    }),
    (
        "/environment/settings/inventory/sourceProhibited",
        |value| value.pointer("/rights/sourceUse/prohibited").cloned(),
    ),
    (
        "/environment/settings/inventory/sourceUnresolved",
        |value| value.pointer("/rights/sourceUse/unresolved").cloned(),
    ),
    ("/environment/settings/inventory/publishable", |value| {
        value.pointer("/rights/publishable").cloned()
    }),
    ("/environment/settings/inventory/readingSource", |value| {
        value.pointer("/plans/readingSource").cloned()
    }),
    ("/environment/settings/inventory/dependencyEdges", |value| {
        value.pointer("/plans/dependencyEdges").cloned()
    }),
    ("/environment/settings/inventory/archiveReads", |value| {
        value.pointer("/reads/archiveReads").cloned()
    }),
    ("/environment/settings/inventory/entryBodyReads", |value| {
        value.pointer("/reads/entryBodyReads").cloned()
    }),
    ("/environment/settings/inventory/digestsBound", |value| {
        value
            .pointer("/boundArtifacts")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
];

pub(crate) const M5_FIGURES: CarriedFigures = CarriedFigures {
    record: "inventory-summary.json",
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
        verify_milestone_figures(Milestone::M5, record, manifest, &M5_FIGURES, violations);
    }
    records
}

#[cfg(test)]
mod tests;
