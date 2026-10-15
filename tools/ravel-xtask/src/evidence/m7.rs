//! M7 closure rules.

use std::path::Path;

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_milestone_figures, verify_record_arithmetic,
};
use crate::evidence::manifest::require_manifest_rule;
use crate::json::{jq_matches, jq_value};

const M7_RECORDS: &[(&str, &str, &str)] = &[(
    "library-summary.json",
    r#".milestone == "M7"
        and .workPackage == "M7-W5"
        and .protocol == "m7-library-v1"
        and .result == "pass"
        and (.entries | type == "object")
        and ([.entries.total, .entries.accepted, .entries.blocked, .entries.unstarted]
             | all(type == "number"))
        and (.correction == null or (
            (.correction | type == "object")
            and ([.correction.from, .correction.total] | all(type == "number"))
            and (.correction.reason | type == "string" and length > 0)))
        and (.waves | type == "array" and length > 0)
        and (.waves | all(
            ([.index, .entries] | all(type == "number"))
            and (.dependsOn | type == "array")
            and (.dependsOn | all(type == "number"))))
        and ([.dependencies.resolved, .dependencies.unresolved,
              .registries.resolved, .registries.unresolved,
              .provenance.present, .provenance.missing,
              .routing.absorbed, .routing.remaining,
              .routing.runnableToday, .routing.blocked,
              .suites.packages.total, .suites.packages.failed,
              .suites.clusters.total, .suites.clusters.failed]
             | all(type == "number"))"#,
    "a library summary carrying the burn-up ledger as numbers, the waves with the waves each \
     depends on, any evidence correction with the figure it replaced and its reason, and the \
     dependency, registry, provenance, routing and suite figures M7-W5 audits",
)];

const M7_ARITHMETIC: &[(&str, &str)] = &[
    (
        ".entries.accepted + .entries.blocked + .entries.unstarted == .entries.total",
        "M7-W4's burn-up ledger asks that the accepted, blocked and unstarted totals equal the \
         verified fixture total",
    ),
    (
        ".entries.blocked == 0 and .entries.unstarted == 0",
        "a wave cannot close with failing or unclassified entries, so the library cannot close \
         with any",
    ),
    (
        ".entries.accepted == .entries.total",
        "M7-W5's audit is the whole fixture, and a counterpart absent from it is not present",
    ),
    (
        r#".entries.total == 324
            or ((.correction | type == "object") and .correction.total == .entries.total)"#,
        "M7-W5 audits 324 of 324 or an evidence-corrected total, and a different figure with no \
         correction beside it is neither",
    ),
    (
        ".dependencies.unresolved == 0",
        "M7-W5's gate row asks for zero unresolved dependencies",
    ),
    (
        ".registries.unresolved == 0",
        "M7-W5's gate row asks for zero unresolved registries",
    ),
    (
        ".provenance.present == .entries.total and .provenance.missing == 0",
        "M7-W5's gate row asks that all provenance be present",
    ),
    (
        "([.waves[].entries] | add) == .entries.total",
        "the waves the work was ordered by carry the whole library between them",
    ),
    (
        "([.waves[].index] | unique | length) == (.waves | length)",
        "two waves sharing an index are one wave counted twice, and order is what a wave is for",
    ),
    (
        ". as $record
            | .waves | all(.dependsOn | all(. as $dependency
                | ([$record.waves[].index] | index($dependency)) != null))",
        "M7-W1's graph closure proves no wave depends on an unavailable wave",
    ),
    (
        ".waves | all(. as $wave | .dependsOn | all(. < $wave.index))",
        "M7-W1's graph closure proves no wave depends on a later wave",
    ),
    (
        ".routing.absorbed + .routing.remaining == .entries.total",
        "M7-W1's routing counts, per entry, how much Ravel already ships and how much remains",
    ),
    (
        ".routing.runnableToday + .routing.blocked == .entries.total",
        "an entry is runnable today or it is blocked, and the two account for the library",
    ),
    (
        ".routing.blocked == 0",
        "a library whose entries cannot be run today is not the complete library M7 delivers",
    ),
    (
        ".suites.packages.total > 0 and .suites.packages.failed == 0",
        "M7-W6's gate row asks that all package suites pass, and a library running none passed \
         none",
    ),
    (
        ".suites.clusters.total > 0 and .suites.clusters.failed == 0",
        "M7-W6's gate row asks that all cluster suites pass, and a library running none passed \
         none",
    ),
];

const M7_CARRIED_FIGURES: &[(&str, &str)] = &[
    (
        ".environment.settings.library.entries",
        ".entries.accepted + .entries.blocked + .entries.unstarted",
    ),
    (
        ".environment.settings.library.accepted",
        ".entries.accepted",
    ),
    (".environment.settings.library.blocked", ".entries.blocked"),
    (
        ".environment.settings.library.unstarted",
        ".entries.unstarted",
    ),
    (".environment.settings.library.waves", ".waves | length"),
    (
        ".environment.settings.library.dependenciesUnresolved",
        ".dependencies.unresolved",
    ),
    (
        ".environment.settings.library.registriesUnresolved",
        ".registries.unresolved",
    ),
    (
        ".environment.settings.library.provenancePresent",
        ".provenance.present",
    ),
    (
        ".environment.settings.library.absorbed",
        ".routing.absorbed",
    ),
    (
        ".environment.settings.library.packageSuites",
        ".suites.packages.total",
    ),
    (
        ".environment.settings.library.clusterSuites",
        ".suites.clusters.total",
    ),
];

pub(crate) fn verify_m7_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M7-W0\", \"M7-W1\", \"M7-W2\", \"M7-W3\", \"M7-W4\", \"M7-W5\", \"M7-W6\"]",
        "M7 manifest must include every work package from M7-W0 through M7-W6 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m7-library-v1"
            and ([.benchmarks.results[].name] == ["library-summary.json"])"#,
        "M7 manifest must record its library summary under the m7-library-v1 protocol, so that \
         the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M7-W6") | .value]
                 | length == 6
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M7 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("official-atm11") != null)
        )"#,
        "M7 closure requires a native Linux attestation for the recorded library run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M7 closes a library of 324 counterparts, and an empty finding set satisfies every rule \
         phrased as a refusal",
        violations,
    );

    verify_closure_records(root, "M7", "evidence/m7", M7_RECORDS, violations);

    verify_m7_library_arithmetic(root, violations);
    verify_m7_verified_fixture_total(root, violations);
    verify_milestone_figures(root, manifest_path, &M7_FIGURES, violations);
}

fn verify_m7_library_arithmetic(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m7/library-summary.json");
    if !record.is_file() {
        return;
    }
    verify_record_arithmetic(
        &record,
        "evidence/m7/library-summary.json",
        M7_ARITHMETIC,
        violations,
    );
}

fn verify_m7_verified_fixture_total(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m7/library-summary.json");
    let inventory = root.join("evidence/m5/inventory-summary.json");
    if !record.is_file() || !inventory.is_file() {
        return;
    }
    let (Ok(audited), Ok(verified)) = (
        jq_value(&record, ".entries.total"),
        jq_value(&inventory, ".artifacts.total"),
    ) else {
        violations.push(
            "evidence/m7/library-summary.json `.entries.total` and \
             evidence/m5/inventory-summary.json `.artifacts.total` cannot both be read, so \
             nothing holds M7's denominator to the inventory that measured it"
                .to_owned(),
        );
        return;
    };
    if audited == verified {
        return;
    }
    let corrects_the_inventory = jq_matches(
        &record,
        &format!(
            "(.correction | type == \"object\") and (.correction.from == {verified}) \
             and (.correction.total == .entries.total)"
        ),
    )
    .unwrap_or(false);
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

#[cfg(test)]
mod tests;
