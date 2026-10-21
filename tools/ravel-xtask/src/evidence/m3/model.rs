use crate::evidence::model::{NonemptyRows, NonemptyString, UpperBoundOperator};
use serde::{Deserialize, de::IgnoredAny};

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
    pub(super) classes_rewritten: u64,

    pub(super) methods_rewritten: u64,

    pub(super) native_invocations: u64,

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
    pub(super) repeats: u64,

    pub(super) baseline_milliseconds: f64,

    pub(super) marginal_cold_milliseconds_per_mod: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Disk {
    pub(super) toolchain_bytes: u64,
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
    #[serde(borrow)]
    pub(super) limits: NonemptyRows<ProfilingLimit<'a>>,
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
pub(super) struct ProfilingLimit<'a> {
    #[serde(borrow)]
    pub(super) metric: NonemptyString<'a>,
    #[serde(rename = "operator")]
    _operator: UpperBoundOperator,

    pub(super) measured: f64,

    pub(super) limit: f64,
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
    #[serde(default)]
    pub(super) matrix_not_carried_forward: Option<PriorMatrix>,
    #[serde(default)]
    pub(super) surface_campaign: Option<SurfaceCampaign>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Instance {
    pub(super) without_another_loader: bool,

    pub(super) stock_components_exact: bool,

    pub(super) other_loader_components: u64,

    pub(super) mod_jars: u64,

    pub(super) recognized_loader_mod_jars: u64,
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
    pub(super) transformation_failures: u64,

    pub(super) classes_rewritten: u64,

    pub(super) native_invocations: u64,
    #[serde(default)]
    pub(super) classes_in_inventory: Option<usize>,
    #[serde(default)]
    pub(super) callable_targets_in_inventory: Option<usize>,
    #[serde(default)]
    pub(super) target_inventory: Option<Vec<Target>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PriorMatrix {
    pub(super) prior_callable_targets: usize,
}

#[derive(Deserialize)]
pub(super) struct SurfaceCampaign {
    pub(super) launches: usize,
}

#[derive(Deserialize)]
pub(super) struct Target {
    pub(super) class: String,
    pub(super) methods: Vec<String>,
}
