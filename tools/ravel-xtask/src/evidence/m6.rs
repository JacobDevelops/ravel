//! M6 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Figure, Rule, all_gates_closed, has_exact_work_packages, has_native_attestation,
};
use crate::json::{array, column_sum, number, same, sum};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::model::admit;
use crate::evidence::rules::require;

mod model;

fn verify_counterpart_record(value: &Value) -> Result<(), String> {
    let summary: model::CounterpartRecord<'_> = admit(value)?;
    require(
        summary.work_package.0 == "M6-W6",
        "/workPackage must be M6-W6",
    )?;
    require(
        summary.protocol.0 == "m6-counterpart-v1",
        "/protocol must be m6-counterpart-v1",
    )?;
    require(summary.result.0 == "pass", "/result must be pass")?;
    require(summary.wave.frozen, "/wave/frozen")?;
    let counterparts = &summary.counterparts.0;
    require(
        counterparts.iter().enumerate().all(|(index, counterpart)| {
            counterparts[..index]
                .iter()
                .all(|other| other.id.0 != counterpart.id.0)
        }),
        "/counterparts must have unique id",
    )
}

fn same_wave_entries(value: &Value) -> bool {
    let counterparts = array(value, "/counterparts");
    let entries = array(value, "/wave/entries");
    counterparts
        .iter()
        .all(|row| entries.iter().any(|entry| row["entry"] == entry["id"]))
        && entries
            .iter()
            .all(|entry| counterparts.iter().any(|row| row["entry"] == entry["id"]))
}

const M6_RECORDS: &[RecordDefinition] = &[RecordDefinition {
    name: "counterpart-summary.json",
    rule: verify_counterpart_record,
    description: "a counterpart summary carrying the frozen wave with a reason per entry, one row per \
 counterpart with its observable areas as numbers, the cluster scenario's steps, the \
 process-validation artefacts, and the rewrite-integrity check's scan counts and both \
 mutation exit statuses",
    required: false,
}];

const M6_ARITHMETIC: &[(Rule, &str)] = &[
    (
        |value| {
            number(value, "/totals/counterparts")
                == Value::from(array(value, "/counterparts").len()).as_f64()
        },
        "the total counts the rows the record itself lists",
    ),
    (
        |value| {
            same(
                column_sum(value, "/counterparts", "/observableAreas"),
                number(value, "/totals/observableAreas"),
            )
        },
        "the observable-area total is the sum of the counterparts' own",
    ),
    (
        |value| {
            same(
                column_sum(value, "/counterparts", "/declarableOnly"),
                number(value, "/totals/declarableOnly"),
            )
        },
        "the declarable-only total is the sum of the counterparts' own",
    ),
    (
        |value| {
            same(
                column_sum(value, "/counterparts", "/exercised"),
                number(value, "/totals/exercised"),
            )
        },
        "the exercised total is the sum of the counterparts' own",
    ),
    (
        |value| {
            array(value, "/counterparts").iter().all(|value| {
                same(
                    sum(value, &["/declarableOnly", "/exercised"]),
                    number(value, "/observableAreas"),
                )
            })
        },
        "an observable area is either exercised by a running process or declarable-only",
    ),
    (
        |value| {
            same(
                sum(value, &["/totals/declarableOnly", "/totals/exercised"]),
                number(value, "/totals/observableAreas"),
            )
        },
        "the denominator M6's green sits over is the sum of its two halves",
    ),
    (
        |value| {
            array(value, "/counterparts")
                .iter()
                .all(|value| number(value, "/observableAreas").is_some_and(|n| n > 0.0))
        },
        "a counterpart with no observable area declared nothing to be green about",
    ),
    (
        |value| same_wave_entries(value),
        "M6-W3 asks for one accepted behaviour specification per frozen wave entry, so a \
         counterpart outside the wave and a wave entry with no counterpart are both refused",
    ),
    (
        |value| {
            array(value, "/counterparts").iter().all(|row| {
                row.pointer("/specification/accepted")
                    .and_then(Value::as_bool)
                    == Some(true)
            })
        },
        "M6-W3 asks that every entry's behaviour specification be accepted",
    ),
    (
        |value| {
            array(value, "/counterparts").iter().all(|row| {
                row.pointer("/specification/carriesUpstreamIdentifier")
                    .and_then(Value::as_bool)
                    == Some(false)
            })
        },
        "M6-W3 asks that no specification carry an upstream identifier",
    ),
    (
        |value| {
            array(value, "/counterparts").iter().all(|row| {
                row.pointer("/specification/structureReviewed")
                    .and_then(Value::as_bool)
                    == Some(true)
            })
        },
        "M6-W5 reviews a specification for structure before an implementer sees it, which is \
         what caught the defect the sampling did not",
    ),
    (
        |value| {
            array(value, "/counterparts").iter().all(|row| {
                row.pointer("/provenance/complete").and_then(Value::as_bool) == Some(true)
            })
        },
        "M6-W6's gate row asks for complete provenance",
    ),
    (
        |value| {
            array(value, "/counterparts").iter().all(|value| {
                (number(value, "/suites/total").is_some_and(|n| n > 0.0))
                    && (number(value, "/suites/failed") == Some(0.0))
            })
        },
        "M6-W3 asks that each package's suites pass, and a package running none passed none",
    ),
    (
        |value| array(value, "/counterparts").len() >= 2,
        "M6-W5 validates the authoring process against a second independent counterpart, which \
         needs two",
    ),
    (
        |value| {
            value
                .pointer("/processValidation/secondCounterpart")
                .is_some_and(|second| {
                    array(value, "/counterparts")
                        .iter()
                        .any(|row| row.get("id") == Some(second))
                })
        },
        "M6-W5's second counterpart has to be one the record carries a row for",
    ),
    (
        |value| {
            ["fresh-play", "save", "exit", "reload"].iter().all(|name| {
                array(value, "/cluster/steps")
                    .iter()
                    .any(|row| row["name"] == *name)
            }) && array(value, "/cluster/steps")
                .iter()
                .all(|row| row["result"] == "pass")
        },
        "M6-W4's verification is a fresh play, save, exit and reload scenario that passes",
    ),
    (
        |value| {
            value
                .pointer("/rewriteIntegrity/enforcing")
                .and_then(Value::as_bool)
                == Some(true)
        },
        "M6-W6's gate row asks that the rewrite-integrity check be enforcing rather than advisory",
    ),
    (
        |value| number(value, "/rewriteIntegrity/violations") == Some(0.0),
        "a counterpart carrying an upstream identifier fails the check, so closure cannot record \
         one that it carried",
    ),
    (
        |value| number(value, "/rewriteIntegrity/specificationViolations") == Some(0.0),
        "M6-W3 asks that no specification carry an upstream identifier, and the check reads the \
         specifications too",
    ),
    (
        |value| {
            (number(value, "/rewriteIntegrity/filesScanned").is_some_and(|n| n > 0.0))
                && (number(value, "/rewriteIntegrity/artifactsScanned").is_some_and(|n| n > 0.0))
        },
        "the check reads every counterpart source file and every generated artefact, and one \
         enforcing the rule over source files only has not satisfied the obligation",
    ),
    (
        |value| {
            number(
                value,
                "/rewriteIntegrity/mutationProof/sourceInjection/exitStatus",
            )
            .is_some_and(|n| n.abs() > 0.0)
        },
        "the check is proved by mutation: an upstream identifier injected into a counterpart \
         source file must produce a nonzero exit",
    ),
    (
        |value| {
            number(
                value,
                "/rewriteIntegrity/mutationProof/artifactInjection/exitStatus",
            )
            .is_some_and(|n| n.abs() > 0.0)
        },
        "the check is proved by mutation: an upstream identifier injected into a generated \
         artefact must produce a nonzero exit",
    ),
    (
        |value| number(value, "/rewriteIntegrity/mutationProof/cleanRun/exitStatus") == Some(0.0),
        "a check that fails the unmutated tree proves nothing about the mutations it also failed",
    ),
];

const M6_CARRIED_FIGURES: &[(&str, Figure)] = &[
    ("/environment/settings/counterparts/counterparts", |value| {
        value
            .pointer("/counterparts")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
    ("/environment/settings/counterparts/waveEntries", |value| {
        value
            .pointer("/wave/entries")
            .and_then(Value::as_array)
            .map(|rows| Value::from(rows.len()))
    }),
    (
        "/environment/settings/counterparts/observableAreas",
        |value| {
            column_sum(value, "/counterparts", "/observableAreas")
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
        },
    ),
    (
        "/environment/settings/counterparts/declarableOnly",
        |value| {
            column_sum(value, "/counterparts", "/declarableOnly")
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
        },
    ),
    ("/environment/settings/counterparts/exercised", |value| {
        column_sum(value, "/counterparts", "/exercised")
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
    }),
    (
        "/environment/settings/rewriteIntegrity/filesScanned",
        |value| value.pointer("/rewriteIntegrity/filesScanned").cloned(),
    ),
    (
        "/environment/settings/rewriteIntegrity/artifactsScanned",
        |value| value.pointer("/rewriteIntegrity/artifactsScanned").cloned(),
    ),
    (
        "/environment/settings/rewriteIntegrity/violations",
        |value| value.pointer("/rewriteIntegrity/violations").cloned(),
    ),
    (
        "/environment/settings/rewriteIntegrity/sourceInjectionExitStatus",
        |value| {
            value
                .pointer("/rewriteIntegrity/mutationProof/sourceInjection/exitStatus")
                .cloned()
        },
    ),
    (
        "/environment/settings/rewriteIntegrity/artifactInjectionExitStatus",
        |value| {
            value
                .pointer("/rewriteIntegrity/mutationProof/artifactInjection/exitStatus")
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
    record: "counterpart-summary.json",
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
        verify_milestone_figures(Milestone::M6, record, manifest, &M6_FIGURES, violations);
    }
    records
}

#[cfg(test)]
mod tests;
