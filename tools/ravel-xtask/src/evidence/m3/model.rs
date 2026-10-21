use crate::evidence::model::{NonemptyRows, NonemptyString, UpperBoundOperator};
use serde::{Deserialize, de::IgnoredAny};
use serde_json::Number;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Identity {
    pub(super) observed: Observed,

    pub(super) conclusion: Conclusion,

    pub(super) rewrite: IdentityRewrite,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Observed {
    pub(super) same_object: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Conclusion {
    pub(super) candidate_two_is_available: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IdentityRewrite {
    pub(super) classes_rewritten: Number,

    pub(super) methods_rewritten: Number,

    pub(super) native_invocations: Number,

    pub(super) reached_native_code: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PackCompile<'a> {
    #[serde(borrow)]
    pub(super) gate: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,

    pub(super) compile: Compile,

    pub(super) disk: Disk,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Compile {
    pub(super) repeats: Number,

    pub(super) baseline_milliseconds: Number,

    pub(super) marginal_cold_milliseconds_per_mod: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Disk {
    pub(super) toolchain_bytes: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Profiling<'a> {
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) budget: Budget<'a>,
    #[serde(rename = "collectors")]
    _collectors: Collectors,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Budget<'a> {
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) hardware_profile: NonemptyString<'a>,
    #[serde(rename = "limits")]
    _limits: NonemptyRows<IgnoredAny>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Collectors {
    #[serde(rename = "attributionAccuracy")]
    _attribution_accuracy: NonemptyRows<IgnoredAny>,
    #[serde(rename = "unsupported")]
    _unsupported: Vec<IgnoredAny>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProfilingLimit {
    #[serde(rename = "operator")]
    _operator: UpperBoundOperator,

    pub(super) measured: Number,

    pub(super) limit: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProcessMatrix<'a> {
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,

    pub(super) result: crate::evidence::model::Verdict,

    pub(super) instance: Instance,

    pub(super) screens: Screens,

    pub(super) command_and_saved_data: CommandAndSavedData,

    pub(super) world_persistence_round_trip: WorldPersistence,

    pub(super) rewrite: MatrixRewrite,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Instance {
    pub(super) without_another_loader: bool,

    pub(super) stock_components_exact: bool,

    pub(super) other_loader_components: Number,

    pub(super) mod_jars: Number,

    pub(super) recognized_loader_mod_jars: Number,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Screens {
    pub(super) title_menu_button_present: bool,

    pub(super) all_three_opened_in_this_campaign: bool,

    pub(super) settings: Settings,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Settings {
    pub(super) persistence_round_trip: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommandAndSavedData {
    pub(super) vanilla_command_executed_through_input: bool,

    pub(super) relaunch_verified_exact_value: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorldPersistence {
    pub(super) create_save_exit_reload: WorldRoundTrip,

    pub(super) fail_closed_on_an_unreadable_world: WorldRefusal,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorldRoundTrip {
    pub(super) aggregate_byte_identical_across_the_round_trip: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorldRefusal {
    pub(super) world_refused: bool,

    pub(super) integrated_server_never_started: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MatrixRewrite {
    pub(super) transformation_failures: Number,

    pub(super) classes_rewritten: Number,

    pub(super) native_invocations: Number,
}
