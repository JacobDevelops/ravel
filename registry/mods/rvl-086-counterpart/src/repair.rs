//! The crafting-grid tool repair of section six.
//!
//! The one genuinely novel mechanic in the entry: a damaged tool laid in a
//! crafting grid beside valid repair materials comes out mended. The rule
//! declares no ingredients, no result and no shape, and it answers "does this
//! grid match?" by running the whole assembly and asking whether the answer is
//! non-empty. [`matches`] is written as exactly that call, so the preview a grid
//! draws and the craft it performs cannot disagree.
//!
//! Every number here is integer arithmetic and both truncations are the
//! observable behaviour, so both are reproduced rather than rounded.

use ravel_sdk::{JsonValue, json};
use rvl_042_counterpart::inventory::Stack;

use crate::matching::Matcher;
use crate::overrides::Table;
use crate::settings::Settings;

/// The shapes a repair rule tells apart.
///
/// Only three of these change what a repair costs; the rest are here because
/// the refusal rules of [`crate::refusal`] read the same shape.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Shape {
    /// A tiered digging tool.
    Digging,
    /// A tiered sword.
    Sword,
    /// The shearing tool.
    Shearing,
    /// A ranged weapon.
    Ranged,
    /// Armour.
    Armour,
    /// Anything else damageable.
    Other,
}

impl Shape {
    /// Every shape, in a fixed order.
    pub const ALL: [Self; 6] = [
        Self::Digging,
        Self::Sword,
        Self::Shearing,
        Self::Ranged,
        Self::Armour,
        Self::Other,
    ];

    /// Whether this shape borrows a material tier's numbers.
    #[must_use]
    pub const fn tiered(self) -> bool {
        matches!(self, Self::Digging | Self::Sword | Self::Shearing)
    }
}

/// One enchantment on a tool, described by what it does rather than named.
///
/// A curse is identified structurally here, as the original does: an enchantment
/// that stops the item being dropped on death or stops it being unequipped. A
/// list of names would go stale the moment a pack adds one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Enchantment {
    /// Whether this enchantment stops the item being dropped on death.
    pub prevents_drop_on_death: bool,
    /// Whether this enchantment stops the item being unequipped.
    pub prevents_unequip: bool,
}

impl Enchantment {
    /// Whether this enchantment is a curse and so survives stripping.
    #[must_use]
    pub const fn curse(self) -> bool {
        self.prevents_drop_on_death || self.prevents_unequip
    }
}

/// One occupied slot of a crafting grid.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Held {
    /// The stack in the slot.
    pub stack: Stack,
    /// How damaged it is, zero being undamaged.
    pub damage: u32,
    /// The most damage it takes before breaking, zero when it is not damageable.
    pub maximum_damage: u32,
    /// What shape it is.
    pub shape: Option<Shape>,
    /// The enchantments it carries.
    pub enchantments: Vec<Enchantment>,
    /// Whether crafting this item gives it back, which is what a reusable item
    /// declares and what makes it never a repair material.
    pub returns_a_remainder: bool,
}

impl Held {
    /// Builds a plain undamageable material of one entry.
    #[must_use]
    pub fn material(entry: &str) -> Self {
        Self {
            stack: Stack::new(entry, 1),
            ..Self::default()
        }
    }

    /// Builds a damaged tool of a shape.
    #[must_use]
    pub fn tool(entry: &str, shape: Shape, damage: u32, maximum_damage: u32) -> Self {
        Self {
            stack: Stack::new(entry, 1),
            damage,
            maximum_damage,
            shape: Some(shape),
            ..Self::default()
        }
    }

    /// Whether the host would let this item take damage at all.
    #[must_use]
    pub const fn damageable(&self) -> bool {
        self.maximum_damage > 0
    }

    /// Whether this item carries any enchantment, curse or not.
    #[must_use]
    pub fn enchanted(&self) -> bool {
        !self.enchantments.is_empty()
    }
}

/// Why a grid does not repair anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    /// The repair rule is switched off, which is asked before anything else.
    SwitchedOff,
    /// No damageable item was found in the grid.
    NoTool,
    /// The tool found is not damaged.
    Undamaged,
    /// The grid holds a tool and nothing else.
    NoMaterials,
    /// The tool is named by the blacklist.
    Blacklisted,
    /// A material was laid after the tool was already full.
    SurplusAfterFull,
    /// A material gives itself back when crafted, so it is never spent.
    MaterialReturns,
    /// A material is worth nothing for this tool.
    WorthlessMaterial,
}

/// What one grid produces.
#[derive(Clone, Debug, PartialEq)]
pub struct Repaired {
    /// The single copy of the tool the grid yields.
    pub result: Stack,
    /// The damage that copy carries, which is at most the damage it went in with.
    pub damage: u32,
    /// The damage one unit of material mends.
    pub per_unit: u32,
    /// The units of material the grid supplied.
    pub units: f64,
    /// How many enchantments were stripped, curses never being among them.
    pub stripped: usize,
    /// The enchantments the copy keeps.
    pub enchantments: Vec<Enchantment>,
}

/// The costs one repair is charged, after the cheaper-shape settings apply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cost {
    /// Units of material for a whole repair.
    pub units: u32,
    /// Extra units charged when the tool carries any enchantment.
    pub enchantment: u32,
}

impl Cost {
    /// The lowest a repair cost falls to, however many halvings apply.
    pub const FLOOR: u32 = 1;

    /// Reads the costs in force for one tool shape.
    ///
    /// Both cheaper-shape settings default on, so the halving is the ordinary
    /// case for the two shapes they name rather than something a pack opts into.
    /// The halving is a truncating division with the repair cost floored at one
    /// and the enchantment component floored at zero. Section 6.4 floors the
    /// halved cost alone, so a cost that was not halved is charged as written.
    #[must_use]
    pub fn of(settings: &Settings, shape: Option<Shape>) -> Self {
        let cheaper = match shape {
            Some(Shape::Digging) => settings.repair_discounts.digging,
            Some(Shape::Shearing) => settings.repair_discounts.shearing,
            Some(Shape::Sword | Shape::Ranged | Shape::Armour | Shape::Other) | None => false,
        };
        if cheaper {
            Self {
                units: settings.repair_cost.div_euclid(2).max(Self::FLOOR),
                enchantment: settings.repair_enchantment_cost.div_euclid(2),
            }
        } else {
            Self {
                units: settings.repair_cost,
                enchantment: settings.repair_enchantment_cost,
            }
        }
    }

    /// The damage one unit of material mends on a tool of this maximum.
    ///
    /// A truncating division, and the truncation is the observable behaviour: a
    /// tool whose maximum does not divide by the cost mends slightly less than a
    /// player would calculate.
    #[must_use]
    pub const fn per_unit(self, maximum_damage: u32, enchanted: bool) -> u32 {
        let divisor = if enchanted {
            self.units + self.enchantment
        } else {
            self.units
        };
        match maximum_damage.checked_div(divisor) {
            Some(mended) => mended,
            None => 0,
        }
    }
}

/// What one material is worth for one tool, by the four branches of section 6.2.
///
/// The branches are asked in order and the first that answers wins. A value of
/// zero is the refusal signal rather than a discount.
#[must_use]
pub fn worth(
    settings: &Settings,
    table: &Table,
    tool: &Held,
    material: &Held,
    tool_tagged: &dyn Fn(&str) -> bool,
    material_tagged: &dyn Fn(&str) -> bool,
    host_accepts: &dyn Fn(&str, &str) -> bool,
) -> f64 {
    if let Some(multiplier) = table.multiplier(
        &tool.stack.entry,
        &material.stack.entry,
        tool_tagged,
        material_tagged,
    ) {
        return multiplier;
    }
    if settings.repair_materials.override_mode && table.names_tool(&tool.stack.entry, tool_tagged) {
        return 0.0;
    }
    if !settings.repair_materials.disable_defaults
        && host_accepts(&tool.stack.entry, &material.stack.entry)
    {
        return 1.0;
    }
    0.0
}

/// Everything a repair needs from outside the grid.
pub struct Context<'ask> {
    /// The values in force.
    pub settings: &'ask Settings,
    /// The override table, matched as the repair asks rather than resolved once.
    pub table: &'ask Table,
    /// The repair blacklist, read from the same settings.
    pub blacklist: &'ask Matcher,
    /// Whether an entry belongs to a tag.
    pub tagged: &'ask dyn Fn(&str, &str) -> bool,
    /// Whether the host calls a material a valid repair item for a tool.
    pub host_accepts: &'ask dyn Fn(&str, &str) -> bool,
}

/// Reads a grid and returns what it repairs.
///
/// The grid is walked in slot order. The first non-empty damageable item is the
/// tool and every other non-empty slot is a material, including a later
/// damageable item — so a grid holding two damaged tools mends the earlier one
/// and spends the later one, if the later one is worth anything at all.
///
/// # Errors
///
/// Returns the first [`Refused`] the grid earns. Every refusal voids the whole
/// craft; nothing here repairs partially.
pub fn repair(grid: &[Option<Held>], context: &Context<'_>) -> Result<Repaired, Refused> {
    if !context.settings.repair_enabled {
        return Err(Refused::SwitchedOff);
    }

    let mut tool: Option<&Held> = None;
    let mut materials: Vec<&Held> = Vec::new();
    for held in grid.iter().flatten() {
        if held.stack.is_empty() {
            continue;
        }
        if tool.is_none() && held.damageable() {
            tool = Some(held);
        } else {
            materials.push(held);
        }
    }

    let tool = tool.ok_or(Refused::NoTool)?;
    if tool.damage == 0 {
        return Err(Refused::Undamaged);
    }
    if materials.is_empty() {
        return Err(Refused::NoMaterials);
    }
    let tool_entry = tool.stack.entry.clone();
    let tool_tagged = |tag: &str| (context.tagged)(&tool_entry, tag);
    if context.blacklist.matches(&tool.stack.entry, &tool_tagged) {
        return Err(Refused::Blacklisted);
    }

    let cost = Cost::of(context.settings, tool.shape);
    let per_unit = cost.per_unit(tool.maximum_damage, tool.enchanted());

    let mut units = 0.0_f64;
    let mut full = false;
    for material in materials {
        if full {
            return Err(Refused::SurplusAfterFull);
        }
        if material.returns_a_remainder {
            return Err(Refused::MaterialReturns);
        }
        let material_entry = material.stack.entry.clone();
        let material_tagged = |tag: &str| (context.tagged)(&material_entry, tag);
        let value = worth(
            context.settings,
            context.table,
            tool,
            material,
            &tool_tagged,
            &material_tagged,
            context.host_accepts,
        );
        if value == 0.0 {
            return Err(Refused::WorthlessMaterial);
        }
        units += value;
        if mended(per_unit, units) >= tool.damage {
            full = true;
        }
    }

    Ok(repair_result(tool, context, per_unit, units))
}

fn repair_result(tool: &Held, context: &Context<'_>, per_unit: u32, units: f64) -> Repaired {
    let reduction = mended(per_unit, units).min(tool.damage);
    let mut result = tool.stack.clone();
    result.count = 1;
    let (enchantments, stripped) = if context.settings.repair_materials.strip_enchantments {
        let kept: Vec<Enchantment> = tool
            .enchantments
            .iter()
            .copied()
            .filter(|enchantment| enchantment.curse())
            .collect();
        let stripped = tool.enchantments.len() - kept.len();
        (kept, stripped)
    } else {
        (tool.enchantments.clone(), 0)
    };

    Repaired {
        result,
        damage: tool.damage - reduction,
        per_unit,
        units,
        stripped,
        enchantments,
    }
}

/// Whether a grid would produce anything, which is the same computation.
///
/// The rule declares no ingredients and no shape, so matching is assembling. A
/// separate cheaper test is what would let a grid preview a result and then
/// refuse to craft it.
#[must_use]
pub fn matches(grid: &[Option<Held>], context: &Context<'_>) -> bool {
    repair(grid, context).is_ok()
}

/// What the rule gives back for each grid slot, which is nothing.
///
/// Every input is consumed, the tool included: it leaves the grid only as the
/// result.
#[must_use]
pub fn remainders(slots: usize) -> Vec<Option<Stack>> {
    vec![None; slots]
}

/// The recipe kind this entry claims, whose two encodings carry nothing.
///
/// Every input the rule needs comes from configuration, so a document that
/// instantiates it and a network frame that carries it are both empty.
pub const KIND: &str = "grid_repair";

/// The empty encoding both of the kind's two forms carry.
#[must_use]
pub fn encoding() -> JsonValue {
    json!({})
}

fn mended(per_unit: u32, units: f64) -> u32 {
    let mended = f64::from(per_unit) * units;
    if !mended.is_finite() || mended <= 0.0 {
        return 0;
    }
    let truncated = mended.trunc();
    if truncated >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        num_traits::ToPrimitive::to_u32(&truncated)
            .expect("a finite nonnegative value below u32::MAX fits after truncation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mending_preserves_truncation_saturation_and_nonfinite_rejection() {
        assert_eq!(mended(3, 0.5), 1);
        assert_eq!(mended(1, 0.999), 0);
        assert_eq!(mended(1, f64::from(u32::MAX) - 0.5), u32::MAX - 1);
        assert_eq!(mended(1, f64::from(u32::MAX)), u32::MAX);
        assert_eq!(mended(u32::MAX, 2.0), u32::MAX);
        for units in [0.0, -0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(mended(1, units), 0);
        }
    }

    fn untagged(_entry: &str, _tag: &str) -> bool {
        false
    }

    fn nothing_accepted(_tool: &str, _material: &str) -> bool {
        false
    }

    fn everything_accepted(_tool: &str, _material: &str) -> bool {
        true
    }

    struct Fixture {
        settings: Settings,
        table: Table,
        blacklist: Matcher,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                settings: Settings::default(),
                table: Table::default(),
                blacklist: Matcher::default(),
            }
        }

        fn context<'ask>(
            &'ask self,
            host_accepts: &'ask dyn Fn(&str, &str) -> bool,
        ) -> Context<'ask> {
            Context {
                settings: &self.settings,
                table: &self.table,
                blacklist: &self.blacklist,
                tagged: &untagged,
                host_accepts,
            }
        }
    }

    fn grid(items: Vec<Held>) -> Vec<Option<Held>> {
        items.into_iter().map(Some).collect()
    }

    #[test]
    fn the_switch_is_asked_before_the_grid_is_read_at_all() {
        let mut fixture = Fixture::new();
        fixture.settings = Settings {
            repair_enabled: false,
            ..Settings::default()
        };
        assert_eq!(
            repair(&[], &fixture.context(&everything_accepted)),
            Err(Refused::SwitchedOff)
        );
    }

    #[test]
    fn an_empty_grid_and_a_grid_of_materials_alone_find_no_tool() {
        let fixture = Fixture::new();
        assert_eq!(
            repair(&[], &fixture.context(&everything_accepted)),
            Err(Refused::NoTool)
        );
        let only_materials = grid(vec![Held::material("example:ingot")]);
        assert_eq!(
            repair(&only_materials, &fixture.context(&everything_accepted)),
            Err(Refused::NoTool)
        );
    }

    #[test]
    fn an_undamaged_tool_and_a_lone_tool_are_both_refused() {
        let fixture = Fixture::new();
        let undamaged = grid(vec![
            Held::tool("example:pick", Shape::Other, 0, 100),
            Held::material("example:ingot"),
        ]);
        assert_eq!(
            repair(&undamaged, &fixture.context(&everything_accepted)),
            Err(Refused::Undamaged)
        );
        let alone = grid(vec![Held::tool("example:pick", Shape::Other, 50, 100)]);
        assert_eq!(
            repair(&alone, &fixture.context(&everything_accepted)),
            Err(Refused::NoMaterials)
        );
    }

    #[test]
    fn the_first_damageable_item_in_slot_order_is_the_tool_and_a_later_one_is_a_material() {
        let fixture = Fixture::new();
        let two_tools = grid(vec![
            Held::tool("example:first", Shape::Other, 60, 60),
            Held::tool("example:second", Shape::Other, 30, 60),
        ]);
        let repaired = repair(&two_tools, &fixture.context(&everything_accepted))
            .expect("the host accepts the later tool as a material");
        assert_eq!(repaired.result.entry, "example:first");
        assert_eq!(repaired.units, 1.0);
    }

    #[test]
    fn an_empty_slot_is_skipped_rather_than_ending_the_read() {
        let fixture = Fixture::new();
        let sparse = vec![
            None,
            Some(Held::tool("example:pick", Shape::Other, 60, 60)),
            None,
            Some(Held::material("example:ingot")),
        ];
        let repaired = repair(&sparse, &fixture.context(&everything_accepted))
            .expect("empty slots are not materials");
        assert_eq!(repaired.units, 1.0);
    }

    #[test]
    fn a_blacklisted_tool_is_refused_after_the_grid_is_read() {
        let mut fixture = Fixture::new();
        fixture.blacklist = Matcher::parse("example:pick");
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:ingot"),
        ]);
        assert_eq!(
            repair(&laid, &fixture.context(&everything_accepted)),
            Err(Refused::Blacklisted)
        );
    }

    #[test]
    fn a_material_worth_nothing_voids_the_whole_craft() {
        let fixture = Fixture::new();
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:sand"),
        ]);
        assert_eq!(
            repair(&laid, &fixture.context(&nothing_accepted)),
            Err(Refused::WorthlessMaterial)
        );
    }

    #[test]
    fn a_material_that_gives_itself_back_can_never_be_a_repair_material() {
        let fixture = Fixture::new();
        let mut reusable = Held::material("example:mesh");
        reusable.returns_a_remainder = true;
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            reusable,
        ]);
        assert_eq!(
            repair(&laid, &fixture.context(&everything_accepted)),
            Err(Refused::MaterialReturns)
        );
    }

    #[test]
    fn a_surplus_material_after_the_tool_is_full_voids_the_craft_rather_than_overflowing() {
        let fixture = Fixture::new();
        // The default cost of three over a maximum of sixty mends twenty a unit,
        // so three units fill a tool damaged by sixty and a fourth is surplus.
        let full = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
            Held::material("example:b"),
            Held::material("example:c"),
        ]);
        assert!(repair(&full, &fixture.context(&everything_accepted)).is_ok());

        let surplus = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
            Held::material("example:b"),
            Held::material("example:c"),
            Held::material("example:d"),
        ]);
        assert_eq!(
            repair(&surplus, &fixture.context(&everything_accepted)),
            Err(Refused::SurplusAfterFull)
        );
    }

    #[test]
    fn the_saturating_material_may_be_the_last_one_without_voiding_anything() {
        let fixture = Fixture::new();
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 20, 60),
            Held::material("example:a"),
        ]);
        let repaired =
            repair(&laid, &fixture.context(&everything_accepted)).expect("one unit fills it");
        assert_eq!(repaired.damage, 0);
    }

    #[test]
    fn the_per_unit_repair_truncates_and_so_does_the_reduction() {
        let mut fixture = Fixture::new();
        fixture.settings.repair_cost = 7;
        // A maximum of one hundred over seven units mends fourteen a unit, not
        // fourteen and a fraction, and two units mend twenty-eight.
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 100, 100),
            Held::material("example:a"),
            Held::material("example:b"),
        ]);
        let repaired = repair(&laid, &fixture.context(&everything_accepted)).expect("two units");
        assert_eq!(repaired.per_unit, 14);
        assert_eq!(repaired.damage, 100 - 28);
    }

    #[test]
    fn a_fractional_multiplier_truncates_once_at_the_end_rather_than_per_material() {
        let mut fixture = Fixture::new();
        fixture.settings.repair_cost = 3;
        fixture.table = Table::parse(
            "example:pick=example:a@0.5, example:pick=example:b@0.5",
            "host",
        );
        // Twenty a unit against one whole unit assembled from two halves.
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
            Held::material("example:b"),
        ]);
        let repaired = repair(&laid, &fixture.context(&nothing_accepted)).expect("half plus half");
        assert_eq!(repaired.units, 1.0);
        assert_eq!(repaired.damage, 40);
    }

    #[test]
    fn an_enchanted_tool_is_charged_the_enchantment_cost_and_a_plain_one_is_not() {
        let fixture = Fixture::new();
        let cost = Cost::of(&fixture.settings, None);
        assert_eq!(cost.units, 3);
        assert_eq!(cost.enchantment, 1);
        assert_eq!(cost.per_unit(120, false), 40);
        assert_eq!(cost.per_unit(120, true), 30);
    }

    #[test]
    fn the_two_cheaper_shapes_halve_both_costs_with_the_repair_cost_floored_at_one() {
        let mut fixture = Fixture::new();
        fixture.settings.repair_cost = 1;
        fixture.settings.repair_enchantment_cost = 1;
        for shape in [Shape::Digging, Shape::Shearing] {
            let cost = Cost::of(&fixture.settings, Some(shape));
            assert_eq!(cost.units, Cost::FLOOR, "{shape:?} floors at one");
            assert_eq!(cost.enchantment, 0, "{shape:?} floors the extra at zero");
        }
        let untouched = Cost::of(&fixture.settings, Some(Shape::Sword));
        assert_eq!(untouched.units, 1);
        assert_eq!(untouched.enchantment, 1);
    }

    #[test]
    fn the_halving_is_the_ordinary_case_for_the_two_shapes_that_default_to_it() {
        let fixture = Fixture::new();
        assert!(fixture.settings.repair_discounts.digging);
        assert!(fixture.settings.repair_discounts.shearing);
        assert_eq!(Cost::of(&fixture.settings, Some(Shape::Digging)).units, 1);
        assert_eq!(Cost::of(&fixture.settings, Some(Shape::Sword)).units, 3);
    }

    #[test]
    fn the_override_table_wins_over_the_hosts_own_notion_of_a_valid_material() {
        let mut fixture = Fixture::new();
        fixture.table = Table::parse("example:pick=example:a@2", "host");
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
        ]);
        let repaired = repair(&laid, &fixture.context(&everything_accepted)).expect("two units");
        assert_eq!(repaired.units, 2.0);
    }

    #[test]
    fn disabling_the_defaults_leaves_only_the_table() {
        let mut fixture = Fixture::new();
        fixture.settings.repair_materials.disable_defaults = true;
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
        ]);
        assert_eq!(
            repair(&laid, &fixture.context(&everything_accepted)),
            Err(Refused::WorthlessMaterial)
        );
    }

    #[test]
    fn override_mode_rejects_a_material_the_table_does_not_name_for_a_tool_it_does() {
        let mut fixture = Fixture::new();
        fixture.settings.repair_materials.override_mode = true;
        fixture.table = Table::parse("example:pick=example:a", "host");
        let named = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
        ]);
        assert!(repair(&named, &fixture.context(&everything_accepted)).is_ok());

        let unnamed = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:b"),
        ]);
        assert_eq!(
            repair(&unnamed, &fixture.context(&everything_accepted)),
            Err(Refused::WorthlessMaterial)
        );

        let untouched_tool = grid(vec![
            Held::tool("example:shovel", Shape::Other, 60, 60),
            Held::material("example:b"),
        ]);
        assert!(repair(&untouched_tool, &fixture.context(&everything_accepted)).is_ok());
    }

    #[test]
    fn stripping_removes_every_enchantment_that_is_not_a_curse() {
        let mut fixture = Fixture::new();
        fixture.settings.repair_materials.strip_enchantments = true;
        let mut tool = Held::tool("example:pick", Shape::Other, 60, 60);
        tool.enchantments = vec![
            Enchantment::default(),
            Enchantment {
                prevents_drop_on_death: true,
                prevents_unequip: false,
            },
            Enchantment {
                prevents_drop_on_death: false,
                prevents_unequip: true,
            },
        ];
        let laid = grid(vec![tool, Held::material("example:a")]);
        let repaired = repair(&laid, &fixture.context(&everything_accepted)).expect("a repair");
        assert_eq!(repaired.stripped, 1);
        assert_eq!(repaired.enchantments.len(), 2);
        assert!(repaired.enchantments.iter().all(|held| held.curse()));
    }

    #[test]
    fn a_curse_is_told_by_what_it_does_and_a_plain_enchantment_is_not_one() {
        assert!(!Enchantment::default().curse());
        assert!(
            Enchantment {
                prevents_drop_on_death: true,
                prevents_unequip: false
            }
            .curse()
        );
        assert!(
            Enchantment {
                prevents_unequip: true,
                prevents_drop_on_death: false
            }
            .curse()
        );
    }

    #[test]
    fn nothing_is_stripped_when_the_setting_is_off() {
        let fixture = Fixture::new();
        let mut tool = Held::tool("example:pick", Shape::Other, 60, 60);
        tool.enchantments = vec![Enchantment::default()];
        let laid = grid(vec![tool, Held::material("example:a")]);
        let repaired = repair(&laid, &fixture.context(&everything_accepted)).expect("a repair");
        assert_eq!(repaired.stripped, 0);
        assert_eq!(repaired.enchantments.len(), 1);
    }

    #[test]
    fn matching_and_assembling_are_the_same_computation() {
        let fixture = Fixture::new();
        let laid = grid(vec![
            Held::tool("example:pick", Shape::Other, 60, 60),
            Held::material("example:a"),
        ]);
        assert_eq!(
            matches(&laid, &fixture.context(&everything_accepted)),
            repair(&laid, &fixture.context(&everything_accepted)).is_ok()
        );
        assert!(!matches(&laid, &fixture.context(&nothing_accepted)));
    }

    #[test]
    fn every_slot_is_consumed_including_the_tools() {
        assert_eq!(remainders(9), vec![None; 9]);
        assert_eq!(remainders(0).len(), 0);
    }

    #[test]
    fn both_of_the_kinds_encodings_carry_nothing() {
        assert_eq!(encoding(), json!({}));
        assert_eq!(encoding().as_object().expect("an object encoding").len(), 0);
    }

    #[test]
    fn a_repair_never_produces_more_than_one_copy_or_less_damage_than_none() {
        let mut fixture = Fixture::new();
        fixture.table = Table::parse("example:pick=example:a@1000000", "host");
        let mut tool = Held::tool("example:pick", Shape::Other, 60, 60);
        tool.stack.count = 64;
        let laid = grid(vec![tool, Held::material("example:a")]);
        let repaired =
            repair(&laid, &fixture.context(&nothing_accepted)).expect("an enormous multiplier");
        assert_eq!(repaired.result.count, 1);
        assert_eq!(repaired.damage, 0);
    }

    #[test]
    fn a_shape_that_borrows_a_tier_is_told_from_one_that_does_not() {
        assert!(Shape::Digging.tiered());
        assert!(Shape::Sword.tiered());
        assert!(Shape::Shearing.tiered());
        assert!(!Shape::Ranged.tiered());
        assert!(!Shape::Armour.tiered());
        assert_eq!(Shape::ALL.len(), 6);
    }
}
