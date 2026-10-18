//! The sixth tier: one object declared with four values and adjusted with four
//! more.
//!
//! The model is the dependency's, so nothing about a tier is restated here. What
//! is here is this entry's one instance of it, the two-pass shape that instance
//! needs, and the one boolean the whole progression gate rests on.

use rvl_042_counterpart::arithmetic::Colour;
use rvl_321_counterpart::model::Tier;

/// The path this entry's tier is registered at, inside its own namespace.
pub const PATH: &str = "6";

/// The numeric value of the tier, one above the highest the dependency declares.
pub const VALUE: u32 = 6;

/// The display colour, a dark violet, used for the ground block and its item.
pub const DISPLAY_COLOUR: u32 = 0x0040_005E;

/// The host text style the tier's name is drawn in.
pub const TEXT_STYLE: TextStyle = TextStyle::DarkViolet;

/// The host formatting a tier's name is drawn with.
///
/// Ravel publishes no text-style vocabulary, so the one style this entry names
/// is carried as a value rather than as a host constant. It is declared data
/// either way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextStyle {
    /// The host's dark violet formatting.
    DarkViolet,
}

impl TextStyle {
    /// The opaque colour this style draws in.
    #[must_use]
    pub const fn colour(self) -> Colour {
        match self {
            Self::DarkViolet => Colour::new(255, 0x40, 0x00, 0x5E),
        }
    }
}

/// The tier as the register pass declares it.
///
/// Four values, and nothing that names content: the ground block and the
/// material item do not exist yet when this pass runs, which is why pointing at
/// them is [`adjust`]'s work and not this function's.
#[must_use]
pub fn registered(id: ravel_sdk::content::ContentId) -> Tier {
    let mut tier = Tier::new(id, VALUE);
    tier.colour = Colour::unpack(DISPLAY_COLOUR).with_opacity(255);
    tier.text_colour = TEXT_STYLE.colour();
    tier
}

/// The tier as the adjust pass leaves it.
///
/// Four more values: the ground block, the material item, the accelerant switch
/// copied out of the settings, and the second-drop flag turned off.
///
/// That last one is the tier's whole interaction with the dependency's harvest
/// rule, and it is the difference between a progression gate and none: a
/// sixth-tier crop never yields a second seed, whatever the general rule
/// computes.
///
/// The accelerant value is *copied* rather than read live, which is the
/// behaviour the specification records and the reason this takes a `bool` rather
/// than a settings reference.
pub fn adjust(
    tier: &mut Tier,
    ground: ravel_sdk::content::ContentId,
    material: ravel_sdk::content::ContentId,
    fertilisable: bool,
) {
    tier.adjust(ground, material);
    tier.fertilisable = fertilisable;
    tier.second_drop = false;
}

/// The tier fully declared, as both passes leave it.
#[must_use]
pub fn declared(
    id: ravel_sdk::content::ContentId,
    ground: ravel_sdk::content::ContentId,
    material: ravel_sdk::content::ContentId,
    fertilisable: bool,
) -> Tier {
    let mut tier = registered(id);
    adjust(&mut tier, ground, material, fertilisable);
    tier
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvl_321_counterpart::model::shipped_tiers;

    use crate::settings::Settings;

    #[test]
    fn the_tier_sits_exactly_one_above_the_highest_the_dependency_ships() {
        let highest = shipped_tiers()
            .iter()
            .map(|tier| tier.value)
            .max()
            .expect("the dependency ships tiers");
        assert_eq!(VALUE, highest + 1);
    }

    #[test]
    fn the_register_pass_declares_four_values_and_points_at_no_content() {
        let tier = registered("test:6".parse().expect("a legal tier identity"));
        assert_eq!(tier.value, VALUE);
        assert_eq!(tier.colour, Colour::new(255, 0x40, 0x00, 0x5E));
        assert_eq!(tier.text_colour, TEXT_STYLE.colour());
        assert_eq!(tier.ground, None);
        assert_eq!(tier.material, None);
    }

    #[test]
    fn the_display_colour_is_the_declared_constant_at_full_opacity() {
        let tier = registered("test:6".parse().expect("a legal tier identity"));
        assert_eq!(tier.colour.red, 0x40);
        assert_eq!(tier.colour.green, 0x00);
        assert_eq!(tier.colour.blue, 0x5E);
        assert_eq!(tier.colour.opacity, 255);
        assert_eq!(tier.colour.pack() & 0x00FF_FFFF, DISPLAY_COLOUR);
    }

    #[test]
    fn the_adjust_pass_turns_the_second_drop_off_whatever_the_tier_arrived_holding() {
        let mut tier = registered("test:6".parse().expect("a legal tier identity"));
        tier.second_drop = true;
        adjust(
            &mut tier,
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
            false,
        );
        assert!(
            !tier.second_drop,
            "a sixth-tier crop never yields a second seed"
        );
    }

    #[test]
    fn the_accelerant_switch_is_copied_once_rather_than_read_live() {
        let mut settings = Settings {
            crops_fertilisable: true,
            ..Settings::default()
        };
        let tier = declared(
            "test:6".parse().expect("a legal tier identity"),
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
            settings.crops_fertilisable,
        );
        assert!(tier.fertilisable);

        settings.crops_fertilisable = false;
        assert!(
            tier.fertilisable,
            "the tier holds a copy the caller's later change cannot reach"
        );
        assert!(!settings.crops_fertilisable);

        let off = declared(
            "test:6".parse().expect("a legal tier identity"),
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
            false,
        );
        assert!(!off.fertilisable);
    }

    #[test]
    fn the_adjust_pass_points_the_tier_at_its_ground_and_its_material() {
        let tier = declared(
            "test:6".parse().expect("a legal tier identity"),
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
            false,
        );
        let ground = "test:ground".parse().expect("a legal ground identity");
        let material = "test:material".parse().expect("a legal material identity");
        let other = "test:other".parse().expect("a legal ground identity");
        assert_eq!(tier.ground.as_ref(), Some(&ground));
        assert_eq!(tier.material.as_ref(), Some(&material));
        assert!(tier.is_effective_ground(Some(&ground)));
        assert!(!tier.is_effective_ground(Some(&other)));
    }

    #[test]
    fn the_registered_path_is_the_number_and_nothing_else() {
        assert_eq!(PATH.parse::<u32>().ok(), Some(VALUE));
    }
}
