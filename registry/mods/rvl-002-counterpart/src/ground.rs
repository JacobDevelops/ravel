//! The sixth-tier ground: the sixth member of a family the dependency
//! specifies.
//!
//! Moisture, the fall rule and the conversion target behaviour are the
//! dependency's and are consumed rather than restated. Three things are this
//! entry's: the block ships no loot document and computes its drops in code, its
//! fall damage is unconditional and the block survives, and conversion into it
//! is gated on the *dependency's* flag rather than on any setting of this
//! entry's.

use ravel_sdk::JsonValue;
use rvl_042_counterpart::flags::FlagTable;
use rvl_042_counterpart::inventory::Stack;
use rvl_321_counterpart::ground::{self, Converted};
use rvl_321_counterpart::model::Tier;
use rvl_321_counterpart::settings as depended;

/// The multiplier fall damage is taken at when an entity lands on this ground.
pub const FALL_MULTIPLIER: f64 = ground::FALL_MULTIPLIER;

/// Whether this block ships a loot document.
///
/// It does not, and that is the fact `loot-parity` needs: nineteen of the twenty
/// block items have a document and this one has a code path instead.
pub const SHIPS_LOOT_DOCUMENT: bool = false;

/// What breaking this ground drops.
///
/// Silk touch yields the block itself; anything else yields one dirt plus, on a
/// draw under the tier material percentage, one of the tier's material. The draw
/// is the level's own, handed in rather than taken here.
#[must_use]
pub fn drops(tier: &Tier, silk_touch: bool, dirt: &str, draw: u32) -> Vec<Stack> {
    ground::drops(tier, silk_touch, dirt, draw)
}

/// What an entity landing on this ground takes, and whether the block survives.
///
/// Full fall damage at a multiplier of one, and the block stays. The host's own
/// tilled ground reverts to dirt instead, which is why this is stated rather
/// than inherited: a block that inherits the host behaviour gets the opposite of
/// both halves.
#[must_use]
pub fn landed_on(fall_damage: f64) -> (f64, bool) {
    (fall_damage * FALL_MULTIPLIER, true)
}

/// Whether conversion into this ground is permitted at all.
///
/// The gate is the dependency's ground-conversion flag, consulted before
/// anything of this entry's. An unpublished flag reads as off, which is the safe
/// reading: a conversion that consumes a material is not something to perform on
/// a flag nobody declared.
#[must_use]
pub fn conversion_enabled(flags: &FlagTable) -> bool {
    depended::flag_id(depended::GROUND_CONVERT_BY_MATERIAL).is_some_and(|flag| flags.enabled(&flag))
}

/// Converts ground with this tier's material held.
///
/// This short-circuits to a pass-through when the dependency's flag is off,
/// before consulting anything else. Gating on a local setting instead diverges
/// whenever the two disagree.
#[must_use]
pub fn convert(
    flags: &FlagTable,
    current: Option<&Tier>,
    plain_tilled: bool,
    held: &Tier,
    moisture: u8,
    draw: u32,
) -> Converted {
    if !conversion_enabled(flags) {
        return Converted::default();
    }
    ground::convert(current, plain_tilled, held, moisture, true, draw)
}

/// The tilling recipe that reaches this ground without owning a converter.
///
/// One hoe, one tier material and one dirt, of the dependency's tilling kind.
/// The document is declared data; this is the shape `recipe-parity` counts.
#[must_use]
pub fn tilling_document(namespace: &str, material: &str, ground_block: &str) -> JsonValue {
    ravel_sdk::json!({
        "type": format!("{}:tilling", rvl_321_counterpart::MOD_ID),
        "namespace": namespace,
        "tool": "minecraft:hoe",
        "ingredients": [material, "minecraft:dirt"],
        "result": { "id": ground_block, "count": 1 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sixth() -> Tier {
        crate::tier::declared("test:6", "test:ground", "test:material", false)
    }

    #[test]
    fn this_block_carries_a_code_path_where_the_other_nineteen_carry_a_document() {
        assert_eq!(
            usize::from(SHIPS_LOOT_DOCUMENT) + crate::data::LOOT_DOCUMENTS,
            crate::census::BLOCK_ITEMS - 1,
            "the twentieth block item has a code path where the others have a document"
        );
    }

    #[test]
    fn a_silk_touch_break_yields_the_block_and_nothing_else() {
        let dropped = drops(&sixth(), true, "minecraft:dirt", 0);
        assert_eq!(dropped, vec![Stack::new("test:ground", 1)]);
    }

    #[test]
    fn an_ordinary_break_yields_dirt_and_the_material_a_quarter_of_the_time() {
        let tier = sixth();
        for draw in 0..100u32 {
            let dropped = drops(&tier, false, "minecraft:dirt", draw);
            assert_eq!(dropped[0], Stack::new("minecraft:dirt", 1));
            if draw < 25 {
                assert_eq!(dropped.len(), 2);
                assert_eq!(dropped[1], Stack::new("test:material", 1));
            } else {
                assert_eq!(dropped.len(), 1);
            }
        }
    }

    #[test]
    fn a_draw_far_above_a_hundred_still_lands_in_the_quarter_it_should() {
        let tier = sixth();
        assert_eq!(drops(&tier, false, "minecraft:dirt", 1024).len(), 2);
        assert_eq!(drops(&tier, false, "minecraft:dirt", 1099).len(), 1);
        assert_eq!(
            drops(&tier, false, "minecraft:dirt", u32::MAX).len(),
            usize::from(u32::MAX % 100 < 25) + 1
        );
    }

    #[test]
    fn fall_damage_is_unconditional_and_the_block_survives() {
        let (damage, survives) = landed_on(7.5);
        assert!((damage - 7.5).abs() < f64::EPSILON);
        assert!(
            survives,
            "the host's own tilled ground would revert to dirt"
        );
        assert!((FALL_MULTIPLIER - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn conversion_is_gated_on_the_dependencys_flag_and_nothing_of_this_entrys() {
        let held = sixth();
        let mut flags = FlagTable::new();
        assert!(!conversion_enabled(&flags));
        assert!(
            convert(&flags, None, true, &held, 4, 0).passed_through(),
            "an unpublished flag short-circuits before anything else is consulted"
        );

        // This entry's own five flags are all published and none of them opens
        // the gate.
        let settings = crate::settings::Settings {
            crops_fertilisable: true,
            ..crate::settings::Settings::default()
        };
        crate::settings::publish(&mut flags, &settings);
        assert!(!conversion_enabled(&flags));
        assert!(convert(&flags, None, true, &held, 4, 0).passed_through());

        let flag = depended::flag_id(depended::GROUND_CONVERT_BY_MATERIAL)
            .expect("the dependency names its flag");
        flags.publish_constant(flag, true);
        assert!(conversion_enabled(&flags));
        let converted = convert(&flags, None, true, &held, 4, 0);
        assert!(!converted.passed_through());
        assert_eq!(converted.ground.as_deref(), Some("test:ground"));
        assert_eq!(converted.moisture, 4, "moisture carries across unchanged");
        assert_eq!(converted.consumed, 1);
    }

    #[test]
    fn the_tilling_document_names_the_dependencys_kind_and_two_ingredients() {
        let document = tilling_document("test", "test:material", "test:ground");
        assert_eq!(
            document["type"],
            JsonValue::from(format!("{}:tilling", rvl_321_counterpart::MOD_ID))
        );
        assert_eq!(
            document["ingredients"]
                .as_array()
                .expect("two ingredients")
                .len(),
            2
        );
        assert_eq!(document["result"]["count"], JsonValue::from(1));
        assert_eq!(document["result"]["id"], JsonValue::from("test:ground"));
    }
}
