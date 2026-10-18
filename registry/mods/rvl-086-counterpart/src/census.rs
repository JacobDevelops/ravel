//! The registered content of section five, as counts, numbers and an order.
//!
//! Section seventeen keeps two things out of the specification and so out of
//! this package: the original's identifiers, and the balance figures of the two
//! added material tiers and the four armour materials. What is here is what the
//! specification does state — the cardinalities, the handful of figures it names
//! outright, the registration order that is load-bearing, and the creative
//! grouping's flag gating, which is the main observable use of the flags.
//!
//! The sixteen tinted variants stay sixteen registered blocks. Collapsing them
//! into one block with a colour property would break every identifier a save
//! holds, which is a statement about cardinality and not about names.

use std::fmt;

use ravel_sdk::content::{
    ContentDeclaration, ContentDeclarationError, ContentId, ContentIdError, ContentKind,
    RegistryKey,
};
use ravel_sdk::tags::TagId;
use rvl_042_counterpart::arithmetic::Colour;
use rvl_042_counterpart::flags::{FlagId, FlagTable};

use crate::repair::Shape;
use crate::settings::Flag;

/// The tinted variants of the common stone drop.
pub const TINTED_VARIANTS: usize = 16;

/// The blocks this entry registers: the tinted variants and one light-emitting
/// block.
pub const BLOCKS: usize = TINTED_VARIANTS + 1;

/// The items this entry registers beyond the one item each block contributes.
pub const ITEMS: usize = 59;

/// The ticks one fuel fragment burns for.
pub const FUEL_TICKS: u32 = 200;

/// The two watering tools, as a radius and a per-block chance.
pub const WATERING: [(u32, f64); 2] = [(3, 0.25), (7, 1.0 / 3.0)];

/// The uses each of the three sieve meshes has.
pub const MESH_USES: [u32; 3] = [20, 64, 512];

/// The durability multipliers of the two head-worn pieces.
pub const HEADPIECE_MULTIPLIERS: [u32; 2] = [15, 30];

/// The flat durability of the shearing tool one added tier also yields.
pub const SHEARING_DURABILITY: u32 = 100;

/// The two restorative foods, as nutrition and saturation.
pub const FOODS: [(u32, f64); 2] = [(6, 1.5), (10, 2.0)];

/// The lowest and highest radius a sweeping harvest shape carries.
pub const SWEEPING_RADIUS_RANGE: (u32, u32) = (1, 3);

/// The two added material tiers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tier {
    /// The lower tier, between the host's stone and iron on durability.
    Lower,
    /// The upper tier, above diamond on durability and speed.
    Upper,
}

/// The material a piece of this entry's gear is repaired with.
///
/// Which mineral or metal each one actually is belongs to the balance figures
/// section seventeen keeps out. What is carried is the only part section 5.3
/// states as behaviour: which tier a repair material belongs to, which is the
/// whole of what the armour defect is about.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Material {
    /// The mineral the lower tier's tools are made of.
    LowerMineral,
    /// The mineral the upper tier's tools are made of.
    UpperMineral,
}

impl Tier {
    /// Both tiers, in the order they are declared.
    pub const BOTH: [Self; 2] = [Self::Lower, Self::Upper];

    /// The material this tier's tools are made of and repaired with.
    #[must_use]
    pub const fn material(self) -> Material {
        match self {
            Self::Lower => Material::LowerMineral,
            Self::Upper => Material::UpperMineral,
        }
    }

    /// The material this tier's armour is repaired with.
    ///
    /// Its own tier's, which is the declared change. The original declares the
    /// upper tier's armour repairable with a different tier's metal entirely,
    /// not the mineral its own tools are made of, so the host's
    /// default-material branch behaves differently for that armour than a
    /// player would expect.
    #[must_use]
    pub const fn armour_repaired_by(self) -> Material {
        self.material()
    }
}

/// One group of registered content: how many entries it holds, what gates them
/// in the creative grouping, and which tool shape they are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Group {
    /// How many entries the group registers.
    pub count: usize,
    /// The flags that must all hold for its entries to be listed.
    pub gates: &'static [Flag],
    /// The tool shape its entries are, where they are a tool at all.
    pub shape: Option<Shape>,
}

/// The seventeen blocks, as the two groups the flags gate them in.
pub const BLOCK_GROUPS: [Group; 2] = [
    Group {
        count: TINTED_VARIANTS,
        gates: &[Flag::TintedStone],
        shape: None,
    },
    Group {
        count: 1,
        gates: &[Flag::LightBlock],
        shape: None,
    },
];

/// The fifty-nine items beyond the block items, in the order they are listed.
///
/// The division is **inferred** and the arithmetic is what supports it. Section
/// 5.2 names seven groups and section 4 names seventeen flags, two entries gated
/// on two flags at once, and three switches carrying no flag. The division below
/// is the one that satisfies all four at once: it sums to [`ITEMS`], it uses
/// each of the seventeen flags at least once, exactly two of its groups carry
/// two gates, and the four ungated entries are the two foods, the plant fibre
/// and the attractor — whose switches are the three that carry no flag. The
/// order within a group is presentation and the gating is not, which is why only
/// the gates are asserted.
pub const ITEM_GROUPS: [Group; 20] = [
    Group {
        count: 2,
        gates: &[Flag::FuelFragments],
        shape: None,
    },
    Group {
        count: 2,
        gates: &[],
        shape: None,
    },
    Group {
        count: 2,
        gates: &[Flag::WateringTools],
        shape: None,
    },
    Group {
        count: 1,
        gates: &[],
        shape: None,
    },
    Group {
        count: 3,
        gates: &[Flag::SieveMeshes],
        shape: None,
    },
    Group {
        count: 1,
        gates: &[],
        shape: None,
    },
    Group {
        count: 2,
        gates: &[Flag::Headpieces],
        shape: None,
    },
    Group {
        count: 1,
        gates: &[Flag::FirstTier],
        shape: Some(Shape::Sword),
    },
    Group {
        count: 4,
        gates: &[Flag::FirstTier],
        shape: Some(Shape::Digging),
    },
    Group {
        count: 1,
        gates: &[Flag::SecondTier],
        shape: Some(Shape::Sword),
    },
    Group {
        count: 4,
        gates: &[Flag::SecondTier],
        shape: Some(Shape::Digging),
    },
    Group {
        count: 1,
        gates: &[Flag::FirstTierShearing],
        shape: Some(Shape::Shearing),
    },
    Group {
        count: 1,
        gates: &[Flag::SecondTierShearing],
        shape: Some(Shape::Shearing),
    },
    Group {
        count: 4,
        gates: &[Flag::FirstTierArmour],
        shape: Some(Shape::Armour),
    },
    Group {
        count: 4,
        gates: &[Flag::SecondTierArmour],
        shape: Some(Shape::Armour),
    },
    Group {
        count: 8,
        gates: &[Flag::CombinedTools],
        shape: Some(Shape::Digging),
    },
    Group {
        count: 8,
        gates: &[Flag::SweepingWide],
        shape: Some(Shape::Digging),
    },
    Group {
        count: 1,
        gates: &[Flag::SweepingWide, Flag::SweepingWideUpper],
        shape: Some(Shape::Digging),
    },
    Group {
        count: 8,
        gates: &[Flag::SweepingTall],
        shape: Some(Shape::Digging),
    },
    Group {
        count: 1,
        gates: &[Flag::SweepingTall, Flag::SweepingTallUpper],
        shape: Some(Shape::Digging),
    },
];

/// The locale the grouping's title is declared in.
pub const LOCALE: &str = "en_us";

/// The key the creative grouping takes its title from.
pub const GROUPING_TITLE_KEY: &str = "itemGroup.rvl_086_counterpart.grouping";

/// The text that key carries.
pub const GROUPING_TITLE: &str = "Additions";

/// The namespace whose three tool-shape tags this entry's tools join.
///
/// Section 5.7 records this membership as a genuine consumption of that entry:
/// the three tags are how its own logic recognises a tool, and there are three
/// of them because [`Shape::tiered`] answers for exactly three shapes.
pub const SHARED_NAMESPACE: &str = "rvl_042_counterpart";

/// The tag one of the two upper-tier sweeping shapes is put in.
pub const SWEEPING_WIDE_TAG: &str = "sweeping_wide_upper";

/// The tag the other is put in.
pub const SWEEPING_TALL_TAG: &str = "sweeping_tall_upper";

/// One entry of the creative grouping: what it lists and what gates it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listed {
    /// The content this entry lists.
    pub entry: String,
    /// The flags that must all hold for it to appear.
    pub gates: Vec<FlagId>,
}

/// The creative grouping, in the hand-written order it is listed in.
///
/// The order is presentation. The gating is not: an entry gated on two flags
/// appears only when both hold, and a disabled feature's content vanishes from
/// this list while staying in the registries, loadable from a save and
/// obtainable by command.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Grouping {
    entries: Vec<Listed>,
}

impl Grouping {
    /// Builds a grouping over entries already in their listed order.
    #[must_use]
    pub const fn new(entries: Vec<Listed>) -> Self {
        Self { entries }
    }

    /// Adds one entry at the end of the listed order.
    pub fn list(&mut self, entry: &str, gates: Vec<FlagId>) {
        self.entries.push(Listed {
            entry: entry.to_owned(),
            gates,
        });
    }

    /// Everything this grouping could list, gated or not.
    #[must_use]
    pub fn declared(&self) -> &[Listed] {
        &self.entries
    }

    /// What the grouping shows, in order, given the flags in force.
    #[must_use]
    pub fn shown(&self, table: &FlagTable) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|listed| listed.gates.iter().all(|gate| table.enabled(gate)))
            .map(|listed| listed.entry.as_str())
            .collect()
    }
}

/// The identifiers this entry claims, in the order it claims them.
///
/// Blocks first, then items, because the item pass receives the one item each
/// block contributes and so has to run after the block pass. That ordering is
/// load-bearing and [`claims`] is written to make it observable.
#[must_use]
pub fn claims() -> Vec<(ContentKind, String)> {
    let mut claimed = Vec::with_capacity(BLOCKS * 2 + ITEMS);
    for name in block_names() {
        claimed.push((ContentKind::Block, name));
    }
    for name in block_names() {
        claimed.push((ContentKind::Item, name));
    }
    for name in item_names() {
        claimed.push((ContentKind::Item, name));
    }
    claimed
}

/// The names of the fifty-nine items, generated in listed order.
///
/// One name per entry of [`ITEM_GROUPS`], so the group a name belongs to is its
/// position and nothing has to be kept in step by hand.
#[must_use]
pub fn item_names() -> Vec<String> {
    (0..ITEM_GROUPS.iter().map(|group| group.count).sum())
        .map(|index| format!("registered_item_{index:02}"))
        .collect()
}

/// The creative grouping, built over everything registered, in listed order.
///
/// The block items come first in block order and the fifty-nine items follow,
/// each carrying the gates of the group it belongs to. Every entry stays in the
/// registries whatever the flags say; what a flag decides is only whether the
/// entry is listed here.
///
/// # Errors
///
/// Returns [`ContentIdError`] when a gate named in the tables above is not a legal
/// flag identity under `namespace`, which for these constants is a packaging
/// failure.
pub fn grouping(namespace: &str) -> Result<Grouping, ContentIdError> {
    let mut grouping = Grouping::default();
    let listed = gated(&BLOCK_GROUPS, block_names())
        .into_iter()
        .chain(gated(&ITEM_GROUPS, item_names()));
    for (name, gates) in listed {
        let mut identified = Vec::with_capacity(gates.len());
        for gate in gates {
            identified.push(FlagId::new(namespace, gate.name())?);
        }
        grouping.list(&name, identified);
    }
    Ok(grouping)
}

/// Pairs each name with the gates of the group its position falls in.
fn gated(groups: &'static [Group], names: Vec<String>) -> Vec<(String, &'static [Flag])> {
    let mut paired = Vec::with_capacity(names.len());
    let mut names = names.into_iter();
    for group in groups {
        for _ in 0..group.count {
            let Some(name) = names.next() else {
                return paired;
            };
            paired.push((name, group.gates));
        }
    }
    paired
}

/// Why a declared tag membership was refused.
#[derive(Debug)]
pub enum TagRefused {
    /// The tag's own name is not a legal identity.
    Name(ContentIdError),
    /// The tag namespace is not legal.
    Namespace(ravel_sdk::ModIdError),
    /// The declaration refused the membership itself.
    Declaration(ContentDeclarationError),
}

impl fmt::Display for TagRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(detail) => write!(formatter, "the tag name is not legal: {detail}"),
            Self::Namespace(source) => {
                write!(formatter, "the tag namespace is not legal: {source}")
            }
            Self::Declaration(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for TagRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Name(source) => Some(source),
            Self::Namespace(source) => Some(source),
            Self::Declaration(source) => Some(source),
        }
    }
}

impl From<ContentDeclarationError> for TagRefused {
    fn from(source: ContentDeclarationError) -> Self {
        Self::Declaration(source)
    }
}

/// Declares the tag memberships whose members are entries this package owns.
///
/// Three of them are RVL-042's tool-shape tags, which section 5.7 records as
/// this entry's one genuine consumption of that entry's tag namespace, and there
/// are three because [`Shape::tiered`] answers for exactly three shapes. Two
/// more are this package's own item tags for the upper-tier sweeping shapes.
/// The memberships whose members would be host or third-party identifiers are
/// not declared, because section seventeen keeps those identifiers out of the
/// specification and a membership needs a member.
///
/// # Errors
///
/// Returns [`TagRefused`] when a tag name is not a legal identity or the
/// declaration refuses a membership.
pub fn declare_tags(
    declaration: &mut ContentDeclaration,
    items: &[RegistryKey],
) -> Result<(), TagRefused> {
    for (name, shape) in item_names().into_iter().zip(item_shapes()) {
        let Some(shape) = shape.filter(|shape| shape.tiered()) else {
            continue;
        };
        if let Some(member) = member(items, &name) {
            declaration.tag(&tag(SHARED_NAMESPACE, shape_tag(shape))?, &member)?;
        }
    }
    for (gate, path) in [
        (Flag::SweepingWideUpper, SWEEPING_WIDE_TAG),
        (Flag::SweepingTallUpper, SWEEPING_TALL_TAG),
    ] {
        let Some(name) = first_gated_on(gate) else {
            continue;
        };
        if let Some(member) = member(items, &name) {
            declaration.tag(&tag(crate::MOD_ID, path)?, &member)?;
        }
    }
    Ok(())
}

/// The shape of each item, in the order [`item_names`] gives them.
#[must_use]
pub fn item_shapes() -> Vec<Option<Shape>> {
    let mut shapes = Vec::with_capacity(ITEMS);
    for group in &ITEM_GROUPS {
        for _ in 0..group.count {
            shapes.push(group.shape);
        }
    }
    shapes
}

/// The name of the first item of the group `gate` gates, if any group does.
///
/// Read out of [`ITEM_GROUPS`] rather than written as a position, so the tables
/// cannot drift apart without the membership disappearing rather than landing on
/// the wrong item.
#[must_use]
pub fn first_gated_on(gate: Flag) -> Option<String> {
    let mut before = 0;
    for group in &ITEM_GROUPS {
        if group.gates.contains(&gate) {
            return item_names().into_iter().nth(before);
        }
        before += group.count;
    }
    None
}

fn member(items: &[RegistryKey], name: &str) -> Option<RegistryKey> {
    items.iter().find(|key| key.id().path() == name).cloned()
}

const fn shape_tag(shape: Shape) -> &'static str {
    match shape {
        Shape::Sword => "swords",
        Shape::Shearing => "shearing_tools",
        Shape::Digging | Shape::Ranged | Shape::Armour | Shape::Other => "digging_tools",
    }
}

fn tag(namespace: &str, path: &str) -> Result<TagId, TagRefused> {
    let namespace = namespace.parse().map_err(TagRefused::Namespace)?;
    let id = ContentId::new(namespace, path).map_err(TagRefused::Name)?;
    Ok(TagId::new(ContentKind::Item, id).expect("item identities support tags"))
}

/// The names of the seventeen blocks, generated rather than transcribed.
#[must_use]
pub fn block_names() -> Vec<String> {
    let mut names: Vec<String> = (0..TINTED_VARIANTS)
        .map(|index| format!("tinted_variant_{index:02}"))
        .collect();
    names.push("light_emitting_block".to_owned());
    names
}

/// The colour one tinted variant carries, as an opaque packed value.
///
/// The sixteen colours are the entry's balance rather than its behaviour and are
/// not transcribed. What is written here is the shape the client tinting reads:
/// one opaque integer per block, distinct across the sixteen.
#[must_use]
pub fn tint(variant: usize) -> Option<Colour> {
    let index = u8::try_from(variant).ok()?;
    if usize::from(index) >= TINTED_VARIANTS {
        return None;
    }
    let step = index.wrapping_mul(17);
    Some(Colour::new(
        u8::MAX,
        step,
        step.wrapping_add(85),
        step.wrapping_add(170),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_tag_names_retain_the_underlying_error() {
        for (namespace, path, context) in [
            ("Bad Namespace", "item", "the tag namespace is not legal"),
            ("example", "Bad Path", "the tag name is not legal"),
        ] {
            let refused = tag(namespace, path).expect_err("an invalid tag identity");
            let source =
                std::error::Error::source(&refused).expect("the underlying identity error");
            assert_eq!(refused.to_string(), format!("{context}: {source}"));
            match namespace {
                "Bad Namespace" => assert!(matches!(refused, TagRefused::Namespace(_))),
                _ => assert!(matches!(refused, TagRefused::Name(_))),
            }
        }
    }

    fn flag(name: &str) -> FlagId {
        FlagId::new("rvl_086_counterpart", name).unwrap_or_else(|_| {
            panic!("the test names a legal flag");
        })
    }

    #[test]
    fn the_tinted_variants_stay_sixteen_registered_blocks() {
        let names = block_names();
        assert_eq!(names.len(), BLOCKS);
        let mut unique = names;
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), BLOCKS, "no two blocks share a name");
    }

    #[test]
    fn every_block_contributes_exactly_one_item_and_the_items_are_claimed_after() {
        let claimed = claims();
        assert_eq!(claimed.len(), BLOCKS * 2 + ITEMS);

        let blocks = claimed
            .iter()
            .filter(|(kind, _)| *kind == ContentKind::Block)
            .count();
        assert_eq!(blocks, BLOCKS);
        assert_eq!(
            claimed
                .iter()
                .filter(|(kind, _)| *kind == ContentKind::Item)
                .count(),
            BLOCKS + ITEMS
        );

        let first_item = claimed
            .iter()
            .position(|(kind, _)| *kind == ContentKind::Item)
            .expect("items are claimed");
        let last_block = claimed
            .iter()
            .rposition(|(kind, _)| *kind == ContentKind::Block)
            .expect("blocks are claimed");
        assert!(
            last_block < first_item,
            "every block is claimed before the first item"
        );
    }

    #[test]
    fn no_identifier_is_claimed_twice() {
        let claimed = claims();
        let mut seen: Vec<(ContentKind, String)> = claimed.clone();
        seen.sort_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then(format!("{:?}", left.0).cmp(&format!("{:?}", right.0)))
        });
        seen.dedup();
        assert_eq!(seen.len(), claimed.len());
    }

    #[test]
    fn each_tiers_armour_names_its_own_tiers_material_and_not_the_other_tiers() {
        assert_ne!(Tier::Lower.material(), Tier::Upper.material());
        for tier in Tier::BOTH {
            assert_eq!(tier.armour_repaired_by(), tier.material());
        }
        let other = Tier::Lower.material();
        assert_ne!(
            Tier::Upper.armour_repaired_by(),
            other,
            "the upper tier's armour is not repaired with the lower tier's metal"
        );
    }

    #[test]
    fn the_item_groups_account_for_every_registered_item_and_every_flag() {
        let counted: usize = ITEM_GROUPS.iter().map(|group| group.count).sum();
        assert_eq!(counted, ITEMS);
        assert_eq!(item_names().len(), ITEMS);

        assert_eq!(
            ITEM_GROUPS
                .iter()
                .filter(|group| group.gates.len() == 2)
                .count(),
            2,
            "two entries are gated on two flags at once"
        );
        assert_eq!(
            ITEM_GROUPS
                .iter()
                .filter(|group| group.gates.is_empty())
                .map(|group| group.count)
                .sum::<usize>(),
            4,
            "the entries whose switches carry no flag are ungated"
        );
    }

    #[test]
    fn the_grouping_lists_everything_registered_and_each_flag_takes_some_of_it_away() {
        let namespace = "rvl_086_counterpart";
        let grouping = grouping(namespace).expect("the declared flag names are legal");
        assert_eq!(grouping.declared().len(), BLOCKS + ITEMS);

        let mut table = FlagTable::default();
        for (name, _) in crate::settings::FLAG_BINDINGS {
            table.publish_constant(flag(name.name()), true);
        }
        let whole = grouping.shown(&table).len();
        assert_eq!(whole, BLOCKS + ITEMS);

        for (name, _) in crate::settings::FLAG_BINDINGS {
            let mut one_off = FlagTable::default();
            for (other, _) in crate::settings::FLAG_BINDINGS {
                one_off.publish_constant(flag(other.name()), other != name);
            }
            assert!(
                grouping.shown(&one_off).len() < whole,
                "{name:?} gates at least one listed entry"
            );
        }
    }

    #[test]
    fn every_tinted_variant_carries_a_distinct_opaque_colour() {
        let mut packed: Vec<u32> = (0..TINTED_VARIANTS)
            .map(|index| {
                tint(index)
                    .unwrap_or_else(|| panic!("variant {index} has a colour"))
                    .pack()
            })
            .collect();
        assert_eq!(packed.len(), TINTED_VARIANTS);
        packed.sort_unstable();
        packed.dedup();
        assert_eq!(packed.len(), TINTED_VARIANTS);
        assert!(tint(TINTED_VARIANTS).is_none());
    }

    #[test]
    fn an_entry_gated_on_two_flags_appears_only_when_both_hold() {
        let mut grouping = Grouping::default();
        grouping.list("plain", Vec::new());
        grouping.list("one_gate", vec![flag("first")]);
        grouping.list("two_gates", vec![flag("first"), flag("second")]);

        let mut table = FlagTable::default();
        table.publish_constant(flag("first"), true);
        table.publish_constant(flag("second"), false);
        assert_eq!(grouping.shown(&table), vec!["plain", "one_gate"]);

        table.publish_constant(flag("second"), true);
        assert_eq!(
            grouping.shown(&table),
            vec!["plain", "one_gate", "two_gates"]
        );

        table.publish_constant(flag("first"), false);
        assert_eq!(grouping.shown(&table), vec!["plain"]);
    }

    #[test]
    fn a_disabled_features_content_leaves_the_listing_and_stays_in_the_registries() {
        let mut grouping = Grouping::default();
        grouping.list("gated", vec![flag("first")]);
        let mut table = FlagTable::default();
        table.publish_constant(flag("first"), false);

        assert_eq!(grouping.shown(&table).len(), 0);
        assert_eq!(grouping.declared().len(), 1);
        assert_eq!(
            claims().len(),
            BLOCKS * 2 + ITEMS,
            "nothing is unregistered"
        );
    }

    #[test]
    fn the_listed_order_is_the_order_entries_were_written() {
        let mut grouping = Grouping::default();
        for name in ["third", "first", "second"] {
            grouping.list(name, Vec::new());
        }
        assert_eq!(
            grouping.shown(&FlagTable::default()),
            vec!["third", "first", "second"]
        );
    }
}
