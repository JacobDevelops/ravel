//! M5 closure rules.

use std::path::Path;

use serde_json::Value;

use crate::json::{
    Figure, Rule, array, benchmark_results, closed_gates, native_attestation, nonempty, number,
    read_json, work_packages,
};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_milestone_figures, verify_record_arithmetic,
};
use crate::evidence::manifest::require_manifest_rule;

fn inventory_record(v: &Value) -> bool {
    v["milestone"] == "M5"
        && v["workPackage"] == "M5-W8"
        && v["protocol"] == "m5-inventory-v1"
        && matches!(v["result"].as_str(), Some("pass" | "provisional"))
        && nonempty(v, "/disclosure/readObservation/bound")
        && v.pointer("/disclosure/judgedClaims")
            .is_some_and(Value::is_array)
        && !array(v, "/disclosure/judgedClaims").is_empty()
        && array(v, "/disclosure/judgedClaims").iter().all(|row| {
            nonempty(row, "/claim") && row["basis"] == "judged" && nonempty(row, "/decidedBy")
        })
        && inventory_numbers(v)
        && v["boundArtifacts"].is_array()
        && !array(v, "/boundArtifacts").is_empty()
        && array(v, "/boundArtifacts").iter().all(|row| {
            nonempty(row, "/name")
                && nonempty(row, "/holds")
                && row["sha256"].as_str().is_some_and(|digest| {
                    digest.len() == 64
                        && digest
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
                && matches!(row["class"].as_str(), Some("material" | "machinery"))
        })
        && array(v, "/boundArtifacts")
            .iter()
            .enumerate()
            .all(|(index, row)| {
                array(v, "/boundArtifacts")[..index]
                    .iter()
                    .all(|other| other["name"] != row["name"])
            })
        && v["primitiveScan"]["class"] == "machinery"
        && v["machineryAmendments"].is_array()
        && array(v, "/machineryAmendments").iter().all(|row| {
            nonempty(row, "/milestone")
                && nonempty(row, "/reason")
                && row["entries"].is_array()
                && !array(row, "/entries").is_empty()
                && row["figures"].is_array()
                && array(row, "/entries").iter().all(|entry| {
                    array(v, "/boundArtifacts").iter().any(|artifact| {
                        artifact["class"] == "machinery" && artifact.get("name") == Some(entry)
                    })
                })
        })
}

fn inventory_numbers(v: &Value) -> bool {
    [
        (
            "/artifacts",
            &["total", "catalogued", "uncatalogued", "planned"][..],
        ),
        (
            "/graph",
            &[
                "nodes",
                "resolvedNodes",
                "blockedNodes",
                "edges",
                "resolvedEdges",
                "blockedEdges",
                "clusters",
                "largestCluster",
            ][..],
        ),
        (
            "/rights/sourceUse",
            &["permitted", "conditional", "prohibited", "unresolved"][..],
        ),
        (
            "/surfaces",
            &[
                "classFiles",
                "models",
                "textures",
                "recipes",
                "catalogued",
                "unclassified",
                "nestedJars",
            ][..],
        ),
        (
            "/plans",
            &[
                "planned",
                "readingSource",
                "notReadingSource",
                "dependencyEdges",
                "canonicalIds",
            ][..],
        ),
        (
            "/reads",
            &["producersWatched", "archiveReads", "entryBodyReads"][..],
        ),
    ]
    .iter()
    .all(|(section, fields)| {
        v.pointer(section).is_some_and(|row| {
            row.is_object() && fields.iter().all(|field| row[*field].is_number())
        })
    }) && v["rights"].is_object()
        && v["rights"]["publishable"].is_number()
}

const M5_RECORDS: &[(&str, Rule, &str)] = &[(
    "inventory-summary.json",
    inventory_record,
    "an inventory summary carrying every aggregate as a number, a digest over every \
     registry file it binds with the class that digest freezes, its trail of machinery \
     amendments, and its own disclosure of what the read observation watches and \
     which of its claims rest on judgement",
)];

pub(crate) fn verify_m5_manifest(root: &Path, manifest_path: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        |v| {
            work_packages(
                v,
                &[
                    "M5-W1", "M5-W2", "M5-W3", "M5-W4", "M5-W5", "M5-W6", "M5-W7", "M5-W8",
                ],
            )
        },
        "M5 manifest must include every work package from M5-W1 through M5-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |value| benchmark_results(value, "m5-inventory-v1", &["inventory-summary.json"]),
        "M5 manifest must record its inventory summary under the m5-inventory-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| closed_gates(v, "M5-W8", 7),
        "M5 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |value| native_attestation(value, "official-atm11"),
        "M5 closure requires a native Linux attestation for the recorded inventory run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| !array(v, "/deviations").is_empty() && !array(v, "/risks").is_empty(),
        "M5 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    let record = root.join("evidence/m5/inventory-summary.json");
    if record.is_file() && read_json(&record).is_ok_and(|value| value["result"] == "provisional") {
        violations.push(
            "M5 closure requires final figures: evidence/m5/inventory-summary.json records itself \
             as provisional, which is a true statement about an inventory under repair and not \
             something a milestone can close over"
                .to_owned(),
        );
    }

    verify_closure_records(root, "M5", "evidence/m5", M5_RECORDS, violations);
}

pub(crate) const M5_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |v| {
            ["/artifacts/catalogued", "/artifacts/uncatalogued"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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
            ["/plans/readingSource", "/plans/notReadingSource"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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
            [
                "/rights/sourceUse/permitted",
                "/rights/sourceUse/conditional",
                "/rights/sourceUse/prohibited",
                "/rights/sourceUse/unresolved",
            ]
            .iter()
            .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
            .zip(number(v, "/artifacts/total"))
            .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every triaged artifact carries exactly one source-use verdict",
    ),
    (
        |v| {
            ["/surfaces/catalogued", "/surfaces/unclassified"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
                .zip(number(v, "/artifacts/total"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every acquired artifact is catalogued or is counted as unclassified",
    ),
    (
        |v| {
            ["/graph/resolvedNodes", "/graph/blockedNodes"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
                .zip(number(v, "/graph/nodes"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "every node resolves or carries a blocking classification",
    ),
    (
        |v| {
            ["/graph/resolvedEdges", "/graph/blockedEdges"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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
        ["/artifacts/catalogued", "/artifacts/uncatalogued"]
            .iter()
            .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/planned", |v| {
        v.pointer("/artifacts/planned").cloned()
    }),
    ("/environment/settings/inventory/graphNodes", |v| {
        ["/graph/resolvedNodes", "/graph/blockedNodes"]
            .iter()
            .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/inventory/graphEdges", |v| {
        ["/graph/resolvedEdges", "/graph/blockedEdges"]
            .iter()
            .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
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
};

pub(crate) fn verify_m5_integrity(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    verify_record_arithmetic(
        root,
        "evidence/m5/inventory-summary.json",
        M5_ARITHMETIC,
        violations,
    );
    verify_milestone_figures(root, manifest, &M5_FIGURES, violations);
}

#[cfg(test)]
mod tests;
