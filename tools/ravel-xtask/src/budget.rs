//! Budget schema and measured-limit comparisons.

use std::path::Path;

use crate::json::{jq_value, redacted_path};

pub(crate) const VOID_LIMITS: &[(&str, &str)] = &[
    (
        "build.packCompile",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; the build cost of source distribution is evidence/m3/pack-compile.json",
    ),
    (
        "build.relink",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; nothing relinks a pack under the selected mechanism",
    ),
    (
        "build.binaryGrowthPerMod",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; per-mod binary growth describes an artefact the mechanism does not produce",
    ),
];

const BUDGET_PROTOCOL: &str = "m2-loader-budget-v1";

pub(crate) const BUDGET_RECORDS: &[&str] = &[
    "evidence/m2/loader-overhead.json",
    "evidence/m3/profiling-overhead.json",
];

/// What comparing a run against the regression budget found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BudgetReport {
    pub violations: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Limit {
    metric: String,
    ceiling: String,
    measured: String,
    operator: String,
}

/// Compares the `candidates`' measurements against the recorded regression
/// budget.
#[must_use]
pub fn verify_budget(root: &Path, candidates: &[&Path]) -> BudgetReport {
    let mut report = BudgetReport::default();
    if candidates.is_empty() {
        report.violations.push(
            "no candidate run was given, and a budget compared against nothing passes \
                   vacuously"
                .to_owned(),
        );
        return report;
    }

    let owned = BUDGET_RECORDS
        .iter()
        .map(|record| root.join(record))
        .collect::<Vec<_>>();
    let baselines = owned
        .iter()
        .map(std::path::PathBuf::as_path)
        .collect::<Vec<_>>();

    for (path, label) in baselines
        .iter()
        .map(|path| (*path, "a budget record"))
        .chain(candidates.iter().map(|path| (*path, "a candidate run")))
    {
        match jq_value(path, ".budget.protocol") {
            Ok(protocol) if protocol == BUDGET_PROTOCOL => {}
            Ok(protocol) => report.violations.push(format!(
                "{label}, {}, declares budget protocol `{protocol}`, and this comparison \
                 understands only `{BUDGET_PROTOCOL}`",
                redacted_path(path)
            )),
            Err(error) => report.violations.push(error),
        }
    }
    if !report.violations.is_empty() {
        return report;
    }

    let required = match hardware_profile(&baselines) {
        Ok(profile) => profile,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    for candidate in candidates {
        match jq_value(candidate, ".identity.hardwareProfile") {
            Ok(actual) if actual == required => {}
            Ok(actual) => report.violations.push(format!(
                "the budget holds on hardware profile `{required}` and {} was measured on \
                 `{actual}`, so it is remeasured rather than compared",
                redacted_path(candidate)
            )),
            Err(error) => report.violations.push(error),
        }
    }
    if !report.violations.is_empty() {
        return report;
    }

    let baseline_limits = match union(&baselines, "budget records") {
        Ok(limits) => limits,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    let candidate_limits = match union(candidates, "candidate runs") {
        Ok(limits) => limits,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };

    for limit in &baseline_limits {
        if let Some((_, reason)) = VOID_LIMITS
            .iter()
            .find(|(metric, _)| *metric == limit.metric)
        {
            report
                .notes
                .push(format!("`{}` is not compared: {reason}", limit.metric));
            continue;
        }
        compare(limit, &candidate_limits, &mut report.violations);
    }

    for measured in &candidate_limits {
        if !baseline_limits
            .iter()
            .any(|limit| limit.metric == measured.metric)
        {
            report.notes.push(format!(
                "`{}` is measured by the candidate run and the budget sets no limit for it, so \
                 nothing here can pass or fail it",
                measured.metric
            ));
        }
    }

    report
}

fn compare(limit: &Limit, candidate: &[Limit], violations: &mut Vec<String>) {
    let Some(measured) = candidate.iter().find(|entry| entry.metric == limit.metric) else {
        violations.push(format!(
            "the budget limits `{}` and the candidate run does not measure it, so the limit \
             cannot be said to hold",
            limit.metric
        ));
        return;
    };

    if limit.operator != "less-than-or-equal" {
        violations.push(format!(
            "`{}` uses comparison `{}`, which this check does not implement",
            limit.metric, limit.operator
        ));
        return;
    }

    match (
        limit.ceiling.parse::<f64>(),
        measured.measured.parse::<f64>(),
    ) {
        (Ok(ceiling), Ok(value)) if value <= ceiling => {}
        (Ok(ceiling), Ok(value)) => violations.push(format!(
            "`{}` measured {value} against a limit of {ceiling}, which is a regression",
            limit.metric
        )),
        _ => violations.push(format!(
            "`{}` has a limit or a measurement that is not a number",
            limit.metric
        )),
    }
}

pub(crate) fn hardware_profile(baselines: &[&Path]) -> Result<String, String> {
    let mut required: Option<String> = None;
    for baseline in baselines {
        let profile = jq_value(baseline, ".budget.hardwareProfile")?;
        match &required {
            None => required = Some(profile),
            Some(first) if *first == profile => {}
            Some(first) => {
                return Err(format!(
                    "the budget records hold on different hardware profiles, `{first}` and \
                     `{profile}`, so together they are not a budget; {} is the second",
                    redacted_path(baseline)
                ));
            }
        }
    }
    required.ok_or_else(|| "no budget record was named".to_owned())
}

fn union(paths: &[&Path], role: &str) -> Result<Vec<Limit>, String> {
    let mut all: Vec<Limit> = Vec::new();
    for path in paths {
        for limit in limits(path)? {
            if all.iter().any(|seen| seen.metric == limit.metric) {
                return Err(format!(
                    "`{}` is stated by two {role}, so it has two ceilings and therefore none; the \
                     second is in {}",
                    limit.metric,
                    redacted_path(path)
                ));
            }
            all.push(limit);
        }
    }
    Ok(all)
}

fn limits(path: &Path) -> Result<Vec<Limit>, String> {
    let rows = jq_value(
        path,
        r#".budget.limits[] | "\(.metric)\t\(.limit)\t\(.measured)\t\(.operator)""#,
    )?;
    if rows.is_empty() {
        return Err(format!("{} states no budget limits", redacted_path(path)));
    }

    rows.lines()
        .map(|row| {
            let mut fields = row.split('\t');
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some(metric), Some(ceiling), Some(measured), Some(operator)) => Ok(Limit {
                    metric: metric.to_owned(),
                    ceiling: ceiling.to_owned(),
                    measured: measured.to_owned(),
                    operator: operator.to_owned(),
                }),
                _ => Err(format!(
                    "{} states a limit that is not complete",
                    redacted_path(path)
                )),
            }
        })
        .collect()
}
