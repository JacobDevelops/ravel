//! Section ten's work and heat readings.
//!
//! Both are single-value readings a dependent polls and neither has any state of
//! its own, so each rule here is a function of what the host object currently
//! is. That shape is what lets the rules be implemented and tested even where
//! section 13.3 blocks the value one of them reads: the arithmetic is this
//! package's, and the blocked value is a hole in what the host supplies.
//!
//! Two of the fields below are [`Option`] for exactly that reason. A smelting
//! station's raw burn timer and a brewing station's fuel count and brewable test
//! sit behind the access widening of section 13.1, which Ravel does not do, so
//! nothing fills them today and the reading answers absent rather than a made-up
//! number. `NOTES.md` lists them.

use rvl_210_counterpart::quantity::Amount;

use crate::kinds::{Portion, Stack};

/// The freezing point of water, which every heat reading is offset from.
///
/// Section ten calls this a constant the vocabulary publishes. RVL-210's
/// counterpart publishes no such constant at the reviewed revision, so it is
/// declared here and `NOTES.md` records the gap rather than the value being
/// quietly invented in three places.
pub const FREEZING: i64 = 273;

/// What a lit campfire reads above the freezing point.
pub const CAMPFIRE_HEAT: i64 = 1_100;

/// The most slots a station snapshot may carry.
pub const MAX_STATION_SLOTS: usize = 64;

/// What a work indicator answers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Work {
    /// Whether there is work to do.
    pub exists: bool,
    /// Whether work can proceed.
    pub can_proceed: bool,
}

impl Work {
    /// Neither work to do nor a way to do it.
    pub const IDLE: Self = Self {
        exists: false,
        can_proceed: false,
    };

    /// Whether the station would actually run.
    #[must_use]
    pub const fn running(self) -> bool {
        self.exists && self.can_proceed
    }
}

/// What a heat reading answers.
///
/// Four values, all absolute temperatures offset from [`FREEZING`]. A reading
/// with no maximum is one whose current value has no stated ceiling, which is
/// two of the three cases in section ten.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Heat {
    /// The current temperature.
    pub current: i64,
    /// The lowest it reads.
    pub minimum: i64,
    /// The highest it reads, absent where section ten states no maximum.
    pub maximum: Option<i64>,
    /// What it reads with nothing happening.
    pub default: i64,
}

impl Heat {
    /// A reading sitting at the freezing point with no maximum.
    pub const COLD: Self = Self {
        current: FREEZING,
        minimum: FREEZING,
        maximum: None,
        default: FREEZING,
    };

    /// How far above freezing the current value reads.
    #[must_use]
    pub const fn above_freezing(self) -> i64 {
        self.current - FREEZING
    }
}

/// What a smelting station currently is.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Smelting {
    /// What is in the input slot.
    pub input: Option<Stack>,
    /// Whether the station is already burning.
    pub burning: bool,
    /// What is in the fuel slot.
    pub fuel: Option<Stack>,
    /// Whether what is in the fuel slot burns.
    pub fuel_burns: bool,
    /// The raw remaining burn ticks.
    ///
    /// Section 13.3 blocks this: it sits behind the access widening of section
    /// 13.1 and no Ravel hook exposes it, so nothing fills it and
    /// [`Smelting::heat`] answers absent.
    pub remaining_ticks: Option<u32>,
}

impl Smelting {
    /// Whether there is work and whether it can proceed.
    ///
    /// Work exists when the input slot is non-empty, a recipe matches it, and
    /// assembling that recipe yields a non-empty result. Work can proceed when
    /// the station is already burning or its fuel slot holds something that
    /// burns.
    #[must_use]
    pub fn work(&self, assemble: impl Fn(&Stack) -> Option<Stack>) -> Work {
        let exists = self
            .input
            .as_ref()
            .filter(|input| !input.is_empty())
            .and_then(assemble)
            .is_some_and(|result| !result.is_empty());
        let can_proceed = self.burning
            || (self.fuel.as_ref().is_some_and(|fuel| !fuel.is_empty()) && self.fuel_burns);
        Work {
            exists,
            can_proceed,
        }
    }

    /// The freezing point plus the raw remaining burn ticks, with no maximum.
    ///
    /// Section ten records what this is: a tick count read as a temperature,
    /// which is not a physical quantity. It is nevertheless the number every
    /// dependent has been reading, so it is reproduced rather than corrected.
    ///
    /// Answers nothing when the host supplies no tick count, which today is
    /// always: the value is one of the four section 13.3 blocks.
    #[must_use]
    pub fn heat(&self) -> Option<Heat> {
        let ticks = self.remaining_ticks?;
        Some(Heat {
            current: FREEZING + i64::from(ticks),
            minimum: FREEZING,
            maximum: None,
            default: FREEZING,
        })
    }
}

/// What a brewing station currently is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Brewing {
    /// Whether the current contents are brewable.
    ///
    /// Section 13.3 blocks this test.
    pub brewable: Option<bool>,
    /// The station's fuel count.
    ///
    /// Section 13.3 blocks this count.
    pub fuel: Option<Amount>,
}

impl Brewing {
    /// Whether there is work and whether it can proceed.
    ///
    /// Work exists when the current contents are brewable and can proceed when
    /// the fuel count is above zero. Both values are blocked, so this answers
    /// nothing until a Ravel hook supplies them.
    #[must_use]
    pub fn work(&self) -> Option<Work> {
        Some(Work {
            exists: self.brewable?,
            can_proceed: self.fuel? > 0,
        })
    }
}

/// What a campfire currently is.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Campfire {
    /// What is in each of its slots.
    slots: Vec<Option<Stack>>,
    /// Whether the block is lit.
    pub lit: bool,
}

impl Campfire {
    /// Builds a snapshot, keeping at most [`MAX_STATION_SLOTS`] slots.
    ///
    /// A slot count reaches this package from a host object, so it is bounded
    /// rather than trusted.
    #[must_use]
    pub fn new(slots: impl IntoIterator<Item = Option<Stack>>, lit: bool) -> Self {
        Self {
            slots: slots.into_iter().take(MAX_STATION_SLOTS).collect(),
            lit,
        }
    }

    /// What each slot holds.
    pub fn slots(&self) -> impl Iterator<Item = Option<&Stack>> + '_ {
        self.slots.iter().map(Option::as_ref)
    }

    /// Whether there is work and whether it can proceed.
    ///
    /// Work exists when any of its slots is non-empty and can proceed when the
    /// block is lit.
    #[must_use]
    pub fn work(&self) -> Work {
        Work {
            exists: self.slots.iter().flatten().any(|held| !held.is_empty()),
            can_proceed: self.lit,
        }
    }

    /// The freezing point plus 1100 when lit and the freezing point otherwise,
    /// with the lit value as its maximum.
    #[must_use]
    pub const fn heat(&self) -> Heat {
        Heat {
            current: if self.lit {
                FREEZING + CAMPFIRE_HEAT
            } else {
                FREEZING
            },
            minimum: FREEZING,
            maximum: Some(FREEZING + CAMPFIRE_HEAT),
            default: FREEZING,
        }
    }
}

/// The temperature a bucket-like item reads.
///
/// The temperature the contained fluid declares, and the freezing point when
/// empty, with no maximum.
#[must_use]
pub fn bucket_heat(held: Option<&Portion>, declared: impl Fn(&str) -> Option<i64>) -> Heat {
    let current = held
        .filter(|portion| !portion.is_empty())
        .and_then(|portion| declared(&portion.fluid))
        .unwrap_or(FREEZING);
    Heat {
        current,
        minimum: FREEZING,
        maximum: None,
        default: FREEZING,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smelts(input: &Stack) -> Option<Stack> {
        if input.identity == "minecraft:iron_ore" {
            Some(Stack::of("minecraft:iron_ingot", 1))
        } else {
            None
        }
    }

    #[test]
    fn a_smelting_station_has_work_only_when_a_recipe_yields_something() {
        let station = Smelting {
            input: Some(Stack::of("minecraft:iron_ore", 1)),
            ..Smelting::default()
        };
        assert!(station.work(smelts).exists);
        let station = Smelting {
            input: Some(Stack::of("minecraft:stone", 1)),
            ..Smelting::default()
        };
        assert!(!station.work(smelts).exists);
    }

    #[test]
    fn an_empty_input_slot_is_no_work_even_when_a_recipe_would_match() {
        let station = Smelting {
            input: Some(Stack::empty()),
            ..Smelting::default()
        };
        assert!(
            !station
                .work(|_| Some(Stack::of("minecraft:iron_ingot", 1)))
                .exists
        );
        let station = Smelting::default();
        assert!(
            !station
                .work(|_| Some(Stack::of("minecraft:iron_ingot", 1)))
                .exists
        );
    }

    #[test]
    fn a_recipe_assembling_to_nothing_is_no_work() {
        let station = Smelting {
            input: Some(Stack::of("minecraft:iron_ore", 1)),
            ..Smelting::default()
        };
        assert!(!station.work(|_| Some(Stack::empty())).exists);
    }

    #[test]
    fn a_smelting_station_proceeds_when_burning_or_holding_fuel_that_burns() {
        let burning = Smelting {
            burning: true,
            ..Smelting::default()
        };
        assert!(burning.work(smelts).can_proceed);
        let fuelled = Smelting {
            fuel: Some(Stack::of("minecraft:coal", 1)),
            fuel_burns: true,
            ..Smelting::default()
        };
        assert!(fuelled.work(smelts).can_proceed);
        let unfuelled = Smelting {
            fuel: Some(Stack::of("minecraft:stone", 1)),
            fuel_burns: false,
            ..Smelting::default()
        };
        assert!(!unfuelled.work(smelts).can_proceed);
        let empty_fuel = Smelting {
            fuel: Some(Stack::empty()),
            fuel_burns: true,
            ..Smelting::default()
        };
        assert!(!empty_fuel.work(smelts).can_proceed);
    }

    #[test]
    fn a_station_runs_only_when_there_is_work_and_it_can_proceed() {
        let station = Smelting {
            input: Some(Stack::of("minecraft:iron_ore", 1)),
            burning: true,
            ..Smelting::default()
        };
        assert!(station.work(smelts).running());
        assert!(!Work::IDLE.running());
    }

    #[test]
    fn the_smelting_heat_is_the_freezing_point_plus_the_raw_tick_count() {
        let station = Smelting {
            remaining_ticks: Some(1_600),
            ..Smelting::default()
        };
        let heat = station.heat().expect("a supplied tick count");
        assert_eq!(heat.current, FREEZING + 1_600);
        assert_eq!(heat.above_freezing(), 1_600);
        assert_eq!(heat.minimum, FREEZING);
        assert_eq!(heat.maximum, None);
        assert_eq!(heat.default, FREEZING);
    }

    #[test]
    fn the_smelting_heat_answers_nothing_where_no_hook_supplies_the_tick_count() {
        assert_eq!(Smelting::default().heat(), None);
    }

    #[test]
    fn a_brewing_stations_work_needs_both_blocked_values() {
        assert_eq!(Brewing::default().work(), None);
        assert_eq!(
            Brewing {
                brewable: Some(true),
                fuel: None
            }
            .work(),
            None
        );
        assert_eq!(
            Brewing {
                brewable: None,
                fuel: Some(3)
            }
            .work(),
            None
        );
    }

    #[test]
    fn a_brewing_station_has_work_when_brewable_and_proceeds_on_fuel_above_zero() {
        assert_eq!(
            Brewing {
                brewable: Some(true),
                fuel: Some(1)
            }
            .work(),
            Some(Work {
                exists: true,
                can_proceed: true
            })
        );
        assert_eq!(
            Brewing {
                brewable: Some(false),
                fuel: Some(0)
            }
            .work(),
            Some(Work::IDLE)
        );
    }

    #[test]
    fn a_campfire_has_work_when_any_slot_is_non_empty() {
        let empty = Campfire::new([None, None], false);
        assert!(!empty.work().exists);
        let held = Campfire::new([None, Some(Stack::of("minecraft:beef", 1))], false);
        assert!(held.work().exists);
        let blank = Campfire::new([Some(Stack::empty())], false);
        assert!(!blank.work().exists);
    }

    #[test]
    fn a_campfire_proceeds_only_when_lit() {
        assert!(Campfire::new([None], true).work().can_proceed);
        assert!(!Campfire::new([None], false).work().can_proceed);
    }

    #[test]
    fn a_campfires_heat_reads_the_lit_value_and_carries_it_as_its_maximum() {
        let lit = Campfire::new([None], true).heat();
        assert_eq!(lit.current, FREEZING + CAMPFIRE_HEAT);
        assert_eq!(lit.maximum, Some(FREEZING + CAMPFIRE_HEAT));
        let dark = Campfire::new([None], false).heat();
        assert_eq!(dark.current, FREEZING);
        assert_eq!(dark.maximum, Some(FREEZING + CAMPFIRE_HEAT));
        assert_eq!(dark.default, FREEZING);
    }

    #[test]
    fn a_campfire_snapshot_is_bounded_at_the_published_slot_ceiling() {
        let held = Campfire::new(
            (0..MAX_STATION_SLOTS + 10).map(|_| Some(Stack::of("minecraft:beef", 1))),
            true,
        );
        assert_eq!(held.slots().count(), MAX_STATION_SLOTS);
    }

    #[test]
    fn a_bucket_reads_the_temperature_its_fluid_declares() {
        let lava = Portion::of("minecraft:lava", 1_000);
        let heat = bucket_heat(Some(&lava), |fluid| {
            (fluid == "minecraft:lava").then_some(FREEZING + 1_300)
        });
        assert_eq!(heat.current, FREEZING + 1_300);
        assert_eq!(heat.maximum, None);
    }

    #[test]
    fn an_empty_bucket_reads_the_freezing_point() {
        assert_eq!(bucket_heat(None, |_| Some(9_999)), Heat::COLD);
        assert_eq!(
            bucket_heat(Some(&Portion::empty()), |_| Some(9_999)),
            Heat::COLD
        );
    }

    #[test]
    fn a_fluid_declaring_no_temperature_reads_the_freezing_point() {
        let water = Portion::of("minecraft:water", 1_000);
        assert_eq!(bucket_heat(Some(&water), |_| None), Heat::COLD);
    }
}
