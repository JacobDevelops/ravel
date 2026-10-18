//! The right-click crop harvest of section thirteen.
//!
//! A main-hand right click on a mature crop, on the server side only, produces
//! the crop's drops less one seed and sets the crop back to its first growth
//! stage. Nothing is consumed, nothing is damaged and no tool is required, so
//! the mechanic is entirely the withholding.
//!
//! The withholding itself is the shared library's rule, reached through its
//! sweeping harvest at a radius of nought: exactly one stack matching the crop's
//! own seed is decremented by one rather than removed, so a drop of several is
//! reduced and not deleted.
//!
//! Section thirteen names two crop kinds, and they take one path here: the
//! nether-growing crop's seed is fixed where the ordinary crop's is identified
//! by the shared library, and both reach this rule as the seed the caller
//! supplies. The kind decides nothing once the seed is in hand, so it is not
//! carried.

use ravel_sdk::content::ContentId;
use rvl_042_counterpart::arithmetic::{Mature, sweep_harvest};
use rvl_042_counterpart::inventory::Stack;

use crate::settings::{Feature, Settings};

/// The growth stage a harvested crop is set back to.
pub const FIRST_STAGE: u32 = 0;

/// Which hand the click came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hand {
    /// The main hand, which is the only one this rule answers.
    Main,
    /// The off hand, where a click does nothing.
    Off,
}

/// What a harvest produced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Harvested {
    /// The stacks that reach the world, the withheld seed already taken out.
    pub drops: Vec<Stack>,
    /// The stage the crop is left at.
    pub stage: u32,
    /// Whether the player's arm swings.
    pub swing: bool,
    /// Whether the block's own break sound plays.
    pub sound: bool,
}

/// What the click landed on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clicked {
    /// Whether it is mature.
    pub mature: bool,
    /// What breaking it outright would produce.
    pub drops: Vec<Stack>,
    /// The entry one withheld seed is taken from.
    pub seed: Option<ContentId>,
}

/// Harvests a clicked crop, or answers nothing when the click does not apply.
///
/// Nothing happens off the server, from the off hand, on an immature crop, or
/// with the feature switched off. The fixed seed of the nether-growing crop is
/// supplied by the caller in the same field as the helper's answer, because from
/// this rule's point of view the two differ only in where the seed came from.
#[must_use]
pub fn harvest(
    settings: &Settings,
    hand: Hand,
    clicked: &Clicked,
    server_side: bool,
) -> Option<Harvested> {
    if !server_side || hand != Hand::Main || !settings.switch(Feature::CropHarvest) {
        return None;
    }
    if !clicked.mature {
        return None;
    }
    let mature = Mature {
        drops: clicked.drops.clone(),
        seed: clicked.seed.clone(),
    };
    let report = sweep_harvest((0, 0, 0), 0, &|_| Some(mature.clone()), &mut |_| true);
    Some(Harvested {
        drops: report.drops,
        stage: FIRST_STAGE,
        swing: true,
        sound: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(entry: &str) -> ContentId {
        entry.parse().expect("a valid test identity")
    }

    fn crop(drops: Vec<Stack>, seed: Option<&str>) -> Clicked {
        Clicked {
            mature: true,
            drops,
            seed: seed.map(|entry| entry.parse().expect("a valid seed identity")),
        }
    }

    #[test]
    fn a_harvest_withholds_exactly_one_seed_and_keeps_the_rest_of_the_stack() {
        let settings = Settings::default();
        let clicked = crop(
            vec![
                Stack::new(id("example:produce"), 2),
                Stack::new(id("example:seed"), 3),
            ],
            Some("example:seed"),
        );
        let harvested =
            harvest(&settings, Hand::Main, &clicked, true).expect("a mature crop harvests");
        assert_eq!(
            harvested.drops,
            vec![
                Stack::new(id("example:produce"), 2),
                Stack::new(id("example:seed"), 2)
            ]
        );
        assert_eq!(harvested.stage, FIRST_STAGE);
        assert!(harvested.swing);
        assert!(harvested.sound);
    }

    #[test]
    fn a_single_seed_stack_is_removed_rather_than_left_holding_nothing() {
        let settings = Settings::default();
        let clicked = crop(
            vec![
                Stack::new(id("example:produce"), 1),
                Stack::new(id("example:seed"), 1),
            ],
            Some("example:seed"),
        );
        let harvested = harvest(&settings, Hand::Main, &clicked, true).expect("a harvest");
        assert_eq!(harvested.drops, vec![Stack::new(id("example:produce"), 1)]);
    }

    #[test]
    fn only_the_first_matching_stack_is_reduced() {
        let settings = Settings::default();
        let clicked = crop(
            vec![
                Stack::new(id("example:seed"), 2),
                Stack::new(id("example:seed"), 2),
            ],
            Some("example:seed"),
        );
        let harvested = harvest(&settings, Hand::Main, &clicked, true).expect("a harvest");
        assert_eq!(
            harvested.drops,
            vec![
                Stack::new(id("example:seed"), 1),
                Stack::new(id("example:seed"), 2)
            ]
        );
    }

    #[test]
    fn a_crop_whose_seed_is_not_identified_drops_everything() {
        let settings = Settings::default();
        let clicked = crop(vec![Stack::new(id("example:produce"), 4)], None);
        let harvested = harvest(&settings, Hand::Main, &clicked, true).expect("a harvest");
        assert_eq!(harvested.drops, vec![Stack::new(id("example:produce"), 4)]);
    }

    #[test]
    fn the_nether_growing_crop_takes_its_fixed_seed_the_same_way() {
        let settings = Settings::default();
        let clicked = Clicked {
            mature: true,
            drops: vec![Stack::new(id("example:nether_seed"), 2)],
            seed: Some(id("example:nether_seed")),
        };
        let harvested = harvest(&settings, Hand::Main, &clicked, true).expect("a harvest");
        assert_eq!(
            harvested.drops,
            vec![Stack::new(id("example:nether_seed"), 1)]
        );
        assert_eq!(harvested.stage, FIRST_STAGE);
    }

    #[test]
    fn an_off_hand_click_a_client_and_an_immature_crop_all_do_nothing() {
        let settings = Settings::default();
        let clicked = crop(
            vec![Stack::new(id("example:seed"), 2)],
            Some("example:seed"),
        );
        assert!(harvest(&settings, Hand::Off, &clicked, true).is_none());
        assert!(harvest(&settings, Hand::Main, &clicked, false).is_none());

        let immature = Clicked {
            mature: false,
            ..clicked
        };
        assert!(harvest(&settings, Hand::Main, &immature, true).is_none());
    }

    #[test]
    fn switching_the_feature_off_stops_the_harvest() {
        let mut settings = Settings::default();
        settings.switches[Feature::CropHarvest.index()] = false;
        let clicked = crop(
            vec![Stack::new(id("example:seed"), 2)],
            Some("example:seed"),
        );
        assert!(harvest(&settings, Hand::Main, &clicked, true).is_none());
    }
}
