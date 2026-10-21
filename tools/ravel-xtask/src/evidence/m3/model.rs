use crate::evidence::model::{NonemptyRows, NonemptyString, UpperBoundOperator};
use serde::{Deserialize, de::IgnoredAny};
use serde_json::Number;

#[derive(Deserialize)]
pub(super) struct Identity {
    #[serde(rename = "observed")]
    pub(super) observed: Observed,
    #[serde(rename = "conclusion")]
    pub(super) conclusion: Conclusion,
    #[serde(rename = "rewrite")]
    pub(super) rewrite: IdentityRewrite,
}
#[derive(Deserialize)]
pub(super) struct Observed {
    #[serde(rename = "sameObject")]
    pub(super) same_object: bool,
}
#[derive(Deserialize)]
pub(super) struct Conclusion {
    #[serde(rename = "candidateTwoIsAvailable")]
    pub(super) candidate_two_is_available: bool,
}
#[derive(Deserialize)]
pub(super) struct IdentityRewrite {
    #[serde(rename = "classesRewritten")]
    pub(super) classes_rewritten: Number,
    #[serde(rename = "methodsRewritten")]
    pub(super) methods_rewritten: Number,
    #[serde(rename = "nativeInvocations")]
    pub(super) native_invocations: Number,
    #[serde(rename = "reachedNativeCode")]
    pub(super) reached_native_code: bool,
}
#[derive(Deserialize)]
pub(super) struct PackCompile<'a> {
    #[serde(rename = "gate")]
    #[serde(borrow)]
    pub(super) gate: NonemptyString<'a>,
    #[serde(rename = "protocol")]
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    #[serde(rename = "compile")]
    pub(super) compile: Compile,
    #[serde(rename = "disk")]
    pub(super) disk: Disk,
}
#[derive(Deserialize)]
pub(super) struct Compile {
    #[serde(rename = "repeats")]
    pub(super) repeats: Number,
    #[serde(rename = "baselineMilliseconds")]
    pub(super) baseline_milliseconds: Number,
    #[serde(rename = "marginalColdMillisecondsPerMod")]
    pub(super) marginal_cold_milliseconds_per_mod: Number,
}
#[derive(Deserialize)]
pub(super) struct Disk {
    #[serde(rename = "toolchainBytes")]
    pub(super) toolchain_bytes: Number,
}
#[derive(Deserialize)]
pub(super) struct Profiling<'a> {
    #[serde(rename = "workPackage")]
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(rename = "budget")]
    #[serde(borrow)]
    pub(super) budget: Budget<'a>,
    #[serde(rename = "collectors")]
    _collectors: Collectors,
}
#[derive(Deserialize)]
pub(super) struct Budget<'a> {
    #[serde(rename = "protocol")]
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    #[serde(rename = "hardwareProfile")]
    #[serde(borrow)]
    pub(super) hardware_profile: NonemptyString<'a>,
    #[serde(rename = "limits")]
    _limits: NonemptyRows<IgnoredAny>,
}
#[derive(Deserialize)]
pub(super) struct Collectors {
    #[serde(rename = "attributionAccuracy")]
    _attribution_accuracy: NonemptyRows<IgnoredAny>,
    #[serde(rename = "unsupported")]
    _unsupported: Vec<IgnoredAny>,
}
#[derive(Deserialize)]
pub(super) struct ProfilingLimit {
    #[serde(rename = "operator")]
    _operator: UpperBoundOperator,
    #[serde(rename = "measured")]
    pub(super) measured: Number,
    #[serde(rename = "limit")]
    pub(super) limit: Number,
}
#[derive(Deserialize)]
pub(super) struct ProcessMatrix<'a> {
    #[serde(rename = "workPackage")]
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(rename = "result")]
    #[serde(borrow)]
    pub(super) result: NonemptyString<'a>,
    #[serde(rename = "instance")]
    pub(super) instance: Instance,
    #[serde(rename = "screens")]
    pub(super) screens: Screens,
    #[serde(rename = "commandAndSavedData")]
    pub(super) command_and_saved_data: CommandAndSavedData,
    #[serde(rename = "worldPersistenceRoundTrip")]
    pub(super) world_persistence_round_trip: WorldPersistence,
    #[serde(rename = "rewrite")]
    pub(super) rewrite: MatrixRewrite,
}
#[derive(Deserialize)]
pub(super) struct Instance {
    #[serde(rename = "withoutAnotherLoader")]
    pub(super) without_another_loader: bool,
    #[serde(rename = "stockComponentsExact")]
    pub(super) stock_components_exact: bool,
    #[serde(rename = "otherLoaderComponents")]
    pub(super) other_loader_components: Number,
    #[serde(rename = "modJars")]
    pub(super) mod_jars: Number,
    #[serde(rename = "recognizedLoaderModJars")]
    pub(super) recognized_loader_mod_jars: Number,
}
#[derive(Deserialize)]
pub(super) struct Screens {
    #[serde(rename = "titleMenuButtonPresent")]
    pub(super) title_menu_button_present: bool,
    #[serde(rename = "allThreeOpenedInThisCampaign")]
    pub(super) all_three_opened_in_this_campaign: bool,
    #[serde(rename = "settings")]
    pub(super) settings: Settings,
}
#[derive(Deserialize)]
pub(super) struct Settings {
    #[serde(rename = "persistenceRoundTrip")]
    pub(super) persistence_round_trip: bool,
}
#[derive(Deserialize)]
pub(super) struct CommandAndSavedData {
    #[serde(rename = "vanillaCommandExecutedThroughInput")]
    pub(super) vanilla_command_executed_through_input: bool,
    #[serde(rename = "relaunchVerifiedExactValue")]
    pub(super) relaunch_verified_exact_value: bool,
}
#[derive(Deserialize)]
pub(super) struct WorldPersistence {
    #[serde(rename = "createSaveExitReload")]
    pub(super) create_save_exit_reload: WorldRoundTrip,
    #[serde(rename = "failClosedOnAnUnreadableWorld")]
    pub(super) fail_closed_on_an_unreadable_world: WorldRefusal,
}
#[derive(Deserialize)]
pub(super) struct WorldRoundTrip {
    #[serde(rename = "aggregateByteIdenticalAcrossTheRoundTrip")]
    pub(super) aggregate_byte_identical_across_the_round_trip: bool,
}
#[derive(Deserialize)]
pub(super) struct WorldRefusal {
    #[serde(rename = "worldRefused")]
    pub(super) world_refused: bool,
    #[serde(rename = "integratedServerNeverStarted")]
    pub(super) integrated_server_never_started: bool,
}
#[derive(Deserialize)]
pub(super) struct MatrixRewrite {
    #[serde(rename = "transformationFailures")]
    pub(super) transformation_failures: Number,
    #[serde(rename = "classesRewritten")]
    pub(super) classes_rewritten: Number,
    #[serde(rename = "nativeInvocations")]
    pub(super) native_invocations: Number,
}
