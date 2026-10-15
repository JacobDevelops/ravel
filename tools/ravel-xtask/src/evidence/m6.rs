//! M6 closure rules.

use std::path::Path;

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_milestone_figures, verify_record_arithmetic,
};
use crate::evidence::manifest::require_manifest_rule;

const M6_RECORDS: &[(&str, &str, &str)] = &[(
    "counterpart-summary.json",
    r#".milestone == "M6"
        and .workPackage == "M6-W6"
        and .protocol == "m6-counterpart-v1"
        and .result == "pass"
        and (.wave | type == "object")
        and (.wave.frozen == true)
        and (.wave.entries | type == "array" and length > 0)
        and (.wave.entries | all(
            (.id | type == "string" and length > 0)
            and (.reason | type == "string" and length > 0)
            and (.dependencies | type == "array")))
        and (.counterparts | type == "array" and length > 0)
        and (.counterparts | all(
            (.id | type == "string" and length > 0)
            and (.entry | type == "string" and length > 0)
            and (.specification | type == "object")
            and (.specification.route == "source" or .specification.route == "observed")
            and ([.specification.accepted, .specification.structureReviewed,
                  .specification.carriesUpstreamIdentifier]
                 | all(type == "boolean"))
            and ([.observableAreas, .declarableOnly, .exercised] | all(type == "number"))
            and (.provenance | type == "object")
            and (.provenance.complete | type == "boolean")
            and ([.suites.total, .suites.failed] | all(type == "number"))))
        and ((.counterparts | map(.id) | unique | length) == (.counterparts | length))
        and (.cluster | type == "object")
        and (.cluster.scenario | type == "string" and length > 0)
        and (.cluster.steps | type == "array" and length > 0)
        and (.cluster.steps | all(
            (.name | type == "string" and length > 0)
            and (.result | type == "string" and length > 0)))
        and (.processValidation | type == "object")
        and ([.processValidation.template, .processValidation.checklist,
              .processValidation.evidenceSchema, .processValidation.secondCounterpart]
             | all(type == "string" and length > 0))
        and (.rewriteIntegrity | type == "object")
        and (.rewriteIntegrity.enforcing | type == "boolean")
        and ([.rewriteIntegrity.lexicon, .rewriteIntegrity.identifierSet]
             | all(type == "string" and length > 0))
        and ([.rewriteIntegrity.filesScanned, .rewriteIntegrity.artifactsScanned,
              .rewriteIntegrity.violations, .rewriteIntegrity.specificationViolations]
             | all(type == "number"))
        and (.rewriteIntegrity.mutationProof | type == "object")
        and ([.rewriteIntegrity.mutationProof.sourceInjection.exitStatus,
              .rewriteIntegrity.mutationProof.artifactInjection.exitStatus,
              .rewriteIntegrity.mutationProof.cleanRun.exitStatus]
             | all(type == "number"))
        and (.totals | type == "object")
        and ([.totals.counterparts, .totals.observableAreas, .totals.declarableOnly,
              .totals.exercised]
             | all(type == "number"))"#,
    "a counterpart summary carrying the frozen wave with a reason per entry, one row per \
     counterpart with its observable areas as numbers, the cluster scenario's steps, the \
     process-validation artefacts, and the rewrite-integrity check's scan counts and both \
     mutation exit statuses",
)];

const M6_ARITHMETIC: &[(&str, &str)] = &[
    (
        ".totals.counterparts == (.counterparts | length)",
        "the total counts the rows the record itself lists",
    ),
    (
        ".totals.observableAreas == ([.counterparts[].observableAreas] | add)",
        "the observable-area total is the sum of the counterparts' own",
    ),
    (
        ".totals.declarableOnly == ([.counterparts[].declarableOnly] | add)",
        "the declarable-only total is the sum of the counterparts' own",
    ),
    (
        ".totals.exercised == ([.counterparts[].exercised] | add)",
        "the exercised total is the sum of the counterparts' own",
    ),
    (
        ".counterparts | all(.declarableOnly + .exercised == .observableAreas)",
        "an observable area is either exercised by a running process or declarable-only",
    ),
    (
        ".totals.declarableOnly + .totals.exercised == .totals.observableAreas",
        "the denominator M6's green sits over is the sum of its two halves",
    ),
    (
        ".counterparts | all(.observableAreas > 0)",
        "a counterpart with no observable area declared nothing to be green about",
    ),
    (
        "([.counterparts[].entry] | unique) == ([.wave.entries[].id] | unique)",
        "M6-W3 asks for one accepted behaviour specification per frozen wave entry, so a \
         counterpart outside the wave and a wave entry with no counterpart are both refused",
    ),
    (
        ".counterparts | all(.specification.accepted == true)",
        "M6-W3 asks that every entry's behaviour specification be accepted",
    ),
    (
        ".counterparts | all(.specification.carriesUpstreamIdentifier == false)",
        "M6-W3 asks that no specification carry an upstream identifier",
    ),
    (
        ".counterparts | all(.specification.structureReviewed == true)",
        "M6-W5 reviews a specification for structure before an implementer sees it, which is \
         what caught the defect the sampling did not",
    ),
    (
        ".counterparts | all(.provenance.complete == true)",
        "M6-W6's gate row asks for complete provenance",
    ),
    (
        ".counterparts | all(.suites.total > 0 and .suites.failed == 0)",
        "M6-W3 asks that each package's suites pass, and a package running none passed none",
    ),
    (
        ".counterparts | length >= 2",
        "M6-W5 validates the authoring process against a second independent counterpart, which \
         needs two",
    ),
    (
        ".processValidation.secondCounterpart as $second
            | [.counterparts[].id] | index($second) != null",
        "M6-W5's second counterpart has to be one the record carries a row for",
    ),
    (
        r#"(["fresh-play", "save", "exit", "reload"] - [.cluster.steps[].name]) == []
            and (.cluster.steps | all(.result == "pass"))"#,
        "M6-W4's verification is a fresh play, save, exit and reload scenario that passes",
    ),
    (
        ".rewriteIntegrity.enforcing == true",
        "M6-W6's gate row asks that the rewrite-integrity check be enforcing rather than advisory",
    ),
    (
        ".rewriteIntegrity.violations == 0",
        "a counterpart carrying an upstream identifier fails the check, so closure cannot record \
         one that it carried",
    ),
    (
        ".rewriteIntegrity.specificationViolations == 0",
        "M6-W3 asks that no specification carry an upstream identifier, and the check reads the \
         specifications too",
    ),
    (
        ".rewriteIntegrity.filesScanned > 0 and .rewriteIntegrity.artifactsScanned > 0",
        "the check reads every counterpart source file and every generated artefact, and one \
         enforcing the rule over source files only has not satisfied the obligation",
    ),
    (
        ".rewriteIntegrity.mutationProof.sourceInjection.exitStatus != 0",
        "the check is proved by mutation: an upstream identifier injected into a counterpart \
         source file must produce a nonzero exit",
    ),
    (
        ".rewriteIntegrity.mutationProof.artifactInjection.exitStatus != 0",
        "the check is proved by mutation: an upstream identifier injected into a generated \
         artefact must produce a nonzero exit",
    ),
    (
        ".rewriteIntegrity.mutationProof.cleanRun.exitStatus == 0",
        "a check that fails the unmutated tree proves nothing about the mutations it also failed",
    ),
];

const M6_CARRIED_FIGURES: &[(&str, &str)] = &[
    (
        ".environment.settings.counterparts.counterparts",
        ".counterparts | length",
    ),
    (
        ".environment.settings.counterparts.waveEntries",
        ".wave.entries | length",
    ),
    (
        ".environment.settings.counterparts.observableAreas",
        "[.counterparts[].observableAreas] | add",
    ),
    (
        ".environment.settings.counterparts.declarableOnly",
        "[.counterparts[].declarableOnly] | add",
    ),
    (
        ".environment.settings.counterparts.exercised",
        "[.counterparts[].exercised] | add",
    ),
    (
        ".environment.settings.rewriteIntegrity.filesScanned",
        ".rewriteIntegrity.filesScanned",
    ),
    (
        ".environment.settings.rewriteIntegrity.artifactsScanned",
        ".rewriteIntegrity.artifactsScanned",
    ),
    (
        ".environment.settings.rewriteIntegrity.violations",
        ".rewriteIntegrity.violations",
    ),
    (
        ".environment.settings.rewriteIntegrity.sourceInjectionExitStatus",
        ".rewriteIntegrity.mutationProof.sourceInjection.exitStatus",
    ),
    (
        ".environment.settings.rewriteIntegrity.artifactInjectionExitStatus",
        ".rewriteIntegrity.mutationProof.artifactInjection.exitStatus",
    ),
];

pub(crate) fn verify_m6_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M6-W1\", \"M6-W2\", \"M6-W3\", \"M6-W4\", \"M6-W5\", \"M6-W6\"]",
        "M6 manifest must include every work package from M6-W1 through M6-W6 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m6-counterpart-v1"
            and ([.benchmarks.results[].name] | index("counterpart-summary.json") != null)"#,
        "M6 manifest must record its counterpart summary under the m6-counterpart-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M6-W6") | .value]
                 | length == 5
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M6 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".environment.settings.rewriteIntegrity.enforcing == true
            and ([.environment.settings.rewriteIntegrity.sourceInjectionExitStatus,
                  .environment.settings.rewriteIntegrity.artifactInjectionExitStatus]
                 | all(type == "number" and . != 0))"#,
        "M6 closure requires the manifest itself to record the rewrite-integrity check as \
         enforcing and to record a nonzero exit status for each of the two injections that \
         prove it",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("official-atm11") != null)
        )"#,
        "M6 closure requires a native Linux attestation for the recorded counterpart run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M6 counts the observable areas nothing runs, and a milestone disclosing that and \
         recording no deviation or risk deleted the finding rather than the defect",
        violations,
    );

    verify_closure_records(root, "M6", "evidence/m6", M6_RECORDS, violations);

    verify_m6_counterpart_arithmetic(root, violations);
    verify_milestone_figures(root, manifest_path, &M6_FIGURES, violations);
}

fn verify_m6_counterpart_arithmetic(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m6/counterpart-summary.json");
    if !record.is_file() {
        return;
    }
    verify_record_arithmetic(
        &record,
        "evidence/m6/counterpart-summary.json",
        M6_ARITHMETIC,
        violations,
    );
}

const M6_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m6/manifest.json",
    record: "evidence/m6/counterpart-summary.json",
    subject: "counterpart set",
    figures: M6_CARRIED_FIGURES,
};

#[cfg(test)]
mod tests;
