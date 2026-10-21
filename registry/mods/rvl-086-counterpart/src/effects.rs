//! The two places a timed effect is applied: sections 10.3 and 14.3.
//!
//! Applying a timed effect to an entity is host-blocked — Ravel publishes no
//! surface for it — so what is here is the arithmetic that decides for how long,
//! and the bookkeeping that decides who is owed one. Both are testable without
//! the application, and the application is the only part missing.
//!
//! Two choices are recorded rather than taken silently. The food duration rule
//! is reproduced including its shared accumulator, which is read behaviour and
//! is observable as longer buffs. The night-vision upkeep is corrected: it
//! removes only the effect it granted, where the original clears whatever night
//! vision the wearer holds.

use std::collections::BTreeMap;

/// One timed effect a restorative food applies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timed {
    /// What the effect is.
    pub effect: String,
    /// The amplifier it is applied at.
    pub amplifier: u32,
    /// The amplifier the eater must already hold for their remaining duration to
    /// be picked up.
    pub threshold: u32,
    /// The ticks added to the accumulator when this effect is applied.
    pub increment: u32,
}

/// What the eater already holds of one effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeldEffect {
    /// The amplifier they hold it at.
    pub amplifier: u32,
    /// The ticks left of it.
    pub remaining: u32,
}

/// One effect as it is applied, with the duration the rule worked out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Applied {
    /// What the effect is.
    pub effect: String,
    /// The amplifier it is applied at, which is the effect's own and fixed.
    pub amplifier: u32,
    /// How long it lasts.
    pub duration: u32,
}

/// Works out how long each of a food's effects lasts.
///
/// A single accumulator starts at nought and is carried across the whole
/// sequence. For each effect in the fixed order: if the eater already holds that
/// effect at or above its threshold, the accumulator is *replaced* by what is
/// left of it; then the effect is applied for the accumulator plus its own
/// increment.
///
/// The accumulator is never reset between effects, so an earlier effect the
/// eater holds extends a later effect they do not hold at all. That is read
/// behaviour rather than a rule anyone wrote down, and it is reproduced because
/// the alternative changes every duration the entry produces.
#[must_use]
pub fn durations(sequence: &[Timed], held: &dyn Fn(&str) -> Option<HeldEffect>) -> Vec<Applied> {
    let mut accumulator = 0_u32;
    let mut applied = Vec::with_capacity(sequence.len());
    for timed in sequence {
        if let Some(already) = held(&timed.effect)
            && already.amplifier >= timed.threshold
        {
            accumulator = already.remaining;
        }
        applied.push(Applied {
            effect: timed.effect.clone(),
            amplifier: timed.amplifier,
            duration: accumulator.saturating_add(timed.increment),
        });
    }
    applied
}

/// The ticks a refresh of the night-vision upkeep grants.
///
/// A little over twelve seconds at twenty ticks a second, against a refresh
/// every tick, which is what makes the removal look instant rather than fading.
pub const REFRESH_TICKS: u32 = 260;

/// How often the upkeep refreshes, in ticks.
pub const REFRESH_INTERVAL_TICKS: u32 = 1;

/// The effect the upkeep grants and takes away.
pub const NIGHT_VISION: &str = "night_vision";

/// The most wearers the upkeep will track at once.
///
/// The list is rebuilt every tick from who is wearing a piece, so its length is
/// the server's population rather than anything a player writes. The ceiling
/// bounds the allocation anyway, because the list is built from data the host
/// hands over and this package does not get to assume how much of it there is.
pub const MAXIMUM_WEARERS: usize = 4096;

/// What one tick of the upkeep did to one wearer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Change {
    /// A wearer who was not listed is now listed and granted the effect.
    Granted(String),
    /// A listed wearer still wearing has the effect refreshed.
    Refreshed(String),
    /// A listed wearer who stopped wearing has this upkeep's grant removed.
    Removed(String),
    /// How many wearers past [`MAXIMUM_WEARERS`] this tick did not look at.
    Truncated(usize),
}

/// The transient list of who is currently wearing a headpiece.
///
/// It is not saved and is not saveable: it is rebuilt from equipment within one
/// tick of a reload, which is the right thing for it to be. It is keyed on the
/// persistent player identifier, where the original keys on the wearer's profile
/// name concatenated with whether they are the local player — a key that
/// collides across a name change. Nothing incorrect could survive into a save
/// either way, because the list never outlives the process; the identifier is
/// used because it is the one that is actually an identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Upkeep {
    granted: BTreeMap<String, u32>,
}

impl Upkeep {
    /// Builds an upkeep tracking nobody.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this upkeep granted the effect this wearer holds.
    #[must_use]
    pub fn granted(&self, wearer: &str) -> bool {
        self.granted.contains_key(wearer)
    }

    /// How many wearers are listed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.granted.len()
    }

    /// Whether nobody is listed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.granted.is_empty()
    }

    /// Runs one tick over everyone currently wearing a piece.
    ///
    /// `wearing` is the whole of who is wearing one this tick, in either the head
    /// slot or the accessory slot; anybody listed and absent from it has stopped
    /// wearing. The ceiling is [`MAXIMUM_WEARERS`] grants, so a run of this
    /// cannot grow without bound on a host that hands over an unreasonable list;
    /// a wearer past it keeps no grant and is reported as [`Change::Truncated`]
    /// rather than as having stopped wearing, which is what stops the effect
    /// being stripped from them every tick. Raising the ceiling is the upgrade
    /// path if a host ever opens a screen that large.
    pub fn tick(&mut self, wearing: &[String]) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut still: BTreeMap<String, u32> = BTreeMap::new();
        for wearer in wearing.iter().take(MAXIMUM_WEARERS) {
            let known = self.granted.contains_key(wearer);
            still.insert(wearer.clone(), REFRESH_TICKS);
            changes.push(if known {
                Change::Refreshed(wearer.clone())
            } else {
                Change::Granted(wearer.clone())
            });
        }
        for wearer in self.granted.keys() {
            if !still.contains_key(wearer) && !wearing.contains(wearer) {
                changes.push(Change::Removed(wearer.clone()));
            }
        }
        let beyond = wearing.len().saturating_sub(MAXIMUM_WEARERS);
        if beyond > 0 {
            changes.push(Change::Truncated(beyond));
        }
        self.granted = still;
        changes
    }

    /// Whether taking a piece off removes an effect this upkeep did not grant.
    ///
    /// It never does, which is the whole of the declared change: the original
    /// clears whatever night vision the wearer holds, so a potion the player
    /// drank for disappears when they take the headpiece off. Here a wearer this
    /// upkeep never granted is left alone.
    #[must_use]
    pub fn removes(&self, wearer: &str) -> bool {
        self.granted(wearer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed(effect: &str, amplifier: u32, threshold: u32, increment: u32) -> Timed {
        Timed {
            effect: effect.to_owned(),
            amplifier,
            threshold,
            increment,
        }
    }

    fn sequence() -> Vec<Timed> {
        vec![
            timed("example:first", 1, 1, 100),
            timed("example:second", 0, 0, 200),
            timed("example:third", 2, 1, 300),
        ]
    }

    #[test]
    fn an_eater_holding_nothing_gets_each_effect_for_its_own_increment() {
        let applied = durations(&sequence(), &|_| None);
        assert_eq!(
            applied.iter().map(|one| one.duration).collect::<Vec<_>>(),
            vec![100, 200, 300]
        );
    }

    #[test]
    fn an_effect_the_eater_holds_above_its_threshold_replaces_the_accumulator() {
        let applied = durations(&sequence(), &|effect| {
            (effect == "example:first").then_some(HeldEffect {
                amplifier: 1,
                remaining: 1000,
            })
        });
        assert_eq!(applied[0].duration, 1100);
    }

    #[test]
    fn the_accumulator_is_shared_so_an_earlier_effect_extends_a_later_one() {
        let applied = durations(&sequence(), &|effect| {
            (effect == "example:first").then_some(HeldEffect {
                amplifier: 1,
                remaining: 1000,
            })
        });
        assert_eq!(applied[1].duration, 1200, "the second is extended too");
        assert_eq!(applied[2].duration, 1300, "and so is the third");
    }

    #[test]
    fn an_effect_held_below_its_threshold_does_not_touch_the_accumulator() {
        let applied = durations(&sequence(), &|effect| {
            (effect == "example:third").then_some(HeldEffect {
                amplifier: 0,
                remaining: 5000,
            })
        });
        assert_eq!(
            applied.iter().map(|one| one.duration).collect::<Vec<_>>(),
            vec![100, 200, 300]
        );
    }

    #[test]
    fn a_later_effect_replaces_the_accumulator_the_earlier_one_set() {
        let applied = durations(&sequence(), &|effect| match effect {
            "example:first" => Some(HeldEffect {
                amplifier: 1,
                remaining: 1000,
            }),
            "example:third" => Some(HeldEffect {
                amplifier: 3,
                remaining: 40,
            }),
            _ => None,
        });
        assert_eq!(applied[1].duration, 1200);
        assert_eq!(applied[2].duration, 340);
    }

    #[test]
    fn each_effect_is_applied_at_its_own_fixed_amplifier() {
        let applied = durations(&sequence(), &|_| None);
        assert_eq!(
            applied.iter().map(|one| one.amplifier).collect::<Vec<_>>(),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn a_remaining_duration_at_the_top_of_its_range_does_not_wrap() {
        let applied = durations(&sequence(), &|effect| {
            (effect == "example:first").then_some(HeldEffect {
                amplifier: 1,
                remaining: u32::MAX,
            })
        });
        assert_eq!(applied[0].duration, u32::MAX);
    }

    #[test]
    fn a_wearer_is_granted_once_and_refreshed_afterwards() {
        let mut upkeep = Upkeep::new();
        assert!(upkeep.is_empty());
        let wearing = vec!["player-one".to_owned()];
        assert_eq!(
            upkeep.tick(&wearing),
            vec![Change::Granted("player-one".to_owned())]
        );
        assert_eq!(
            upkeep.tick(&wearing),
            vec![Change::Refreshed("player-one".to_owned())]
        );
        assert_eq!(upkeep.len(), 1);
    }

    #[test]
    fn a_wearer_who_stops_wearing_has_this_upkeeps_grant_removed_and_is_dropped() {
        let mut upkeep = Upkeep::new();
        upkeep.tick(&["player-one".to_owned()]);
        assert_eq!(
            upkeep.tick(&[]),
            vec![Change::Removed("player-one".to_owned())]
        );
        assert!(upkeep.is_empty());
        assert_eq!(upkeep.tick(&[]), Vec::new());
    }

    #[test]
    fn a_wearer_this_upkeep_never_granted_is_left_alone() {
        let upkeep = Upkeep::new();
        assert!(!upkeep.removes("someone-who-drank-for-it"));
        assert!(!upkeep.granted("someone-who-drank-for-it"));
    }

    #[test]
    fn the_list_holds_nothing_that_would_reach_a_save() {
        let mut upkeep = Upkeep::new();
        upkeep.tick(&["player-one".to_owned(), "player-two".to_owned()]);
        assert_eq!(upkeep.len(), 2);
        // A reload builds a fresh upkeep, which lists nobody until the next tick.
        assert!(Upkeep::new().is_empty());
    }

    #[test]
    fn an_unreasonable_list_of_wearers_is_bounded_rather_than_tracked_whole() {
        let mut upkeep = Upkeep::new();
        let wearing: Vec<String> = (0..MAXIMUM_WEARERS + 10)
            .map(|index| format!("player-{index}"))
            .collect();
        let changes = upkeep.tick(&wearing);
        assert_eq!(upkeep.len(), MAXIMUM_WEARERS);
        assert!(
            changes.contains(&Change::Truncated(10)),
            "the surplus is reported as truncated"
        );
    }

    #[test]
    fn a_wearer_pushed_past_the_ceiling_is_not_reported_as_having_stopped_wearing() {
        let mut upkeep = Upkeep::new();
        let early = "player-early".to_owned();
        upkeep.tick(std::slice::from_ref(&early));
        assert!(upkeep.granted(&early));

        let mut crowded: Vec<String> = (0..MAXIMUM_WEARERS)
            .map(|index| format!("player-{index}"))
            .collect();
        crowded.push(early.clone());
        let changes = upkeep.tick(&crowded);

        assert!(
            !changes.contains(&Change::Removed(early)),
            "a wearer the ceiling cut off is still wearing, not gone"
        );
        assert!(changes.contains(&Change::Truncated(1)));
    }

    #[test]
    fn the_refresh_outlasts_its_interval_by_far_enough_that_removal_looks_instant() {
        assert!(
            REFRESH_TICKS
                .checked_div(REFRESH_INTERVAL_TICKS)
                .is_some_and(|refreshes| refreshes > 200),
            "the grant outlasts its interval by two hundred refreshes"
        );
    }
}
