//! M6 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::path::Path;

use serde_json::Value;

use crate::evidence::rules::{
    Rule, all_gates_closed, has_exact_work_packages, has_native_attestation,
};
use crate::json::{array, number};

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_manifest_closure, verify_milestone_figures,
};
use crate::evidence::model::{Verdict, admit, count_sum};
use crate::evidence::rules::require;

mod model;

fn verify_counterpart_record(value: &Value) -> Result<(), String> {
    let summary: model::CounterpartRecord<'_> = admit(value)?;
    crate::evidence::model::require_identity(summary.work_package.0, "M6-W6", "/workPackage")?;
    crate::evidence::model::require_identity(summary.protocol.0, "m6-counterpart-v1", "/protocol")?;
    require(summary.result == Verdict::Pass, "/result must be pass")?;
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

fn same_wave_entries(summary: &model::CounterpartRecord<'_>) -> bool {
    let counterparts = &summary.counterparts.0;
    let entries = &summary.wave.entries.0;
    counterparts
        .iter()
        .all(|row| entries.iter().any(|entry| row.entry.0 == entry.id.0))
        && entries
            .iter()
            .all(|entry| counterparts.iter().any(|row| row.entry.0 == entry.id.0))
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

type CounterpartRule = for<'a> fn(&model::CounterpartRecord<'a>) -> bool;
type CounterpartFigure = for<'a> fn(&model::CounterpartRecord<'a>) -> Option<Value>;

const M6_ARITHMETIC: &[(CounterpartRule, &str)] = &[
    (
        |r| usize::try_from(r.totals.counterparts) == Ok(r.counterparts.0.len()),
        "the total counts the rows the record itself lists",
    ),
    (
        |r| {
            count_sum(r.counterparts.0.iter().map(|c| c.observable_areas))
                == Some(r.totals.observable_areas)
        },
        "the observable-area total is the sum of the counterparts' own",
    ),
    (
        |r| {
            count_sum(r.counterparts.0.iter().map(|c| c.declarable_only))
                == Some(r.totals.declarable_only)
        },
        "the declarable-only total is the sum of the counterparts' own",
    ),
    (
        |r| count_sum(r.counterparts.0.iter().map(|c| c.exercised)) == Some(r.totals.exercised),
        "the exercised total is the sum of the counterparts' own",
    ),
    (
        |r| {
            r.counterparts
                .0
                .iter()
                .all(|c| c.declarable_only.checked_add(c.exercised) == Some(c.observable_areas))
        },
        "an observable area is either exercised by a running process or declarable-only",
    ),
    (
        |r| {
            r.totals.declarable_only.checked_add(r.totals.exercised)
                == Some(r.totals.observable_areas)
        },
        "the denominator M6's green sits over is the sum of its two halves",
    ),
    (
        |r| r.counterparts.0.iter().all(|c| c.observable_areas > 0),
        "a counterpart with no observable area declared nothing to be green about",
    ),
    (
        same_wave_entries,
        "M6-W3 asks for one accepted behaviour specification per frozen wave entry, so a \
         counterpart outside the wave and a wave entry with no counterpart are both refused",
    ),
    (
        |r| r.counterparts.0.iter().all(|c| c.specification.accepted),
        "M6-W3 asks that every entry's behaviour specification be accepted",
    ),
    (
        |r| {
            r.counterparts
                .0
                .iter()
                .all(|c| !c.specification.carries_upstream_identifier)
        },
        "M6-W3 asks that no specification carry an upstream identifier",
    ),
    (
        |r| {
            r.counterparts
                .0
                .iter()
                .all(|c| c.specification.structure_reviewed)
        },
        "M6-W5 reviews a specification for structure before an implementer sees it, which is \
         what caught the defect the sampling did not",
    ),
    (
        |r| r.counterparts.0.iter().all(|c| c.provenance.complete),
        "M6-W6's gate row asks for complete provenance",
    ),
    (
        |r| {
            r.counterparts
                .0
                .iter()
                .all(|c| c.suites.total > 0 && c.suites.failed == 0)
        },
        "M6-W3 asks that each package's suites pass, and a package running none passed none",
    ),
    (
        |r| r.counterparts.0.len() >= 2,
        "M6-W5 validates the authoring process against a second independent counterpart, which \
         needs two",
    ),
    (
        |r| {
            r.counterparts
                .0
                .iter()
                .any(|c| c.id.0 == r.process_validation.second_counterpart.0)
        },
        "M6-W5's second counterpart has to be one the record carries a row for",
    ),
    (
        |r| {
            ["fresh-play", "save", "exit", "reload"]
                .iter()
                .all(|name| r.cluster.steps.0.iter().any(|step| step.name.0 == *name))
                && r.cluster
                    .steps
                    .0
                    .iter()
                    .all(|step| step.result == Verdict::Pass)
        },
        "M6-W4's verification is a fresh play, save, exit and reload scenario that passes",
    ),
    (
        |r| r.rewrite_integrity.enforcing,
        "M6-W6's gate row asks that the rewrite-integrity check be enforcing rather than advisory",
    ),
    (
        |r| r.rewrite_integrity.violations == 0,
        "a counterpart carrying an upstream identifier fails the check, so closure cannot record \
         one that it carried",
    ),
    (
        |r| r.rewrite_integrity.specification_violations == 0,
        "M6-W3 asks that no specification carry an upstream identifier, and the check reads the \
         specifications too",
    ),
    (
        |r| r.rewrite_integrity.files_scanned > 0 && r.rewrite_integrity.artifacts_scanned > 0,
        "the check reads every counterpart source file and every generated artefact, and one \
         enforcing the rule over source files only has not satisfied the obligation",
    ),
    (
        |r| {
            r.rewrite_integrity
                .mutation_proof
                .source_injection
                .exit_status
                != 0
        },
        "the check is proved by mutation: an upstream identifier injected into a counterpart \
         source file must produce a nonzero exit",
    ),
    (
        |r| {
            r.rewrite_integrity
                .mutation_proof
                .artifact_injection
                .exit_status
                != 0
        },
        "the check is proved by mutation: an upstream identifier injected into a generated \
         artefact must produce a nonzero exit",
    ),
    (
        |r| r.rewrite_integrity.mutation_proof.clean_run.exit_status == 0,
        "a check that fails the unmutated tree proves nothing about the mutations it also failed",
    ),
];

const M6_CARRIED_FIGURES: &[(&str, CounterpartFigure)] = &[
    ("/environment/settings/counterparts/counterparts", |r| {
        Some(Value::from(r.counterparts.0.len()))
    }),
    ("/environment/settings/counterparts/waveEntries", |r| {
        Some(Value::from(r.wave.entries.0.len()))
    }),
    ("/environment/settings/counterparts/observableAreas", |r| {
        count_sum(r.counterparts.0.iter().map(|c| c.observable_areas)).map(Value::from)
    }),
    ("/environment/settings/counterparts/declarableOnly", |r| {
        count_sum(r.counterparts.0.iter().map(|c| c.declarable_only)).map(Value::from)
    }),
    ("/environment/settings/counterparts/exercised", |r| {
        count_sum(r.counterparts.0.iter().map(|c| c.exercised)).map(Value::from)
    }),
    ("/environment/settings/rewriteIntegrity/filesScanned", |r| {
        Some(Value::from(r.rewrite_integrity.files_scanned))
    }),
    (
        "/environment/settings/rewriteIntegrity/artifactsScanned",
        |r| Some(Value::from(r.rewrite_integrity.artifacts_scanned)),
    ),
    ("/environment/settings/rewriteIntegrity/violations", |r| {
        Some(Value::from(r.rewrite_integrity.violations))
    }),
    (
        "/environment/settings/rewriteIntegrity/sourceInjectionExitStatus",
        |r| {
            Some(Value::from(
                r.rewrite_integrity
                    .mutation_proof
                    .source_injection
                    .exit_status,
            ))
        },
    ),
    (
        "/environment/settings/rewriteIntegrity/artifactInjectionExitStatus",
        |r| {
            Some(Value::from(
                r.rewrite_integrity
                    .mutation_proof
                    .artifact_injection
                    .exit_status,
            ))
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
    verify: |milestone, raw, manifest, carried, violations| match admit::<
        model::CounterpartRecord<'_>,
    >(raw)
    {
        Ok(record) => crate::evidence::closure::verify_typed_figures(
            milestone,
            &record,
            manifest,
            carried,
            M6_CARRIED_FIGURES,
            M6_ARITHMETIC,
            violations,
        ),
        Err(error) => violations.push(format!(
            "{}/{}: {error}",
            milestone.directory(),
            carried.record
        )),
    },
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
