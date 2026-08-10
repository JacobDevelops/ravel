#![forbid(unsafe_code)]

//! Render stage ordering test mod.
//!
//! The mod for [`ravel_sdk::render_stages`]. It contributes to two published
//! stages and declares that it follows another mod at them, which is what
//! `ROADMAP.md`'s M6-W2 row asks an original mod to exercise.
//!
//! It draws nothing, and nothing draws at a stage yet. What a contribution
//! declares today is where a mod's drawing would go and who it would go after;
//! the SDK module says so to the author in the same words.

use ravel_sdk::render_stages::{RenderStage, StageDeclaration, StageOrder, StageRefused};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "render_stages";

/// The stages this mod contributes to.
const STAGES: [RenderStage; 2] = [RenderStage::AfterTerrain, RenderStage::AfterInterface];

/// A mod that contributes to two stages and asks to follow another mod.
///
/// # Examples
///
/// ```
/// use ravel_sdk::{Declared, ModId};
/// use ravel_test_mod_render_stages::RenderStagesMod;
///
/// let partner: ModId = "partner_mod".parse()?;
/// let order = RenderStagesMod::behind(&partner).expect("the pair resolves");
///
/// // This mod asked to follow the partner, so it is second at both stages.
/// assert_eq!(order.at("after_terrain"), [partner.clone(), RenderStagesMod::declared_id().clone()]);
/// assert_eq!(order.at("after_interface"), [partner, RenderStagesMod::declared_id().clone()]);
/// # Ok::<(), ravel_sdk::ModIdError>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderStagesMod {
    entered: Vec<LifecycleState>,
}

impl RenderStagesMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the states this mod was taken through, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the stages this mod contributes to.
    #[must_use]
    pub const fn stages() -> [RenderStage; 2] {
        STAGES
    }

    /// Declares this mod's contributions into `stages`.
    pub fn contribute(stages: &mut StageDeclaration) {
        for stage in STAGES {
            stages.contribute(Self::declared_id(), stage.as_str());
        }
    }

    /// Resolves an order in which this mod follows `partner` at every stage.
    ///
    /// # Errors
    ///
    /// Returns [`StageRefused`] when the pair has no order, which this pair does
    /// not: the one edge runs one way.
    pub fn behind(partner: &ModId) -> Result<StageOrder, StageRefused> {
        let mut stages = StageDeclaration::default();
        Self::contribute(&mut stages);
        for stage in STAGES {
            stages.contribute(partner, stage.as_str());
        }
        stages.after(Self::declared_id(), partner);
        stages.resolve()
    }
}

#[declare_mod(id = "render_stages")]
impl Mod for RenderStagesMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::render_stages::{RenderStage, StageDeclaration};
    use ravel_sdk::{Declared, ModId, Side};

    use super::{MOD_ID, RenderStagesMod};

    fn partner() -> ModId {
        "partner_mod".parse().expect("a legal mod id")
    }

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(RenderStagesMod::declared_id().as_str(), MOD_ID);
    }

    /// The order is read back positionally rather than compared to a list this
    /// test also wrote, so the assertion is about the edge the mod declared.
    #[test]
    fn this_mod_is_entered_after_the_mod_it_declared_it_follows() {
        let partner = partner();
        let order = RenderStagesMod::behind(&partner).expect("the pair resolves");

        for stage in RenderStagesMod::stages() {
            let at = order.at(stage.as_str());
            let mine = at
                .iter()
                .position(|id| id == RenderStagesMod::declared_id())
                .expect("this mod contributes here");
            let theirs = at
                .iter()
                .position(|id| id == &partner)
                .expect("the partner contributes here");
            assert!(theirs < mine, "{stage}");
        }
    }

    #[test]
    fn this_mod_contributes_to_nothing_it_did_not_declare() {
        let mut stages = StageDeclaration::default();
        RenderStagesMod::contribute(&mut stages);

        for stage in RenderStage::ALL {
            let declared = RenderStagesMod::stages().contains(&stage);
            assert_eq!(stages.contributors(stage).is_empty(), !declared, "{stage}");
        }
    }

    #[test]
    fn a_stage_this_mod_contributes_to_is_reached_on_the_client_alone() {
        for stage in RenderStagesMod::stages() {
            assert!(stage.happens_on(Side::Client));
            assert!(!stage.happens_on(Side::DedicatedServer));
            assert!(!stage.happens_on(Side::IntegratedServer));
        }
    }

    #[test]
    fn asking_to_follow_a_mod_that_contributes_nothing_is_refused() {
        let absent: ModId = "absent_mod".parse().expect("a legal mod id");
        let mut stages = StageDeclaration::default();
        RenderStagesMod::contribute(&mut stages);
        stages.after(RenderStagesMod::declared_id(), &absent);

        let refusal = stages
            .resolve()
            .expect_err("an order against nothing is refused");
        assert_eq!(refusal.unknown_contributors(), [absent]);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(RenderStagesMod::new());
