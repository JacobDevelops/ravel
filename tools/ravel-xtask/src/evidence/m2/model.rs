use crate::evidence::model::{NonemptyRows, NonemptyString, UpperBoundOperator};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BudgetRecord<'a> {
    #[serde(borrow)]
    pub(super) budget: Budget<'a>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Budget<'a> {
    #[serde(borrow)]
    pub(super) limits: NonemptyRows<BudgetMetric<'a>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BudgetMetric<'a> {
    #[serde(borrow)]
    pub(super) metric: NonemptyString<'a>,
    #[serde(rename = "unit")]
    #[serde(borrow)]
    _unit: NonemptyString<'a>,
    #[serde(rename = "operator")]
    _operator: UpperBoundOperator,

    pub(super) measured: f64,

    pub(super) limit: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Noop {
    pub(super) runs: NonemptyRows<Run>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Run {
    pub(super) repeated_run_is_identical: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PreWorld {
    pub(super) gate: Gate,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Gate {
    pub(super) refuses_world_access: bool,

    pub(super) ran_mod_code: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrismProbe {
    pub(super) launch: LauncherEvidence,

    pub(super) ravel: Ravel,

    pub(super) instance: Instance,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LauncherEvidence {
    pub(super) proves_stock_launch: bool,
    #[serde(rename = "objection")]
    _objection: (),

    pub(super) attestation: Attestation,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Attestation {
    pub(super) stock_launcher: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Ravel {
    pub(super) reached_game_process: bool,

    pub(super) rewrite: Rewrite,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Rewrite {
    pub(super) reported: bool,

    pub(super) reached_ravel: bool,

    pub(super) classes_rewritten: u64,

    pub(super) methods_rewritten: u64,

    pub(super) native_invocations: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Instance {
    pub(super) without_another_loader: bool,
}
