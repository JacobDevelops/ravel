//! M2 closure rules.

use std::path::Path;

use crate::evidence::closure::verify_closure_records;
use crate::evidence::manifest::require_manifest_rule;
use crate::inventory::VOID_LIMITS;

pub(crate) fn verify_m2_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M2-W1\", \"M2-W2\", \"M2-W3\", \"M2-W4\", \"M2-W5\", \"M2-W6\", \"M2-W7\", \"M2-W8\"]",
        "M2 manifest must include every work package from M2-W1 through M2-W8 exactly once",
        violations,
    );

    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m2-loader-budget-v1"
            and ([.benchmarks.results[].name] | sort
                == ["loader-overhead.json", "noop-pipeline.json",
                    "pre-world-gate.json", "prism-launch.json"])"#,
        "M2 manifest must record all four M2 results under the m2-loader-budget-v1 protocol, so \
         that each one is covered by a recomputed digest",
        violations,
    );

    require_manifest_rule(
        manifest_path,
        r#".attestations | any(.platform == "linux" and .architecture == "x86_64")"#,
        "M2 closure requires a native Linux attestation for the operator-run launch",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M2 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    for (metric, why) in VOID_LIMITS {
        println!(
            "- evidence/m2/manifest.json: the budget limit {metric} is checked for shape and not \
             for meaning, because it is {why}"
        );
    }

    verify_closure_records(root, "M2", "evidence/m2", M2_RECORDS, violations);
}

const M2_RECORDS: &[(&str, &str, &str)] = &[
    (
        "noop-pipeline.json",
        ".runs | length > 0 and all(.repeatedRunIsIdentical == true)",
        "a no-op pipeline record whose run is identical when repeated",
    ),
    (
        "pre-world-gate.json",
        ".gate.refusesWorldAccess == true and .gate.ranModCode == false",
        "a pre-world gate record showing invalid packages refused before any mod code ran",
    ),
    (
        "loader-overhead.json",
        r#".budget.limits
            | length > 0
            and all(
                (.metric | type == "string" and length > 0)
                and (.unit | type == "string" and length > 0)
                and .operator == "less-than-or-equal"
                and (.measured | type == "number")
                and .measured > 0
                and (.limit | type == "number")
                and .limit > .measured
            )
            and (map(.metric) | unique | length) == length"#,
        "a loader overhead record whose budget lines each name one metric and a unit, and set an \
         upper limit above the positive measurement that limit was derived from",
    ),
    (
        "prism-launch.json",
        r#".launch.provesStockLaunch == true
            and (.launch | has("objection"))
            and .launch.objection == null
            and .launch.attestation.stockLauncher == true
            and .ravel.reachedGameProcess == true
            and .ravel.rewrite.reported == true
            and .ravel.rewrite.reachedRavel == true
            and (.ravel.rewrite.classesRewritten | type == "number")
            and .ravel.rewrite.classesRewritten > 0
            and (.ravel.rewrite.methodsRewritten | type == "number")
            and .ravel.rewrite.methodsRewritten > 0
            and (.ravel.rewrite.nativeInvocations | type == "number")
            and .ravel.rewrite.nativeInvocations > 0
            and .instance.withoutAnotherLoader == true"#,
        "a launch record showing the official client reaching Ravel from an attested stock Prism, \
         with the transformer's own count of what it rewrote inside that process and of the hook \
         invocations that reached native code, and no other loader present",
    ),
];
