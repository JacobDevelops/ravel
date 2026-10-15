//! The portable arithmetic and the base behaviours built on it.
//!
//! Everything here is host-free: it takes positions, numbers and stacks and
//! answers with the same. What the answers are then applied to is the host's
//! business, which is why a sweep here reports what it would harvest rather than
//! harvesting anything.

use num_traits::ToPrimitive;

use crate::inventory::Stack;

/// A block position, as offsets resolve to.
pub type Position = (i32, i32, i32);

/// The horizontal square a sweeping harvest reaches, at the clicked height.
///
/// The clicked block is included, so a radius of one is nine positions.
#[must_use]
pub fn sweep_positions(origin: Position, radius: i32) -> Vec<Position> {
    let radius = radius.max(0);
    let mut positions = Vec::new();
    for x in -radius..=radius {
        for z in -radius..=radius {
            positions.push((origin.0 + x, origin.1, origin.2 + z));
        }
    }
    positions
}

/// The cube a radial mine reaches around the mined block.
#[must_use]
pub fn radial_positions(origin: Position, radius: i32) -> Vec<Position> {
    let radius = radius.max(0);
    let mut positions = Vec::new();
    for x in -radius..=radius {
        for y in -radius..=radius {
            for z in -radius..=radius {
                positions.push((origin.0 + x, origin.1 + y, origin.2 + z));
            }
        }
    }
    positions
}

/// Whether a radial mine breaks a neighbour of a given hardness.
///
/// A neighbour harder than the mined block by more than five is left, which is
/// what stops a tool tearing through a stronger material it happens to be valid
/// for. A tool the neighbour is not correct for is left whatever its hardness.
#[must_use]
pub fn breaks_neighbour(mined_hardness: f64, neighbour_hardness: f64, correct_tool: bool) -> bool {
    correct_tool && neighbour_hardness - mined_hardness <= 5.0
}

/// The probability a neighbour that breaks instantly charges one damage.
pub const INSTANT_CHARGE_PROBABILITY: f64 = 1.0 / 3.0;

/// Whether a neighbour that breaks instantly charges the tool one damage.
///
/// The caller supplies the draw from wherever its randomness lives, which keeps
/// this file host-free and puts both sides of the threshold under a test.
#[must_use]
pub fn charges_instant_break(draw: f64) -> bool {
    draw < INSTANT_CHARGE_PROBABILITY
}

/// What one sweep did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SweepReport {
    /// The positions actually harvested, in the order they were reached.
    pub harvested: Vec<Position>,
    /// The positions a listener cancelled.
    pub cancelled: Vec<Position>,
    /// The damage the tool takes, one if anything was harvested.
    pub damage: u32,
    /// Whether the sound plays, which it does once if anything was harvested.
    pub sound: bool,
    /// What the sweep produced, with exactly one seed of each crop withheld.
    pub drops: Vec<Stack>,
}

/// What one position offers a sweep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mature {
    /// The stacks the crop would produce if it were broken outright.
    pub drops: Vec<Stack>,
    /// The entry a single withheld seed is taken from, for a replanting crop.
    pub seed: Option<String>,
}

/// Sweeps a square, asking a cancellable listener at each position first.
///
/// `offer` answers what a position holds, or nothing when it holds no mature
/// crop. `allow` is the cancellable event: a cancel stops that one position
/// without stopping the rest of the sweep. Exactly one of a crop's own seed is
/// withheld from its drops, which is what makes the sweep replanting rather than
/// farming, and the tool takes one damage if anything was harvested at all.
pub fn sweep_harvest(
    origin: Position,
    radius: i32,
    offer: &dyn Fn(Position) -> Option<Mature>,
    allow: &mut dyn FnMut(Position) -> bool,
) -> SweepReport {
    let mut report = SweepReport::default();
    for position in sweep_positions(origin, radius) {
        let Some(mature) = offer(position) else {
            continue;
        };
        if !allow(position) {
            report.cancelled.push(position);
            continue;
        }
        report.harvested.push(position);
        report.drops.extend(withhold_one_seed(mature));
    }
    if !report.harvested.is_empty() {
        report.damage = 1;
        report.sound = true;
    }
    report
}

fn withhold_one_seed(mature: Mature) -> Vec<Stack> {
    let Some(seed) = mature.seed else {
        return mature.drops;
    };
    let mut withheld = false;
    let mut drops = Vec::new();
    for mut stack in mature.drops {
        if !withheld && stack.entry == seed && stack.count > 0 {
            withheld = true;
            stack.count -= 1;
            if stack.count == 0 {
                continue;
            }
        }
        drops.push(stack);
    }
    drops
}

/// The radius a sweeping harvest tool reaches as a weapon.
///
/// One at radius one, growing by a quarter for every step beyond.
#[must_use]
pub fn weapon_radius(radius: u32) -> f64 {
    if radius == 0 {
        return 0.0;
    }
    let growth = 0.25 * f64::from(radius - 1);
    1.0 + growth
}

/// The damage a bystander takes from a full-strength sweeping attack.
///
/// The attacker, the target and the target's allies are excluded by the caller;
/// this is only the share.
#[must_use]
pub fn sweep_damage(attack_damage: f64) -> f64 {
    attack_damage * 2.0 / 3.0
}

/// The three ability families a combination tool answers yes to.
///
/// The order is the fixed one its use-on interaction tries: the first family
/// that applies to what was clicked wins and no later one is asked.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Family {
    /// Stripping the surface off what was clicked.
    Stripping,
    /// Flattening what was clicked into a path.
    Flattening,
    /// Tilling what was clicked into soil.
    Tilling,
}

impl Family {
    /// Every family, in the order a use-on interaction tries them.
    pub const ORDER: [Self; 3] = [Self::Stripping, Self::Flattening, Self::Tilling];
}

/// The family a combination tool's use-on interaction settles on.
///
/// `applies` answers whether one family has anything to do with what was
/// clicked. The first that does is taken and the rest are never asked, so two
/// families that both apply are decided by [`Family::ORDER`] rather than by the
/// caller.
#[must_use]
pub fn combination_family(applies: &dyn Fn(Family) -> bool) -> Option<Family> {
    Family::ORDER.into_iter().find(|family| applies(*family))
}

/// The durability a combination tool carries: one and a half times its material's.
#[must_use]
pub fn combination_durability(material_durability: u32) -> u32 {
    u32::try_from((u64::from(material_durability) * 3).div_euclid(2)).unwrap_or(u32::MAX)
}

/// The half-extent of the cube a refillable tool wets and grows.
///
/// The declared side length less one, halved and truncated. A declared side of
/// one is a single block, and an even side behaves as the odd number below it.
/// Computing the half-extent the obvious way gives a different area.
#[must_use]
pub const fn half_extent(side: u32) -> u32 {
    side.saturating_sub(1).div_euclid(2)
}

/// What one block in a refillable tool's cube offers it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Soaked {
    /// Whether the block is soil the tool wets to its maximum wetness.
    pub wettable: bool,
    /// Whether the block advances a growth step when the draw permits it.
    pub growable: bool,
}

/// What one use of a refillable liquid tool did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SoakReport {
    /// The positions wetted to maximum, in the order they were reached.
    pub wetted: Vec<Position>,
    /// The positions advanced one growth step.
    pub grown: Vec<Position>,
}

/// Wets and grows the cube a refillable liquid tool reaches.
///
/// The cube's half-extent is [`half_extent`] of the declared side. Every soil
/// block in it goes to maximum wetness unconditionally; every growable block
/// advances one step when its own draw falls under the declared probability, so
/// the probability is per block rather than per use. What maximum wetness is and
/// what a step advances to are the host's; this decides which positions get one.
#[must_use]
pub fn soak(
    origin: Position,
    side: u32,
    probability: f64,
    offer: &dyn Fn(Position) -> Soaked,
    draw: &mut dyn FnMut(Position) -> f64,
) -> SoakReport {
    let radius = i32::try_from(half_extent(side)).unwrap_or(i32::MAX);
    let mut report = SoakReport::default();
    for position in radial_positions(origin, radius) {
        let offered = offer(position);
        if offered.wettable {
            report.wetted.push(position);
        }
        if offered.growable && draw(position) < probability {
            report.grown.push(position);
        }
    }
    report
}

/// Grows a stack, held to a limit, answering with what did not fit.
#[must_use]
pub fn grow(stack: &Stack, by: u64, limit: u64) -> (Stack, u64) {
    let mut grown = stack.clone();
    let room = limit.saturating_sub(grown.count);
    let taken = by.min(room);
    grown.count += taken;
    (grown, by - taken)
}

/// Reduces a stack, optionally yielding the crafting remainder at zero.
///
/// Reducing to nothing answers with the remainder when one was supplied, which
/// is what a reusable entry relies on, and with nothing otherwise.
#[must_use]
pub fn reduce(stack: &Stack, by: u64, remainder: Option<Stack>) -> Option<Stack> {
    let left = stack.count.saturating_sub(by);
    if left == 0 {
        return remainder;
    }
    let mut reduced = stack.clone();
    reduced.count = left;
    Some(reduced)
}

/// Merges one stack into another, answering with the merged stack and the rest.
///
/// Two stacks combine only on the same entry and the same data; the merged stack
/// is filled up to the limit and any overflow comes back as the rest. A stack
/// that cannot combine comes back whole.
#[must_use]
pub fn insert(into: &Stack, incoming: &Stack, limit: u64) -> (Stack, Option<Stack>) {
    if into.is_empty() {
        let (merged, over) = grow(&Stack::new(&incoming.entry, 0), incoming.count, limit);
        let mut merged = merged;
        merged.data = incoming.data.clone();
        return (merged, rest(incoming, over));
    }
    if !into.merges_with(incoming) {
        return (into.clone(), Some(incoming.clone()));
    }
    let (merged, over) = grow(into, incoming.count, limit);
    (merged, rest(incoming, over))
}

fn rest(incoming: &Stack, over: u64) -> Option<Stack> {
    if over == 0 {
        return None;
    }
    let mut left = incoming.clone();
    left.count = over;
    Some(left)
}

/// A colour, as four channels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Colour {
    /// The opacity channel.
    pub opacity: u8,
    /// The red channel.
    pub red: u8,
    /// The green channel.
    pub green: u8,
    /// The blue channel.
    pub blue: u8,
}

impl Colour {
    /// Builds a colour from four channels.
    #[must_use]
    pub const fn new(opacity: u8, red: u8, green: u8, blue: u8) -> Self {
        Self {
            opacity,
            red,
            green,
            blue,
        }
    }

    /// Packs the four channels into one number, opacity highest.
    #[must_use]
    pub const fn pack(self) -> u32 {
        u32::from_be_bytes([self.opacity, self.red, self.green, self.blue])
    }

    /// Reads four channels back out of one number.
    #[must_use]
    pub const fn unpack(packed: u32) -> Self {
        let [opacity, red, green, blue] = packed.to_be_bytes();
        Self {
            opacity,
            red,
            green,
            blue,
        }
    }

    /// Returns this colour with a different opacity.
    #[must_use]
    pub const fn with_opacity(self, opacity: u8) -> Self {
        Self { opacity, ..self }
    }

    /// Scales this colour's saturation, keeping hue, brightness and opacity.
    #[must_use]
    pub fn scale_saturation(self, factor: f64) -> Self {
        let (hue, saturation, brightness) = self.to_hsb();
        Self::from_hsb(hue, (saturation * factor).clamp(0.0, 1.0), brightness)
            .with_opacity(self.opacity)
    }

    /// Interpolates between two colours through hue, saturation and brightness.
    #[must_use]
    pub fn interpolate(self, other: Self, ratio: f64) -> Self {
        let ratio = ratio.clamp(0.0, 1.0);
        let (from_hue, from_saturation, from_brightness) = self.to_hsb();
        let (to_hue, to_saturation, to_brightness) = other.to_hsb();
        // Around the short way, so a hue near one and a hue near zero meet at the
        // wrap rather than sweeping the whole circle between them.
        let mut span = to_hue - from_hue;
        if span > 0.5 {
            span -= 1.0;
        } else if span < -0.5 {
            span += 1.0;
        }
        let hue_step = span * ratio;
        let saturation_step = (to_saturation - from_saturation) * ratio;
        let brightness_step = (to_brightness - from_brightness) * ratio;
        let opacity_step = (f64::from(other.opacity) - f64::from(self.opacity)) * ratio;
        let hue = (from_hue + hue_step).rem_euclid(1.0);
        let saturation = from_saturation + saturation_step;
        let brightness = from_brightness + brightness_step;
        let opacity = f64::from(self.opacity) + opacity_step;
        Self::from_hsb(hue, saturation, brightness).with_opacity(round_channel(opacity))
    }

    fn to_hsb(self) -> (f64, f64, f64) {
        let red = f64::from(self.red) / 255.0;
        let green = f64::from(self.green) / 255.0;
        let blue = f64::from(self.blue) / 255.0;
        let high = red.max(green).max(blue);
        let low = red.min(green).min(blue);
        let span = high - low;
        let saturation = if high == 0.0 { 0.0 } else { span / high };
        let hue = if span == 0.0 {
            0.0
        } else if (high - red).abs() < f64::EPSILON {
            ((green - blue) / span).rem_euclid(6.0)
        } else if (high - green).abs() < f64::EPSILON {
            (blue - red) / span + 2.0
        } else {
            (red - green) / span + 4.0
        };
        (hue / 6.0, saturation, high)
    }

    fn from_hsb(hue: f64, saturation: f64, brightness: f64) -> Self {
        let hue = hue.rem_euclid(1.0) * 6.0;
        let saturation = saturation.clamp(0.0, 1.0);
        let brightness = brightness.clamp(0.0, 1.0);
        let sector = hue.floor();
        let offset = hue - sector;
        let p = brightness * (1.0 - saturation);
        let q_saturation = saturation * offset;
        let t_saturation = saturation * (1.0 - offset);
        let q = brightness * (1.0 - q_saturation);
        let t = brightness * (1.0 - t_saturation);
        // Finite wrapped sectors fit i32; a non-finite hue keeps the original zero sector.
        let (red, green, blue) = match sector.to_i32().unwrap_or(0) % 6 {
            0 => (brightness, t, p),
            1 => (q, brightness, p),
            2 => (p, brightness, t),
            3 => (p, q, brightness),
            4 => (t, p, brightness),
            _ => (brightness, p, q),
        };
        Self::new(
            255,
            round_channel(red * 255.0),
            round_channel(green * 255.0),
            round_channel(blue * 255.0),
        )
    }
}

fn round_channel(value: f64) -> u8 {
    let rounded = value.round();
    if rounded <= 0.0 {
        0
    } else if rounded >= 255.0 {
        255
    } else {
        // The bounds above leave only an in-range channel or NaN, which encodes as zero.
        rounded.to_u8().unwrap_or(0)
    }
}

/// An opacity derived from a ratio.
///
/// The original truncates the ratio to an integer before multiplying, so it can
/// only ever answer zero or the maximum. That is a defect; this computes it in
/// floating point.
#[must_use]
pub fn opacity_from_ratio(ratio: f64) -> u8 {
    round_channel(ratio.clamp(0.0, 1.0) * 255.0)
}

/// Resolves declared offsets against an origin.
#[must_use]
pub fn resolve_offsets(origin: Position, offsets: &[Position]) -> Vec<Position> {
    offsets
        .iter()
        .map(|offset| {
            (
                origin.0 + offset.0,
                origin.1 + offset.1,
                origin.2 + offset.2,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mature(drops: Vec<Stack>, seed: &str) -> Mature {
        Mature {
            drops,
            seed: Some(seed.to_owned()),
        }
    }

    #[test]
    fn a_sweep_reaches_the_square_of_its_radius_at_the_clicked_height() {
        let positions = sweep_positions((0, 5, 0), 1);
        assert_eq!(positions.len(), 9);
        assert!(positions.iter().all(|position| position.1 == 5));
        assert!(positions.contains(&(0, 5, 0)));
        assert_eq!(sweep_positions((0, 0, 0), 0).len(), 1);
    }

    #[test]
    fn a_radial_mine_reaches_the_cube_of_its_radius() {
        assert_eq!(radial_positions((0, 0, 0), 1).len(), 27);
        assert_eq!(radial_positions((0, 0, 0), 2).len(), 125);
    }

    #[test]
    fn a_neighbour_harder_by_more_than_five_is_left_and_so_is_a_wrong_tool() {
        assert!(breaks_neighbour(3.0, 8.0, true));
        assert!(!breaks_neighbour(3.0, 8.1, true));
        assert!(!breaks_neighbour(3.0, 3.0, false));
    }

    #[test]
    fn a_sweep_withholds_exactly_one_seed_and_damages_the_tool_once() {
        let offer = |position: Position| {
            (position.0 == 0)
                .then(|| mature(vec![Stack::new("seed", 3), Stack::new("crop", 2)], "seed"))
        };
        let mut allow = |_: Position| true;
        let report = sweep_harvest((0, 0, 0), 1, &offer, &mut allow);

        assert_eq!(report.harvested.len(), 3);
        assert_eq!(report.damage, 1);
        assert!(report.sound);
        let seeds: u64 = report
            .drops
            .iter()
            .filter(|stack| stack.entry == "seed")
            .map(|stack| stack.count)
            .sum();
        assert_eq!(
            seeds, 6,
            "one seed of the three is withheld at each position"
        );
    }

    #[test]
    fn withholding_the_last_seed_drops_the_stack_rather_than_leaving_an_empty_one() {
        let drops = withhold_one_seed(mature(vec![Stack::new("seed", 1)], "seed"));
        assert_eq!(drops, Vec::<Stack>::new());
    }

    #[test]
    fn a_cancel_stops_one_position_without_stopping_the_sweep() {
        let offer = |_: Position| Some(mature(vec![Stack::new("crop", 1)], "seed"));
        let mut allow = |position: Position| position != (0, 0, 0);
        let report = sweep_harvest((0, 0, 0), 1, &offer, &mut allow);

        assert_eq!(report.cancelled, [(0, 0, 0)]);
        assert_eq!(report.harvested.len(), 8);
        assert!(report.sound);
    }

    #[test]
    fn a_sweep_that_harvested_nothing_takes_no_damage_and_plays_no_sound() {
        let offer = |_: Position| None;
        let mut allow = |_: Position| true;
        let report = sweep_harvest((0, 0, 0), 2, &offer, &mut allow);
        assert_eq!(report.damage, 0);
        assert!(!report.sound);
        assert_eq!(report.drops, Vec::<Stack>::new());
    }

    #[test]
    fn the_weapon_radius_is_one_at_radius_one_and_grows_by_a_quarter() {
        assert!((weapon_radius(1) - 1.0).abs() < f64::EPSILON);
        assert!((weapon_radius(2) - 1.25).abs() < f64::EPSILON);
        assert!((weapon_radius(5) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_bystander_takes_two_thirds_of_the_attack_damage() {
        assert!((sweep_damage(9.0) - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_combination_tool_carries_one_and_a_half_times_its_material() {
        assert_eq!(combination_durability(250), 375);
        assert_eq!(combination_durability(3), 4);
    }

    #[test]
    fn a_use_on_stops_at_the_first_family_that_applies() {
        assert_eq!(
            combination_family(&|family| family != Family::Stripping),
            Some(Family::Flattening),
            "the order decides, not which families the caller says apply"
        );
        assert_eq!(
            combination_family(&|family| family == Family::Tilling),
            Some(Family::Tilling)
        );
        assert_eq!(combination_family(&|_| false), None);
        assert_eq!(combination_family(&|_| true), Some(Family::Stripping));
        assert_eq!(Family::ORDER.len(), 3);
    }

    #[test]
    fn an_instantly_breakable_neighbour_charges_on_a_draw_under_a_third() {
        assert!(charges_instant_break(0.0));
        assert!(charges_instant_break(0.33));
        assert!(!charges_instant_break(INSTANT_CHARGE_PROBABILITY));
        assert!(!charges_instant_break(0.99));
    }

    #[test]
    fn a_refillable_tool_wets_every_soil_block_and_grows_only_where_the_draw_permits() {
        let offer = |position: Position| Soaked {
            wettable: position.1 == 0,
            growable: position.1 == 1,
        };
        let mut draws = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut draw = move |_| draws.pop().unwrap_or(1.0);
        let report = soak((0, 0, 0), 3, 0.5, &offer, &mut draw);

        assert_eq!(
            report.wetted.len(),
            9,
            "the whole layer at the aimed height"
        );
        assert_eq!(
            report.grown.len(),
            8,
            "the one draw at or above the probability grows nothing"
        );
        assert!(report.wetted.iter().all(|position| position.1 == 0));
        assert!(report.grown.iter().all(|position| position.1 == 1));
    }

    #[test]
    fn a_refillable_tool_declaring_a_side_of_one_reaches_the_aimed_block_alone() {
        let offer = |_| Soaked {
            wettable: true,
            growable: true,
        };
        let mut draw = |_| 0.0;
        let report = soak((4, 5, 6), 1, 1.0, &offer, &mut draw);

        assert_eq!(report.wetted, [(4, 5, 6)]);
        assert_eq!(report.grown, [(4, 5, 6)]);
    }

    #[test]
    fn the_half_extent_truncates_so_an_even_side_behaves_as_the_odd_one_below() {
        assert_eq!(half_extent(1), 0);
        assert_eq!(half_extent(3), 1);
        assert_eq!(half_extent(4), 1);
        assert_eq!(half_extent(5), 2);
        assert_eq!(half_extent(0), 0);
    }

    #[test]
    fn growing_a_stack_answers_with_what_did_not_fit() {
        let (grown, over) = grow(&Stack::new("one", 60), 10, 64);
        assert_eq!(grown.count, 64);
        assert_eq!(over, 6);
    }

    #[test]
    fn reducing_to_nothing_yields_the_remainder_when_one_was_supplied() {
        let stack = Stack::new("one", 1);
        assert_eq!(reduce(&stack, 1, None), None);
        assert_eq!(
            reduce(&stack, 1, Some(Stack::new("empty", 1))),
            Some(Stack::new("empty", 1))
        );
        assert_eq!(
            reduce(&Stack::new("one", 5), 2, None),
            Some(Stack::new("one", 3))
        );
    }

    #[test]
    fn two_stacks_on_the_same_entry_and_data_merge_up_to_the_limit() {
        let (merged, rest) = insert(&Stack::new("one", 30), &Stack::new("one", 40), 64);
        assert_eq!(merged.count, 64);
        assert_eq!(rest, Some(Stack::new("one", 6)));

        let (kept, refused) = insert(&Stack::new("one", 1), &Stack::new("two", 1), 64);
        assert_eq!(kept, Stack::new("one", 1));
        assert_eq!(refused, Some(Stack::new("two", 1)));
    }

    #[test]
    fn an_insert_into_an_empty_slot_takes_the_incoming_data() {
        let mut incoming = Stack::new("one", 3);
        incoming.data = ravel_sdk::json!({ "charge": 2 });
        let (merged, rest) = insert(&Stack::default(), &incoming, 64);
        assert_eq!(merged.count, 3);
        assert_eq!(merged.data, incoming.data);
        assert_eq!(rest, None);
    }

    #[test]
    fn a_colour_packs_and_unpacks_through_one_number() {
        let colour = Colour::new(0x80, 0x11, 0x22, 0x33);
        assert_eq!(colour.pack(), 0x8011_2233);
        assert_eq!(Colour::unpack(colour.pack()), colour);
    }

    #[test]
    fn scaling_saturation_to_nothing_leaves_a_grey_of_the_same_brightness() {
        let grey = Colour::new(255, 200, 0, 0).scale_saturation(0.0);
        assert_eq!(grey, Colour::new(255, 200, 200, 200));
    }

    #[test]
    fn interpolating_ends_at_each_end_and_carries_the_opacity_across() {
        let from = Colour::new(0, 255, 0, 0);
        let to = Colour::new(255, 0, 0, 255);
        assert_eq!(from.interpolate(to, 0.0), from);
        assert_eq!(from.interpolate(to, 1.0), to);
        assert_eq!(from.interpolate(to, 0.5).opacity, 128);
    }

    #[test]
    fn an_opacity_from_a_ratio_is_computed_in_floating_point_rather_than_truncated() {
        assert_eq!(opacity_from_ratio(0.0), 0);
        assert_eq!(opacity_from_ratio(1.0), 255);
        assert_eq!(
            opacity_from_ratio(0.5),
            128,
            "truncating the ratio first would answer zero here"
        );
        assert_eq!(opacity_from_ratio(2.0), 255);
    }

    #[test]
    fn offsets_resolve_against_their_origin() {
        assert_eq!(
            resolve_offsets((10, 20, 30), &[(0, 0, 0), (-1, 2, 3)]),
            [(10, 20, 30), (9, 22, 33)]
        );
    }
}
