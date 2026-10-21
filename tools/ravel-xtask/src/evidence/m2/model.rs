use crate::evidence::model::{NonemptyRows, NonemptyString, UpperBoundOperator};
use serde::Deserialize;
use serde_json::Number;

#[derive(Deserialize)]
pub(super) struct BudgetRecord<'a> {
    #[serde(rename = "budget")]
    #[serde(borrow)]
    pub(super) budget: Budget<'a>,
}
#[derive(Deserialize)]
pub(super) struct Budget<'a> {
    #[serde(rename = "limits")]
    #[serde(borrow)]
    pub(super) limits: NonemptyRows<BudgetMetric<'a>>,
}
#[derive(Deserialize)]
pub(super) struct BudgetMetric<'a> {
    #[serde(rename = "metric")]
    #[serde(borrow)]
    pub(super) metric: NonemptyString<'a>,
    #[serde(rename = "unit")]
    #[serde(borrow)]
    _unit: NonemptyString<'a>,
    #[serde(rename = "operator")]
    _operator: UpperBoundOperator,
    #[serde(rename = "measured")]
    pub(super) measured: Number,
    #[serde(rename = "limit")]
    pub(super) limit: Number,
}
#[derive(Deserialize)]
pub(super) struct Noop {
    #[serde(rename = "runs")]
    pub(super) runs: NonemptyRows<Run>,
}
#[derive(Deserialize)]
pub(super) struct Run {
    #[serde(rename = "repeatedRunIsIdentical")]
    pub(super) repeated_run_is_identical: bool,
}
#[derive(Deserialize)]
pub(super) struct PreWorld {
    #[serde(rename = "gate")]
    pub(super) gate: Gate,
}
#[derive(Deserialize)]
pub(super) struct Gate {
    #[serde(rename = "refusesWorldAccess")]
    pub(super) refuses_world_access: bool,
    #[serde(rename = "ranModCode")]
    pub(super) ran_mod_code: bool,
}
#[derive(Deserialize)]
pub(super) struct PrismProbe {
    #[serde(rename = "launch")]
    pub(super) launch: LauncherEvidence,
    #[serde(rename = "ravel")]
    pub(super) ravel: Ravel,
    #[serde(rename = "instance")]
    pub(super) instance: Instance,
}
#[derive(Deserialize)]
pub(super) struct LauncherEvidence {
    #[serde(rename = "provesStockLaunch")]
    pub(super) proves_stock_launch: bool,
    #[serde(rename = "objection")]
    _objection: (),
    #[serde(rename = "attestation")]
    pub(super) attestation: Attestation,
}
#[derive(Deserialize)]
pub(super) struct Attestation {
    #[serde(rename = "stockLauncher")]
    pub(super) stock_launcher: bool,
}
#[derive(Deserialize)]
pub(super) struct Ravel {
    #[serde(rename = "reachedGameProcess")]
    pub(super) reached_game_process: bool,
    #[serde(rename = "rewrite")]
    pub(super) rewrite: Rewrite,
}
#[derive(Deserialize)]
pub(super) struct Rewrite {
    #[serde(rename = "reported")]
    pub(super) reported: bool,
    #[serde(rename = "reachedRavel")]
    pub(super) reached_ravel: bool,
    #[serde(rename = "classesRewritten")]
    pub(super) classes_rewritten: Number,
    #[serde(rename = "methodsRewritten")]
    pub(super) methods_rewritten: Number,
    #[serde(rename = "nativeInvocations")]
    pub(super) native_invocations: Number,
}
#[derive(Deserialize)]
pub(super) struct Instance {
    #[serde(rename = "withoutAnotherLoader")]
    pub(super) without_another_loader: bool,
}
