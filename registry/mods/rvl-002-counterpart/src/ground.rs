//! The sixth-tier ground: the sixth member of a family the dependency
//! specifies.
//!
//! Moisture, drops, the fall rule and the conversion target behaviour are the
//! dependency's and are consumed rather than restated. Conversion into this
//! entry's ground is gated on the *dependency's* flag rather than on any setting
//! of this entry's.

use ravel_sdk::JsonValue;
use rvl_321_counterpart::ground::{self, Converted};
use rvl_321_counterpart::model::Tier;

/// Converts ground with this tier's material held.
///
/// This short-circuits to a pass-through when the dependency's flag is off,
/// before consulting anything else. Gating on a local setting instead diverges
/// whenever the two disagree.
#[must_use]
pub fn convert(
    conversion_enabled: bool,
    current: Option<&Tier>,
    plain_tilled: bool,
    held: &Tier,
    moisture: u8,
    draw: u32,
) -> Converted {
    if !conversion_enabled {
        return Converted::default();
    }
    ground::convert(current, plain_tilled, held, moisture, draw)
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
        crate::tier::declared(
            "test:6".parse().expect("a legal tier identity"),
            "test:ground".parse().expect("a legal ground identity"),
            "test:material".parse().expect("a legal material identity"),
            false,
        )
    }

    #[test]
    fn conversion_is_gated_on_the_dependencys_resolved_flag() {
        let held = sixth();
        assert!(convert(false, None, true, &held, 4, 0).passed_through());
        let converted = convert(true, None, true, &held, 4, 0);
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
