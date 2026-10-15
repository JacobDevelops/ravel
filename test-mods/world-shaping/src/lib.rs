#![forbid(unsafe_code)]

//! Exercises world-shaping declarations and persistent world binding.
//! It validates admission without generating terrain.

use ravel_sdk::world_shaping::{ShapingMismatch, ShapingRecord, WorldShaping};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "world_shaping";

/// A mod that shapes a world and is bound to the worlds it made.
///
/// # Examples
///
/// ```
/// use ravel_test_mod_world_shaping::WorldShapingMod;
///
/// let at_creation = WorldShapingMod::shaping();
/// let written = WorldShapingMod::record();
///
/// // The world it made opens under the same set.
/// written.admit(&at_creation).expect("what made the world may open it");
///
/// // A launch that no longer supplies this mod is refused, by name.
/// let refusal = WorldShapingMod::admit_without()
///     .expect_err("a world is refused when what shaped it is gone");
/// assert_eq!(refusal.missing().len(), 1);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorldShapingMod {
    entered: Vec<LifecycleState>,
}

impl WorldShapingMod {
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

    /// Returns this mod's own identity.
    #[must_use]
    pub fn id() -> &'static ModId {
        Self::declared_id()
    }

    /// Returns the shaping set this mod takes part in.
    #[must_use]
    pub fn shaping() -> WorldShaping {
        let mut shaping = WorldShaping::default();
        shaping.declare(Self::declared_id().clone());
        shaping
    }

    /// Returns what a world made with this mod holds.
    #[must_use]
    pub fn record() -> ShapingRecord {
        ShapingRecord::at_creation(&Self::shaping())
    }

    /// Answers whether a world this mod made opens without it.
    ///
    /// # Errors
    ///
    /// Always returns [`ShapingMismatch`], because the recorded mod is absent.
    pub fn admit_without() -> Result<(), ShapingMismatch> {
        Self::record().admit(&WorldShaping::default())
    }
}

#[declare_mod(id = "world_shaping")]
impl Mod for WorldShapingMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::world_shaping::{ShapingRecord, WorldShaping};
    use ravel_sdk::{Declared, ModId};

    use super::{MOD_ID, WorldShapingMod};

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(WorldShapingMod::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn what_this_mod_writes_into_a_world_reads_back_naming_this_mod() {
        let written = WorldShapingMod::record();
        let read = ShapingRecord::read(&written.bytes()).expect("what was written reads back");

        assert_eq!(read.mods(), [WorldShapingMod::declared_id().clone()]);
    }

    #[test]
    fn a_world_this_mod_made_is_refused_to_a_launch_without_it() {
        let refusal = WorldShapingMod::admit_without()
            .expect_err("a world is refused when what shaped it is gone");

        assert_eq!(refusal.missing(), [WorldShapingMod::declared_id().clone()]);
        assert_eq!(refusal.arrived(), []);
    }

    #[test]
    fn a_second_shaping_mod_arriving_later_is_refused_too() {
        let arrival: ModId = "another_shaper".parse().expect("a legal mod id");
        let mut now = WorldShapingMod::shaping();
        now.declare(arrival.clone());

        let refusal = WorldShapingMod::record()
            .admit(&now)
            .expect_err("a world is refused when something new would shape it");

        assert_eq!(refusal.arrived(), [arrival]);
        assert_eq!(refusal.missing(), []);
    }

    #[test]
    fn a_world_nothing_shaped_is_not_opened_by_this_mod_either() {
        let refusal = ShapingRecord::at_creation(&WorldShaping::default())
            .admit(&WorldShapingMod::shaping())
            .expect_err("a world made without this mod is not shaped by it now");

        assert_eq!(refusal.arrived(), [WorldShapingMod::declared_id().clone()]);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(WorldShapingMod::new());
