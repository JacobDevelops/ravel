//! The six tools: one grade table, one unbreakability rule, one dispatch rule.
//!
//! Three things here carry behaviour rather than data. A declared durability of
//! zero means unbreakable and is acted on at construction, so the top two grades
//! never take damage. The repair material is resolved at first repair rather than
//! at construction, because the item it names belongs to another entry and is not
//! registered while a tool is being built. And every attached modifier is offered
//! every interaction with no short-circuit, so a modifier that consumes an event
//! does not prevent the ones after it from seeing it.

/// How many grades this entry declares.
pub const COUNT: usize = 6;

/// The modifier categories every grade accepts: the generic one plus the three
/// tool shapes this item combines.
pub const MODIFIER_CATEGORIES: [&str; 4] = ["generic", "mining", "digging", "cutting"];

/// One grade's declared values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    /// The grade, 1 through 6.
    pub grade: u32,
    /// Declared durability. Zero means unbreakable.
    pub durability: u32,
    /// Mining speed.
    pub speed: f64,
    /// Attack bonus.
    pub attack: f64,
    /// Enchantability.
    pub enchantability: u32,
    /// How many modifier slots the tool carries.
    pub slots: u32,
    /// The highest modifier tier the tool accepts.
    pub modifier_tier: u32,
}

impl Grade {
    /// Whether a tool of this grade carries the host's unbreakable marker.
    ///
    /// A declared durability of zero means unbreakable, and the marker is given
    /// at construction rather than derived at every damage event. Reading zero
    /// as "breaks immediately" inverts the top two grades.
    #[must_use]
    pub const fn unbreakable(&self) -> bool {
        self.durability == 0
    }

    /// Whether a tool of this grade can be repaired at all.
    #[must_use]
    pub const fn repairable(&self) -> bool {
        !self.unbreakable()
    }

    /// Whether a tool of this grade is enchantable at a table.
    ///
    /// The gate is the dependency's flag, not one of this entry's. When it is
    /// off the answer falls back to whatever the base tool kind allows, which is
    /// this caller's `base` and not a decision made here.
    #[must_use]
    pub const fn enchantable(&self, top_tools_enchantable: bool, base: bool) -> bool {
        top_tools_enchantable || base
    }
}

/// The six grades, lowest first.
pub const DECLARED: [Grade; COUNT] = [
    Grade {
        grade: 1,
        durability: 2000,
        speed: 9.0,
        attack: 4.0,
        enchantability: 12,
        slots: 1,
        modifier_tier: 1,
    },
    Grade {
        grade: 2,
        durability: 2800,
        speed: 11.0,
        attack: 6.0,
        enchantability: 14,
        slots: 1,
        modifier_tier: 2,
    },
    Grade {
        grade: 3,
        durability: 4000,
        speed: 14.0,
        attack: 9.0,
        enchantability: 16,
        slots: 1,
        modifier_tier: 3,
    },
    Grade {
        grade: 4,
        durability: 6000,
        speed: 19.0,
        attack: 13.0,
        enchantability: 18,
        slots: 1,
        modifier_tier: 4,
    },
    Grade {
        grade: 5,
        durability: 0,
        speed: 25.0,
        attack: 20.0,
        enchantability: 20,
        slots: 1,
        modifier_tier: 5,
    },
    Grade {
        grade: 6,
        durability: 0,
        speed: 30.0,
        attack: 25.0,
        enchantability: 22,
        slots: 2,
        modifier_tier: 5,
    },
];

/// The grade declared at a position, or nothing.
#[must_use]
pub fn at(grade: u32) -> Option<Grade> {
    DECLARED.into_iter().find(|held| held.grade == grade)
}

/// The one grade produced by a ritual rather than by a grid recipe.
pub const AWAKENED_GRADE: u32 = 6;

/// The grade the awakening consumes.
pub const AWAKENED_FROM: u32 = 5;

/// The five interaction points a modifier is offered, plus the tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interaction {
    /// Use on a block.
    UseOnBlock,
    /// Use in the air.
    UseInAir,
    /// Use on an entity.
    UseOnEntity,
    /// Hitting an entity.
    HitEntity,
    /// Breaking a block.
    BreakBlock,
}

/// Every interaction point, in the order the specification lists them.
pub const INTERACTIONS: [Interaction; 5] = [
    Interaction::UseOnBlock,
    Interaction::UseInAir,
    Interaction::UseOnEntity,
    Interaction::HitEntity,
    Interaction::BreakBlock,
];

/// A modifier attached to a tool.
pub trait ToolModifier {
    /// Offers the modifier one interaction, answering whether it acted.
    fn offer(&mut self, interaction: Interaction) -> bool;

    /// Offers the modifier an inventory tick, which answers nothing.
    fn tick(&mut self);
}

/// Offers one interaction to every attached modifier, in attachment order.
///
/// All of them are offered, with no short-circuit, and the interaction reports
/// success if any one of them acted. So a modifier that consumes an event does
/// not prevent a later one from seeing it, and attachment order is observable
/// only through side effects rather than through which modifier wins.
pub fn dispatch(modifiers: &mut [Box<dyn ToolModifier>], interaction: Interaction) -> bool {
    let mut acted = false;
    for modifier in modifiers.iter_mut() {
        acted |= modifier.offer(interaction);
    }
    acted
}

/// Breaking a block: the base handling runs first, then the modifiers.
///
/// The base result is discarded and only the modifiers' verdict is returned.
/// Whether that discard is reachable as a behavioural difference depends on what
/// the base tool kind answers, which this entry does not decide.
pub fn break_block(modifiers: &mut [Box<dyn ToolModifier>], base: &dyn Fn() -> bool) -> bool {
    let _discarded = base();
    dispatch(modifiers, Interaction::BreakBlock)
}

/// Offers the inventory tick to every modifier, answering nothing.
pub fn tick(modifiers: &mut [Box<dyn ToolModifier>]) {
    for modifier in modifiers.iter_mut() {
        modifier.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Debug)]
    struct Recorder {
        name: &'static str,
        acts: bool,
        seen: Seen,
        ticks: Ticks,
    }

    impl ToolModifier for Recorder {
        fn offer(&mut self, _interaction: Interaction) -> bool {
            self.seen.borrow_mut().push(self.name);
            self.acts
        }

        fn tick(&mut self) {
            *self.ticks.borrow_mut() += 1;
        }
    }

    type Seen = Rc<RefCell<Vec<&'static str>>>;
    type Ticks = Rc<RefCell<u32>>;
    type Pair = (Vec<Box<dyn ToolModifier>>, Seen, Ticks);

    fn pair(first_acts: bool, second_acts: bool) -> Pair {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let ticks = Rc::new(RefCell::new(0));
        let modifiers: Vec<Box<dyn ToolModifier>> = vec![
            Box::new(Recorder {
                name: "first",
                acts: first_acts,
                seen: Rc::clone(&seen),
                ticks: Rc::clone(&ticks),
            }),
            Box::new(Recorder {
                name: "second",
                acts: second_acts,
                seen: Rc::clone(&seen),
                ticks: Rc::clone(&ticks),
            }),
        ];
        (modifiers, seen, ticks)
    }

    #[test]
    fn all_six_rows_carry_the_values_the_grade_table_states() {
        let durability: Vec<u32> = DECLARED.iter().map(|grade| grade.durability).collect();
        assert_eq!(durability, [2000, 2800, 4000, 6000, 0, 0]);
        let speed: Vec<f64> = DECLARED.iter().map(|grade| grade.speed).collect();
        assert_eq!(speed, [9.0, 11.0, 14.0, 19.0, 25.0, 30.0]);
        let attack: Vec<f64> = DECLARED.iter().map(|grade| grade.attack).collect();
        assert_eq!(attack, [4.0, 6.0, 9.0, 13.0, 20.0, 25.0]);
        let enchantability: Vec<u32> = DECLARED.iter().map(|grade| grade.enchantability).collect();
        assert_eq!(enchantability, [12, 14, 16, 18, 20, 22]);
        let slots: Vec<u32> = DECLARED.iter().map(|grade| grade.slots).collect();
        assert_eq!(slots, [1, 1, 1, 1, 1, 2]);
        let tiers: Vec<u32> = DECLARED.iter().map(|grade| grade.modifier_tier).collect();
        assert_eq!(tiers, [1, 2, 3, 4, 5, 5]);
    }

    #[test]
    fn a_zero_durability_grade_is_unbreakable_rather_than_immediately_broken() {
        for grade in DECLARED.iter().filter(|held| held.grade <= 4) {
            assert!(!grade.unbreakable());
            assert!(grade.repairable());
        }
        for grade in DECLARED.iter().filter(|held| held.grade >= 5) {
            assert!(grade.unbreakable());
            assert!(!grade.repairable(), "it never needs repair");
        }
    }

    #[test]
    fn the_top_grade_shares_the_fifths_modifier_tier_and_adds_one_slot() {
        let fifth = at(5).expect("a declared grade");
        let sixth = at(AWAKENED_GRADE).expect("a declared grade");
        assert_eq!(sixth.modifier_tier, fifth.modifier_tier);
        assert_eq!(sixth.slots, fifth.slots + 1);
        assert!(sixth.speed > fifth.speed);
        assert!(sixth.attack > fifth.attack);
        assert_eq!(AWAKENED_FROM, fifth.grade);
        assert_eq!(at(0), None);
        assert_eq!(at(7), None);
    }

    #[test]
    fn every_modifier_sees_every_interaction_even_after_one_has_acted() {
        for interaction in INTERACTIONS {
            let (mut modifiers, seen, _) = pair(true, false);
            assert!(dispatch(&mut modifiers, interaction));
            assert_eq!(*seen.borrow(), ["first", "second"]);
        }
    }

    #[test]
    fn the_interaction_succeeds_when_any_one_modifier_acted_and_not_otherwise() {
        let (mut none, _, _) = pair(false, false);
        assert!(!dispatch(&mut none, Interaction::UseInAir));

        let (mut last, seen, _) = pair(false, true);
        assert!(dispatch(&mut last, Interaction::UseInAir));
        assert_eq!(*seen.borrow(), ["first", "second"]);

        let (mut both, _, _) = pair(true, true);
        assert!(dispatch(&mut both, Interaction::UseInAir));
    }

    #[test]
    fn breaking_a_block_runs_the_base_first_and_returns_only_the_modifiers_verdict() {
        let order = RefCell::new(Vec::new());
        let (mut modifiers, seen, _) = pair(false, false);
        let base = || {
            order.borrow_mut().push("base");
            true
        };
        assert!(
            !break_block(&mut modifiers, &base),
            "the base result is discarded"
        );
        assert_eq!(*order.borrow(), ["base"]);
        assert_eq!(*seen.borrow(), ["first", "second"]);

        let (mut acting, _, _) = pair(false, true);
        assert!(break_block(&mut acting, &|| false));
    }

    #[test]
    fn the_inventory_tick_reaches_every_modifier_and_answers_nothing() {
        let (mut modifiers, seen, ticks) = pair(true, true);
        tick(&mut modifiers);
        assert_eq!(*ticks.borrow(), 2);
        assert_eq!(
            seen.borrow().len(),
            0,
            "a tick is not one of the five interaction points"
        );
    }

    #[test]
    fn enchantability_follows_the_dependencys_flag_and_falls_back_to_the_base_kind() {
        let grade = at(6).expect("a declared grade");
        assert!(!grade.enchantable(false, false));
        assert!(grade.enchantable(false, true));
        assert!(grade.enchantable(true, false));
        assert!(grade.enchantable(true, true));
    }

    #[test]
    fn every_grade_accepts_the_generic_category_and_the_three_shapes() {
        let mut sorted = MODIFIER_CATEGORIES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 4, "the four categories are distinct");
        assert!(MODIFIER_CATEGORIES.contains(&"generic"));
    }
}
