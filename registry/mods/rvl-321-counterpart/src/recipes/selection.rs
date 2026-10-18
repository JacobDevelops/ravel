//! Crop-derived seed and conversion recipes, independent of recipe matching.

use ravel_sdk::content::ContentId;
use rvl_042_counterpart::Stack;

use crate::model::{Crafted, Crop, CropKind, Tier};

use super::{FourStation, Ingredient};

/// One recipe derived from a crop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedRecipe {
    /// The four-station ritual producing one seed.
    RitualSeed(FourStation),
    /// The plain-grid seed recipe, gated on a setting that ships off.
    GridSeed {
        /// The recipe's own identifier.
        id: String,
        /// The nine grid positions, reading across then down.
        grid: Vec<Option<Ingredient>>,
        /// What the recipe produces.
        result: Stack,
    },
    /// One seed turned into two of the crop's produce.
    Conversion {
        /// The recipe's own identifier.
        id: String,
        /// The seed consumed.
        input: Ingredient,
        /// What the recipe produces.
        result: Stack,
    },
}

impl DerivedRecipe {
    /// The recipe's own identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::RitualSeed(ritual) => &ritual.id,
            Self::GridSeed { id, .. } | Self::Conversion { id, .. } => id,
        }
    }
}

/// The suffix the ritual seed recipe is named with.
pub const RITUAL_SUFFIX: &str = "_seed_ritual";
/// The suffix the plain-grid seed recipe is named with.
pub const GRID_SUFFIX: &str = "_seed_grid";
/// The suffix the seed-conversion recipe is named with.
pub const CONVERSION_SUFFIX: &str = "_seed_conversion";

/// Derives the three recipes a crop contributes.
///
/// A disabled crop contributes nothing. A crop whose crafting material resolves
/// to nothing contributes no seed recipe and still contributes its conversion
/// recipe, so a seed obtained another way stays usable.
#[must_use]
pub fn derive(
    crop: &Crop,
    tier: &Tier,
    kind: &CropKind,
    grid_seeds_enabled: bool,
    resolves: &dyn Fn(&ContentId) -> usize,
) -> Vec<DerivedRecipe> {
    if !crop.enabled {
        return Vec::new();
    }
    let path = crop.path();
    let seed: ContentId = crate::registries::registered_id(crop, crate::registries::Role::Seed)
        .parse()
        .expect("a generated seed identity");
    let produce: ContentId =
        crate::registries::registered_id(crop, crate::registries::Role::Material)
            .parse()
            .expect("a generated material identity");
    let mut derived = Vec::new();

    let crafted = crafted_ingredient(&crop.crafted, resolves);
    if let (Some(crafted), Some(material), Some(base)) =
        (crafted, tier.material.as_ref(), kind.base_seed.as_ref())
    {
        if crop.derived.ritual_seed {
            derived.push(ritual_seed(path, &seed, &crafted, material, base));
        }
        if crop.derived.grid_seed && grid_seeds_enabled {
            derived.push(grid_seed(path, &seed, crafted, material, base));
        }
    }

    if crop.derived.conversion {
        derived.push(DerivedRecipe::Conversion {
            id: format!("{}:{path}{CONVERSION_SUFFIX}", crate::MOD_ID),
            input: Ingredient::Entry(seed),
            result: Stack::new(produce, 2),
        });
    }
    derived
}

fn ritual_seed(
    path: &str,
    seed: &ContentId,
    crafted: &Ingredient,
    material: &ContentId,
    base: &ContentId,
) -> DerivedRecipe {
    let mut ring = Vec::with_capacity(8);
    for index in 0..8 {
        ring.push(if index % 2 == 0 {
            crafted.clone()
        } else {
            Ingredient::Entry(material.clone())
        });
    }
    DerivedRecipe::RitualSeed(FourStation {
        id: format!("{}:{path}{RITUAL_SUFFIX}", crate::MOD_ID),
        altar: Ingredient::Entry(base.clone()),
        ring,
        result: Stack::new(seed.clone(), 1),
        copies_data: false,
    })
}

fn grid_seed(
    path: &str,
    seed: &ContentId,
    crafted: Ingredient,
    material: &ContentId,
    base: &ContentId,
) -> DerivedRecipe {
    let edge = Ingredient::Entry(material.clone());
    let grid = vec![
        Some(crafted.clone()),
        Some(edge.clone()),
        Some(crafted.clone()),
        Some(edge.clone()),
        Some(Ingredient::Entry(base.clone())),
        Some(edge.clone()),
        Some(crafted.clone()),
        Some(edge),
        Some(crafted),
    ];
    DerivedRecipe::GridSeed {
        id: format!("{}:{path}{GRID_SUFFIX}", crate::MOD_ID),
        grid,
        result: Stack::new(seed.clone(), 1),
    }
}

fn crafted_ingredient(
    crafted: &Crafted,
    resolves: &dyn Fn(&ContentId) -> usize,
) -> Option<Ingredient> {
    match crafted {
        Crafted::Entry { id, .. } => Some(Ingredient::Entry(id.clone())),
        Crafted::Tag(tag) => (resolves(tag) > 0).then(|| Ingredient::Tag(tag.clone())),
        Crafted::Nothing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DerivedRecipe, derive};
    use crate::model::{Crafted, Crop, CropKind, Derived, Tier};

    fn declared() -> (Crop, Tier, CropKind) {
        let mut tier = Tier::new("test:tier".parse().expect("tier id"), 1);
        tier.material = Some("test:material".parse().expect("material id"));
        let mut kind = CropKind::new("test:kind".parse().expect("kind id"), "stem");
        kind.base_seed = Some("test:base".parse().expect("base id"));
        let crop = Crop::new(
            "test:crop".parse().expect("crop id"),
            tier.id.clone(),
            kind.id.clone(),
            Crafted::Tag("test:crafted".parse().expect("tag id")),
        );
        (crop, tier, kind)
    }

    #[test]
    fn each_crop_recipe_switch_independently_removes_its_recipe() {
        let (mut crop, tier, kind) = declared();
        for (switches, expected) in [
            (
                Derived {
                    ritual_seed: true,
                    grid_seed: false,
                    conversion: false,
                },
                "_seed_ritual",
            ),
            (
                Derived {
                    ritual_seed: false,
                    grid_seed: true,
                    conversion: false,
                },
                "_seed_grid",
            ),
            (
                Derived {
                    ritual_seed: false,
                    grid_seed: false,
                    conversion: true,
                },
                "_seed_conversion",
            ),
        ] {
            crop.derived = switches;
            let recipes = derive(&crop, &tier, &kind, true, &|tag| {
                assert_eq!(tag, &"test:crafted".parse().expect("tag id"));
                1
            });
            assert_eq!(recipes.len(), 1);
            assert_eq!(recipes[0].id(), format!("{}:crop{expected}", crate::MOD_ID));
        }
        crop.derived = Derived {
            ritual_seed: false,
            grid_seed: false,
            conversion: false,
        };
        assert_eq!(
            derive(&crop, &tier, &kind, true, &|_| 1),
            Vec::<DerivedRecipe>::new()
        );
    }

    #[test]
    fn nothing_keeps_conversion_and_disabled_crops_never_resolve() {
        let (mut crop, tier, kind) = declared();
        crop.crafted = Crafted::Nothing;
        let recipes = derive(&crop, &tier, &kind, true, &|_| {
            panic!("Nothing must not resolve crafting materials")
        });
        assert_eq!(recipes.len(), 1);
        assert!(matches!(recipes[0], DerivedRecipe::Conversion { .. }));
        crop.enabled = false;
        crop.crafted = Crafted::Tag("test:crafted".parse().expect("tag id"));
        assert_eq!(
            derive(&crop, &tier, &kind, true, &|_| {
                panic!("disabled crops must not resolve crafting materials")
            }),
            Vec::<DerivedRecipe>::new()
        );
    }
}
