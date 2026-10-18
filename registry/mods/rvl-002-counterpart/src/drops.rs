//! Boss drops: one subscriber on the host's entity-drops event.
//!
//! Registered at common setup and therefore server-side. Three rules, in order,
//! and the order is the load-bearing part: the game rule is tested before any
//! draw, so turning mob loot off gates both drops and consumes nothing.
//!
//! One declared change. The original draws from a process-wide random source, so
//! its drop is not reproducible from a world seed and is not synchronised with
//! anything else in the world. This takes the draw from the level's own source,
//! which makes the drop seed-reproducible and will disagree with the original on
//! any replay comparison.

use ravel_sdk::content::ContentId;
use rvl_042_counterpart::inventory::Stack;

use crate::settings::Settings;

/// Which boss died.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Boss {
    /// The wither-class boss, whose drop is a draw.
    WitherClass,
    /// The dragon-class boss, whose drop is a certainty.
    DragonClass,
}

/// What one death adds to the event's drop list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Added {
    /// The stacks added, at the boss's exact position.
    pub stacks: Vec<Stack>,
    /// Whether a draw was taken at all.
    pub drew: bool,
}

/// What this entry adds to a drop event.
///
/// `draw` is the level's own source, offered as a fraction in `[0, 1)`. It is
/// only consulted for the wither-class branch, and only after the game rule has
/// been tested: the rule gates both drops before anything is drawn.
///
/// The dragon-class branch fires on every death of that boss, including
/// respawned ones, because nothing tracks first-kill state.
#[must_use]
pub fn on_death(
    boss: Boss,
    mob_loot: bool,
    settings: &Settings,
    soul: &ContentId,
    scale: &ContentId,
    draw: f64,
) -> Added {
    if !mob_loot {
        return Added::default();
    }
    match boss {
        Boss::WitherClass => {
            let dropped = draw < settings.drops_soul_chance;
            Added {
                stacks: if dropped {
                    vec![Stack::new(soul.clone(), 1)]
                } else {
                    Vec::new()
                },
                drew: true,
            }
        }
        Boss::DragonClass => Added {
            stacks: if settings.drops_scale_count > 0 {
                vec![Stack::new(
                    scale.clone(),
                    u64::from(settings.drops_scale_count),
                )]
            } else {
                Vec::new()
            },
            drew: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(entry: &str) -> ContentId {
        entry.parse().expect("a valid test identity")
    }

    const SOUL: &str = "test:soul";
    const SCALE: &str = "test:scale";

    #[test]
    fn the_game_rule_is_tested_before_any_draw_is_taken() {
        let settings = Settings::default();
        for boss in [Boss::WitherClass, Boss::DragonClass] {
            let added = on_death(boss, false, &settings, &id(SOUL), &id(SCALE), 0.0);
            assert_eq!(added.stacks.len(), 0);
            assert!(!added.drew, "the rule gates the drop and consumes nothing");
        }
    }

    #[test]
    fn the_wither_class_drop_is_a_draw_against_the_configured_chance() {
        let settings = Settings::default();
        let below = on_death(
            Boss::WitherClass,
            true,
            &settings,
            &id(SOUL),
            &id(SCALE),
            0.34,
        );
        assert_eq!(below.stacks, vec![Stack::new(id(SOUL), 1)]);
        assert!(below.drew);

        let above = on_death(
            Boss::WitherClass,
            true,
            &settings,
            &id(SOUL),
            &id(SCALE),
            0.35,
        );
        assert_eq!(above.stacks.len(), 0);
        assert!(above.drew, "a draw was still taken");
    }

    #[test]
    fn a_chance_of_zero_never_drops_and_a_chance_of_one_always_does() {
        let mut settings = Settings {
            drops_soul_chance: 0.0,
            ..Settings::default()
        };
        for draw in [0.0, 0.5, 0.999] {
            assert_eq!(
                on_death(
                    Boss::WitherClass,
                    true,
                    &settings,
                    &id(SOUL),
                    &id(SCALE),
                    draw
                )
                .stacks
                .len(),
                0
            );
        }
        settings.drops_soul_chance = 1.0;
        for draw in [0.0, 0.5, 0.999] {
            assert_eq!(
                on_death(
                    Boss::WitherClass,
                    true,
                    &settings,
                    &id(SOUL),
                    &id(SCALE),
                    draw
                )
                .stacks
                .len(),
                1
            );
        }
    }

    #[test]
    fn the_dragon_class_drop_is_a_certainty_and_takes_no_draw() {
        let settings = Settings::default();
        let added = on_death(
            Boss::DragonClass,
            true,
            &settings,
            &id(SOUL),
            &id(SCALE),
            0.999,
        );
        assert_eq!(added.stacks, vec![Stack::new(id(SCALE), 8)]);
        assert!(!added.drew, "the count is a certainty");
    }

    #[test]
    fn a_scale_count_of_zero_turns_the_drop_off_rather_than_dropping_nothing_visibly() {
        let settings = Settings {
            drops_scale_count: 0,
            ..Settings::default()
        };
        let added = on_death(
            Boss::DragonClass,
            true,
            &settings,
            &id(SOUL),
            &id(SCALE),
            0.0,
        );
        assert_eq!(added.stacks.len(), 0);
        assert!(!added.drew);
    }

    #[test]
    fn the_dragon_class_branch_fires_on_every_death_including_a_later_one() {
        let settings = Settings::default();
        for _ in 0..3 {
            let added = on_death(
                Boss::DragonClass,
                true,
                &settings,
                &id(SOUL),
                &id(SCALE),
                0.0,
            );
            assert_eq!(added.stacks.len(), 1, "nothing tracks first-kill state");
        }
    }

    #[test]
    fn the_handed_in_draw_is_the_only_thing_the_drop_depends_on() {
        let settings = Settings::default();
        let below = on_death(
            Boss::WitherClass,
            true,
            &settings,
            &id(SOUL),
            &id(SCALE),
            0.2,
        );
        let above = on_death(
            Boss::WitherClass,
            true,
            &settings,
            &id(SOUL),
            &id(SCALE),
            0.9,
        );
        assert_ne!(
            below, above,
            "either side of the configured chance answers differently"
        );
        for _ in 0..16 {
            assert_eq!(
                on_death(
                    Boss::WitherClass,
                    true,
                    &settings,
                    &id(SOUL),
                    &id(SCALE),
                    0.2
                ),
                below,
                "no source of the drop's own is consulted"
            );
        }
    }
}
