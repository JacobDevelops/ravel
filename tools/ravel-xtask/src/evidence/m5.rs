//! M5 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Rule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation,
};
use crate::json::array;

use crate::evidence::closure::{CarriedFigures, verify_closure_records, verify_manifest_closure};
use crate::evidence::model::{Verdict, admit, count_sum};
use crate::evidence::rules::require;

mod model;

fn verify_inventory_record(value: &Value) -> Result<(), String> {
    verify_inventory_record_typed(&admit::<model::Inventory<'_>>(value)?)
}

fn verify_inventory_record_typed(inventory: &model::Inventory<'_>) -> Result<(), String> {
    crate::evidence::model::require_identity(inventory.work_package.0, "M5-W8", "/workPackage")?;
    crate::evidence::model::require_identity(inventory.protocol.0, "m5-inventory-v1", "/protocol")?;
    require(
        matches!(inventory.result, Verdict::Pass | Verdict::Provisional),
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

    let provisional = records.is_provisional("inventory-summary.json")
        || (!records.has_checked("inventory-summary.json")
            && records.get("inventory-summary.json").is_some_and(|value| {
                admit::<model::Inventory<'_>>(value)
                    .is_ok_and(|record| record.result == Verdict::Provisional)
            }));
    if provisional {
        violations.push(
            "M5 closure requires final figures: evidence/m5/inventory-summary.json records itself \
             as provisional, which is a true statement about an inventory under repair and not \
             something a milestone can close over"
                .to_owned(),
        );
    }
}

type InventoryRule = for<'a> fn(&model::Inventory<'a>) -> bool;
type InventoryFigure = for<'a> fn(&model::Inventory<'a>) -> Option<Value>;

const M5_ARITHMETIC: &[(InventoryRule, &str)] = &[
    (
        |r| r.artifacts.catalogued.checked_add(r.artifacts.uncatalogued) == Some(r.artifacts.total),
        "every artifact is either catalogued or not",
    ),
    (
        |r| r.artifacts.planned == r.artifacts.total && r.artifacts.uncatalogued == 0,
        "M5-W8's gate row asks that every artifact carry a complete plan",
    ),
    (
        |r| r.plans.planned == r.artifacts.total,
        "M5-W8's gate row asks for a plan per catalogued artifact",
    ),
    (
        |r| {
            r.plans
                .reading_source
                .checked_add(r.plans.not_reading_source)
                == Some(r.artifacts.total)
        },
        "a plan either reads source or does not",
    ),
    (
        |r| r.plans.canonical_ids == r.artifacts.total,
        "M5-W6 assigns one canonical ID per entry",
    ),
    (
        |r| {
            count_sum([
                r.rights.source_use.permitted,
                r.rights.source_use.conditional,
                r.rights.source_use.prohibited,
                r.rights.source_use.unresolved,
            ]) == Some(r.artifacts.total)
        },
        "every triaged artifact carries exactly one source-use verdict",
    ),
    (
        |r| r.surfaces.catalogued.checked_add(r.surfaces.unclassified) == Some(r.artifacts.total),
        "every acquired artifact is catalogued or is counted as unclassified",
    ),
    (
        |r| r.graph.resolved_nodes.checked_add(r.graph.blocked_nodes) == Some(r.graph.nodes),
        "every node resolves or carries a blocking classification",
    ),
    (
        |r| r.graph.resolved_edges.checked_add(r.graph.blocked_edges) == Some(r.graph.edges),
        "every edge resolves or carries a blocking classification",
    ),
    (
        |r| {
            r.graph.clusters > 0
                && r.graph.largest_cluster > 0
                && r.graph.largest_cluster <= r.graph.nodes
        },
        "a cluster is a set of nodes, so the largest cannot exceed the graph",
    ),
    (
        |r| r.reads.producers_watched > 0 && r.reads.archive_reads > 0,
        "an observation that classified nothing observed nothing",
    ),
    (
        |r| r.reads.entry_body_reads == 0,
        "reading an entry body is what the metadata-only method excludes",
    ),
];

const M5_CARRIED_FIGURES: &[(&str, InventoryFigure)] = &[
    ("/environment/settings/inventory/artifacts", |r| {
        r.artifacts
            .catalogued
            .checked_add(r.artifacts.uncatalogued)
            .map(Value::from)
    }),
    ("/environment/settings/inventory/planned", |r| {
        Some(Value::from(r.artifacts.planned))
    }),
    ("/environment/settings/inventory/graphNodes", |r| {
        r.graph
            .resolved_nodes
            .checked_add(r.graph.blocked_nodes)
            .map(Value::from)
    }),
    ("/environment/settings/inventory/graphEdges", |r| {
        r.graph
            .resolved_edges
            .checked_add(r.graph.blocked_edges)
            .map(Value::from)
    }),
    ("/environment/settings/inventory/clusters", |r| {
        Some(Value::from(r.graph.clusters))
    }),
    ("/environment/settings/inventory/sourceProhibited", |r| {
        Some(Value::from(r.rights.source_use.prohibited))
    }),
    ("/environment/settings/inventory/sourceUnresolved", |r| {
        Some(Value::from(r.rights.source_use.unresolved))
    }),
    ("/environment/settings/inventory/publishable", |r| {
        Some(Value::from(r.rights.publishable))
    }),
    ("/environment/settings/inventory/readingSource", |r| {
        Some(Value::from(r.plans.reading_source))
    }),
    ("/environment/settings/inventory/dependencyEdges", |r| {
        Some(Value::from(r.plans.dependency_edges))
    }),
    ("/environment/settings/inventory/archiveReads", |r| {
        Some(Value::from(r.reads.archive_reads))
    }),
    ("/environment/settings/inventory/entryBodyReads", |r| {
        Some(Value::from(r.reads.entry_body_reads))
    }),
    ("/environment/settings/inventory/digestsBound", |r| {
        Some(Value::from(r.bound_artifacts.0.len()))
    }),
];

pub(super) fn inventory_total(raw: &Value) -> Result<u64, String> {
    Ok(admit::<model::Inventory<'_>>(raw)?.artifacts.total)
}

pub(crate) const M5_FIGURES: CarriedFigures = CarriedFigures {
    record: "inventory-summary.json",
    subject: "inventory",
};

#[cfg(test)]
pub(crate) fn verify_m5_figures(
    milestone: Milestone,
    raw: &Value,
    manifest: &Value,
    carried: &CarriedFigures,
    violations: &mut Vec<String>,
) {
    match admit::<model::Inventory<'_>>(raw) {
        Ok(record) => crate::evidence::closure::verify_typed_figures(
            milestone,
            &record,
            manifest,
            carried,
            M5_CARRIED_FIGURES,
            M5_ARITHMETIC,
            violations,
        ),
        Err(error) => violations.push(format!(
            "{}/{}: {error}",
            milestone.directory(),
            carried.record
        )),
    }
}

pub(crate) fn verify_m5_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let mut records = read_records(root, Milestone::M5, M5_RECORDS, violations);
    if let Some(record) = records.get(M5_FIGURES.record) {
        let (result, provisional) = match admit::<model::Inventory<'_>>(record) {
            Ok(record) => {
                crate::evidence::closure::verify_typed_figures(
                    Milestone::M5,
                    &record,
                    manifest,
                    &M5_FIGURES,
                    M5_CARRIED_FIGURES,
                    M5_ARITHMETIC,
                    violations,
                );
                (
                    Ok(verify_inventory_record_typed(&record)),
                    record.result == Verdict::Provisional,
                )
            }
            Err(error) => (Err(error), false),
        };
        if provisional {
            records.mark_provisional(M5_FIGURES.record);
        }
        records.checked(Milestone::M5, &M5_RECORDS[0], result, violations);
    }
    records
}

#[cfg(test)]
mod tests;
