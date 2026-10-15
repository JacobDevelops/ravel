//! The three generated deposits and the substitution inside the third.
//!
//! Ravel publishes no world feature and no biome modifier, so the parameters
//! are declared as data and the one piece of behaviour that is not a parameter
//! is the substitution predicate, which is portable and is written out.
//!
//! What Ravel does publish is the *set* of mods a world was shaped by, so this
//! entry declares itself in a [`WorldShaping`]. That is what makes removing it
//! from a world it deposited into a refusal rather than chunks that quietly
//! stop matching the ones already on disk. A deposit switched off in
//! configuration does not undeclare the entry: it has shaped whatever generated
//! while it was on.
//!
//! Whether a deposit generates at all is the setting itself, read at the moment
//! biomes are constructed, so switching one off affects chunks generated
//! afterwards and leaves generated chunks alone.

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
///
/// # Errors
///
/// Returns [`ravel_sdk::ModIdError`] when the entry's own identity is not a
/// legal mod id, which for a constant is a packaging failure.
pub fn shaping(into: &mut WorldShaping) -> Result<(), ravel_sdk::ModIdError> {
    into.declare(crate::MOD_ID.parse()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_third_deposit_runs_before_the_ores_and_the_other_two_run_with_them() {
        assert_eq!(DEPOSITS[0].stage, Stage::UndergroundOres);
        assert_eq!(DEPOSITS[1].stage, Stage::UndergroundOres);
        assert_eq!(
            DEPOSITS[2].stage,
            Stage::Raw,
            "the stone it places is a target for anything generating later"
        );
    }

    #[test]
    fn each_deposit_carries_the_parameters_the_table_states() {
        assert_eq!(DEPOSITS[0].size, 8);
        assert_eq!(DEPOSITS[0].count, 16);
        assert_eq!((DEPOSITS[0].lowest, DEPOSITS[0].highest), (-32, 64));
        assert_eq!(DEPOSITS[1].count, 12);
        assert_eq!((DEPOSITS[1].lowest, DEPOSITS[1].highest), (-60, 24));
        assert_eq!(DEPOSITS[2].size, 64);
        assert_eq!(DEPOSITS[2].count, 4);
        assert_eq!((DEPOSITS[2].lowest, DEPOSITS[2].highest), (0, 128));
    }

    #[test]
    fn only_the_third_deposit_is_this_entrys_own_feature_and_only_it_is_nether() {
        assert!(!DEPOSITS[0].own_feature);
        assert!(!DEPOSITS[1].own_feature);
        assert!(DEPOSITS[2].own_feature);
        assert_eq!(DEPOSITS[2].biomes, Where::Nether);
        assert_eq!(DEPOSITS[0].biomes, Where::Overworld);
        assert_eq!(DEPOSITS[1].biomes, Where::Overworld);
    }

    #[test]
    fn nothing_is_discarded_for_touching_air() {
        assert!(AIR_DISCARD.abs() < f64::EPSILON);
    }

    #[test]
    fn the_substitution_needs_both_a_bound_entry_and_a_draw_under_the_chance() {
        assert_eq!(place(true, 0.05, 0.04), Placed::Rare);
        assert_eq!(place(true, 0.05, 0.05), Placed::Target);
        assert_eq!(place(false, 1.0, 0.0), Placed::Target);
        assert_eq!(place(true, 0.0, 0.0), Placed::Target);
    }

    #[test]
    fn a_world_this_entry_deposited_into_is_refused_once_the_entry_is_gone() {
        use ravel_sdk::world_shaping::ShapingRecord;

        let mut at_creation = WorldShaping::default();
        shaping(&mut at_creation).expect("the entry's own identity is a legal mod id");
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

        shaping(&mut at_creation).expect("a legal mod id");
        assert_eq!(at_creation.mods().len(), 1, "declaring twice declares once");
    }
}
