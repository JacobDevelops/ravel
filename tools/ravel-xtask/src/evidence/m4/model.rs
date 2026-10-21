use crate::evidence::model::{NonemptyRows, NonemptyString};
use serde::{Deserialize, de::IgnoredAny};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProcessMatrix<'a> {
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,

    pub(super) result: crate::evidence::model::Verdict,

    #[serde(borrow)]
    pub(super) platform: Platform<'a>,

    #[serde(borrow)]
    pub(super) game_launch: GameLaunch<'a>,

    pub(super) gate: Gate,

    #[serde(borrow)]
    pub(super) processes: NonemptyRows<Process<'a>>,
    #[serde(default)]
    pub(super) checks_outside_the_gate: Vec<Task>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Platform<'a> {
    #[serde(borrow)]
    pub(super) os: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) architecture: NonemptyString<'a>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GameLaunch<'a> {
    pub(super) occurred: bool,
    #[serde(rename = "whyItIsNotHere")]
    #[serde(borrow)]
    _why_it_is_not_here: NonemptyString<'a>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Gate {
    pub(super) exit_status: i32,

    pub(super) tests: Tests,
    #[serde(default)]
    pub(super) coverage: Option<Coverage>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Tests {
    pub(super) run: u64,
    #[serde(default)]
    pub(super) passed: Option<u64>,
    #[serde(default)]
    pub(super) failed: Option<u64>,
    #[serde(default)]
    pub(super) skipped: Option<u64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Process<'a> {
    #[serde(rename = "surface")]
    #[serde(borrow)]
    _surface: NonemptyString<'a>,
    #[serde(flatten)]
    pub(super) task: Task,
    #[serde(rename = "proves")]
    #[serde(borrow)]
    _proves: NonemptyString<'a>,
    #[serde(rename = "doesNotProve")]
    #[serde(borrow)]
    _does_not_prove: NonemptyString<'a>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Surface<'a> {
    #[serde(borrow)]
    pub(super) work_package: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) protocol: NonemptyString<'a>,

    #[serde(borrow)]
    pub(super) work_packages: NonemptyRows<WorkPackage<'a>>,
    pub(super) inventory: Inventory,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkPackage<'a> {
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
#[serde(rename_all = "camelCase")]
pub(super) struct Inventory {
    pub(super) crates: NonemptyRows<String>,
    pub(super) test_mods: NonemptyRows<String>,
    pub(super) examples: NonemptyRows<String>,
    pub(super) guide_documents: NonemptyRows<String>,
    pub(super) conformance_suites: NonemptyRows<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Task {
    #[serde(default)]
    pub(super) command: Command,
    #[serde(default)]
    pub(super) exit_status: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Coverage {
    pub(super) lines_percent: Option<f64>,
}

#[derive(Default, Deserialize)]
#[serde(untagged)]
pub(super) enum Command {
    Named(String),
    Invalid(IgnoredAny),
    #[default]
    Missing,
}

impl Command {
    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Named(command) => Some(command),
            Self::Invalid(_) | Self::Missing => None,
        }
    }
}
