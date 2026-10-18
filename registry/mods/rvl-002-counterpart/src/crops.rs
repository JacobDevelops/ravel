//! The six crops and the three states of absence.
//!
//! All six sit at the sixth tier, all six are of the dependency's resource kind,
//! and all six are registered unconditionally. Four of them then name one
//! companion mod each and are marked unobtainable when it is absent.
//!
//! The three states are the dependency's and are not restated. What is here is
//! this entry's six instances of them, the one crop that reaches the third state
//! in practice, and the fact that pointing a crop at its anchor block happens
//! for all six including the unobtainable ones.

use rvl_321_counterpart::model::{Crafted, Crop, Derived};
use rvl_321_counterpart::{Availability, availability};

use crate::data::OWN_TAG;
use crate::{CONVENTION_NAMESPACE, HOST_NAMESPACE};

/// The crop kind every one of the six is declared at.
pub const KIND: &str = "rvl_321_counterpart:resource";

/// How many crops this entry declares.
pub const COUNT: usize = 6;

/// The two spellings the one tag of this entry's own carries.
///
/// Both are declared not-required, so the tag resolves empty when the companion
/// mod is loaded and provides neither. That is what makes the third state
/// reachable rather than hypothetical: the companion renamed the item at some
/// point and both names are carried.
pub const OPTIONAL_TAG_MEMBERS: usize = 2;

/// The two spellings that tag carries, in the companion mod's namespace.
///
/// Both are declared not-required. The companion renamed the item at some point
/// and a counterpart that carried one spelling would resolve the tag whenever
/// the original resolved it empty.
pub const OPTIONAL_TAG_SPELLINGS: [&str; OPTIONAL_TAG_MEMBERS] = ["material", "renamed_material"];

/// The item path a single crafting material is named at.
pub const MATERIAL_PATH: &str = "material";

/// How a crop's crafting material is named.
///
/// The shape matters rather than the name: it decides which of the three states
/// a crop can reach, and the specification excludes the names themselves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Material {
    /// A single host item. Always resolves.
    HostEntry,
    /// A single item of this entry's own. Always resolves.
    OwnEntry,
    /// A single item of another mod's. Resolves whenever that mod is loaded.
    CompanionEntry,
    /// A conventional cross-mod tag. Resolves when anything filled it.
    ConventionTag,
    /// A tag of this entry's own, both members optional. May resolve empty.
    OwnTag,
}

/// One of the six, as this entry declares it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Declared {
    /// The path the crop is registered at.
    pub path: &'static str,
    /// How its crafting material is named.
    pub material: Material,
    /// The companion mod it names, if it names one.
    pub companion: Option<&'static str>,
    /// The anchor block it is pointed at in the adjust pass.
    pub anchor: &'static str,
}

impl Declared {
    /// Whether this crop declares a companion mod at all.
    #[must_use]
    pub const fn declares_companion(&self) -> bool {
        self.companion.is_some()
    }

    /// The identifier this crop's crafting material is named by.
    ///
    /// Each of the five shapes names a different space: the host's, this
    /// entry's, the companion's, the convention namespace no mod owns, and this
    /// entry's own two-member tag. Collapsing them onto one name would put the
    /// two tag-shaped crops in the third state of section six permanently.
    #[must_use]
    pub fn material_id(&self, namespace: &str) -> String {
        let companion = self.companion.unwrap_or(namespace);
        match self.material {
            Material::HostEntry => format!("{HOST_NAMESPACE}:{MATERIAL_PATH}"),
            Material::OwnEntry => format!("{namespace}:{MATERIAL_PATH}"),
            Material::CompanionEntry => format!("{companion}:{MATERIAL_PATH}"),
            Material::ConventionTag => format!("{CONVENTION_NAMESPACE}:{}", self.path),
            Material::OwnTag => format!("{namespace}:{OWN_TAG}"),
        }
    }

    /// The crop as the register pass declares it.
    ///
    /// Every derivation is switched off, because the extension of section four
    /// opts out of all three for this namespace and ships equivalent documents
    /// instead.
    #[must_use]
    pub fn crop(
        &self,
        id: ravel_sdk::content::ContentId,
        tier: &ravel_sdk::content::ContentId,
        material_id: ravel_sdk::content::ContentId,
    ) -> Crop {
        let crafted = match self.material {
            Material::HostEntry | Material::OwnEntry | Material::CompanionEntry => Crafted::Entry {
                id: material_id,
                data: None,
            },
            Material::ConventionTag | Material::OwnTag => Crafted::Tag(material_id),
        };
        let mut crop = Crop::new(
            id,
            tier.clone(),
            KIND.parse().expect("a legal kind identity"),
            crafted,
        );
        crop.derived = OPT_OUT;
        crop
    }
}

/// What the extension of section four contributes: nothing derived.
pub const OPT_OUT: Derived = Derived {
    ritual_seed: false,
    grid_seed: false,
    conversion: false,
};

/// The six, in the order the creative listing walks their anchor blocks.
pub const DECLARED: [Declared; COUNT] = [
    Declared {
        path: "host_entry_crop",
        material: Material::HostEntry,
        companion: None,
        anchor: "host_entry_anchor",
    },
    Declared {
        path: "own_entry_crop",
        material: Material::OwnEntry,
        companion: None,
        anchor: "own_entry_anchor",
    },
    Declared {
        path: "companion_entry_crop_a",
        material: Material::CompanionEntry,
        companion: Some("companion_a"),
        anchor: "companion_entry_anchor_a",
    },
    Declared {
        path: "companion_tag_crop_b",
        material: Material::ConventionTag,
        companion: Some("companion_b"),
        anchor: "companion_tag_anchor_b",
    },
    Declared {
        path: "own_tag_crop_c",
        material: Material::OwnTag,
        companion: Some("companion_c"),
        anchor: "own_tag_anchor_c",
    },
    Declared {
        path: "companion_entry_crop_d",
        material: Material::CompanionEntry,
        companion: Some("companion_d"),
        anchor: "companion_entry_anchor_d",
    },
];

/// The members of the one tag this entry owns, as identifiers.
///
/// They are the companion's names rather than this entry's: the tag exists to
/// carry both spellings of an item that mod renamed.
#[must_use]
pub fn optional_tag_members() -> Vec<String> {
    DECLARED
        .iter()
        .filter(|declared| declared.material == Material::OwnTag)
        .filter_map(|declared| declared.companion)
        .flat_map(|companion| {
            OPTIONAL_TAG_SPELLINGS
                .iter()
                .map(move |spelling| format!("{companion}:{spelling}"))
        })
        .collect()
}

/// Which state a declared crop is in.
///
/// `loaded` answers whether a named mod is present. A development environment
/// enables every crop regardless of that answer, which is the escape hatch the
/// dependency records and the reason a development run and a production run of
/// the same build have different content.
#[must_use]
pub fn state(
    declared: &Declared,
    crop: &Crop,
    development: bool,
    loaded: &dyn Fn(&str) -> bool,
    resolves: &dyn Fn(&str) -> usize,
) -> Availability {
    let companions = declared
        .companion
        .filter(|name| loaded(name))
        .map_or(0, |_| 1);
    availability(
        crop,
        companions,
        declared.declares_companion(),
        development,
        resolves,
    )
}

/// Whether a crop contributes to the creative listing.
///
/// The six anchor blocks are each guarded by their crop's state, evaluated when
/// the listing is built. An unobtainable crop's anchor drops out; everything
/// else in the listing is unconditional.
#[must_use]
pub const fn listed(state: Availability) -> bool {
    !matches!(state, Availability::Unobtainable)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMESPACE: &str = "test";
    const TIER: &str = "test:6";

    fn own_tag_crop() -> Declared {
        *DECLARED
            .iter()
            .find(|held| held.material == Material::OwnTag)
            .expect("the crop whose material is this entry's own tag")
    }

    fn crop_of(declared: &Declared) -> Crop {
        declared.crop(
            format!("{NAMESPACE}:{}", declared.path)
                .parse()
                .expect("a legal crop identity"),
            &TIER.parse().expect("a legal tier identity"),
            declared
                .material_id(NAMESPACE)
                .parse()
                .expect("a legal material identity"),
        )
    }

    #[test]
    fn six_crops_are_declared_and_four_of_them_name_one_companion_each() {
        assert_eq!(DECLARED.len(), COUNT);
        let naming = DECLARED
            .iter()
            .filter(|declared| declared.declares_companion())
            .count();
        assert_eq!(naming, 4);
        let unconditional = DECLARED
            .iter()
            .filter(|declared| !declared.declares_companion())
            .count();
        assert_eq!(unconditional, 2);
    }

    #[test]
    fn every_declared_path_and_anchor_is_distinct() {
        let mut paths: Vec<&str> = DECLARED.iter().map(|declared| declared.path).collect();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), COUNT);

        let mut anchors: Vec<&str> = DECLARED.iter().map(|declared| declared.anchor).collect();
        anchors.sort_unstable();
        anchors.dedup();
        assert_eq!(anchors.len(), COUNT);
    }

    #[test]
    fn every_crop_sits_at_the_sixth_tier_and_derives_nothing() {
        for declared in &DECLARED {
            let crop = crop_of(declared);
            assert_eq!(crop.tier.to_string(), TIER);
            assert_eq!(crop.kind.to_string(), KIND);
            assert_eq!(crop.derived, OPT_OUT);
            assert!(!crop.derived.ritual_seed);
            assert!(!crop.derived.grid_seed);
            assert!(!crop.derived.conversion);
            assert!(crop.enabled, "every one of the six is registered enabled");
        }
    }

    #[test]
    fn a_companion_crop_whose_mod_is_absent_is_unobtainable_and_keeps_its_identifiers() {
        let declared = *DECLARED
            .iter()
            .find(|held| held.material == Material::CompanionEntry)
            .expect("a companion-entry crop");
        let crop = crop_of(&declared);
        assert_eq!(
            state(&declared, &crop, false, &|_| false, &|_| 1),
            Availability::Unobtainable
        );
        assert!(!listed(Availability::Unobtainable));
        assert_eq!(
            crop.path(),
            declared.path,
            "the identifier exists whether or not the companion does"
        );
    }

    #[test]
    fn a_development_run_enables_every_companion_crop_a_production_run_hides() {
        for declared in DECLARED.iter().filter(|held| held.declares_companion()) {
            let crop = crop_of(declared);
            assert_eq!(
                state(declared, &crop, false, &|_| false, &|_| 1),
                Availability::Unobtainable
            );
            assert_eq!(
                state(declared, &crop, true, &|_| false, &|_| 1),
                Availability::Whole
            );
        }
    }

    #[test]
    fn the_own_tag_crop_reaches_the_third_state_and_the_others_cannot() {
        let own_tag = own_tag_crop();
        assert_eq!(own_tag.material, Material::OwnTag);
        let crop = crop_of(&own_tag);
        assert_eq!(
            state(&own_tag, &crop, false, &|_| true, &|_| 0),
            Availability::WithoutSeedRecipes,
            "an enabled crop whose tag resolves empty keeps its identifiers"
        );
        assert!(listed(Availability::WithoutSeedRecipes));
        assert_eq!(
            state(&own_tag, &crop, false, &|_| true, &|_| OPTIONAL_TAG_MEMBERS),
            Availability::Whole
        );

        for declared in DECLARED.iter().filter(|held| {
            matches!(
                held.material,
                Material::HostEntry | Material::OwnEntry | Material::CompanionEntry
            )
        }) {
            let crop = crop_of(declared);
            assert_eq!(
                state(declared, &crop, false, &|_| true, &|_| 0),
                Availability::Whole,
                "a single entry resolves whatever a tag lookup answers"
            );
        }
    }

    #[test]
    fn unobtainable_and_unsatisfiable_are_two_states_and_not_one() {
        let own_tag = own_tag_crop();
        let crop = crop_of(&own_tag);
        let absent = state(&own_tag, &crop, false, &|_| false, &|_| 0);
        let empty = state(&own_tag, &crop, false, &|_| true, &|_| 0);
        assert_ne!(absent, empty);
        assert_eq!(absent, Availability::Unobtainable);
        assert_eq!(empty, Availability::WithoutSeedRecipes);
    }

    #[test]
    fn the_six_materials_are_six_names_and_each_shape_names_its_own_space() {
        let mut ids: Vec<String> = DECLARED
            .iter()
            .map(|declared| declared.material_id(NAMESPACE))
            .collect();
        assert_eq!(ids.len(), COUNT);
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            COUNT,
            "the five shapes do not collapse to one name"
        );

        for declared in &DECLARED {
            let id = declared.material_id(NAMESPACE);
            let (namespace, _) = id.split_once(':').expect("a namespaced identifier");
            match declared.material {
                Material::HostEntry => assert_eq!(namespace, HOST_NAMESPACE),
                Material::OwnEntry | Material::OwnTag => assert_eq!(namespace, NAMESPACE),
                Material::CompanionEntry => assert_eq!(Some(namespace), declared.companion),
                Material::ConventionTag => assert_eq!(namespace, CONVENTION_NAMESPACE),
            }
        }
        assert_eq!(
            own_tag_crop().material_id(NAMESPACE),
            format!("{NAMESPACE}:{OWN_TAG}"),
            "the own-tag crop names the one tag this entry declares"
        );
    }

    #[test]
    fn the_two_tag_shaped_crops_are_crafted_from_a_tag_and_the_rest_from_an_entry() {
        for declared in &DECLARED {
            let crop = crop_of(declared);
            let tag_shaped = matches!(
                declared.material,
                Material::ConventionTag | Material::OwnTag
            );
            assert_eq!(
                matches!(crop.crafted, Crafted::Tag(_)),
                tag_shaped,
                "{} is crafted from the shape it declares",
                declared.path
            );
        }
    }

    #[test]
    fn the_own_tag_carries_both_of_the_companions_spellings() {
        let members = optional_tag_members();
        assert_eq!(members.len(), OPTIONAL_TAG_MEMBERS);
        let companion = own_tag_crop().companion.expect("a companion is named");
        for member in &members {
            assert_eq!(member.split_once(':').map(|split| split.0), Some(companion));
        }
        let mut distinct = members;
        distinct.sort();
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            OPTIONAL_TAG_MEMBERS,
            "two spellings, not one"
        );
    }

    #[test]
    fn every_crop_is_pointed_at_an_anchor_whatever_state_it_is_in() {
        for declared in &DECLARED {
            let mut crop = crop_of(declared);
            crop.foundation = Some(
                format!("{NAMESPACE}:{}", declared.anchor)
                    .parse()
                    .expect("the declared anchor is an identifier"),
            );
            let held = state(declared, &crop, false, &|_| false, &|_| 0);
            assert!(
                crop.foundation.is_some(),
                "the pointing is unconditional and this crop is {held:?}"
            );
        }
    }
}
