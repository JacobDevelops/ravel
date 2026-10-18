//! M6 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Figure, Rule, all_gates_closed, has_exact_work_packages, has_native_attestation,
};
use crate::json::{array, column_sum, nonempty, number, sum};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::rules::{require, require_fields, require_rows, unique_by};

fn counterpart_record(v: &Value) -> Result<(), String> {
    require(v["workPackage"] == "M6-W6", "/workPackage must be M6-W6")?;
    require(
        v["protocol"] == "m6-counterpart-v1",
        "/protocol must be m6-counterpart-v1",
    )?;
    require(v["result"] == "pass", "/result must be pass")?;
    counterpart_wave(v)?;
    let counterparts = array(v, "/counterparts");
    require_rows(v, "/counterparts", counterpart_row)?;
    require(
        unique_by(counterparts, |left, right| left["id"] == right["id"]),
        "/counterparts must have unique id",
    )?;
    counterpart_process(v)?;
    rewrite_integrity_record(v)?;
    require(v["totals"].is_object(), "/totals")?;
    require_fields(
        v,
        &[
            "/totals/counterparts",
            "/totals/observableAreas",
            "/totals/declarableOnly",
            "/totals/exercised",
        ],
        Value::is_number,
    )?;
    Ok(())
}

fn counterpart_wave(v: &Value) -> Result<(), String> {
    require(v["wave"].is_object(), "/wave")?;
    require(v["wave"]["frozen"] == true, "/wave/frozen")?;
    require_rows(v, "/wave/entries", |row| {
        require(nonempty(row, "/id"), "/id")?;
        require(nonempty(row, "/reason"), "/reason")?;
        require(row["dependencies"].is_array(), "/dependencies")?;

        Ok(())
    })?;
    Ok(())
}

fn counterpart_process(v: &Value) -> Result<(), String> {
    require(v["cluster"].is_object(), "/cluster")?;
    require(nonempty(v, "/cluster/scenario"), "/cluster/scenario")?;
    require_rows(v, "/cluster/steps", |row| {
        require(nonempty(row, "/name"), "/name")?;
        require(nonempty(row, "/result"), "/result")?;

        Ok(())
    })?;
    require_fields(
        v,
        &[
            "/processValidation/template",
            "/processValidation/checklist",
            "/processValidation/evidenceSchema",
            "/processValidation/secondCounterpart",
        ],
        |value| value.as_str().is_some_and(|text| !text.is_empty()),
    )
}

fn rewrite_integrity_record(v: &Value) -> Result<(), String> {
    require_fields(v, &["/rewriteIntegrity/enforcing"], Value::is_boolean)?;
    require_fields(
        v,
        &[
            "/rewriteIntegrity/lexicon",
            "/rewriteIntegrity/identifierSet",
        ],
        |v| v.as_str().is_some_and(|s| !s.is_empty()),
    )?;
    require_fields(
        v,
        &[
            "/rewriteIntegrity/filesScanned",
            "/rewriteIntegrity/artifactsScanned",
            "/rewriteIntegrity/violations",
            "/rewriteIntegrity/specificationViolations",
            "/rewriteIntegrity/mutationProof/sourceInjection/exitStatus",
            "/rewriteIntegrity/mutationProof/artifactInjection/exitStatus",
            "/rewriteIntegrity/mutationProof/cleanRun/exitStatus",
        ],
        Value::is_number,
    )
}

fn counterpart_row(row: &Value) -> Result<(), String> {
    require(nonempty(row, "/id"), "/id")?;
    require(nonempty(row, "/entry"), "/entry")?;
    require(row["specification"].is_object(), "/specification")?;
    require(
        matches!(
            row["specification"]["route"].as_str(),
            Some("source" | "observed")
        ),
        "/specification/route must be source or observed",
    )?;
    require_fields(
        row,
        &[
            "/specification/accepted",
            "/specification/structureReviewed",
            "/specification/carriesUpstreamIdentifier",
        ],
        Value::is_boolean,
    )?;
    require_fields(
        row,
        &["/observableAreas", "/declarableOnly", "/exercised"],
        Value::is_number,
    )?;
    require(row["provenance"].is_object(), "/provenance")?;
    require(
        row["provenance"]["complete"].is_boolean(),
        "/provenance/complete",
    )?;
    require_fields(row, &["/suites/total", "/suites/failed"], Value::is_number)?;
    Ok(())
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

const M6_RECORDS: &[RecordDefinition] = &[RecordDefinition {
    name: "counterpart-summary.json",
    rule: counterpart_record,
    description: "a counterpart summary carrying the frozen wave with a reason per entry, one row per \
 counterpart with its observable areas as numbers, the cluster scenario's steps, the \
 process-validation artefacts, and the rewrite-integrity check's scan counts and both \
 mutation exit statuses",
    required: false,
}];

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
            column_sum(v, "/counterparts", "/observableAreas")
                .zip(number(v, "/totals/observableAreas"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the observable-area total is the sum of the counterparts' own",
    ),
    (
        |v| {
            column_sum(v, "/counterparts", "/declarableOnly")
                .zip(number(v, "/totals/declarableOnly"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the declarable-only total is the sum of the counterparts' own",
    ),
    (
        |v| {
            column_sum(v, "/counterparts", "/exercised")
                .zip(number(v, "/totals/exercised"))
                .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
        },
        "the exercised total is the sum of the counterparts' own",
    ),
    (
        |v| {
            array(v, "/counterparts").iter().all(|v| {
                sum(v, &["/declarableOnly", "/exercised"])
                    .zip(number(v, "/observableAreas"))
                    .is_some_and(|(sum, total)| sum.total_cmp(&total).is_eq())
            })
        },
        "an observable area is either exercised by a running process or declarable-only",
    ),
    (
        |v| {
            sum(v, &["/totals/declarableOnly", "/totals/exercised"])
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
        column_sum(v, "/counterparts", "/observableAreas")
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/counterparts/declarableOnly", |v| {
        column_sum(v, "/counterparts", "/declarableOnly")
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    ("/environment/settings/counterparts/exercised", |v| {
        column_sum(v, "/counterparts", "/exercised")
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

const M6_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &["M6-W1", "M6-W2", "M6-W3", "M6-W4", "M6-W5", "M6-W6"],
            )
        },
        "M6 manifest must include every work package from M6-W1 through M6-W6 exactly once",
    ),
    (
        |value| {
            value["benchmarks"]["protocolVersion"] == "m6-counterpart-v1"
                && array(value, "/benchmarks/results")
                    .iter()
                    .any(|row| row["name"] == "counterpart-summary.json")
        },
        "M6 manifest must record its counterpart summary under the m6-counterpart-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
    ),
    (
        |value| all_gates_closed(value, "M6-W6", 5),
        "M6 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
    ),
    (
        |value| {
            value["environment"]["settings"]["rewriteIntegrity"]["enforcing"] == true
                && [
                    "/environment/settings/rewriteIntegrity/sourceInjectionExitStatus",
                    "/environment/settings/rewriteIntegrity/artifactInjectionExitStatus",
                ]
                .iter()
                .all(|path| number(value, path).is_some_and(|count| count.abs() > 0.0))
        },
        "M6 closure requires the manifest itself to record the rewrite-integrity check as \
         enforcing and to record a nonzero exit status for each of the two injections that \
         prove it",
    ),
    (
        |value| has_native_attestation(value, "official-atm11"),
        "M6 closure requires a native Linux attestation for the recorded counterpart run",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M6 counts the observable areas nothing runs, and a milestone disclosing that and \
         recording no deviation or risk deleted the finding rather than the defect",
    ),
];

pub(crate) fn verify_m6_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    verify_manifest_closure(manifest, M6_MANIFEST_RULES, violations);

    verify_closure_records(Milestone::M6, records, M6_RECORDS, violations);
}

const M6_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m6/manifest.json",
    record: "evidence/m6/counterpart-summary.json",
    subject: "counterpart set",
    figures: M6_CARRIED_FIGURES,
    arithmetic: M6_ARITHMETIC,
};

pub(crate) fn verify_m6_integrity(
    root: &Path,
    manifest: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M6, M6_RECORDS, violations);
    if let Some(record) = records.carried(&M6_FIGURES) {
        verify_milestone_figures(record, manifest, &M6_FIGURES, violations);
    }
    records
}

#[cfg(test)]
mod tests;
