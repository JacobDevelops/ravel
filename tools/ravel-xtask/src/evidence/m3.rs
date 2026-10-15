//! M3 closure rules.

use std::path::Path;

use crate::evidence::closure::verify_closure_records;
use crate::evidence::manifest::require_manifest_rule;

pub(crate) fn verify_m3_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M3-W0\", \"M3-W1\", \"M3-W2\", \"M3-W3\", \"M3-W4\", \"M3-W5\", \"M3-W6\", \"M3-W7\", \"M3-W8\"]",
        "M3 manifest must include every work package from M3-W0 through M3-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m3-prototype-v1"
            and ([.benchmarks.results[].name] | sort
                == ["class-loader-identity.json", "linux-process-matrix.json",
                    "pack-compile.json", "profiling-overhead.json"])"#,
        "M3 manifest must record all four M3 results under the m3-prototype-v1 protocol, so that \
         each one is covered by a recomputed digest",
        violations,
    );

    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M3-W8") | .value]
                 | length == 8
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M3 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );

    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("ravel") != null)
        )"#,
        "M3 closure requires a native Linux attestation for the operator-run Ravel launch",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M3 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    verify_closure_records(root, "M3", "evidence/m3", M3_RECORDS, violations);
}

const M3_RECORDS: &[(&str, &str, &str)] = &[
    (
        "class-loader-identity.json",
        r#".milestone == "M3"
            and .observed.sameObject == true
            and .conclusion.candidateTwoIsAvailable == true
            and (.rewrite.classesRewritten | type == "number")
            and .rewrite.classesRewritten > 0
            and (.rewrite.methodsRewritten | type == "number")
            and .rewrite.methodsRewritten > 0
            and .rewrite.reachedNativeCode == true
            and (.rewrite.nativeInvocations | type == "number")
            and .rewrite.nativeInvocations > 0"#,
        "a class-loader identity record showing the loader that resolved a rewritten game class \
         is the system loader, measured in a launch that rewrote classes and reached native code",
    ),
    (
        "pack-compile.json",
        r#".milestone == "M3"
            and .gate == "ADG-2"
            and .protocol == "m3-pack-compile-v1"
            and (.compile.repeats | type == "number")
            and .compile.repeats > 0
            and (.compile.baselineMilliseconds | type == "number")
            and .compile.baselineMilliseconds > 0
            and (.compile.marginalColdMillisecondsPerMod | type == "number")
            and .compile.marginalColdMillisecondsPerMod > 0
            and (.disk.toolchainBytes | type == "number")
            and .disk.toolchainBytes > 0"#,
        "a pack-compile record pricing ADG-2's source distribution with a repeated measurement, \
         a positive marginal per-mod cost, and the toolchain size the decision accepted",
    ),
    (
        "profiling-overhead.json",
        r#".milestone == "M3"
            and .workPackage == "M3-W6"
            and .budget.protocol == "m2-loader-budget-v1"
            and .budget.hardwareProfile == "linux-reference-v1"
            and (.budget.limits | any(
                .metric == "profiling.off.perTick"
                and .operator == "less-than-or-equal"
                and (.measured | type == "number")
                and (.limit | type == "number")
                and .measured > 0
                and .measured <= .limit))
            and (.collectors.attributionAccuracy | type == "array" and length > 0)
            and (.collectors.unsupported | type == "array")"#,
        "a profiling record whose off-tick measurement comes in under the limit M3-W6 fixed, with \
         per-mod attribution accuracy stated and the unsupported metrics named",
    ),
    (
        "linux-process-matrix.json",
        r#".milestone == "M3"
            and .workPackage == "M3-W7"
            and .result == "pass"
            and .instance.withoutAnotherLoader == true
            and .instance.stockComponentsExact == true
            and .instance.otherLoaderComponents == 0
            and .instance.modJars == 0
            and .instance.recognizedLoaderModJars == 0
            and (.rewrite.classesRewritten | type == "number")
            and .rewrite.classesRewritten > 0
            and .rewrite.transformationFailures == 0
            and (.rewrite.nativeInvocations | type == "number")
            and .rewrite.nativeInvocations > 0
            and .screens.titleMenuButtonPresent == true
            and .screens.allThreeOpenedInThisCampaign == true
            and .screens.settings.persistenceRoundTrip == true
            and .commandAndSavedData.vanillaCommandExecutedThroughInput == true
            and .commandAndSavedData.relaunchVerifiedExactValue == true
            and .worldPersistenceRoundTrip.createSaveExitReload
                .aggregateByteIdenticalAcrossTheRoundTrip == true
            and .worldPersistenceRoundTrip.failClosedOnAnUnreadableWorld.worldRefused == true
            and .worldPersistenceRoundTrip.failClosedOnAnUnreadableWorld
                .integratedServerNeverStarted == true"#,
        "a Linux process-matrix record showing every M3 surface passing natively in an instance \
         the scan proves holds no other loader, with the world round trip preserving bytes and \
         refusing a world it cannot read",
    ),
];

#[cfg(test)]
mod tests;
