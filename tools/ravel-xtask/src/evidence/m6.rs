//! M6 closure rules.

use std::path::Path;

use serde_json::Value;

use crate::json::{
    Figure, Rule, array, closed_gates, native_attestation, nonempty, number, work_packages,
};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_milestone_figures, verify_record_arithmetic,
};
use crate::evidence::manifest::require_manifest_rule;

fn counterpart_record(v: &Value) -> bool {
    v["milestone"] == "M6"
        && v["workPackage"] == "M6-W6"
        && v["protocol"] == "m6-counterpart-v1"
        && v["result"] == "pass"
        && v["wave"].is_object()
        && v["wave"]["frozen"] == true
        && v["wave"]["entries"].is_array()
        && !array(v, "/wave/entries").is_empty()
        && array(v, "/wave/entries").iter().all(|row| {
            nonempty(row, "/id") && nonempty(row, "/reason") && row["dependencies"].is_array()
        })
        && v["counterparts"].is_array()
        && !array(v, "/counterparts").is_empty()
        && array(v, "/counterparts").iter().all(counterpart_row)
        && array(v, "/counterparts")
            .iter()
            .enumerate()
            .all(|(index, row)| {
                array(v, "/counterparts")[..index]
                    .iter()
                    .all(|other| other["id"] != row["id"])
            })
        && v["cluster"].is_object()
        && nonempty(v, "/cluster/scenario")
        && v["cluster"]["steps"].is_array()
        && !array(v, "/cluster/steps").is_empty()
        && array(v, "/cluster/steps")
            .iter()
            .all(|row| nonempty(row, "/name") && nonempty(row, "/result"))
        && v["processValidation"].is_object()
        && [
            "template",
            "checklist",
            "evidenceSchema",
            "secondCounterpart",
        ]
        .iter()
        .all(|key| {
            v["processValidation"][*key]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        })
        && v["rewriteIntegrity"].is_object()
        && v["rewriteIntegrity"]["enforcing"].is_boolean()
        && ["lexicon", "identifierSet"].iter().all(|key| {
            v["rewriteIntegrity"][*key]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        })
        && [
            "filesScanned",
            "artifactsScanned",
            "violations",
            "specificationViolations",
        ]
        .iter()
        .all(|key| v["rewriteIntegrity"][*key].is_number())
        && v["rewriteIntegrity"]["mutationProof"].is_object()
        && ["sourceInjection", "artifactInjection", "cleanRun"]
            .iter()
            .all(|key| v["rewriteIntegrity"]["mutationProof"][*key]["exitStatus"].is_number())
        && v["totals"].is_object()
        && [
            "counterparts",
            "observableAreas",
            "declarableOnly",
            "exercised",
        ]
        .iter()
        .all(|key| v["totals"][*key].is_number())
}

fn counterpart_row(row: &Value) -> bool {
    nonempty(row, "/id")
        && nonempty(row, "/entry")
        && row["specification"].is_object()
        && matches!(
            row["specification"]["route"].as_str(),
            Some("source" | "observed")
        )
        && ["accepted", "structureReviewed", "carriesUpstreamIdentifier"]
            .iter()
            .all(|key| row["specification"][*key].is_boolean())
        && ["observableAreas", "declarableOnly", "exercised"]
            .iter()
            .all(|key| row[*key].is_number())
        && row["provenance"].is_object()
        && row["provenance"]["complete"].is_boolean()
        && ["total", "failed"]
            .iter()
            .all(|key| row["suites"][*key].is_number())
}

fn same_wave_entries(v: &Value) -> bool {
    let counterparts = array(v, "/counterparts");
    let entries = array(v, "/wave/entries");
    counterparts
        .iter()
        .all(|row| entries.iter().any(|entry| row["entry"] == entry["id"]))
        && entries
            .iter()
            .all(|entry| counterparts.iter().any(|row| row["entry"] == entry["id"]))
}

const M6_RECORDS: &[(&str, Rule, &str)] = &[(
    "counterpart-summary.json",
    counterpart_record,
    "a counterpart summary carrying the frozen wave with a reason per entry, one row per \
     counterpart with its observable areas as numbers, the cluster scenario's steps, the \
     process-validation artefacts, and the rewrite-integrity check's scan counts and both \
     mutation exit statuses",
)];

const M6_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |v| {
            number(v, "/totals/counterparts")
                == Value::from(array(v, "/counterparts").len()).as_f64()
        },
        "the total counts the rows the record itself lists",
    ),
    (
        |v| {
            array(v, "/counterparts")
                .iter()
                .try_fold(0.0, |sum, row| Some(sum + number(row, "/observableAreas")?))
                .zip(number(v, "/totals/observableAreas"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the observable-area total is the sum of the counterparts' own",
    ),
    (
        |v| {
            array(v, "/counterparts")
                .iter()
                .try_fold(0.0, |sum, row| Some(sum + number(row, "/declarableOnly")?))
                .zip(number(v, "/totals/declarableOnly"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the declarable-only total is the sum of the counterparts' own",
    ),
    (
        |v| {
            array(v, "/counterparts")
                .iter()
                .try_fold(0.0, |sum, row| Some(sum + number(row, "/exercised")?))
                .zip(number(v, "/totals/exercised"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the exercised total is the sum of the counterparts' own",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|v| {
                ["/declarableOnly", "/exercised"]
                    .iter()
                    .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
                    .zip(number(v, "/observableAreas"))
                    .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
            })
        },
        "an observable area is either exercised by a running process or declarable-only",
    ),
    (
        |v| {
            ["/totals/declarableOnly", "/totals/exercised"]
                .iter()
                .try_fold(0.0, |sum, path| Some(sum + number(v, path)?))
                .zip(number(v, "/totals/observableAreas"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the denominator M6's green sits over is the sum of its two halves",
    ),
    (
        |v| {
            array(v, "/counterparts")
                .iter()
                .all(|v| number(v, "/observableAreas").is_some_and(|n| n > 0.0))
        },
        "a counterpart with no observable area declared nothing to be green about",
    ),
    (
        |v| same_wave_entries(v),
        "M6-W3 asks for one accepted behaviour specification per frozen wave entry, so a \
         counterpart outside the wave and a wave entry with no counterpart are both refused",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|row| {
                row.pointer("/specification/accepted")
                    .and_then(Value::as_bool)
                    == Some(true)
            })
        },
        "M6-W3 asks that every entry's behaviour specification be accepted",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|row| {
                row.pointer("/specification/carriesUpstreamIdentifier")
                    .and_then(Value::as_bool)
                    == Some(false)
            })
        },
        "M6-W3 asks that no specification carry an upstream identifier",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|row| {
                row.pointer("/specification/structureReviewed")
                    .and_then(Value::as_bool)
                    == Some(true)
            })
        },
        "M6-W5 reviews a specification for structure before an implementer sees it, which is \
         what caught the defect the sampling did not",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|row| {
                row.pointer("/provenance/complete").and_then(Value::as_bool) == Some(true)
            })
        },
        "M6-W6's gate row asks for complete provenance",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|v| {
                (number(v, "/suites/total").is_some_and(|n| n > 0.0))
                    && (number(v, "/suites/failed") == Some(0.0))
            })
        },
        "M6-W3 asks that each package's suites pass, and a package running none passed none",
    ),
    (
        |v| array(v, "/counterparts").len() >= 2,
        "M6-W5 validates the authoring process against a second independent counterpart, which \
         needs two",
    ),
    (
        |v| {
            v.pointer("/processValidation/secondCounterpart")
                .is_some_and(|second| {
                    array(v, "/counterparts")
                        .iter()
                        .any(|row| row.get("id") == Some(second))
                })
        },
        "M6-W5's second counterpart has to be one the record carries a row for",
    ),
    (
        |v| {
            ["fresh-play", "save", "exit", "reload"].iter().all(|name| {
                array(v, "/cluster/steps")
                    .iter()
                    .any(|row| row["name"] == *name)
            }) && array(v, "/cluster/steps")
                .iter()
                .all(|row| row["result"] == "pass")
        },
        "M6-W4's verification is a fresh play, save, exit and reload scenario that passes",
    ),
    (
        |v| {
            v.pointer("/rewriteIntegrity/enforcing")
                .and_then(Value::as_bool)
                == Some(true)
        },
        "M6-W6's gate row asks that the rewrite-integrity check be enforcing rather than advisory",
    ),
    (
        |v| number(v, "/rewriteIntegrity/violations") == Some(0.0),
        "a counterpart carrying an upstream identifier fails the check, so closure cannot record \
         one that it carried",
    ),
    (
        |v| number(v, "/rewriteIntegrity/specificationViolations") == Some(0.0),
        "M6-W3 asks that no specification carry an upstream identifier, and the check reads the \
         specifications too",
    ),
    (
        |v| {
            (number(v, "/rewriteIntegrity/filesScanned").is_some_and(|n| n > 0.0))
                && (number(v, "/rewriteIntegrity/artifactsScanned").is_some_and(|n| n > 0.0))
        },
        "the check reads every counterpart source file and every generated artefact, and one \
         enforcing the rule over source files only has not satisfied the obligation",
    ),
    (
        |v| {
            number(
                v,
                "/rewriteIntegrity/mutationProof/sourceInjection/exitStatus",
            )
            .is_some_and(|n| n.abs() > 0.0)
        },
        "the check is proved by mutation: an upstream identifier injected into a counterpart \
         source file must produce a nonzero exit",
    ),
    (
        |v| {
            number(
                v,
                "/rewriteIntegrity/mutationProof/artifactInjection/exitStatus",
            )
            .is_some_and(|n| n.abs() > 0.0)
        },
        "the check is proved by mutation: an upstream identifier injected into a generated \
         artefact must produce a nonzero exit",
    ),
    (
        |v| number(v, "/rewriteIntegrity/mutationProof/cleanRun/exitStatus") == Some(0.0),
        "a check that fails the unmutated tree proves nothing about the mutations it also failed",
    ),
];

const M6_CARRIED_FIGURES: &[(&str, Figure)] = &[
    ("/environment/settings/counterparts/counterparts", |v| {
        v.pointer("/counterparts")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
    ("/environment/settings/counterparts/waveEntries", |v| {
        v.pointer("/wave/entries")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
    ("/environment/settings/counterparts/observableAreas", |v| {
        array(v, "/counterparts")
            .iter()
            .try_fold(0.0, |sum, row| Some(sum + number(row, "/observableAreas")?))
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/counterparts/declarableOnly", |v| {
        array(v, "/counterparts")
            .iter()
            .try_fold(0.0, |sum, row| Some(sum + number(row, "/declarableOnly")?))
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/counterparts/exercised", |v| {
        array(v, "/counterparts")
            .iter()
            .try_fold(0.0, |sum, row| Some(sum + number(row, "/exercised")?))
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/rewriteIntegrity/filesScanned", |v| {
        v.pointer("/rewriteIntegrity/filesScanned").cloned()
    }),
    (
        "/environment/settings/rewriteIntegrity/artifactsScanned",
        |v| v.pointer("/rewriteIntegrity/artifactsScanned").cloned(),
    ),
    ("/environment/settings/rewriteIntegrity/violations", |v| {
        v.pointer("/rewriteIntegrity/violations").cloned()
    }),
    (
        "/environment/settings/rewriteIntegrity/sourceInjectionExitStatus",
        |v| {
            v.pointer("/rewriteIntegrity/mutationProof/sourceInjection/exitStatus")
                .cloned()
        },
    ),
    (
        "/environment/settings/rewriteIntegrity/artifactInjectionExitStatus",
        |v| {
            v.pointer("/rewriteIntegrity/mutationProof/artifactInjection/exitStatus")
                .cloned()
        },
    ),
];

pub(crate) fn verify_m6_manifest(root: &Path, manifest_path: &Value, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        |v| work_packages(v, &["M6-W1", "M6-W2", "M6-W3", "M6-W4", "M6-W5", "M6-W6"]),
        "M6 manifest must include every work package from M6-W1 through M6-W6 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| {
            v["benchmarks"]["protocolVersion"] == "m6-counterpart-v1"
                && array(v, "/benchmarks/results")
                    .iter()
                    .any(|row| row["name"] == "counterpart-summary.json")
        },
        "M6 manifest must record its counterpart summary under the m6-counterpart-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| closed_gates(v, "M6-W6", 5),
        "M6 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| {
            v["environment"]["settings"]["rewriteIntegrity"]["enforcing"] == true
                && [
                    "/environment/settings/rewriteIntegrity/sourceInjectionExitStatus",
                    "/environment/settings/rewriteIntegrity/artifactInjectionExitStatus",
                ]
                .iter()
                .all(|path| number(v, path).is_some_and(|n| n.abs() > 0.0))
        },
        "M6 closure requires the manifest itself to record the rewrite-integrity check as \
         enforcing and to record a nonzero exit status for each of the two injections that \
         prove it",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        native_attestation,
        "M6 closure requires a native Linux attestation for the recorded counterpart run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        |v| !array(v, "/deviations").is_empty() && !array(v, "/risks").is_empty(),
        "M6 counts the observable areas nothing runs, and a milestone disclosing that and \
         recording no deviation or risk deleted the finding rather than the defect",
        violations,
    );

    verify_closure_records(root, "M6", "evidence/m6", M6_RECORDS, violations);
}

const M6_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m6/manifest.json",
    record: "evidence/m6/counterpart-summary.json",
    subject: "counterpart set",
    figures: M6_CARRIED_FIGURES,
};

pub(crate) fn verify_m6_integrity(root: &Path, manifest: &Value, violations: &mut Vec<String>) {
    verify_record_arithmetic(
        root,
        "evidence/m6/counterpart-summary.json",
        M6_ARITHMETIC,
        violations,
    );
    verify_milestone_figures(root, manifest, &M6_FIGURES, violations);
}

#[cfg(test)]
mod tests;
