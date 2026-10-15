//! M5 closure rules.

use std::path::Path;

use crate::evidence::closure::{
    CarriedFigures, verify_closure_records, verify_milestone_figures, verify_record_arithmetic,
};
use crate::evidence::manifest::require_manifest_rule;
use crate::json::jq_matches;

const M5_RECORDS: &[(&str, &str, &str)] = &[(
    "inventory-summary.json",
    r#".milestone == "M5"
        and .workPackage == "M5-W8"
        and .protocol == "m5-inventory-v1"
        and (.result == "pass" or .result == "provisional")
        and .disclosure.readObservation.bound != null
        and (.disclosure.readObservation.bound | type == "string" and length > 0)
        and (.disclosure.judgedClaims | type == "array" and length > 0)
        and (.disclosure.judgedClaims | all(
            (.claim | type == "string" and length > 0)
            and (.basis == "judged")
            and (.decidedBy | type == "string" and length > 0)))
        and (.artifacts | type == "object")
        and ([.artifacts.total, .artifacts.catalogued, .artifacts.uncatalogued,
              .artifacts.planned]
             | all(type == "number"))
        and (.graph | type == "object")
        and ([.graph.nodes, .graph.resolvedNodes, .graph.blockedNodes, .graph.edges,
              .graph.resolvedEdges, .graph.blockedEdges, .graph.clusters,
              .graph.largestCluster]
             | all(type == "number"))
        and (.rights | type == "object")
        and ([.rights.sourceUse.permitted, .rights.sourceUse.conditional,
              .rights.sourceUse.prohibited, .rights.sourceUse.unresolved,
              .rights.publishable]
             | all(type == "number"))
        and (.surfaces | type == "object")
        and ([.surfaces.classFiles, .surfaces.models, .surfaces.textures,
              .surfaces.recipes, .surfaces.catalogued, .surfaces.unclassified,
              .surfaces.nestedJars]
             | all(type == "number"))
        and (.plans | type == "object")
        and ([.plans.planned, .plans.readingSource, .plans.notReadingSource,
              .plans.dependencyEdges, .plans.canonicalIds]
             | all(type == "number"))
        and (.reads | type == "object")
        and ([.reads.producersWatched, .reads.archiveReads, .reads.entryBodyReads]
             | all(type == "number"))
        and (.boundArtifacts | type == "array" and length > 0)
        and (.boundArtifacts | all(
            (.name | type == "string" and length > 0)
            and (.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
            and (.holds | type == "string" and length > 0)
            and (.class == "material" or .class == "machinery")))
        and ((.boundArtifacts | map(.name) | unique | length)
             == (.boundArtifacts | length))
        and (.primitiveScan.class == "machinery")
        and (.machineryAmendments | type == "array")
        and (.machineryAmendments | all(
            (.milestone | type == "string" and length > 0)
            and (.reason | type == "string" and length > 0)
            and (.entries | type == "array" and length > 0)
            and (.figures | type == "array")))
        and ((.boundArtifacts | map(select(.class == "machinery") | .name)) as $machinery
             | .machineryAmendments | all(.entries | all(. as $e | $machinery | index($e) != null)))"#,
    "an inventory summary carrying every aggregate as a number, a digest over every \
     private file it binds with the class that digest freezes, its trail of machinery \
     amendments, and its own disclosure of what the read observation watches and \
     which of its claims rest on judgement",
)];

pub(crate) fn verify_m5_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M5-W1\", \"M5-W2\", \"M5-W3\", \"M5-W4\", \"M5-W5\", \"M5-W6\", \"M5-W7\", \"M5-W8\"]",
        "M5 manifest must include every work package from M5-W1 through M5-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m5-inventory-v1"
            and ([.benchmarks.results[].name] == ["inventory-summary.json"])"#,
        "M5 manifest must record its inventory summary under the m5-inventory-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M5-W8") | .value]
                 | length == 7
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M5 closure requires a gateStatus row for every work package, each implementation row \
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
        "M5 closure requires a native Linux attestation for the recorded inventory run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M5 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    let record = root.join("evidence/m5/inventory-summary.json");
    if record.is_file() && jq_matches(&record, r#".result == "provisional""#).unwrap_or(false) {
        violations.push(
            "M5 closure requires final figures: evidence/m5/inventory-summary.json records itself \
             as provisional, which is a true statement about an inventory under repair and not \
             something a milestone can close over"
                .to_owned(),
        );
    }

    verify_closure_records(root, "M5", "evidence/m5", M5_RECORDS, violations);

    verify_m5_inventory_arithmetic(root, violations);
    verify_milestone_figures(root, manifest_path, &M5_FIGURES, violations);
}

const M5_ARITHMETIC: &[(&str, &str)] = &[
    (
        ".artifacts.catalogued + .artifacts.uncatalogued == .artifacts.total",
        "every artifact is either catalogued or not",
    ),
    (
        ".artifacts.planned == .artifacts.total and .artifacts.uncatalogued == 0",
        "M5-W8's gate row asks that every artifact carry a complete plan",
    ),
    (
        ".plans.planned == .artifacts.total",
        "M5-W8's gate row asks for a plan per catalogued artifact",
    ),
    (
        ".plans.readingSource + .plans.notReadingSource == .artifacts.total",
        "a plan either reads source or does not",
    ),
    (
        ".plans.canonicalIds == .artifacts.total",
        "M5-W6 assigns one canonical ID per entry",
    ),
    (
        ".rights.sourceUse.permitted + .rights.sourceUse.conditional
            + .rights.sourceUse.prohibited + .rights.sourceUse.unresolved
            == .artifacts.total",
        "every triaged artifact carries exactly one source-use verdict",
    ),
    (
        ".surfaces.catalogued + .surfaces.unclassified == .artifacts.total",
        "every acquired artifact is catalogued or is counted as unclassified",
    ),
    (
        ".graph.resolvedNodes + .graph.blockedNodes == .graph.nodes",
        "every node resolves or carries a blocking classification",
    ),
    (
        ".graph.resolvedEdges + .graph.blockedEdges == .graph.edges",
        "every edge resolves or carries a blocking classification",
    ),
    (
        ".graph.clusters > 0 and .graph.largestCluster > 0
            and .graph.largestCluster <= .graph.nodes",
        "a cluster is a set of nodes, so the largest cannot exceed the graph",
    ),
    (
        ".reads.producersWatched > 0 and .reads.archiveReads > 0",
        "an observation that classified nothing observed nothing",
    ),
    (
        ".reads.entryBodyReads == 0",
        "reading an entry body is what the metadata-only method excludes",
    ),
];

pub(crate) fn verify_m5_inventory_arithmetic(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m5/inventory-summary.json");
    if !record.is_file() {
        return;
    }
    verify_record_arithmetic(
        &record,
        "evidence/m5/inventory-summary.json",
        M5_ARITHMETIC,
        violations,
    );
}

const M5_CARRIED_FIGURES: &[(&str, &str)] = &[
    (
        ".environment.settings.inventory.artifacts",
        ".artifacts.catalogued + .artifacts.uncatalogued",
    ),
    (
        ".environment.settings.inventory.planned",
        ".artifacts.planned",
    ),
    (
        ".environment.settings.inventory.graphNodes",
        ".graph.resolvedNodes + .graph.blockedNodes",
    ),
    (
        ".environment.settings.inventory.graphEdges",
        ".graph.resolvedEdges + .graph.blockedEdges",
    ),
    (
        ".environment.settings.inventory.clusters",
        ".graph.clusters",
    ),
    (
        ".environment.settings.inventory.sourceProhibited",
        ".rights.sourceUse.prohibited",
    ),
    (
        ".environment.settings.inventory.sourceUnresolved",
        ".rights.sourceUse.unresolved",
    ),
    (
        ".environment.settings.inventory.publishable",
        ".rights.publishable",
    ),
    (
        ".environment.settings.inventory.readingSource",
        ".plans.readingSource",
    ),
    (
        ".environment.settings.inventory.dependencyEdges",
        ".plans.dependencyEdges",
    ),
    (
        ".environment.settings.inventory.archiveReads",
        ".reads.archiveReads",
    ),
    (
        ".environment.settings.inventory.entryBodyReads",
        ".reads.entryBodyReads",
    ),
    (
        ".environment.settings.inventory.digestsBound",
        ".boundArtifacts | length",
    ),
];

pub(crate) const M5_FIGURES: CarriedFigures = CarriedFigures {
    manifest: "evidence/m5/manifest.json",
    record: "evidence/m5/inventory-summary.json",
    subject: "inventory",
    figures: M5_CARRIED_FIGURES,
};

#[cfg(test)]
mod tests;
