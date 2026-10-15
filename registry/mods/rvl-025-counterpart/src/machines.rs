//! The upgrading machine of §7: two slots, a five-tick cycle, and one energy
//! payment made at the start of it.
//!
//! This is the parity-testable core of the entry. The arithmetic is written the
//! way §7 states it — single precision, truncated toward zero — because a
//! counterpart that computed the discount in double precision or rounded to
//! nearest would disagree with the original at some inputs, and the boundary
//! values and one mid-range factor are held to assertions below.
//!
//! §7 cites the shared library for what the infusion factor is, and the cited
//! section defines an owner, an identity, a security channel and a redstone mode
//! and no factor at all. What RVL-025 does state is §3's ceiling: how many
//! completed steps count as fully infused. [`factor`] is read out of that — the
//! machine's own completed steps over §3's ceiling, clamped into zero to one —
//! rather than invented, and `NOTES.md` records it as a reading of §3 with the
//! citation it replaces.

use num_traits::AsPrimitive;

use crate::data::Stack;
use crate::settings::Settings;

/// How many ticks one unit of material takes to become one upgrade step.
pub const TICKS_PER_UNIT: u8 = 5;

/// The most material slot zero holds.
pub const MATERIAL_LIMIT: u8 = 64;

/// The most upgradable items slot one holds.
pub const SUBJECT_LIMIT: u8 = 1;

/// The tag an item must carry to be accepted as the upgrade material.
pub const MATERIAL_TAG: &str = "rvl_025_counterpart:upgrade_material";

/// The four capabilities the machine exposes.
///
/// §7 names them and Ravel publishes no capability surface for a block, so this
/// is the declaration rather than the wiring: what the machine offers, in the
/// order §7 lists it.
pub const CAPABILITIES: [&str; 4] = [
    "item_handler",
    "energy_buffer",
    "menu_provider",
    "infusion_level",
];

/// The four values a broken machine keeps, copied into the dropped item.
///
/// §5 states the loot rule exactly because it is a cross-entry contract: two of
/// these belong to the shared library and one belongs here, so the identifier
/// vocabulary is not an implementation detail of either side.
pub const DROPPED_COMPONENTS: [Kept; 4] = [
    Kept {
        identifier: SHARED_STORED_ENERGY,
        owner: Vocabulary::SharedLibrary,
    },
    Kept {
        identifier: SHARED_INVENTORY,
        owner: Vocabulary::SharedLibrary,
    },
    Kept {
        identifier: "infusion_progress",
        owner: Vocabulary::ThisEntry,
    },
    Kept {
        identifier: "infusion_level",
        owner: Vocabulary::Unstated,
    },
];

/// The shared library's name for the energy a block holds.
pub const SHARED_STORED_ENERGY: &str = "stored_energy";

/// The shared library's name for the items a block holds.
pub const SHARED_INVENTORY: &str = "inventory";

/// Which side of the cross-entry contract an identifier belongs to.
///
/// §5 attributes two of the four to the shared library and one to this entry,
/// and says nothing about the fourth. [`Vocabulary::Unstated`] is that silence
/// carried rather than resolved: guessing an owner for the fourth would be
/// writing a cross-entry contract this specification does not state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Vocabulary {
    /// RVL-120's, and named out of RVL-120 rather than respelled here.
    SharedLibrary,
    /// This entry's own, and one of §4's five records.
    ThisEntry,
    /// §5 attributes it to neither side.
    Unstated,
}

/// One component a broken machine keeps, and whose vocabulary names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Kept {
    /// The identifier the loot rule copies.
    pub identifier: &'static str,
    /// Which side of the contract owns that identifier.
    pub owner: Vocabulary,
}

/// Resolves a shared-library identifier out of the vocabulary that owns it.
///
/// §5 calls the identifier vocabulary a cross-entry contract rather than an
/// implementation detail of either side, so a name the shared library owns is
/// looked up in what that library publishes instead of being respelled here. A
/// name the library no longer publishes answers with nothing, which is the
/// contract breaking loudly rather than this package carrying a dead spelling.
#[must_use]
pub fn shared_component(identifier: &str) -> Option<&'static str> {
    rvl_120_counterpart::census::COMPONENTS
        .into_iter()
        .find(|published| *published == identifier)
}

/// Returns the machine's own infusion factor, clamped into zero to one.
///
/// Zero completed steps is a factor of zero and a machine at §3's ceiling is a
/// factor of one. A ceiling of zero or less would be a division by nothing, so
/// it is read as one step, which is also the lowest value the setting's declared
/// range admits.
#[must_use]
pub fn factor(level: u32, ceiling: i32) -> f32 {
    let ceiling = u32::try_from(ceiling.max(1)).unwrap_or(1);
    if level >= ceiling {
        return 1.0;
    }
    // Both are below 2^32 and f32 rounds each once, which is the same single
    // rounding the original's own conversion performs.
    let level: f32 = level.as_();
    let ceiling: f32 = ceiling.as_();
    level / ceiling
}

/// Returns what one tick of infusing costs at this factor.
///
/// The configured per-tick figure multiplied by `(2 - f) / 2`, in single
/// precision, truncated toward zero. At a factor of zero the price is the
/// configured figure; at a factor of one it is half of it.
#[must_use]
pub fn price(configured: i32, factor: f32) -> i32 {
    let configured_float: f32 = configured.as_();
    let scaled = configured_float * (2.0 - factor) / 2.0;
    if !scaled.is_finite() {
        return configured;
    }
    let truncated = scaled.trunc();
    if truncated <= 0.0 {
        return 0;
    }
    let maximum: f32 = i32::MAX.as_();
    if truncated >= maximum {
        return i32::MAX;
    }
    truncated.as_()
}

/// What one tick of the machine did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ticked {
    /// Nothing: either the slots do not permit a start or the buffer is short.
    Idle,
    /// A cycle started, paying `price` and consuming one unit of material.
    Started {
        /// What was deducted from the buffer.
        price: i32,
    },
    /// A cycle advanced, with `remaining` ticks still to run.
    Running {
        /// How many ticks are left.
        remaining: u8,
    },
    /// A cycle finished and raised the item in slot one by one step.
    Completed,
    /// A cycle finished with nothing in slot one to raise.
    ///
    /// The material was spent when the cycle started, so this is the case §7
    /// warns about: the output is re-read at completion rather than remembered,
    /// and an item extracted mid-cycle silently loses the payment.
    CompletedIntoNothing,
}

/// The machine, as far as its decision rules go.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Machine {
    material: Option<Stack>,
    subject: Option<Stack>,
    energy: i32,
    progress: u8,
    infusion: u32,
}

impl Machine {
    /// Builds an empty machine with an empty buffer, itself uninfused.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a machine that has itself been infused `level` times.
    ///
    /// This is the value §7 discounts on: the block entity's own record, not the
    /// record of whatever is in slot one.
    #[must_use]
    pub fn infused(level: u32) -> Self {
        Self {
            infusion: level,
            ..Self::default()
        }
    }

    /// Returns the machine's own infusion level.
    ///
    /// One of the four capabilities §7 says the machine exposes, and one of the
    /// four components §5's loot rule copies into the dropped item.
    #[must_use]
    pub const fn infusion(&self) -> u32 {
        self.infusion
    }

    /// Sets the machine's own infusion level, as reading its record does.
    pub const fn set_infusion(&mut self, level: u32) {
        self.infusion = level;
    }

    /// Returns what slot zero holds.
    #[must_use]
    pub fn material(&self) -> Option<&Stack> {
        self.material.as_ref()
    }

    /// Returns what slot one holds.
    #[must_use]
    pub fn subject(&self) -> Option<&Stack> {
        self.subject.as_ref()
    }

    /// Returns how much the buffer holds.
    #[must_use]
    pub const fn energy(&self) -> i32 {
        self.energy
    }

    /// Returns how many ticks of the current cycle are left.
    #[must_use]
    pub const fn progress(&self) -> u8 {
        self.progress
    }

    /// Adds energy, saturating at the configured capacity and taking at most the
    /// configured accept rate in one tick.
    pub fn accept_energy(&mut self, offered: i32, settings: &Settings) -> i32 {
        let room = settings.energy_capacity.saturating_sub(self.energy).max(0);
        let taken = offered.max(0).min(settings.energy_accepted).min(room);
        self.energy = self.energy.saturating_add(taken);
        taken
    }

    /// Answers whether slot zero would take this stack.
    ///
    /// The test is the tag alone, and it is the same test the automation-facing
    /// handler applies, so a hopper cannot put in what the menu refuses.
    #[must_use]
    pub fn accepts_material(stack: &Stack) -> bool {
        !stack.is_empty()
            && stack
                .components
                .get(crate::matching::TAG_COMPONENT)
                .is_some_and(|carried| carried.split(',').any(|tag| tag == MATERIAL_TAG))
    }

    /// Answers whether slot one would take this stack.
    ///
    /// The ceiling is re-read on every call, which is §7's rule: lowering it
    /// immediately makes an already-infused item unacceptable.
    #[must_use]
    pub fn accepts_subject(stack: &Stack, settings: &Settings) -> bool {
        !stack.is_empty()
            && stack.upgradable
            && u64::from(stack.infusion)
                < u64::try_from(settings.upgrade_ceiling.max(1)).unwrap_or(1)
    }

    /// Puts material into slot zero, up to the slot's own limit.
    ///
    /// # Errors
    ///
    /// Returns [`MachineRefused::Rejected`] for a stack the slot's insertion
    /// test refuses and [`MachineRefused::Full`] for a count above the limit.
    pub fn insert_material(&mut self, stack: Stack) -> Result<(), MachineRefused> {
        if !Self::accepts_material(&stack) {
            return Err(MachineRefused::Rejected { slot: 0 });
        }
        if stack.count > MATERIAL_LIMIT {
            return Err(MachineRefused::Full {
                slot: 0,
                limit: MATERIAL_LIMIT,
            });
        }
        self.material = Some(stack);
        Ok(())
    }

    /// Puts an upgradable item into slot one.
    ///
    /// # Errors
    ///
    /// Returns [`MachineRefused::Rejected`] for a stack the slot's insertion
    /// test refuses and [`MachineRefused::Full`] for a count above one.
    pub fn insert_subject(
        &mut self,
        stack: Stack,
        settings: &Settings,
    ) -> Result<(), MachineRefused> {
        if !Self::accepts_subject(&stack, settings) {
            return Err(MachineRefused::Rejected { slot: 1 });
        }
        if stack.count > SUBJECT_LIMIT {
            return Err(MachineRefused::Full {
                slot: 1,
                limit: SUBJECT_LIMIT,
            });
        }
        self.subject = Some(stack);
        Ok(())
    }

    /// Takes whatever is in slot one out, which is the mid-cycle extraction
    /// §7's second warning is about.
    pub fn take_subject(&mut self) -> Option<Stack> {
        self.subject.take()
    }

    /// Runs one tick.
    ///
    /// With no cycle running, the machine starts one if both slots permit it and
    /// the buffer holds the price; the payment is made once, at the start, and
    /// one unit of material is consumed with it. With a cycle running, the tick
    /// decrements it, and the tick that reaches zero raises the item in slot one
    /// by one step.
    ///
    /// The price is discounted on the machine's **own** infusion level. Slot
    /// one's level is what the completion step raises and has no part in what
    /// the cycle costs.
    pub fn tick(&mut self, settings: &Settings) -> Ticked {
        if self.progress > 0 {
            self.progress -= 1;
            if self.progress > 0 {
                return Ticked::Running {
                    remaining: self.progress,
                };
            }
            let Some(subject) = self.subject.as_mut() else {
                return Ticked::CompletedIntoNothing;
            };
            subject.infusion = subject.infusion.saturating_add(1);
            return Ticked::Completed;
        }

        let ready = self
            .material
            .as_ref()
            .is_some_and(|stack| Self::accepts_material(stack) && stack.count > 0)
            && self
                .subject
                .as_ref()
                .is_some_and(|stack| Self::accepts_subject(stack, settings));
        if !ready {
            return Ticked::Idle;
        }

        let price = price(
            settings.energy_per_tick,
            factor(self.infusion, settings.upgrade_ceiling),
        );
        if self.energy < price {
            return Ticked::Idle;
        }
        self.energy -= price;
        if let Some(material) = self.material.as_mut() {
            material.count -= 1;
            if material.count == 0 {
                self.material = None;
            }
        }
        self.progress = TICKS_PER_UNIT;
        Ticked::Started { price }
    }
}

/// Why the machine refused an insertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineRefused {
    /// The slot's insertion test refused the stack.
    Rejected {
        /// Which slot refused.
        slot: usize,
    },
    /// The stack is larger than the slot holds.
    Full {
        /// Which slot refused.
        slot: usize,
        /// What that slot holds.
        limit: u8,
    },
}

impl std::fmt::Display for MachineRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected { slot } => write!(formatter, "slot {slot} does not take that item"),
            Self::Full { slot, limit } => {
                write!(formatter, "slot {slot} holds at most {limit}")
            }
        }
    }
}

impl std::error::Error for MachineRefused {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::TAG_COMPONENT;

    fn material(count: u8) -> Stack {
        let mut stack = Stack::of("counterpart:raw_material").expect("a legal identity");
        stack.count = count;
        stack
            .components
            .insert(TAG_COMPONENT.to_owned(), MATERIAL_TAG.to_owned());
        stack
    }

    fn subject(infusion: u32) -> Stack {
        let mut stack = Stack::of("counterpart:device").expect("a legal identity");
        stack.upgradable = true;
        stack.infusion = infusion;
        stack
    }

    fn loaded(settings: &Settings) -> Machine {
        let mut machine = Machine::new();
        machine
            .insert_material(material(4))
            .expect("the material carries the tag");
        machine
            .insert_subject(subject(0), settings)
            .expect("the item is upgradable and below the ceiling");
        machine.energy = settings.energy_capacity;
        machine
    }

    #[test]
    fn the_price_is_the_configured_figure_at_the_bottom_and_half_at_the_top() {
        assert_eq!(price(600, 0.0), 600);
        assert_eq!(price(600, 1.0), 300);
        assert_eq!(price(0, 0.5), 0);
    }

    #[test]
    fn the_price_is_truncated_toward_zero_rather_than_rounded_to_nearest() {
        // 601 * (2 - 0.5) / 2 is 450.75, which rounds to 451 and truncates to 450.
        assert_eq!(price(601, 0.5), 450);
        // 3 * (2 - 0.9) / 2 is 1.65, which rounds to 2 and truncates to 1.
        assert_eq!(price(3, 0.9), 1);
    }

    #[test]
    fn a_price_that_would_overflow_is_carried_at_the_ceiling_rather_than_wrapping() {
        assert_eq!(price(i32::MAX, 0.0), i32::MAX);
        assert!(price(i32::MAX, 1.0) > 0);
    }

    #[test]
    fn the_factor_is_the_completed_steps_over_the_ceiling_and_never_leaves_zero_to_one() {
        assert!((factor(0, 256) - 0.0).abs() < f32::EPSILON);
        assert!((factor(256, 256) - 1.0).abs() < f32::EPSILON);
        assert!((factor(u32::MAX, 256) - 1.0).abs() < f32::EPSILON);
        assert!((factor(128, 256) - 0.5).abs() < f32::EPSILON);
        assert!((factor(1, 0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn one_unit_of_material_costs_five_ticks_and_one_payment_made_at_the_start() {
        let settings = Settings::default();
        let mut machine = loaded(&settings);
        let before = machine.energy();

        assert_eq!(machine.tick(&settings), Ticked::Started { price: 600 });
        assert_eq!(machine.energy(), before - 600);
        assert_eq!(machine.progress(), TICKS_PER_UNIT);
        assert_eq!(
            machine.material().expect("three units are left").count,
            3,
            "exactly one unit is consumed"
        );

        for remaining in (1..TICKS_PER_UNIT).rev() {
            assert_eq!(machine.tick(&settings), Ticked::Running { remaining });
            assert_eq!(
                machine.energy(),
                before - 600,
                "the payment is not spread across the five ticks"
            );
        }
        assert_eq!(machine.tick(&settings), Ticked::Completed);
        assert_eq!(machine.subject().expect("it is still there").infusion, 1);
    }

    #[test]
    fn a_buffer_shorter_than_the_price_refuses_and_changes_nothing() {
        let settings = Settings::default();
        let mut machine = loaded(&settings);
        machine.energy = 599;

        assert_eq!(machine.tick(&settings), Ticked::Idle);
        assert_eq!(machine.energy(), 599);
        assert_eq!(machine.progress(), 0);
        assert_eq!(machine.material().expect("nothing was spent").count, 4);
    }

    #[test]
    fn an_item_extracted_mid_cycle_loses_the_payment_and_the_material() {
        let settings = Settings::default();
        let mut machine = loaded(&settings);
        assert!(matches!(machine.tick(&settings), Ticked::Started { .. }));
        machine.take_subject().expect("it was there to take");

        for _ in 1..TICKS_PER_UNIT {
            machine.tick(&settings);
        }
        assert_eq!(machine.tick(&settings), Ticked::CompletedIntoNothing);
        assert_eq!(machine.material().expect("three are left").count, 3);
    }

    #[test]
    fn lowering_the_ceiling_makes_an_already_infused_item_unacceptable_at_once() {
        let generous = Settings::default();
        let mut machine = Machine::new();
        machine
            .insert_subject(subject(10), &generous)
            .expect("ten is below two hundred and fifty-six");

        let strict = Settings {
            upgrade_ceiling: 10,
            ..Settings::default()
        };
        assert!(!Machine::accepts_subject(&subject(10), &strict));
        assert!(Machine::accepts_subject(&subject(9), &strict));
    }

    #[test]
    fn slot_zero_takes_only_the_tagged_material_and_slot_one_only_an_upgradable_item() {
        let settings = Settings::default();
        let mut machine = Machine::new();
        let untagged = Stack::of("counterpart:pebble").expect("a legal identity");

        assert_eq!(
            machine.insert_material(untagged.clone()),
            Err(MachineRefused::Rejected { slot: 0 })
        );
        assert_eq!(
            machine.insert_subject(untagged, &settings),
            Err(MachineRefused::Rejected { slot: 1 })
        );
    }

    #[test]
    fn the_two_slot_limits_are_enforced_where_automation_reaches_them() {
        let settings = Settings::default();
        let mut machine = Machine::new();

        assert_eq!(
            machine.insert_material(material(MATERIAL_LIMIT + 1)),
            Err(MachineRefused::Full {
                slot: 0,
                limit: MATERIAL_LIMIT,
            })
        );
        let mut two = subject(0);
        two.count = 2;
        assert_eq!(
            machine.insert_subject(two, &settings),
            Err(MachineRefused::Full {
                slot: 1,
                limit: SUBJECT_LIMIT,
            })
        );
    }

    #[test]
    fn the_buffer_takes_at_most_the_accept_rate_and_never_past_capacity() {
        let settings = Settings::default();
        let mut machine = Machine::new();

        assert_eq!(machine.accept_energy(i32::MAX, &settings), 600);
        assert_eq!(machine.energy(), 600);

        machine.energy = settings.energy_capacity;
        assert_eq!(machine.accept_energy(600, &settings), 0);
        assert_eq!(machine.energy(), settings.energy_capacity);
    }

    #[test]
    fn a_machine_that_is_itself_infused_pays_less_for_the_same_work() {
        let settings = Settings::default();
        let mut fresh = loaded(&settings);
        let mut infused = loaded(&settings);
        infused.set_infusion(settings.upgrade_ceiling.unsigned_abs());

        assert_eq!(fresh.tick(&settings), Ticked::Started { price: 600 });
        assert_eq!(
            infused.tick(&settings),
            Ticked::Started { price: 300 },
            "the discount is the machine's own level, read through the cycle"
        );
    }

    #[test]
    fn the_discount_ignores_the_infusion_level_of_the_item_being_upgraded() {
        let settings = Settings::default();
        let mut machine = Machine::new();
        machine
            .insert_material(material(4))
            .expect("the material carries the tag");
        machine
            .insert_subject(
                subject(settings.upgrade_ceiling.unsigned_abs() - 1),
                &settings,
            )
            .expect("one below the ceiling is acceptable");
        machine.energy = settings.energy_capacity;

        assert_eq!(
            machine.tick(&settings),
            Ticked::Started { price: 600 },
            "a nearly-finished subject in a fresh machine pays the undiscounted price"
        );
    }

    #[test]
    fn a_machine_at_the_ceiling_reaches_the_half_price_the_specification_states() {
        let settings = Settings::default();
        let mut machine = loaded(&settings);
        machine.set_infusion(settings.upgrade_ceiling.unsigned_abs());

        assert_eq!(machine.infusion(), 256);
        assert_eq!(
            machine.tick(&settings),
            Ticked::Started { price: 300 },
            "the boundary the discount is stated at is reachable through the shipped path"
        );
    }

    #[test]
    fn the_four_capabilities_and_the_four_kept_components_are_named_once_each() {
        let mut names = CAPABILITIES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CAPABILITIES.len());

        let mut kept: Vec<&str> = DROPPED_COMPONENTS
            .iter()
            .map(|component| component.identifier)
            .collect();
        kept.sort_unstable();
        kept.dedup();
        assert_eq!(kept.len(), 4);
    }

    #[test]
    fn the_two_shared_identifiers_are_named_out_of_the_library_that_owns_them() {
        let shared: Vec<&str> = DROPPED_COMPONENTS
            .iter()
            .filter(|component| component.owner == Vocabulary::SharedLibrary)
            .map(|component| component.identifier)
            .collect();
        assert_eq!(
            shared.len(),
            2,
            "§5 attributes exactly two of the four to the shared library"
        );
        for identifier in shared {
            assert_eq!(
                shared_component(identifier),
                Some(identifier),
                "a shared identifier this package spells and the library does not publish \
                 is the cross-entry contract broken"
            );
        }
        assert_eq!(shared_component("infusion_progress"), None);
    }

    #[test]
    fn exactly_one_kept_component_is_this_entrys_own_record() {
        let mine: Vec<&str> = DROPPED_COMPONENTS
            .iter()
            .filter(|component| component.owner == Vocabulary::ThisEntry)
            .map(|component| component.identifier)
            .collect();
        assert_eq!(mine, [crate::data::Record::InfusionProgress.as_str()]);
    }
}
