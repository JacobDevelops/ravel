//! M3 closure rules.

use crate::evidence::closure::{LoadedRecords, RecordDefinition, read_records};
use crate::evidence::rules::Milestone;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::evidence::closure::{verify_closure_records, verify_manifest_closure};
use crate::evidence::rules::{
    Rule, all_gates_closed, has_exact_benchmark_results, has_exact_work_packages,
    has_native_attestation, require,
};
use crate::json::array;
use crate::text::read_text;
use crate::text::{NumberAt, numbers_before, stated_counts, verify_stated_counts};

use crate::evidence::model::admit;

mod model;

const CAMPAIGN_MATRIX: &str = "evidence/m3/linux-process-matrix.json";
const CAMPAIGN_DOCUMENT: &str = "evidence/m3/README.md";
const PERSISTENCE_HARNESS: &str = "jvm/harness/run-harness.sh";

pub(crate) fn verify_m3_integrity(
    root: &Path,
    _: &Value,
    violations: &mut Vec<String>,
) -> LoadedRecords {
    let records = read_records(root, Milestone::M3, M3_RECORDS, violations);
    if let Some(record) = records.get("linux-process-matrix.json") {
        verify_campaign_inventory(root, record, violations);
    }
    verify_harness_lifecycles(root, &records, violations);
    records
}

const M3_MANIFEST_RULES: &[(Rule, &str)] = &[
    (
        |value| {
            has_exact_work_packages(
                value,
                &[
                    "M3-W0", "M3-W1", "M3-W2", "M3-W3", "M3-W4", "M3-W5", "M3-W6", "M3-W7", "M3-W8",
                ],
            )
        },
        "M3 manifest must include every work package from M3-W0 through M3-W8 exactly once",
    ),
    (
        |value| {
            has_exact_benchmark_results(
                value,
                "m3-prototype-v1",
                &[
                    "class-loader-identity.json",
                    "linux-process-matrix.json",
                    "pack-compile.json",
                    "profiling-overhead.json",
                ],
            )
        },
        "M3 manifest must record all four M3 results under the m3-prototype-v1 protocol, so that each one is covered by a recomputed digest",
    ),
    (
        |value| all_gates_closed(value, "M3-W8", 8),
        "M3 closure requires a gateStatus row for every work package, each implementation row closed and naming its evidence",
    ),
    (
        |value| has_native_attestation(value, "ravel"),
        "M3 closure requires a native Linux attestation for the operator-run Ravel launch",
    ),
    (
        |value| !array(value, "/deviations").is_empty() && !array(value, "/risks").is_empty(),
        "M3 recorded deviations and risks, and an empty finding set satisfies every rule phrased as a refusal, so closure requires them to still be there",
    ),
];

pub(crate) fn verify_m3_manifest(
    manifest: &Value,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    verify_manifest_closure(manifest, M3_MANIFEST_RULES, violations);
    verify_closure_records(Milestone::M3, records, M3_RECORDS, violations);
}

const M3_RECORDS: &[RecordDefinition] = &[
    RecordDefinition {
        name: "class-loader-identity.json",
        rule: |value| {
            let record: model::Identity = admit(value)?;
            require(record.observed.same_object, "/observed/sameObject")?;
            require(
                record.conclusion.candidate_two_is_available,
                "/conclusion/candidateTwoIsAvailable",
            )?;
            for (count, requirement) in [
                (
                    &record.rewrite.classes_rewritten,
                    "/rewrite/classesRewritten",
                ),
                (
                    &record.rewrite.methods_rewritten,
                    "/rewrite/methodsRewritten",
                ),
                (
                    &record.rewrite.native_invocations,
                    "/rewrite/nativeInvocations",
                ),
            ] {
                require(count.as_f64().is_some_and(|count| count > 0.0), requirement)?;
            }
            require(
                record.rewrite.reached_native_code,
                "/rewrite/reachedNativeCode",
            )
        },
        description: "a class-loader identity record showing the loader that resolved a rewritten game class \
     is the system loader, measured in a launch that rewrote classes and reached native code",
        required: false,
    },
    RecordDefinition {
        name: "pack-compile.json",
        rule: |value| {
            let record: model::PackCompile<'_> = admit(value)?;
            crate::evidence::model::require_identity(record.gate.0, "ADG-2", "/gate")?;
            crate::evidence::model::require_identity(
                record.protocol.0,
                "m3-pack-compile-v1",
                "/protocol",
            )?;
            for (count, requirement) in [
                (&record.compile.repeats, "/compile/repeats"),
                (
                    &record.compile.baseline_milliseconds,
                    "/compile/baselineMilliseconds",
                ),
                (
                    &record.compile.marginal_cold_milliseconds_per_mod,
                    "/compile/marginalColdMillisecondsPerMod",
                ),
                (&record.disk.toolchain_bytes, "/disk/toolchainBytes"),
            ] {
                require(count.as_f64().is_some_and(|count| count > 0.0), requirement)?;
            }
            Ok(())
        },
        description: "a pack-compile record pricing ADG-2's source distribution with a repeated measurement, \
     a positive marginal per-mod cost, and the toolchain size the decision accepted",
        required: false,
    },
    RecordDefinition {
        name: "profiling-overhead.json",
        rule: |value| {
            let record: model::Profiling<'_> = admit(value)?;
            crate::evidence::model::require_identity(
                record.work_package.0,
                "M3-W6",
                "/workPackage",
            )?;
            crate::evidence::model::require_identity(
                record.budget.protocol.0,
                "m2-loader-budget-v1",
                "/budget/protocol",
            )?;
            crate::evidence::model::require_identity(
                record.budget.hardware_profile.0,
                "linux-reference-v1",
                "/budget/hardwareProfile",
            )?;
            verify_profiling_off_tick_limit(value)?;
            Ok(())
        },
        description: "a profiling record whose off-tick measurement comes in under the limit M3-W6 fixed, with \
     per-mod attribution accuracy stated and the unsupported metrics named",
        required: false,
    },
    RecordDefinition {
        name: "linux-process-matrix.json",
        rule: verify_m3_process_matrix,
        description: "a Linux process-matrix record showing every M3 surface passing natively in an instance \
     the scan proves holds no other loader, with the world round trip preserving bytes and \
     refusing a world it cannot read",
        required: true,
    },
];

fn verify_m3_process_matrix(value: &Value) -> Result<(), String> {
    let record: model::ProcessMatrix<'_> = admit(value)?;
    crate::evidence::model::require_identity(record.work_package.0, "M3-W7", "/workPackage")?;
    require(
        record.result == crate::evidence::model::Verdict::Pass,
        "/result must be pass",
    )?;
    verify_m3_process_flags(&record)?;
    for (count, requirement) in [
        (
            &record.instance.other_loader_components,
            "/instance/otherLoaderComponents",
        ),
        (&record.instance.mod_jars, "/instance/modJars"),
        (
            &record.instance.recognized_loader_mod_jars,
            "/instance/recognizedLoaderModJars",
        ),
        (
            &record.rewrite.transformation_failures,
            "/rewrite/transformationFailures",
        ),
    ] {
        require(count.as_f64() == Some(0.0), requirement)?;
    }
    for (count, requirement) in [
        (
            &record.rewrite.classes_rewritten,
            "/rewrite/classesRewritten",
        ),
        (
            &record.rewrite.native_invocations,
            "/rewrite/nativeInvocations",
        ),
    ] {
        require(count.as_f64().is_some_and(|count| count > 0.0), requirement)?;
    }
    Ok(())
}

fn verify_m3_process_flags(record: &model::ProcessMatrix<'_>) -> Result<(), String> {
    let persistence = &record.world_persistence_round_trip;
    for (holds, requirement) in [
        (
            record.instance.without_another_loader,
            "/instance/withoutAnotherLoader",
        ),
        (
            record.instance.stock_components_exact,
            "/instance/stockComponentsExact",
        ),
        (
            record.screens.title_menu_button_present,
            "/screens/titleMenuButtonPresent",
        ),
        (
            record.screens.all_three_opened_in_this_campaign,
            "/screens/allThreeOpenedInThisCampaign",
        ),
        (
            record.screens.settings.persistence_round_trip,
            "/screens/settings/persistenceRoundTrip",
        ),
        (
            record
                .command_and_saved_data
                .vanilla_command_executed_through_input,
            "/commandAndSavedData/vanillaCommandExecutedThroughInput",
        ),
        (
            record.command_and_saved_data.relaunch_verified_exact_value,
            "/commandAndSavedData/relaunchVerifiedExactValue",
        ),
        (
            persistence
                .create_save_exit_reload
                .aggregate_byte_identical_across_the_round_trip,
            "/worldPersistenceRoundTrip/createSaveExitReload/aggregateByteIdenticalAcrossTheRoundTrip",
        ),
        (
            persistence.fail_closed_on_an_unreadable_world.world_refused,
            "/worldPersistenceRoundTrip/failClosedOnAnUnreadableWorld/worldRefused",
        ),
        (
            persistence
                .fail_closed_on_an_unreadable_world
                .integrated_server_never_started,
            "/worldPersistenceRoundTrip/failClosedOnAnUnreadableWorld/integratedServerNeverStarted",
        ),
    ] {
        require(holds, requirement)?;
    }
    Ok(())
}
fn verify_profiling_off_tick_limit(value: &Value) -> Result<(), String> {
    let mut failure = Err("/budget/limits must contain metric profiling.off.perTick".to_owned());
    for (index, limit) in array(value, "/budget/limits")
        .iter()
        .enumerate()
        .filter(|(_, limit)| limit["metric"] == "profiling.off.perTick")
    {
        let checked = (|| {
            let limit: model::ProfilingLimit = admit(limit)?;
            require(
                limit
                    .measured
                    .as_f64()
                    .is_some_and(|measured| measured > 0.0),
                "/measured must be positive",
            )?;
            require(
                limit
                    .measured
                    .as_f64()
                    .zip(limit.limit.as_f64())
                    .is_some_and(|(measured, bound)| measured <= bound),
                "/limit must not be below measured",
            )
        })();
        if checked.is_ok() {
            return Ok(());
        }
        failure = checked.map_err(|requirement| format!("/budget/limits/{index}{requirement}"));
    }
    failure
}

struct CampaignInventory {
    classes: usize,
    targets: usize,
    prior: usize,
    launches: usize,
}

pub(crate) fn verify_campaign_inventory(root: &Path, matrix: &Value, violations: &mut Vec<String>) {
    let Some(derived) = campaign_inventory(matrix, violations) else {
        return;
    };

    for (field, expected, subject) in [
        (
            "classesInInventory",
            derived.classes,
            "classes its own target inventory enumerates",
        ),
        (
            "callableTargetsInInventory",
            derived.targets,
            "callable targets its own target inventory enumerates",
        ),
    ] {
        match matrix_count(matrix, &format!("/rewrite/{field}")) {
            Ok(recorded) if recorded == expected => {}
            Ok(recorded) => violations.push(format!(
                "{CAMPAIGN_MATRIX} records {field} {recorded} against the {expected} {subject}"
            )),
            Err(error) => violations.push(error),
        }
    }

    match read_text(root, Path::new(CAMPAIGN_DOCUMENT)) {
        Ok(document) => violations.extend(campaign_document_violations(&document, &derived)),
        Err(error) => violations.push(error),
    }
}

fn campaign_inventory(matrix: &Value, violations: &mut Vec<String>) -> Option<CampaignInventory> {
    let Some(targets) = matrix
        .pointer("/rewrite/targetInventory")
        .and_then(Value::as_array)
    else {
        violations.push(format!(
            "{CAMPAIGN_MATRIX} does not record a target inventory array"
        ));
        return None;
    };
    let mut classes = BTreeSet::new();
    let mut methods = BTreeSet::new();
    for target in targets {
        let Some(class) = target["class"].as_str() else {
            violations.push(format!(
                "{CAMPAIGN_MATRIX} records a target without a class string"
            ));
            return None;
        };
        if !classes.insert(class) {
            violations.push(format!(
                "{CAMPAIGN_MATRIX}'s target inventory lists a class more than once"
            ));
            return None;
        }
        let Some(entries) = target["methods"].as_array() else {
            violations.push(format!(
                "{CAMPAIGN_MATRIX} records a target without a methods array"
            ));
            return None;
        };
        for method in entries {
            let Some(method) = method.as_str() else {
                violations.push(format!(
                    "{CAMPAIGN_MATRIX} records a non-string callable target"
                ));
                return None;
            };
            if !methods.insert(method) {
                violations.push(format!(
                    "{CAMPAIGN_MATRIX}'s target inventory lists a callable target more than once"
                ));
                return None;
            }
        }
    }
    match (
        matrix_count(matrix, "/matrixNotCarriedForward/priorCallableTargets"),
        matrix_count(matrix, "/surfaceCampaign/launches"),
    ) {
        (Ok(prior), Ok(launches)) => Some(CampaignInventory {
            classes: classes.len(),
            targets: methods.len(),
            prior,
            launches,
        }),
        (Err(error), _) | (_, Err(error)) => {
            violations.push(error);
            None
        }
    }
}

fn matrix_count(matrix: &Value, pointer: &str) -> Result<usize, String> {
    matrix
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("{CAMPAIGN_MATRIX} does not record a whole number at `{pointer}`"))
}

#[derive(Clone, Copy)]
enum CampaignCountMatcher {
    Phrase(&'static str, NumberAt),
    Words(&'static [&'static str]),
}

impl CampaignCountMatcher {
    fn label(self) -> Cow<'static, str> {
        match self {
            Self::Phrase(phrase, _) => Cow::Borrowed(phrase),
            Self::Words(words) => Cow::Owned(words.join(" ")),
        }
    }
}

fn campaign_document_violations(document: &str, derived: &CampaignInventory) -> Vec<String> {
    let mut violations = Vec::new();
    let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
    let document = document.as_str();
    for (matcher, expected, subject, absent_subject) in [
        (
            CampaignCountMatcher::Phrase(" callable targets", NumberAt::BeforePhrase),
            derived.targets,
            "callable targets",
            None,
        ),
        (
            CampaignCountMatcher::Phrase(
                " classes in the reviewed inventory",
                NumberAt::BeforePhrase,
            ),
            derived.classes,
            "classes in the reviewed inventory",
            None,
        ),
        (
            CampaignCountMatcher::Phrase("moved from ", NumberAt::AfterPhrase),
            derived.prior,
            "callable targets before the count moved",
            None,
        ),
        (
            CampaignCountMatcher::Words(&["surface", "launches"]),
            derived.launches,
            "surface launches",
            Some("launches in the surface campaign"),
        ),
    ] {
        let stated = match matcher {
            CampaignCountMatcher::Phrase(phrase, side) => stated_counts(document, phrase, side),
            CampaignCountMatcher::Words(words) => numbers_before(document, words),
        };
        verify_stated_counts(
            &stated,
            expected,
            || {
                let phrase = matcher.label();
                let absent_subject = absent_subject.unwrap_or(subject);
                format!(
                    "{CAMPAIGN_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it \
                     is held to {CAMPAIGN_MATRIX}'s {expected} {absent_subject}"
                )
            },
            |count| {
                format!(
                    "{CAMPAIGN_DOCUMENT} states {count} {subject} where {CAMPAIGN_MATRIX} records \
                 {expected}"
                )
            },
            &mut violations,
        );
    }
    violations
}
pub(crate) fn verify_harness_lifecycles(
    root: &Path,
    records: &LoadedRecords,
    violations: &mut Vec<String>,
) {
    let harness = match read_text(root, Path::new(PERSISTENCE_HARNESS)) {
        Ok(harness) => harness,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let declared = harness
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("persistence_lifecycles_declared="))
        .and_then(|value| value.parse::<usize>().ok());
    let Some(declared) = declared else {
        violations.push(format!(
            "{PERSISTENCE_HARNESS} no longer declares `persistence_lifecycles_declared`, so no \
             record's world-lifecycle count is held to it"
        ));
        return;
    };

    match read_text(root, Path::new(CAMPAIGN_DOCUMENT)) {
        Ok(text) => verify_lifecycle_counts(
            CAMPAIGN_DOCUMENT,
            &numbers_before(&text, &["world", "lifecycles"]),
            declared,
            violations,
        ),
        Err(error) => violations.push(error),
    }
    if let Some(record) = records.get("linux-process-matrix.json") {
        verify_lifecycle_counts(
            CAMPAIGN_MATRIX,
            &matrix_lifecycle_counts(record),
            declared,
            violations,
        );
    }
}

fn verify_lifecycle_counts(
    relative: &str,
    stated: &[usize],
    declared: usize,
    violations: &mut Vec<String>,
) {
    verify_stated_counts(
        stated,
        declared,
        || {
            format!(
                "{relative} no longer writes a count against `world lifecycles`, so \
             nothing in it is held to {PERSISTENCE_HARNESS}'s {declared}"
            )
        },
        |count| {
            format!(
                "{relative} states {count} world lifecycles where \
             {PERSISTENCE_HARNESS} declares {declared}"
            )
        },
        violations,
    );
}

fn matrix_lifecycle_counts(record: &Value) -> Vec<usize> {
    let mut counts = Vec::new();
    collect_lifecycle_counts(record, &mut counts);
    counts
}

fn collect_lifecycle_counts(value: &Value, counts: &mut Vec<usize>) {
    match value {
        Value::String(text) => counts.extend(numbers_before(text, &["world", "lifecycles"])),
        Value::Array(values) => {
            for value in values {
                collect_lifecycle_counts(value, counts);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_lifecycle_counts(value, counts);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests;
