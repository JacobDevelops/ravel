//! The three generated deposits and the substitution inside the third.
//!
//! Ravel publishes no world feature and no biome modifier, so the parameters
//! are declared as data and the one piece of behaviour that is not a parameter
//! is the substitution predicate, which is portable and is written out.
//!
//! The entry also describes its shaping set in a [`WorldShaping`] declaration.
//! The published host does not admit that declaration or enforce saved-world
//! removal refusals. Switching off a deposit does not change the declaration.
//!
//! Whether a deposit generates at all is the setting itself, read at the moment
//! biomes are constructed, so switching one off affects chunks generated
//! afterwards and leaves generated chunks alone.

use ravel_sdk::Declared;
use ravel_sdk::world_shaping::WorldShaping;

/// Which decoration step a deposit generates at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// The earliest decoration step, before ores.
    Raw,
    /// The step the ordinary ores generate at.
    UndergroundOres,
}

/// Which dimension a deposit generates in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Where {
    /// The overworld.
    Overworld,
    /// The nether.
    Nether,
}

/// One deposit, as declared parameters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Deposit {
    /// The name this deposit is declared under.
    pub name: &'static str,
    /// How many blocks one cluster places at most.
    pub size: u32,
    /// How many targets the feature replaces into.
    pub targets: u32,
    /// How many clusters generate per chunk.
    pub count: u32,
    /// The lowest absolute height a cluster generates at.
    pub lowest: i32,
    /// The highest absolute height a cluster generates at.
    pub highest: i32,
    /// Which dimension it generates in.
    pub biomes: Where,
    /// Which decoration step it generates at.
    pub stage: Stage,
    /// Whether the feature is this entry's own rather than the host's.
    pub own_feature: bool,
}

/// The chance a cluster is discarded for touching air.
///
/// Zero for all three, so a cluster exposed to air is kept.
pub const AIR_DISCARD: f64 = 0.0;

/// The three deposits, in the order the table states them.
pub const DEPOSITS: [Deposit; 3] = [
    Deposit {
        name: "first_ore",
        size: 8,
        targets: 2,
        count: 16,
        lowest: -32,
        highest: 64,
        biomes: Where::Overworld,
        stage: Stage::UndergroundOres,
        own_feature: false,
    },
    Deposit {
        name: "second_ore",
        size: 8,
        targets: 2,
        count: 12,
        lowest: -60,
        highest: 24,
        biomes: Where::Overworld,
        stage: Stage::UndergroundOres,
        own_feature: false,
    },
    Deposit {
        name: "nether_stone",
        size: 64,
        targets: 1,
        count: 4,
        lowest: 0,
        highest: 128,
        biomes: Where::Nether,
        stage: Stage::Raw,
        own_feature: true,
    },
];

/// What the third deposit places at one position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placed {
    /// The ordinary target state.
    Target,
    /// The rarer ore that sometimes stands in for it.
    Rare,
}

/// What the third deposit places at a position it would otherwise fill.
///
/// The substitution happens only when the rarer ore's entry is bound and a draw
/// against the configured chance succeeds. The draw is per block rather than
/// per cluster and is taken from the feature's own supplied source, so the
/// whole deposit is reproducible from a world seed.
#[must_use]
pub fn place(bound: bool, chance: f64, draw: f64) -> Placed {
    if bound && draw < chance {
        Placed::Rare
    } else {
        Placed::Target
    }
}

/// This entry declared into a world's shaping set.
pub fn shaping(into: &mut WorldShaping) {
    into.declare(crate::Entry::declared_id().clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_substitution_needs_both_a_bound_entry_and_a_draw_under_the_chance() {
        assert_eq!(place(true, 0.05, 0.04), Placed::Rare);
        assert_eq!(place(true, 0.05, 0.05), Placed::Target);
        assert_eq!(place(false, 1.0, 0.0), Placed::Target);
        assert_eq!(place(true, 0.0, 0.0), Placed::Target);
    }

    #[test]
    fn a_shaping_record_can_compare_the_declared_set_without_host_admission() {
        use ravel_sdk::world_shaping::ShapingRecord;

        let mut at_creation = WorldShaping::default();
        shaping(&mut at_creation);
        assert_eq!(at_creation.mods().len(), 1);
        let written = ShapingRecord::at_creation(&at_creation);
        written
            .admit(&at_creation)
            .expect("what deposited into the world may open it");

        let refusal = written
            .admit(&WorldShaping::default())
            .expect_err("a world is refused once what shaped it is gone");
        assert_eq!(refusal.missing().len(), 1);
        assert_eq!(refusal.missing()[0].as_str(), crate::MOD_ID);

        shaping(&mut at_creation);
        assert_eq!(at_creation.mods().len(), 1, "declaring twice declares once");
    }
}
