//! The machines, the two rituals, and what each of them persists.
//!
//! Every store here is the shared library's persistence envelope, so the counts
//! above the normal maximum, the partial-load tolerance and the slot and output
//! rules all come from there rather than being restated. What is added on top —
//! progress, the burn, the energy amount, the upgrade and the scan cursor — is
//! this entry's own and is what a reload has to bring back.
//!
//! Ravel hands a mod a payload for its saved data rather than a key-addressed
//! compound, so each block entity here carries one payload shape and the write
//! itself belongs to Ravel's saved-data lifecycle. One shape is written per
//! field, not two: the original writes the energy amount in two different
//! spellings across five machines, both of which produce the same integer.

use ravel_sdk::save::SavedDataVersion;
use ravel_sdk::{JsonValue, json};
use rvl_042_counterpart::arithmetic::insert;
use rvl_042_counterpart::inventory::{Store, StoreRefused};
use rvl_042_counterpart::{Position, Stack};

/// The energy a machine's store holds with no upgrade.
pub const BASE_CAPACITY: u64 = 80_000;

/// The version every machine payload here is written at.
pub const DATA_VERSION: SavedDataVersion = SavedDataVersion::MIN;

/// The multiplier a burnable item's burn time is scaled by.
pub const BURN_SCALE: u64 = 20;

/// The progress a ritual completes at.
pub const RITUAL_COMPLETION: u32 = 100;

/// The slot limit a reservoir holds.
pub const RESERVOIR_LIMIT: u64 = 40;

/// The input cap on the creature-summoning machine.
pub const SUMMONING_INPUT_LIMIT: u64 = 512;

/// The number of living creatures of a kind that refuses a summoning.
pub const CROWD_LIMIT: usize = 32;

/// The half-extent of the cube a summoning counts and places within.
pub const SUMMONING_REACH: i32 = 3;

/// How many positions a summoning tries before giving up.
pub const SUMMONING_ATTEMPTS: u32 = 20;

/// One upgrade, and everything derived from holding it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Upgrade {
    /// Where this upgrade sits in the table, counting from one.
    position: u32,
    /// The tier value the upgrade borrows for its colour and its acceptance.
    pub tier_value: u32,
    /// What the operation time is multiplied by.
    pub operation: f64,
    /// What the fuel usage is multiplied by.
    pub usage: u32,
    /// What the fuel capacity is multiplied by.
    pub capacity: u32,
    /// How much range the upgrade adds.
    pub range: u32,
}

/// The six upgrades, in the order the table states them.
///
/// The sixth borrows the fifth tier's value, which is the whole of the defect
/// in the acceptance rule below.
pub const UPGRADES: [Upgrade; 6] = [
    Upgrade {
        position: 1,
        tier_value: 1,
        operation: 0.833,
        usage: 2,
        capacity: 2,
        range: 1,
    },
    Upgrade {
        position: 2,
        tier_value: 2,
        operation: 0.625,
        usage: 4,
        capacity: 3,
        range: 2,
    },
    Upgrade {
        position: 3,
        tier_value: 3,
        operation: 0.4,
        usage: 6,
        capacity: 4,
        range: 3,
    },
    Upgrade {
        position: 4,
        tier_value: 4,
        operation: 0.125,
        usage: 16,
        capacity: 5,
        range: 4,
    },
    Upgrade {
        position: 5,
        tier_value: 5,
        operation: 0.025,
        usage: 48,
        capacity: 6,
        range: 5,
    },
    Upgrade {
        position: 6,
        tier_value: 5,
        operation: 0.005,
        usage: 96,
        capacity: 7,
        range: 6,
    },
];

impl Upgrade {
    /// The declared table position, counting from one.
    #[must_use]
    pub const fn position(self) -> u32 {
        self.position
    }

    /// The upgrade sitting at `position` in the table, counting from one.
    ///
    /// This is what makes a saved position readable: a payload names an
    /// upgrade by its position and nothing else, so a position outside the
    /// table is a payload this build cannot honour rather than a multiplier to
    /// invent.
    #[must_use]
    pub fn at(position: u32) -> Option<Self> {
        UPGRADES
            .iter()
            .copied()
            .find(|upgrade| upgrade.position == position)
    }

    /// The operation time a base is reduced to, truncating toward zero.
    #[must_use]
    pub fn operation_ticks(self, base: u32) -> u32 {
        let ticks = (f64::from(base) * self.operation).trunc();
        num_traits::ToPrimitive::to_u32(&ticks).unwrap_or_else(|| {
            if ticks.is_nan() || ticks < 0.0 {
                0
            } else {
                u32::MAX
            }
        })
    }

    /// The fuel usage a base is raised to.
    #[must_use]
    pub const fn usage_per_tick(self, base: u32) -> u32 {
        base * self.usage
    }

    /// The energy capacity this upgrade gives a machine.
    #[must_use]
    pub const fn capacity(self) -> u64 {
        let [first, second, third, fourth] = self.capacity.to_le_bytes();
        BASE_CAPACITY * u64::from_le_bytes([first, second, third, fourth, 0, 0, 0, 0])
    }
}

/// Whether a machine holding one upgrade accepts another.
///
/// Upgrades are ordered by their own position in the table rather than by the
/// tier value they borrow. The original compares the borrowed value, and
/// because the sixth upgrade borrows the fifth tier's, a machine already
/// holding the fifth refuses the sixth outright even though every multiplier
/// says the sixth is a step up.
#[must_use]
pub const fn accepts(held: Option<Upgrade>, incoming: Upgrade) -> bool {
    match held {
        None => true,
        Some(held) => held.position < incoming.position,
    }
}

/// The base operation time and fuel usage a machine runs at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bases {
    /// Ticks one operation takes with no upgrade.
    pub operation: u32,
    /// Energy drawn per tick with no upgrade.
    pub usage: u32,
}

/// The effective operation figures after applying any upgrade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timing {
    /// Ticks required for one operation after upgrade scaling.
    pub operation: u32,
    /// Energy charged for one harvest after upgrade scaling.
    pub usage: u32,
}

/// The powered smelting machine's bases.
pub const SMELTING: Bases = Bases {
    operation: 200,
    usage: 20,
};
/// The seed-conversion machine's bases.
pub const CONVERSION: Bases = Bases {
    operation: 200,
    usage: 20,
};
/// The charge-extraction machine's bases.
pub const EXTRACTION: Bases = Bases {
    operation: 100,
    usage: 40,
};
/// The harvesting machine's bases.
pub const HARVESTING: Bases = Bases {
    operation: 100,
    usage: 40,
};
/// The creature-summoning machine's bases.
pub const SUMMONING: Bases = Bases {
    operation: 200,
    usage: 20,
};

/// What a harvesting step that found nothing to harvest costs.
pub const SCAN_COST: u32 = 10;

/// A machine's fuel and energy store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Energy {
    /// How much energy is held.
    pub amount: u64,
    /// How much the store holds at most.
    pub capacity: u64,
    /// How much of the current burn is left.
    pub remaining: u64,
    /// How long the current burn was when it started.
    pub total: u64,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            amount: 0,
            capacity: BASE_CAPACITY,
            remaining: 0,
            total: 0,
        }
    }
}

impl Energy {
    /// Whether the store is full.
    #[must_use]
    pub const fn full(&self) -> bool {
        self.amount >= self.capacity
    }

    /// Starts a burn from one fuel item, answering whether one was consumed.
    ///
    /// A burn starts only while the store has room and nothing is already
    /// burning, and the item's burn time is scaled before it is recorded.
    pub fn light(&mut self, burn_time: Option<u64>) -> bool {
        if self.full() || self.remaining > 0 {
            return false;
        }
        let Some(time) = burn_time else {
            return false;
        };
        let scaled = time * BURN_SCALE;
        self.remaining = scaled;
        self.total = scaled;
        true
    }

    /// Moves one tick of the burn into the store, answering how much moved.
    ///
    /// The move is capped by what is left of the burn, by twice the usage, and
    /// by the room the store has. What the store did not accept stays in the
    /// burn, and reaching zero clears the recorded total.
    pub fn advance_burn(&mut self, usage: u32) -> u64 {
        if self.remaining == 0 {
            return 0;
        }
        let headroom = self.capacity.saturating_sub(self.amount);
        let moved = self.remaining.min(u64::from(usage) * 2).min(headroom);
        self.amount += moved;
        self.remaining -= moved;
        if self.remaining == 0 {
            self.total = 0;
        }
        moved
    }

    /// Charges the store, answering whether it could pay.
    pub fn charge(&mut self, cost: u32) -> bool {
        let cost = u64::from(cost);
        if self.amount < cost {
            return false;
        }
        self.amount -= cost;
        true
    }

    /// Sets the capacity to whatever the held upgrade says.
    ///
    /// Energy already stored above a reduced capacity is not clamped, which is
    /// the original's behaviour and is left alone: clamping would destroy a
    /// player's energy on removing an upgrade.
    pub const fn retune(&mut self, upgrade: Option<Upgrade>) {
        self.capacity = match upgrade {
            None => BASE_CAPACITY,
            Some(upgrade) => upgrade.capacity(),
        };
    }

    /// The saved shape, one spelling for the amount.
    #[must_use]
    pub fn save(&self) -> JsonValue {
        json!({
            "amount": self.amount,
            "capacity": self.capacity,
            "remaining": self.remaining,
            "total": self.total,
        })
    }

    /// Reads the saved shape, defaulting only absent fields.
    ///
    /// # Errors
    ///
    /// Refuses a non-object payload or any present field that is not a `u64`.
    pub fn load(payload: &JsonValue) -> Result<Self, SavedFieldError> {
        if !payload.is_object() {
            return Err(SavedFieldError("energy"));
        }
        Ok(Self {
            amount: saved_u64(payload, "amount", 0)?,
            capacity: saved_u64(payload, "capacity", BASE_CAPACITY)?,
            remaining: saved_u64(payload, "remaining", 0)?,
            total: saved_u64(payload, "total", 0)?,
        })
    }
}

/// Why a machine did not advance this tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stalled {
    /// A neighbouring redstone signal disables this machine.
    Redstone,
    /// The store holds less than one tick's usage.
    Unpowered,
}

impl std::fmt::Display for Stalled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Redstone => "a neighbouring redstone signal disables the machine",
            Self::Unpowered => "the machine stores less energy than one tick requires",
        })
    }
}

impl std::error::Error for Stalled {}

/// What one tick of a recipe machine did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ticked {
    /// The progress the machine now holds.
    pub progress: u32,
    /// Whether the operation completed on this tick.
    pub completed: bool,
    /// Whether the input went away and the progress was reset to zero.
    pub reset: bool,
}

/// The persisted progress and derived timing of a recipe machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipeStep {
    /// Progress toward the current operation.
    pub progress: u32,
    /// Ticks required to finish an operation.
    pub operation: u32,
    /// Energy charged for each advancing tick.
    pub usage: u32,
}

/// Whether a machine observes the neighbouring redstone signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Redstone {
    /// This machine ignores redstone.
    Ignored,
    /// A powered signal disables this machine.
    Gated {
        /// Whether the neighbouring signal is powered.
        powered: bool,
    },
}

/// Advances one of the four recipe machines by a tick.
///
/// The four run only while the store holds at least one tick's usage, charge
/// that usage on every tick they advance, and complete when progress reaches or
/// exceeds the derived operation time. Losing power holds progress where it is;
/// losing the input resets it to zero and charges nothing.
///
/// What counts as having an input is the caller's, and it is where the
/// creature-summoning machine differs from the other three: that machine passes
/// whether its input slot is occupied at all, so an input short of the recipe's
/// declared count or matching no recipe holds progress, while the other three
/// pass whether a recipe matches.
///
/// # Errors
///
/// Answers with the rule that stopped the machine.
pub fn tick_recipe_machine(
    energy: &mut Energy,
    step: RecipeStep,
    has_input: bool,
    redstone: Redstone,
) -> Result<Ticked, Stalled> {
    if redstone == (Redstone::Gated { powered: true }) {
        return Err(Stalled::Redstone);
    }
    if !has_input {
        return Ok(Ticked {
            progress: 0,
            completed: false,
            reset: true,
        });
    }
    if energy.amount < u64::from(step.usage) {
        return Err(Stalled::Unpowered);
    }
    energy.charge(step.usage);
    let progress = step.progress + 1;
    Ok(Ticked {
        progress: if progress >= step.operation {
            0
        } else {
            progress
        },
        completed: progress >= step.operation,
        reset: false,
    })
}

/// The harvesting machine's own two persisted numbers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Held {
    /// The progress toward the current step.
    pub progress: u32,
    /// The scan cursor.
    pub cursor: u32,
}

/// What one tick of the harvesting machine did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Swept {
    /// The progress the machine now holds.
    pub progress: u32,
    /// The cursor the machine now holds.
    pub cursor: u32,
    /// Whether a step completed and the cursor moved on.
    pub stepped: bool,
    /// What the completed step was charged.
    pub charged: u32,
}

/// What the harvesting machine found at the position under its cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Found {
    /// A mature crop whose seed resolves.
    Harvestable,
    /// A crop that is immature, or whose seed cannot be resolved.
    Crop,
    /// Anything that is not a crop at all.
    Nothing,
}

/// Advances the harvesting machine by a tick.
///
/// It charges nothing per tick and completes only when progress is strictly
/// greater than the operation time, charging at that point alone: the full
/// usage for a harvest, the scan cost for a step that found a crop it could not
/// take, and nothing at all for a step that found no crop.
///
/// # Errors
///
/// Answers with the rule that stopped the machine.
pub fn tick_harvester(
    energy: &mut Energy,
    held: Held,
    area: u32,
    timing: Timing,
    redstone: Redstone,
    found: Found,
) -> Result<Swept, Stalled> {
    if redstone == (Redstone::Gated { powered: true }) {
        return Err(Stalled::Redstone);
    }
    if energy.amount < u64::from(timing.usage) {
        return Err(Stalled::Unpowered);
    }
    let progress = held.progress + 1;
    if progress <= timing.operation {
        return Ok(Swept {
            progress,
            cursor: held.cursor,
            stepped: false,
            charged: 0,
        });
    }
    let charged = match found {
        Found::Harvestable => timing.usage,
        Found::Crop => SCAN_COST,
        Found::Nothing => 0,
    };
    energy.charge(charged);
    Ok(Swept {
        progress: 0,
        cursor: if area == 0 {
            0
        } else {
            (held.cursor + 1) % area
        },
        stepped: true,
        charged,
    })
}

/// Which way a machine faces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Facing {
    /// Toward decreasing depth.
    North,
    /// Toward increasing depth.
    South,
    /// Toward increasing width.
    East,
    /// Toward decreasing width.
    West,
}

impl Facing {
    /// The unit step one block forward.
    #[must_use]
    pub const fn step(self) -> (i32, i32) {
        match self {
            Self::North => (0, -1),
            Self::South => (0, 1),
            Self::East => (1, 0),
            Self::West => (-1, 0),
        }
    }

    /// Maps a local offset into world axes.
    #[must_use]
    pub const fn map(self, x: i32, z: i32) -> (i32, i32) {
        match self {
            Self::South => (x, z),
            Self::North => (-x, -z),
            Self::East => (-z, x),
            Self::West => (z, -x),
        }
    }
}

/// The square a harvesting machine sweeps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sweep {
    /// The machine's own position.
    pub origin: Position,
    /// Which way the machine faces.
    pub facing: Facing,
    /// The reach, one plus whatever the upgrade adds.
    pub range: i32,
}

impl Sweep {
    /// The reach a machine with this upgrade has.
    #[must_use]
    pub fn range(upgrade: Option<Upgrade>) -> i32 {
        1 + upgrade.map_or(0, |upgrade| i32::try_from(upgrade.range).unwrap_or(0))
    }

    /// The square's side.
    #[must_use]
    pub const fn side(&self) -> i32 {
        2 * self.range + 1
    }

    /// How many positions the cursor wraps at.
    #[must_use]
    pub const fn area(&self) -> u32 {
        (self.side() * self.side()).cast_unsigned()
    }

    /// The square's centre, the range plus one blocks in the facing direction.
    #[must_use]
    pub const fn centre(&self) -> Position {
        let (dx, dz) = self.facing.step();
        let forward = self.range + 1;
        (
            self.origin.0 + dx * forward,
            self.origin.1,
            self.origin.2 + dz * forward,
        )
    }

    /// The position one cursor index names.
    ///
    /// A single integer, decomposed into the square and then mapped into world
    /// axes by the facing. This is a wrapping scan rather than a search for the
    /// nearest mature crop, which is what makes throughput proportional to crop
    /// density rather than to where the crops are.
    #[must_use]
    pub fn at(&self, index: u32) -> Position {
        let side = self.side();
        let index = i32::try_from(index % self.area()).unwrap_or(0);
        let x = index % side - self.range;
        let z = index.div_euclid(side) - self.range;
        let (dx, dz) = self.facing.map(x, z);
        let centre = self.centre();
        (centre.0 + dx, centre.1, centre.2 + dz)
    }
}

/// Fills a machine's output slots with what a harvest produced.
///
/// The empty test is applied first at each slot, so the first empty slot wins
/// over a matching stack that sits later. The remainder is tracked into that
/// slot rather than the original count: writing the original count is what
/// makes the original an item duplicator, since the portion already merged into
/// an earlier slot is then created a second time.
///
/// Each slot is filled to its own limit rather than to one figure the caller
/// supplies, because the store is where a raised or lowered slot limit lives
/// and a write past it is not something the store itself refuses.
///
/// # Errors
///
/// Returns whatever the store refuses when a slot is out of range.
pub fn fill_outputs(
    store: &mut Store,
    from: usize,
    incoming: Stack,
) -> Result<Option<Stack>, StoreRefused> {
    let mut left = Some(incoming);
    for slot in from..store.len() {
        let Some(rest) = left.take() else {
            break;
        };
        let limit = store.limit(slot);
        if store.slot(slot).is_some_and(|held| held.count > limit) {
            left = Some(rest);
            continue;
        }
        let (merged, over) = if let Some(held) = store.slot(slot) {
            insert(held, &rest, limit)
        } else {
            let mut merged = rest;
            let over = if merged.count > limit {
                let mut over = merged.clone();
                over.count -= limit;
                merged.count = limit;
                Some(over)
            } else {
                None
            };
            (merged, over)
        };
        store.place(slot, Some(merged))?;
        left = over;
    }
    Ok(left)
}

/// The eight ring offsets around an altar, at its own height.
///
/// Distance three on the axes and two on the diagonals. The order is observable
/// because the ring stations are matched to a recipe's slots by position in
/// this list, so the two shipped sets are the same positions in two orders.
pub const RING_AXES_FIRST: [Position; 8] = [
    (3, 0, 0),
    (-3, 0, 0),
    (0, 0, 3),
    (0, 0, -3),
    (2, 0, 2),
    (2, 0, -2),
    (-2, 0, 2),
    (-2, 0, -2),
];

/// The same eight offsets, diagonals first.
pub const RING_DIAGONALS_FIRST: [Position; 8] = [
    (2, 0, 2),
    (2, 0, -2),
    (-2, 0, 2),
    (-2, 0, -2),
    (3, 0, 0),
    (-3, 0, 0),
    (0, 0, 3),
    (0, 0, -3),
];

/// An altar's progress and its activation latch.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Altar {
    /// The progress toward the craft.
    pub progress: u32,
    /// Whether the altar has been activated.
    pub active: bool,
}

impl Altar {
    /// Activates the altar, by redstone or by the activation tool.
    ///
    /// The latch holds until the craft completes or the input is removed.
    pub const fn activate(&mut self) {
        self.active = true;
    }

    /// Whether an item may be inserted into the altar's input.
    ///
    /// Refused while the output is occupied, which is what stops a craft
    /// starting on top of a result nobody took.
    #[must_use]
    pub const fn accepts_input(output_occupied: bool) -> bool {
        !output_occupied
    }

    /// Advances the altar by a tick, answering whether the craft completed.
    ///
    /// Progress rises by one while the altar is active and a recipe matches,
    /// and the craft completes at [`RITUAL_COMPLETION`]. Removing the input at
    /// any point resets both the progress and the latch.
    pub fn tick(&mut self, has_input: bool, matches: bool, covered: bool) -> bool {
        if !has_input {
            self.progress = 0;
            self.active = false;
            return false;
        }
        if !self.active || !matches || !covered {
            return false;
        }
        self.progress += 1;
        if self.progress >= RITUAL_COMPLETION {
            self.progress = 0;
            self.active = false;
            return true;
        }
        false
    }
}

/// Whether a summoning may place a creature of this kind at all.
#[must_use]
pub const fn crowded(nearby: usize) -> bool {
    nearby >= CROWD_LIMIT
}

/// The square a summoning draws a position from.
#[must_use]
pub const fn summoning_side() -> u32 {
    (2 * SUMMONING_REACH + 1).unsigned_abs()
}

/// Picks a position for a summoned creature.
///
/// The draw is over a half-open range, so no index folds to a position one row
/// beyond the square, and a position that is unobstructed on the final attempt
/// is accepted. Both differ from the original, which draws inclusively and
/// decrements its attempt counter before testing it.
pub fn summoning_position(
    origin: Position,
    draw: &mut dyn FnMut(u32) -> u32,
    unobstructed: &dyn Fn(Position) -> bool,
) -> Option<Position> {
    let side = summoning_side();
    let reach = SUMMONING_REACH;
    for _ in 0..SUMMONING_ATTEMPTS {
        let index = draw(side * side) % (side * side);
        let x = i32::try_from(index % side).unwrap_or(0) - reach;
        let z = i32::try_from(index.div_euclid(side)).unwrap_or(0) - reach;
        let position = (origin.0 + x, origin.1, origin.2 + z);
        if unobstructed(position) {
            return Some(position);
        }
    }
    None
}

/// A slotted modifier written onto a target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Slotted {
    /// The modifier's identifier.
    pub modifier: String,
    /// The slot it occupies on the target.
    pub slot: usize,
}

/// The target a fitting bench is writing on.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Target {
    /// The kinds the target declares.
    pub kinds: Vec<String>,
    /// The target's own tier value.
    pub tier: u32,
    /// How many modifier slots the target has.
    pub slots: usize,
}

/// Whether a modifier may be applied to a target.
///
/// It has to share at least one declared kind with the target and its own tier
/// has to be at most the target's; a slot at or beyond the target's slot count
/// is ignored.
#[must_use]
pub fn accepts_modifier(
    modifier: &crate::registries::Modifier,
    target: &Target,
    slot: usize,
) -> bool {
    slot < target.slots
        && modifier.tier <= target.tier
        && modifier
            .kinds
            .iter()
            .any(|kind| target.kinds.iter().any(|held| held == kind))
}

/// What one of the bench's two modifier slots offers on a content change.
///
/// A bench slot is not a target slot: the modifier it holds names the slot it
/// is written into, so an empty bench slot has to name the target slot it last
/// wrote or there is nothing to remove.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Offer {
    /// A modifier to write, and the target slot it goes in.
    Write(crate::registries::Modifier, usize),
    /// The bench slot went empty, removing what it last wrote.
    Clear(usize),
    /// The bench slot is empty and wrote nothing to remove.
    Idle,
}

/// What the fitting bench did to a target when its contents changed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Fitted {
    /// The modifiers now written onto the target.
    pub slotted: Vec<Slotted>,
    /// Whether the target's area offset was cleared.
    pub cleared_offset: bool,
}

/// Applies the bench's two modifier slots to a target.
///
/// The bench is not a craft: it writes on every content change. A slot holding
/// a modifier writes it into the target slot the modifier names if it differs
/// from what is there, and a slot that went empty removes what it wrote, which
/// also clears the target's area offset. A modifier the target does not accept
/// is ignored, so the acceptance rules gate the add path rather than sitting
/// beside it.
#[must_use]
pub fn fit(target: &Target, held: &[Slotted], offers: [Offer; 2]) -> Fitted {
    let mut fitted = Fitted {
        slotted: held.to_vec(),
        cleared_offset: false,
    };
    for offer in offers {
        match offer {
            Offer::Write(modifier, slot) => {
                if !accepts_modifier(&modifier, target, slot) {
                    continue;
                }
                let written = Slotted {
                    modifier: modifier.id.to_string(),
                    slot,
                };
                match fitted.slotted.iter().position(|held| held.slot == slot) {
                    Some(position) => {
                        if fitted.slotted[position] != written {
                            fitted.slotted[position] = written;
                        }
                    }
                    None => fitted.slotted.push(written),
                }
            }
            Offer::Clear(slot) => {
                if let Some(position) = fitted.slotted.iter().position(|held| held.slot == slot) {
                    fitted.slotted.remove(position);
                    fitted.cleared_offset = true;
                }
            }
            Offer::Idle => {}
        }
    }
    fitted.slotted.sort_by_key(|held| held.slot);
    fitted
}

/// What a powered machine writes into its saved payload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Persisted {
    /// The progress toward the current operation.
    pub progress: u32,
    /// The energy store.
    pub energy: Energy,
    /// The validated upgrade held.
    pub upgrade: Option<Upgrade>,
    /// The scan cursor, which only the harvesting machine keeps.
    pub cursor: Option<u32>,
}

impl Persisted {
    /// The payload this machine hands Ravel.
    #[must_use]
    pub fn save(&self, inventory: JsonValue) -> JsonValue {
        let mut payload = json!({
            "progress": self.progress,
            "energy": self.energy.save(),
            "upgrade": self.upgrade.map(Upgrade::position),
            "cursor": self.cursor,
        });
        payload["inventory"] = inventory;
        payload
    }

    /// Reads a payload, refusing malformed present fields.
    ///
    /// # Errors
    ///
    /// Refuses a non-object payload, wrongly typed or out-of-range field, or
    /// an upgrade position the declared table does not hold.
    pub fn load(payload: &JsonValue) -> Result<Self, SavedFieldError> {
        if !payload.is_object() {
            return Err(SavedFieldError("machine"));
        }
        let upgrade = saved_optional_u32(payload, "upgrade")?
            .map(|position| Upgrade::at(position).ok_or(SavedFieldError("upgrade")))
            .transpose()?;
        Ok(Self {
            progress: u32::try_from(saved_u64(payload, "progress", 0)?)
                .map_err(|_| SavedFieldError("progress"))?,
            energy: payload
                .get("energy")
                .map_or_else(|| Ok(Energy::default()), Energy::load)?,
            upgrade,
            cursor: saved_optional_u32(payload, "cursor")?,
        })
    }
}

/// A present saved field has the wrong kind or an unsupported value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SavedFieldError(pub &'static str);

impl std::fmt::Display for SavedFieldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the stored {} field is malformed", self.0)
    }
}

impl std::error::Error for SavedFieldError {}

fn saved_u64(
    payload: &JsonValue,
    field: &'static str,
    default: u64,
) -> Result<u64, SavedFieldError> {
    payload.get(field).map_or(Ok(default), |value| {
        value.as_u64().ok_or(SavedFieldError(field))
    })
}

fn saved_optional_u32(
    payload: &JsonValue,
    field: &'static str,
) -> Result<Option<u32>, SavedFieldError> {
    match payload.get(field) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or(SavedFieldError(field)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_derived_operation_time_truncates_toward_zero() {
        assert_eq!(UPGRADES[0].operation_ticks(200), 166);
        assert_eq!(UPGRADES[1].operation_ticks(200), 125);
        assert_eq!(UPGRADES[2].operation_ticks(200), 80);
        assert_eq!(UPGRADES[3].operation_ticks(100), 12);
        assert_eq!(UPGRADES[4].operation_ticks(100), 2);
        assert_eq!(UPGRADES[5].operation_ticks(100), 0);
    }

    #[test]
    fn floating_counts_truncate_and_saturate_at_unsigned_bounds() {
        for value in [f64::NAN, f64::NEG_INFINITY, -1.0, -0.0, 0.0, 0.999] {
            assert_eq!(
                Upgrade {
                    operation: value,
                    ..UPGRADES[0]
                }
                .operation_ticks(1),
                0
            );
        }
        let ticks = |operation| {
            Upgrade {
                operation,
                ..UPGRADES[0]
            }
            .operation_ticks(1)
        };
        assert_eq!(ticks(1.0), 1);
        assert_eq!(ticks(1.999), 1);
        assert_eq!(ticks(4_294_967_294.75), u32::MAX - 1);
        for value in [f64::from(u32::MAX), 4_294_967_296.0, f64::INFINITY] {
            assert_eq!(ticks(value), u32::MAX);
        }
    }

    #[test]
    fn the_usage_and_capacity_multipliers_are_the_ones_the_table_states() {
        assert_eq!(UPGRADES[0].usage_per_tick(20), 40);
        assert_eq!(UPGRADES[5].usage_per_tick(40), 3840);
        assert_eq!(UPGRADES[0].capacity(), 160_000);
        assert_eq!(UPGRADES[5].capacity(), 560_000);
        assert_eq!(UPGRADES.map(|upgrade| upgrade.range), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn the_sixth_upgrade_borrows_the_fifth_tiers_value_and_is_still_accepted() {
        assert_eq!(UPGRADES[4].tier_value, UPGRADES[5].tier_value);
        assert!(
            accepts(Some(UPGRADES[4]), UPGRADES[5]),
            "ordering by position rather than by the borrowed value"
        );
        assert!(!accepts(Some(UPGRADES[5]), UPGRADES[4]));
        assert!(!accepts(Some(UPGRADES[2]), UPGRADES[2]));
        assert!(accepts(None, UPGRADES[5]));
    }

    #[test]
    fn a_burn_starts_only_with_room_and_nothing_already_burning() {
        let mut energy = Energy::default();
        assert!(energy.light(Some(100)));
        assert_eq!(energy.remaining, 2000);
        assert_eq!(energy.total, 2000);
        assert!(!energy.light(Some(100)), "a burn is already in progress");

        let mut full = Energy {
            amount: BASE_CAPACITY,
            ..Energy::default()
        };
        assert!(!full.light(Some(100)));
        assert!(!Energy::default().light(None));
    }

    #[test]
    fn a_tick_of_the_burn_is_capped_by_twice_the_usage_and_by_the_headroom() {
        let mut energy = Energy::default();
        energy.light(Some(100));
        assert_eq!(energy.advance_burn(20), 40);
        assert_eq!(energy.amount, 40);
        assert_eq!(energy.remaining, 1960);

        let mut nearly = Energy {
            amount: BASE_CAPACITY - 5,
            remaining: 1000,
            total: 1000,
            ..Energy::default()
        };
        assert_eq!(nearly.advance_burn(20), 5, "the headroom caps the move");
        assert_eq!(nearly.remaining, 995, "what did not fit stays in the burn");
    }

    #[test]
    fn the_recorded_total_is_cleared_when_the_burn_reaches_zero() {
        let mut energy = Energy {
            remaining: 10,
            total: 2000,
            ..Energy::default()
        };
        assert_eq!(energy.advance_burn(20), 10);
        assert_eq!(energy.remaining, 0);
        assert_eq!(energy.total, 0);
        assert_eq!(energy.advance_burn(20), 0);
    }

    #[test]
    fn retuning_the_capacity_does_not_clamp_the_energy_already_stored() {
        let mut energy = Energy {
            amount: 500_000,
            ..Energy::default()
        };
        energy.retune(Some(UPGRADES[5]));
        assert_eq!(energy.capacity, 560_000);
        energy.retune(None);
        assert_eq!(energy.capacity, BASE_CAPACITY);
        assert_eq!(energy.amount, 500_000, "the stored amount is left alone");
    }

    #[test]
    fn a_recipe_machine_charges_every_tick_and_completes_at_the_operation_time() {
        let mut energy = Energy {
            amount: 1000,
            ..Energy::default()
        };
        let step = RecipeStep {
            progress: 0,
            operation: 3,
            usage: 20,
        };
        let ticked = tick_recipe_machine(&mut energy, step, true, Redstone::Ignored)
            .expect("a powered machine with input advances");
        assert_eq!(ticked.progress, 1);
        assert!(!ticked.completed);
        assert_eq!(energy.amount, 980);

        let done = tick_recipe_machine(
            &mut energy,
            RecipeStep {
                progress: 2,
                ..step
            },
            true,
            Redstone::Ignored,
        )
        .expect("the third tick completes");
        assert!(done.completed);
        assert_eq!(done.progress, 0);
    }

    #[test]
    fn stalled_errors_explain_each_refusal_without_an_underlying_cause() {
        for (refusal, message) in [
            (
                Stalled::Redstone,
                "a neighbouring redstone signal disables the machine",
            ),
            (
                Stalled::Unpowered,
                "the machine stores less energy than one tick requires",
            ),
        ] {
            let error: &dyn std::error::Error = &refusal;
            assert_eq!(error.to_string(), message);
            assert!(error.source().is_none());
        }
    }

    #[test]
    fn losing_power_stalls_a_recipe_machine_and_losing_the_input_is_its_own_answer() {
        let mut flat = Energy::default();
        let step = RecipeStep {
            progress: 5,
            operation: 100,
            usage: 20,
        };
        assert_eq!(
            tick_recipe_machine(&mut flat, step, true, Redstone::Ignored),
            Err(Stalled::Unpowered)
        );
        let mut energy = Energy {
            amount: 1000,
            ..Energy::default()
        };
        let lost = tick_recipe_machine(&mut energy, step, false, Redstone::Ignored)
            .expect("losing the input is not a stall");
        assert_eq!(lost.progress, 0, "losing the input resets progress");
        assert!(lost.reset);
        assert!(!lost.completed);
        assert_eq!(energy.amount, 1000, "and charges nothing");
        assert_eq!(
            tick_recipe_machine(&mut energy, step, true, Redstone::Gated { powered: true }),
            Err(Stalled::Redstone)
        );
        assert!(
            tick_recipe_machine(&mut energy, step, true, Redstone::Ignored).is_ok(),
            "three of the five machines ignore redstone entirely"
        );
        assert!(
            tick_recipe_machine(&mut energy, step, true, Redstone::Gated { powered: false })
                .is_ok()
        );
    }

    #[test]
    fn the_harvester_charges_at_completion_alone_and_completes_strictly_beyond() {
        let mut energy = Energy {
            amount: 1000,
            ..Energy::default()
        };
        let held = tick_harvester(
            &mut energy,
            Held {
                progress: 0,
                cursor: 0,
            },
            9,
            Timing {
                operation: 1,
                usage: 40,
            },
            Redstone::Gated { powered: false },
            Found::Harvestable,
        )
        .expect("a powered harvester advances");
        assert!(!held.stepped);
        assert_eq!(energy.amount, 1000, "nothing is charged per tick");

        let done = tick_harvester(
            &mut energy,
            Held {
                progress: 1,
                cursor: 0,
            },
            9,
            Timing {
                operation: 1,
                usage: 40,
            },
            Redstone::Gated { powered: false },
            Found::Harvestable,
        )
        .expect("progress strictly greater completes");
        assert!(done.stepped);
        assert_eq!(done.charged, 40);
        assert_eq!(done.cursor, 1);
        assert_eq!(energy.amount, 960);
    }

    #[test]
    fn a_step_that_finds_a_crop_it_cannot_take_costs_the_scan_and_nothing_costs_nothing() {
        let mut energy = Energy {
            amount: 1000,
            ..Energy::default()
        };
        let scanned = tick_harvester(
            &mut energy,
            Held {
                progress: 1,
                cursor: 3,
            },
            9,
            Timing {
                operation: 1,
                usage: 40,
            },
            Redstone::Gated { powered: false },
            Found::Crop,
        )
        .expect("the step completes");
        assert_eq!(scanned.charged, SCAN_COST);
        assert_eq!(energy.amount, 990);

        let free = tick_harvester(
            &mut energy,
            Held {
                progress: 1,
                cursor: 8,
            },
            9,
            Timing {
                operation: 1,
                usage: 40,
            },
            Redstone::Gated { powered: false },
            Found::Nothing,
        )
        .expect("the step completes");
        assert_eq!(free.charged, 0);
        assert_eq!(free.cursor, 0, "the cursor wraps at the square's area");
    }

    #[test]
    fn a_redstone_signal_stops_the_harvester_before_it_scans_at_all() {
        let mut energy = Energy {
            amount: 1000,
            ..Energy::default()
        };
        assert_eq!(
            tick_harvester(
                &mut energy,
                Held {
                    progress: 0,
                    cursor: 0
                },
                9,
                Timing {
                    operation: 1,
                    usage: 40
                },
                Redstone::Gated { powered: true },
                Found::Harvestable
            ),
            Err(Stalled::Redstone)
        );
        assert_eq!(energy.amount, 1000);
        let ungated = tick_harvester(
            &mut energy,
            Held {
                progress: 1,
                cursor: 0,
            },
            9,
            Timing {
                operation: 1,
                usage: 40,
            },
            Redstone::Ignored,
            Found::Harvestable,
        )
        .expect("an ungated harvester may scan");
        assert!(ungated.stepped);
        assert_eq!(ungated.charged, 40);
    }

    #[test]
    fn the_sweep_is_a_square_of_three_with_no_upgrade_and_fifteen_with_the_sixth() {
        assert_eq!(Sweep::range(None), 1);
        assert_eq!(Sweep::range(Some(UPGRADES[5])), 7);
        let bare = Sweep {
            origin: (0, 64, 0),
            facing: Facing::South,
            range: 1,
        };
        assert_eq!(bare.side(), 3);
        assert_eq!(bare.area(), 9);
        assert_eq!(bare.centre(), (0, 64, 2));
    }

    #[test]
    fn the_cursor_decomposes_into_the_square_and_covers_it_exactly_once() {
        let sweep = Sweep {
            origin: (0, 64, 0),
            facing: Facing::South,
            range: 1,
        };
        assert_eq!(sweep.at(0), (-1, 64, 1));
        assert_eq!(sweep.at(4), (0, 64, 2), "the middle index is the centre");
        assert_eq!(sweep.at(8), (1, 64, 3));
        assert_eq!(sweep.at(9), sweep.at(0), "the cursor wraps");

        let mut seen: Vec<Position> = (0..sweep.area()).map(|index| sweep.at(index)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn the_facing_moves_the_square_and_never_changes_its_size() {
        for facing in [Facing::North, Facing::South, Facing::East, Facing::West] {
            let sweep = Sweep {
                origin: (0, 64, 0),
                facing,
                range: 2,
            };
            let mut seen: Vec<Position> = (0..sweep.area()).map(|index| sweep.at(index)).collect();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), 25);
            assert!(seen.iter().all(|position| position.1 == 64));
        }
        let east = Sweep {
            origin: (0, 64, 0),
            facing: Facing::East,
            range: 1,
        };
        assert_eq!(east.centre(), (2, 64, 0));
    }

    #[test]
    fn an_empty_output_slot_preserves_metadata_when_splitting_a_stack() {
        let mut store = Store::new(1, 4).expect("a legal store");
        let mut incoming = Stack::new("test:produce".parse().expect("legal item identity"), 7);
        incoming.data = json!({"quality": 3});
        let remainder = fill_outputs(&mut store, 0, incoming.clone())
            .expect("the slot accepts output")
            .expect("three items remain");
        let placed = store.slot(0).expect("four items were placed");
        assert_eq!(placed.count, 4);
        assert_eq!(remainder.count, 3);
        assert_eq!(placed.entry, incoming.entry);
        assert_eq!(remainder.entry, incoming.entry);
        assert_eq!(placed.data, incoming.data);
        assert_eq!(remainder.data, incoming.data);
    }

    #[test]
    fn the_output_fill_takes_the_first_empty_slot_over_a_later_matching_stack() {
        let mut store = Store::new(4, 64).expect("a legal store");
        store
            .place(
                2,
                Some(Stack::new(
                    "test:produce".parse().expect("legal item identity"),
                    10,
                )),
            )
            .expect("a legal slot");
        let over = fill_outputs(
            &mut store,
            1,
            Stack::new("test:produce".parse().expect("legal item identity"), 5),
        )
        .expect("the store accepts");
        assert!(over.is_none());
        assert_eq!(
            store.slot(1),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                5
            ))
        );
        assert_eq!(
            store.slot(2),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                10
            )),
            "the later matching stack is untouched"
        );
    }

    #[test]
    fn a_partial_merge_carries_its_remainder_rather_than_creating_it_twice() {
        let mut store = Store::new(4, 64).expect("a legal store");
        store
            .place(
                1,
                Some(Stack::new(
                    "test:produce".parse().expect("legal item identity"),
                    60,
                )),
            )
            .expect("a legal slot");
        let over = fill_outputs(
            &mut store,
            1,
            Stack::new("test:produce".parse().expect("legal item identity"), 10),
        )
        .expect("the store accepts");
        assert!(over.is_none());
        assert_eq!(
            store.slot(1),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                64
            ))
        );
        assert_eq!(
            store.slot(2),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                6
            )),
            "six is what was left, not the original ten"
        );
    }

    #[test]
    fn what_still_does_not_fit_comes_back_to_be_dropped_on_the_ground() {
        let mut store = Store::new(2, 64).expect("a legal store");
        store
            .place(
                1,
                Some(Stack::new(
                    "test:produce".parse().expect("legal item identity"),
                    64,
                )),
            )
            .expect("a legal slot");
        let over = fill_outputs(
            &mut store,
            1,
            Stack::new("test:produce".parse().expect("legal item identity"), 5),
        )
        .expect("the store accepts");
        assert_eq!(
            over,
            Some(Stack::new(
                "test:produce".parse().expect("legal item identity"),
                5
            ))
        );
    }

    #[test]
    fn the_two_ring_orders_name_the_same_eight_positions() {
        let mut first = RING_AXES_FIRST.to_vec();
        let mut second = RING_DIAGONALS_FIRST.to_vec();
        assert_ne!(first, second, "the order is observable");
        first.sort_unstable();
        second.sort_unstable();
        assert_eq!(first, second);
        assert!(RING_AXES_FIRST.iter().all(|offset| offset.1 == 0));
    }

    #[test]
    fn an_altar_is_inert_until_it_is_activated_and_completes_at_a_hundred() {
        let mut altar = Altar::default();
        assert!(!altar.tick(true, true, true));
        assert_eq!(altar.progress, 0, "an inactive altar does not advance");

        altar.activate();
        for _ in 0..(RITUAL_COMPLETION - 1) {
            assert!(!altar.tick(true, true, true));
        }
        assert_eq!(altar.progress, RITUAL_COMPLETION - 1);
        assert!(altar.tick(true, true, true));
        assert_eq!(altar.progress, 0);
        assert!(!altar.active, "the latch clears on completion");
    }

    #[test]
    fn removing_the_input_resets_the_progress_and_the_latch() {
        let mut altar = Altar::default();
        altar.activate();
        altar.tick(true, true, true);
        altar.tick(true, true, true);
        assert_eq!(altar.progress, 2);
        assert!(!altar.tick(false, true, true));
        assert_eq!(altar.progress, 0);
        assert!(!altar.active);
    }

    #[test]
    fn uncovered_reservoirs_stop_the_eight_station_ritual_advancing_at_all() {
        let mut altar = Altar::default();
        altar.activate();
        assert!(!altar.tick(true, true, false));
        assert_eq!(altar.progress, 0);
        assert!(altar.active, "the latch survives an uncovered tick");
    }

    #[test]
    fn an_altar_refuses_an_insertion_while_its_output_is_occupied() {
        assert!(Altar::accepts_input(false));
        assert!(!Altar::accepts_input(true));
    }

    #[test]
    fn thirty_two_creatures_of_a_kind_refuse_a_summoning_and_thirty_one_do_not() {
        assert!(!crowded(31));
        assert!(crowded(32));
        assert!(crowded(100));
    }

    #[test]
    fn a_summoning_draws_over_the_square_and_accepts_the_final_attempt() {
        let side = summoning_side();
        assert_eq!(side, 7);

        let mut count = 0;
        let mut draw = |bound: u32| {
            assert_eq!(bound, side * side);
            count += 1;
            0
        };
        let position = summoning_position((0, 64, 0), &mut draw, &|_| true);
        assert_eq!(position, Some((-3, 64, -3)));

        let mut attempt = 0;
        let mut counting = |_| {
            attempt += 1;
            attempt - 1
        };
        let last_only = |position: Position| position == (2, 64, -1);
        assert_eq!(
            summoning_position((0, 64, 0), &mut counting, &last_only),
            Some((2, 64, -1)),
            "the twentieth attempt is not thrown away"
        );
    }

    #[test]
    fn a_summoning_that_never_finds_a_place_answers_with_nothing() {
        let mut draw = |_| 0;
        assert_eq!(summoning_position((0, 64, 0), &mut draw, &|_| false), None);
    }

    #[test]
    fn every_drawn_position_lands_inside_the_square() {
        let reach = SUMMONING_REACH;
        for index in 0..(summoning_side() * summoning_side()) {
            let mut draw = |_| index;
            let position = summoning_position((0, 64, 0), &mut draw, &|_| true)
                .expect("every index names a place");
            assert!(position.0.abs() <= reach && position.2.abs() <= reach);
        }
    }

    fn target(slots: usize) -> Target {
        Target {
            kinds: vec!["tool".to_owned()],
            tier: 2,
            slots,
        }
    }

    #[test]
    fn a_modifier_needs_a_shared_kind_a_low_enough_tier_and_a_slot_that_exists() {
        let modifier = crate::registries::Modifier::new(
            "a:m".parse().expect("a legal modifier identity"),
            2,
            &["tool"],
        );
        assert!(accepts_modifier(&modifier, &target(3), 0));
        assert!(
            !accepts_modifier(
                &modifier,
                &Target {
                    tier: 1,
                    ..target(3)
                },
                0
            ),
            "its tier is above"
        );
        assert!(!accepts_modifier(
            &modifier,
            &Target {
                kinds: vec!["armour".to_owned()],
                tier: 5,
                slots: 3,
            },
            0
        ));
        assert!(
            !accepts_modifier(&modifier, &target(3), 3),
            "the slot is beyond"
        );
    }

    #[test]
    fn the_bench_writes_on_a_change_and_an_empty_slot_removes_and_clears_the_offset() {
        let modifier = crate::registries::Modifier::new(
            "a:one".parse().expect("a legal modifier identity"),
            2,
            &["tool"],
        );
        let written = fit(&target(4), &[], [Offer::Write(modifier, 0), Offer::Idle]);
        assert_eq!(written.slotted.len(), 1);
        assert!(!written.cleared_offset);

        let removed = fit(&target(4), &written.slotted, [Offer::Clear(0), Offer::Idle]);
        assert_eq!(removed.slotted.len(), 0);
        assert!(removed.cleared_offset);
    }

    #[test]
    fn a_modifier_written_above_the_second_target_slot_is_the_one_removed() {
        let first = crate::registries::Modifier::new(
            "a:one".parse().expect("a legal modifier identity"),
            2,
            &["tool"],
        );
        let second = crate::registries::Modifier::new(
            "a:two".parse().expect("a legal modifier identity"),
            2,
            &["tool"],
        );
        let written = fit(
            &target(4),
            &[],
            [Offer::Write(first, 2), Offer::Write(second, 3)],
        );
        assert_eq!(
            written.slotted,
            vec![
                Slotted {
                    modifier: "a:one".to_owned(),
                    slot: 2,
                },
                Slotted {
                    modifier: "a:two".to_owned(),
                    slot: 3,
                },
            ],
            "each modifier is keyed by the target slot it names, not the bench slot"
        );

        let removed = fit(&target(4), &written.slotted, [Offer::Clear(2), Offer::Idle]);
        assert_eq!(
            removed.slotted,
            vec![Slotted {
                modifier: "a:two".to_owned(),
                slot: 3,
            }],
            "the modifier at target slot two comes back out"
        );
        assert!(removed.cleared_offset);
    }

    #[test]
    fn the_bench_refuses_a_modifier_the_target_does_not_accept() {
        let over_tier = crate::registries::Modifier::new(
            "a:high".parse().expect("a legal modifier identity"),
            5,
            &["tool"],
        );
        let wrong_kind = crate::registries::Modifier::new(
            "a:armour".parse().expect("a legal modifier identity"),
            1,
            &["armour"],
        );
        let beyond = crate::registries::Modifier::new(
            "a:far".parse().expect("a legal modifier identity"),
            1,
            &["tool"],
        );
        let fitted = fit(
            &target(1),
            &[],
            [Offer::Write(over_tier, 0), Offer::Write(wrong_kind, 0)],
        );
        assert_eq!(fitted.slotted.len(), 0);
        assert_eq!(
            fit(&target(1), &[], [Offer::Write(beyond, 1), Offer::Idle])
                .slotted
                .len(),
            0
        );
    }

    #[test]
    fn a_slot_holding_the_same_modifier_is_left_alone() {
        let held = vec![Slotted {
            modifier: "a:one".to_owned(),
            slot: 0,
        }];
        let modifier = crate::registries::Modifier::new(
            "a:one".parse().expect("a legal modifier identity"),
            2,
            &["tool"],
        );
        let same = fit(&target(4), &held, [Offer::Write(modifier, 0), Offer::Idle]);
        assert_eq!(same.slotted, held);
        assert!(!same.cleared_offset);
    }

    #[test]
    fn a_harvesting_machine_brings_its_cursor_back_across_a_reload() {
        let held = Persisted {
            progress: 42,
            energy: Energy {
                amount: 1234,
                capacity: 160_000,
                remaining: 20,
                total: 2000,
            },
            upgrade: Some(UPGRADES[0]),
            cursor: Some(37),
        };
        let payload = held.save(json!({"slots": 16}));
        let read = Persisted::load(&payload).expect("valid saved fields");
        assert_eq!(read, held, "the sweep resumes where it left off");
        assert_eq!(payload.get("inventory"), Some(&json!({"slots": 16})));
    }

    #[test]
    fn a_saved_upgrade_position_brings_back_every_multiplier_it_names() {
        let held = Persisted {
            progress: 3,
            energy: Energy {
                amount: 500,
                capacity: UPGRADES[5].capacity(),
                remaining: 0,
                total: 0,
            },
            upgrade: Some(UPGRADES[5]),
            cursor: Some(9),
        };
        let payload = held.save(json!({"slots": 16}));
        let read = Persisted::load(&payload).expect("valid saved fields");
        let upgrade = read.upgrade.expect("the machine holds an upgrade");
        assert_eq!(upgrade.operation_ticks(HARVESTING.operation), 0);
        assert_eq!(upgrade.usage_per_tick(HARVESTING.usage), 3840);
        assert_eq!(upgrade.capacity(), 560_000);
        assert_eq!(Sweep::range(Some(upgrade)), 7);
        assert_eq!(read, held);
    }

    #[test]
    fn malformed_present_fields_are_refused() {
        for payload in [
            json!({"upgrade": 0}),
            json!({"upgrade": 7}),
            json!({"upgrade": "1"}),
            json!({"progress": -1}),
            json!({"progress": 4_294_967_296_u64}),
            json!({"cursor": false}),
            json!({"energy": null}),
            json!({"energy": {"amount": "100"}}),
            json!({"energy": {"capacity": -1}}),
            json!({"energy": {"remaining": false}}),
            json!({"energy": {"total": null}}),
        ] {
            assert!(Persisted::load(&payload).is_err(), "{payload}");
        }
        assert_eq!(
            Persisted::load(&json!({"upgrade": 6}))
                .expect("known upgrade")
                .upgrade,
            Some(UPGRADES[5])
        );
    }

    #[test]
    fn energy_fields_default_only_when_absent() {
        assert_eq!(Energy::load(&json!({})), Ok(Energy::default()));
        assert_eq!(
            Energy::load(&json!({"amount": 42})),
            Ok(Energy {
                amount: 42,
                ..Energy::default()
            })
        );
        for payload in [json!(null), json!([]), json!(42)] {
            assert!(Energy::load(&payload).is_err());
        }
    }

    #[test]
    fn an_output_slot_is_filled_to_its_own_limit_and_never_past_it() {
        let mut store = Store::new(4, 64).expect("a legal store");
        store.set_limit(1, 8).expect("a legal slot");
        store.set_limit(2, 16).expect("a legal slot");
        let over = fill_outputs(
            &mut store,
            1,
            Stack::new("test:produce".parse().expect("legal item identity"), 40),
        )
        .expect("the store accepts");
        assert_eq!(
            store.slot(1),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                8
            ))
        );
        assert_eq!(
            store.slot(2),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                16
            ))
        );
        assert_eq!(
            store.slot(3),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                16
            ))
        );
        assert_eq!(over, None);
        assert!(
            (1..store.len()).all(|slot| store
                .slot(slot)
                .is_none_or(|held| held.count <= store.limit(slot))),
            "no slot is written past what it holds"
        );
    }

    #[test]
    fn a_slot_already_over_its_limit_is_stepped_over_rather_than_grown() {
        let mut store = Store::new(3, 64).expect("a legal store");
        store
            .place(
                1,
                Some(Stack::new(
                    "test:produce".parse().expect("legal item identity"),
                    90,
                )),
            )
            .expect("a legal slot");
        store.set_limit(1, 8).expect("a legal slot");
        let over = fill_outputs(
            &mut store,
            1,
            Stack::new("test:produce".parse().expect("legal item identity"), 4),
        )
        .expect("the store accepts");
        assert_eq!(
            store.slot(1),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                90
            )),
            "the over-limit slot is left exactly as it was"
        );
        assert_eq!(
            store.slot(2),
            Some(&Stack::new(
                "test:produce".parse().expect("legal item identity"),
                4
            ))
        );
        assert_eq!(over, None);
    }

    #[test]
    fn a_machine_that_keeps_no_cursor_reads_none_back() {
        let held = Persisted {
            progress: 7,
            energy: Energy::default(),
            upgrade: None,
            cursor: None,
        };
        let read = Persisted::load(&held.save(JsonValue::Null)).expect("valid saved fields");
        assert_eq!(read.cursor, None);
        assert_eq!(read.upgrade, None);
        assert_eq!(read.progress, 7);
    }

    #[test]
    fn an_empty_payload_reads_back_as_a_shipped_machine() {
        let read = Persisted::load(&json!({})).expect("absent fields default");
        assert_eq!(read, Persisted::default());
        assert_eq!(read.energy.capacity, BASE_CAPACITY);
    }
}
