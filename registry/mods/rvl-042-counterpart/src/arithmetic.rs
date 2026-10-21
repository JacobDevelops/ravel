//! The portable arithmetic and the base behaviours built on it.
//!
//! Everything here is host-free: it takes positions, numbers and stacks and
//! answers with the same. What the answers are then applied to is the host's
//! business, which is why a sweep here reports what it would harvest rather than
//! harvesting anything.

use ravel_sdk::content::ContentId;

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
    pub seed: Option<ContentId>,
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
    let radius = half_extent(side).cast_signed();
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

    fn id(entry: &str) -> ContentId {
        entry.parse().expect("a valid test identity")
    }

    fn mature(drops: Vec<Stack>, seed: &str) -> Mature {
        Mature {
            drops,
            seed: Some(
                format!("test:{seed}")
                    .parse()
                    .expect("a valid seed identity"),
            ),
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
            (position.0 == 0).then(|| {
                mature(
                    vec![
                        Stack::new(id("test:seed"), 3),
                        Stack::new(id("test:crop"), 2),
                    ],
                    "seed",
                )
            })
        };
        let mut allow = |_: Position| true;
        let report = sweep_harvest((0, 0, 0), 1, &offer, &mut allow);

        assert_eq!(report.harvested.len(), 3);
        assert_eq!(report.damage, 1);
        assert!(report.sound);
        let seeds: u64 = report
            .drops
            .iter()
            .filter(|stack| stack.entry == id("test:seed"))
            .map(|stack| stack.count)
            .sum();
        assert_eq!(
            seeds, 6,
            "one seed of the three is withheld at each position"
        );
    }

    #[test]
    fn withholding_the_last_seed_drops_the_stack_rather_than_leaving_an_empty_one() {
        let drops = withhold_one_seed(mature(vec![Stack::new(id("test:seed"), 1)], "seed"));
        assert_eq!(drops, Vec::<Stack>::new());
    }

    #[test]
    fn a_cancel_stops_one_position_without_stopping_the_sweep() {
        let offer = |_: Position| Some(mature(vec![Stack::new(id("test:crop"), 1)], "seed"));
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
    fn offsets_resolve_against_their_origin() {
        assert_eq!(
            resolve_offsets((10, 20, 30), &[(0, 0, 0), (-1, 2, 3)]),
            [(10, 20, 30), (9, 22, 33)]
        );
    }
}
