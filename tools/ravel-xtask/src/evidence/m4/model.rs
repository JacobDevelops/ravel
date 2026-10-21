use crate::evidence::model::{NonemptyRows, NonemptyString};
use serde::{Deserialize, de::IgnoredAny};
use serde_json::Number;

#[derive(Deserialize)]
pub(super) struct ProcessMatrix<'a> {
    #[serde(rename = "workPackage")]
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(rename = "protocol")]
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    #[serde(rename = "result")]
    #[serde(borrow)]
    pub(super) result: NonemptyString<'a>,
    #[serde(rename = "platform")]
    #[serde(borrow)]
    pub(super) platform: Platform<'a>,
    #[serde(rename = "gameLaunch")]
    #[serde(borrow)]
    pub(super) game_launch: GameLaunch<'a>,
    #[serde(rename = "gate")]
    pub(super) gate: Gate,
    #[serde(rename = "processes")]
    #[serde(borrow)]
    pub(super) processes: NonemptyRows<Process<'a>>,
}
#[derive(Deserialize)]
pub(super) struct Platform<'a> {
    #[serde(rename = "os")]
    #[serde(borrow)]
    pub(super) os: NonemptyString<'a>,
    #[serde(rename = "architecture")]
    #[serde(borrow)]
    pub(super) architecture: NonemptyString<'a>,
}
#[derive(Deserialize)]
pub(super) struct GameLaunch<'a> {
    #[serde(rename = "occurred")]
    pub(super) occurred: bool,
    #[serde(rename = "whyItIsNotHere")]
    #[serde(borrow)]
    _why_it_is_not_here: NonemptyString<'a>,
}
#[derive(Deserialize)]
pub(super) struct Gate {
    #[serde(rename = "exitStatus")]
    pub(super) exit_status: Number,
    #[serde(rename = "tests")]
    pub(super) tests: Tests,
}
#[derive(Deserialize)]
pub(super) struct Tests {
    #[serde(rename = "run")]
    pub(super) run: Number,
}
#[derive(Deserialize)]
pub(super) struct Process<'a> {
    #[serde(rename = "surface")]
    #[serde(borrow)]
    _surface: NonemptyString<'a>,
    #[serde(rename = "command")]
    #[serde(borrow)]
    _command: NonemptyString<'a>,
    #[serde(rename = "proves")]
    #[serde(borrow)]
    _proves: NonemptyString<'a>,
    #[serde(rename = "doesNotProve")]
    #[serde(borrow)]
    _does_not_prove: NonemptyString<'a>,
    #[serde(rename = "exitStatus")]
    pub(super) exit_status: Number,
}
#[derive(Deserialize)]
pub(super) struct Surface<'a> {
    #[serde(rename = "workPackage")]
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,
    #[serde(rename = "protocol")]
    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,
    #[serde(rename = "workPackages")]
    #[serde(borrow)]
    pub(super) work_packages: NonemptyRows<WorkPackage<'a>>,
    #[serde(rename = "inventory")]
    _inventory: Inventory,
}
#[derive(Deserialize)]
pub(super) struct WorkPackage<'a> {
    #[serde(rename = "id")]
    #[serde(borrow)]
    pub(super) id: NonemptyString<'a>,
    #[serde(rename = "shipped")]
    #[serde(borrow)]
    _shipped: NonemptyString<'a>,
    #[serde(rename = "notProven")]
    #[serde(borrow)]
    _not_proven: NonemptyString<'a>,
    #[serde(rename = "provenBy")]
    _proven_by: NonemptyRows<IgnoredAny>,
}
#[derive(Deserialize)]
pub(super) struct Inventory {
    #[serde(rename = "crates")]
    _crates: NonemptyRows<IgnoredAny>,
    #[serde(rename = "testMods")]
    _test_mods: NonemptyRows<IgnoredAny>,
    #[serde(rename = "examples")]
    _examples: NonemptyRows<IgnoredAny>,
    #[serde(rename = "guideDocuments")]
    _guide_documents: NonemptyRows<IgnoredAny>,
    #[serde(rename = "conformanceSuites")]
    _conformance_suites: NonemptyRows<IgnoredAny>,
}
