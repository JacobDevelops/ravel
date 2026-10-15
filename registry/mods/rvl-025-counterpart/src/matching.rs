//! The stack-match decision of §8, its store, and how the store is filled.
//!
//! One carrier item holds a set of stacks, a set of tag names and four mode
//! flags, and answers one question: does this stack pass. The verdict order is
//! the behaviour, so it is written here in the order §8 states it and each step
//! is held to its own test.
//!
//! Two things §8 asks a counterpart to decide rather than copy are decided here.
//! The removal loop is written correctly rather than reproducing the index it
//! fails to adjust, and the tooltip's tag expansion is bounded, which the
//! original leaves unbounded on a path that runs on the client for every hover.

use std::fmt;

use crate::data::{MAX_ENTRIES, Record, RecordRefused, Stack, check_name, read_strict};
use ravel_sdk::JsonValue;

/// The most stacks one rule may store.
///
/// A rule arrives from the network as well as from a menu, so this is a trust
/// boundary and the ceiling is enforced on insertion as well as on decoding.
pub const MAX_STACKS: usize = 512;

/// The most tag names one rule may store.
pub const MAX_TAGS: usize = 128;

/// The most lines a tooltip expansion produces.
///
/// A known ceiling rather than a rule: expanding every stored tag to its members
/// is unbounded work on the client for every hover, and the upgrade path is a
/// tooltip that pages rather than a larger number here.
pub const MAX_TOOLTIP_LINES: usize = 256;

/// Whether the raw match verdict is included or excluded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VerdictMode {
    /// Keep the raw match verdict.
    #[default]
    Inclusion,
    /// Invert the raw match verdict.
    Exclusion,
}

/// The four independent matching choices a rule carries.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Modes {
    /// Reject a stored entry whose damage differs from the tested stack's.
    pub damage: bool,
    /// Reject a stored entry that is not the same item with the same components.
    pub components: bool,
    /// Match on the namespace half of the identity rather than on the whole.
    pub namespace: bool,
    /// Keep or invert the raw verdict.
    pub verdict: VerdictMode,
}

impl fmt::Debug for Modes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Modes")
            .field("damage", &self.damage)
            .field("components", &self.components)
            .field("namespace", &self.namespace)
            .field("exclusion", &(self.verdict == VerdictMode::Exclusion))
            .finish()
    }
}

/// A rule that answers whether a stack passes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MatchRule {
    stacks: Vec<Stack>,
    tags: Vec<String>,
    modes: Modes,
}

impl MatchRule {
    /// Builds an empty rule in the default mode, which excludes nothing and
    /// matches nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the mode flags in force.
    #[must_use]
    pub const fn modes(&self) -> Modes {
        self.modes
    }

    /// Sets the mode flags.
    pub const fn set_modes(&mut self, modes: Modes) {
        self.modes = modes;
    }

    /// Returns the stored stacks, in the order the scan walks them.
    #[must_use]
    pub fn stacks(&self) -> &[Stack] {
        &self.stacks
    }

    /// Returns the stored tag names.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Answers §8's one question.
    ///
    /// The order is load-bearing and each step is written where §8 numbers it:
    /// an empty stack never passes and is decided before the inversion; the tag
    /// test is skipped entirely when the tag set is empty; the stack scan runs
    /// only when the tag test did not match; and the raw verdict is inverted by
    /// the exclusion flag last.
    #[must_use]
    pub fn passes(&self, candidate: &Stack) -> bool {
        if candidate.is_empty() {
            return false;
        }
        let raw = self.matches_tag(candidate) || self.matches_stack(candidate);
        raw != (self.modes.verdict == VerdictMode::Exclusion)
    }

    /// Answers whether any tag the candidate carries is in the stored set.
    ///
    /// The candidate's own tags are the caller's to supply, so this reads the
    /// component named for them; a stack that declares none matches no tag. §8's
    /// first consequence is here: the two rejection flags apply to the stack
    /// test only, and a tag match ignores damage and components entirely.
    fn matches_tag(&self, candidate: &Stack) -> bool {
        if self.tags.is_empty() {
            return false;
        }
        candidate
            .components
            .get(TAG_COMPONENT)
            .is_some_and(|carried| {
                carried
                    .split(',')
                    .any(|tag| self.tags.iter().any(|stored| stored == tag))
            })
    }

    /// Scans the stored stacks in order and answers on the first that matches.
    fn matches_stack(&self, candidate: &Stack) -> bool {
        self.stacks.iter().any(|stored| {
            if self.modes.damage && stored.damage != candidate.damage {
                return false;
            }
            if self.modes.components && !stored.same_with_components(candidate) {
                return false;
            }
            if self.modes.namespace {
                return stored.namespace() == candidate.namespace();
            }
            stored.identity == candidate.identity
        })
    }

    /// Stores a stack, deduplicating and normalising the count to one.
    ///
    /// An addition is abandoned when an existing entry is the same item with the
    /// same components, which is what makes §8's removal defect unreachable: two
    /// adjacent entries cannot both match the predicate removal scans on.
    ///
    /// # Errors
    ///
    /// Returns [`FillRefused::Full`] at [`MAX_STACKS`] and whatever
    /// [`Stack::of`] refuses for an unusable identity.
    pub fn insert(&mut self, stack: &Stack) -> Result<bool, FillRefused> {
        check_name(&stack.identity).map_err(FillRefused::Record)?;
        if self
            .stacks
            .iter()
            .any(|stored| stored.same_with_components(stack))
        {
            return Ok(false);
        }
        if self.stacks.len() >= MAX_STACKS {
            return Err(FillRefused::Full { limit: MAX_STACKS });
        }
        let mut copied = stack.clone();
        copied.count = 1;
        self.stacks.push(copied);
        Ok(true)
    }

    /// Removes every stored entry that is the same item with the same
    /// components, answering how many went.
    ///
    /// The loop is the corrected one §8 asks for: it removes in place and does
    /// not skip the entry that moved into the index just vacated.
    pub fn remove(&mut self, stack: &Stack) -> usize {
        let before = self.stacks.len();
        self.stacks
            .retain(|stored| !stored.same_with_components(stack));
        before - self.stacks.len()
    }

    /// Stores a tag name.
    ///
    /// # Errors
    ///
    /// Returns [`FillRefused::Full`] at [`MAX_TAGS`] and [`FillRefused::Record`]
    /// for an empty or over-long name.
    pub fn insert_tag(&mut self, tag: &str) -> Result<bool, FillRefused> {
        check_name(tag).map_err(FillRefused::Record)?;
        if self.tags.iter().any(|stored| stored == tag) {
            return Ok(false);
        }
        if self.tags.len() >= MAX_TAGS {
            return Err(FillRefused::Full { limit: MAX_TAGS });
        }
        self.tags.push(tag.to_owned());
        Ok(true)
    }

    /// Fills the rule from a block that was used on while crouching, which is
    /// §8.1's second filling route.
    ///
    /// A block with an inventory contributes every distinct item in it; a block
    /// without one contributes its own item form. Deduplication is the same
    /// predicate insertion uses, so the distinctness is the rule's rather than
    /// the caller's.
    ///
    /// # Errors
    ///
    /// Returns whatever [`MatchRule::insert`] refuses, so a block holding more
    /// distinct items than the rule may store refuses rather than truncating
    /// silently.
    pub fn fill_from_block(
        &mut self,
        contents: &[Stack],
        block_item: &Stack,
    ) -> Result<usize, FillRefused> {
        let mut added = 0;
        if contents.is_empty() {
            if self.insert(block_item)? {
                added += 1;
            }
            return Ok(added);
        }
        for stack in contents {
            if stack.is_empty() {
                continue;
            }
            if self.insert(stack)? {
                added += 1;
            }
        }
        Ok(added)
    }

    /// Expands the rule into the lines a tooltip lists, bounded.
    ///
    /// Every stored stack is one line and every stored tag expands to its
    /// members, which is what the original does on the client for every hover
    /// and leaves unbounded. The expansion stops at [`MAX_TOOLTIP_LINES`] and
    /// says how many lines it did not write, so a rule naming a very large tag
    /// costs a bounded amount of work per hover.
    #[must_use]
    pub fn tooltip(&self, members: &dyn Fn(&str) -> Vec<String>) -> Tooltip {
        let mut lines = Vec::new();
        let mut omitted = 0_usize;
        let push = |line: String, lines: &mut Vec<String>, omitted: &mut usize| {
            if lines.len() >= MAX_TOOLTIP_LINES {
                *omitted += 1;
                return;
            }
            lines.push(line);
        };
        for stack in &self.stacks {
            push(stack.identity.clone(), &mut lines, &mut omitted);
        }
        for tag in &self.tags {
            for member in members(tag) {
                push(member, &mut lines, &mut omitted);
            }
        }
        Tooltip { lines, omitted }
    }

    /// Writes the rule as its persisted document.
    #[must_use]
    pub fn to_saved(&self) -> JsonValue {
        ravel_sdk::json!({
            "stacks": self.stacks.iter().map(Stack::to_saved).collect::<Vec<_>>(),
            "tags": self.tags,
            "damage": self.modes.damage,
            "components": self.modes.components,
            "namespace": self.modes.namespace,
            "exclusion": self.modes.verdict == VerdictMode::Exclusion,
        })
    }

    /// Reads the rule back, refusing an absent entry in the stack list.
    ///
    /// This is the one record of §4 that decodes strictly. A rule that lost an
    /// entry answers a different question from the one that was written, so the
    /// whole record is refused rather than silently narrowed or widened.
    ///
    /// # Errors
    ///
    /// Returns [`RecordRefused`] for a document that is not the rule's shape,
    /// for a stack list carrying an absence, and for either list above its
    /// ceiling.
    pub fn from_saved(document: &JsonValue) -> Result<Self, RecordRefused> {
        let object = document.as_object().ok_or(RecordRefused::Shape {
            record: Some(Record::MatchRule),
        })?;
        let stacks = read_strict(
            Record::MatchRule,
            object.get("stacks").unwrap_or(&JsonValue::Null),
            MAX_STACKS,
        )?;

        let declared =
            object
                .get("tags")
                .and_then(JsonValue::as_array)
                .ok_or(RecordRefused::Shape {
                    record: Some(Record::MatchRule),
                })?;
        if declared.len() > MAX_TAGS.min(MAX_ENTRIES) {
            return Err(RecordRefused::TooMany {
                limit: MAX_TAGS,
                found: declared.len(),
            });
        }
        let mut tags = Vec::with_capacity(declared.len());
        for entry in declared {
            let tag = entry.as_str().ok_or(RecordRefused::Shape {
                record: Some(Record::MatchRule),
            })?;
            check_name(tag)?;
            tags.push(tag.to_owned());
        }

        let flag = |name: &str| {
            object
                .get(name)
                .and_then(JsonValue::as_bool)
                .unwrap_or_default()
        };
        Ok(Self {
            stacks,
            tags,
            modes: Modes {
                damage: flag("damage"),
                components: flag("components"),
                namespace: flag("namespace"),
                verdict: if flag("exclusion") {
                    VerdictMode::Exclusion
                } else {
                    VerdictMode::Inclusion
                },
            },
        })
    }
}

/// The component a stack carries its own tag memberships under.
///
/// §8's tag test asks whether any tag the item carries is in the stored set, and
/// what an item's tags are is the host's. This is the portable stand-in: the
/// membership list travels with the stack as one comma-separated component, so
/// the decision rule can be stated and tested without a host registry.
pub const TAG_COMPONENT: &str = "tags";

/// What a tooltip expansion produced, and what it did not.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tooltip {
    /// The lines, in stored order, stacks before tags.
    pub lines: Vec<String>,
    /// How many lines the ceiling kept out.
    pub omitted: usize,
}

/// Why a rule refused something offered to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillRefused {
    /// The rule already holds as many entries as it may.
    Full {
        /// The ceiling that was reached.
        limit: usize,
    },
    /// The offered entry is not one a record may carry.
    Record(RecordRefused),
}

impl fmt::Display for FillRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full { limit } => write!(formatter, "a rule holds at most {limit} entries"),
            Self::Record(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for FillRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(identity: &str) -> Stack {
        Stack::of(identity).expect("a legal identity")
    }

    fn tagged(identity: &str, tags: &str) -> Stack {
        let mut built = stack(identity);
        built
            .components
            .insert(TAG_COMPONENT.to_owned(), tags.to_owned());
        built
    }

    #[test]
    fn an_empty_stack_never_passes_even_in_exclusion_mode() {
        let mut rule = MatchRule::new();
        rule.set_modes(Modes {
            verdict: VerdictMode::Exclusion,
            ..Modes::default()
        });
        let empty = Stack::default();

        assert!(empty.is_empty());
        assert!(
            !rule.passes(&empty),
            "the emptiness is decided before the inversion"
        );
    }

    #[test]
    fn an_empty_tag_set_skips_the_tag_test_entirely() {
        let mut rule = MatchRule::new();
        rule.insert(&stack("counterpart:first"))
            .expect("the rule takes one entry");

        assert_eq!(rule.tags().len(), 0);
        assert!(rule.passes(&tagged("counterpart:first", "counterpart:group")));
        assert!(!rule.passes(&tagged("counterpart:second", "counterpart:group")));
    }

    #[test]
    fn a_tag_match_ignores_damage_and_components_where_the_stack_test_would_not() {
        let mut rule = MatchRule::new();
        rule.insert_tag("counterpart:group")
            .expect("the rule takes one tag");
        rule.set_modes(Modes {
            damage: true,
            components: true,
            ..Modes::default()
        });

        let mut worn = tagged("counterpart:first", "counterpart:group");
        worn.damage = 12;
        assert!(
            rule.passes(&worn),
            "the two rejection flags apply to the stack test alone"
        );
    }

    #[test]
    fn the_damage_flag_rejects_a_stored_entry_whose_damage_differs() {
        let mut rule = MatchRule::new();
        rule.insert(&stack("counterpart:first"))
            .expect("the rule takes one entry");
        rule.set_modes(Modes {
            damage: true,
            ..Modes::default()
        });

        let mut worn = stack("counterpart:first");
        worn.damage = 3;
        assert!(!rule.passes(&worn));
        assert!(rule.passes(&stack("counterpart:first")));
    }

    #[test]
    fn the_component_rejection_runs_before_the_namespace_comparison() {
        let mut stored = stack("counterpart:first");
        stored
            .components
            .insert("label".to_owned(), "first".to_owned());

        let mut rule = MatchRule::new();
        rule.insert(&stored).expect("the rule takes one entry");
        rule.set_modes(Modes {
            components: true,
            namespace: true,
            ..Modes::default()
        });

        let same_namespace = stack("counterpart:second");
        assert!(
            !rule.passes(&same_namespace),
            "a namespace rule with the component flag set matches only \
             component-identical entries, which is why the namespace \
             comparison is almost always redundant"
        );
        assert!(rule.passes(&stored));
    }

    #[test]
    fn the_namespace_flag_matches_a_different_item_in_the_same_namespace() {
        let mut rule = MatchRule::new();
        rule.insert(&stack("counterpart:first"))
            .expect("the rule takes one entry");
        rule.set_modes(Modes {
            namespace: true,
            ..Modes::default()
        });

        assert!(rule.passes(&stack("counterpart:second")));
        assert!(!rule.passes(&stack("elsewhere:second")));
    }

    #[test]
    fn the_exclusion_flag_inverts_the_raw_verdict_and_nothing_else() {
        let mut rule = MatchRule::new();
        rule.insert(&stack("counterpart:first"))
            .expect("the rule takes one entry");
        assert!(rule.passes(&stack("counterpart:first")));

        rule.set_modes(Modes {
            verdict: VerdictMode::Exclusion,
            ..Modes::default()
        });
        assert!(!rule.passes(&stack("counterpart:first")));
        assert!(rule.passes(&stack("counterpart:second")));
    }

    #[test]
    fn insertion_normalises_the_count_to_one_and_abandons_a_duplicate() {
        let mut crowded = stack("counterpart:first");
        crowded.count = 64;

        let mut rule = MatchRule::new();
        assert!(rule.insert(&crowded).expect("the first goes in"));
        assert!(!rule.insert(&crowded).expect("the second is abandoned"));
        assert_eq!(rule.stacks().len(), 1);
        assert_eq!(
            rule.stacks()[0].count,
            1,
            "a stored entry's count is always one, so no rule can depend on a stack size"
        );
    }

    #[test]
    fn removal_does_not_skip_the_entry_that_moved_into_the_vacated_index() {
        let mut rule = MatchRule::new();
        for identity in ["counterpart:first", "counterpart:second"] {
            rule.insert(&stack(identity)).expect("the rule takes it");
        }
        // Two adjacent equal entries cannot arrive through insertion, so they
        // are placed here directly: this is the shape the original's index skips.
        rule.stacks.insert(1, stack("counterpart:first"));
        assert_eq!(rule.stacks().len(), 3);

        assert_eq!(rule.remove(&stack("counterpart:first")), 2);
        assert_eq!(rule.stacks().len(), 1);
        assert_eq!(rule.stacks()[0].identity, "counterpart:second");
    }

    #[test]
    fn a_block_with_no_inventory_contributes_its_own_item_form() {
        let mut rule = MatchRule::new();
        let added = rule
            .fill_from_block(&[], &stack("counterpart:machine"))
            .expect("the rule takes it");

        assert_eq!(added, 1);
        assert_eq!(rule.stacks()[0].identity, "counterpart:machine");
    }

    #[test]
    fn a_block_with_an_inventory_contributes_every_distinct_item_in_it() {
        let mut rule = MatchRule::new();
        let contents = [
            stack("counterpart:first"),
            Stack::default(),
            stack("counterpart:first"),
            stack("counterpart:second"),
        ];
        let added = rule
            .fill_from_block(&contents, &stack("counterpart:machine"))
            .expect("the rule takes them");

        assert_eq!(added, 2);
        assert_eq!(rule.stacks().len(), 2);
    }

    #[test]
    fn a_rule_refuses_a_stack_beyond_its_ceiling_rather_than_growing() {
        let mut rule = MatchRule::new();
        for index in 0..MAX_STACKS {
            rule.insert(&stack(&format!("counterpart:entry{index}")))
                .expect("the rule takes it");
        }
        assert_eq!(
            rule.insert(&stack("counterpart:one_too_many")),
            Err(FillRefused::Full { limit: MAX_STACKS })
        );
    }

    #[test]
    fn a_tooltip_expansion_is_bounded_and_says_what_it_left_out() {
        let mut rule = MatchRule::new();
        rule.insert_tag("counterpart:group")
            .expect("the rule takes one tag");

        let enormous = |_: &str| {
            (0..MAX_TOOLTIP_LINES * 4)
                .map(|index| format!("counterpart:member{index}"))
                .collect::<Vec<_>>()
        };
        let tooltip = rule.tooltip(&enormous);

        assert_eq!(tooltip.lines.len(), MAX_TOOLTIP_LINES);
        assert_eq!(tooltip.omitted, MAX_TOOLTIP_LINES * 3);
    }

    #[test]
    fn a_rule_round_trips_and_an_absent_stored_entry_refuses_the_whole_record() {
        let mut rule = MatchRule::new();
        rule.insert(&stack("counterpart:first"))
            .expect("the rule takes it");
        rule.insert_tag("counterpart:group")
            .expect("the rule takes it");
        rule.set_modes(Modes {
            damage: true,
            verdict: VerdictMode::Exclusion,
            ..Modes::default()
        });

        let read = MatchRule::from_saved(&rule.to_saved()).expect("what was written reads back");
        assert_eq!(read, rule);

        let torn = ravel_sdk::json!({
            "stacks": [null],
            "tags": [],
            "damage": false,
            "components": false,
            "namespace": false,
            "exclusion": false,
        });
        assert_eq!(
            MatchRule::from_saved(&torn),
            Err(RecordRefused::AbsentEntry {
                record: Record::MatchRule,
                index: 0,
            })
        );
    }

    #[test]
    fn a_hostile_tag_list_is_refused_by_its_count() {
        let tags: Vec<String> = (0..=MAX_TAGS)
            .map(|index| format!("counterpart:group{index}"))
            .collect();
        let document = ravel_sdk::json!({
            "stacks": [],
            "tags": tags,
            "damage": false,
            "components": false,
            "namespace": false,
            "exclusion": false,
        });
        assert!(matches!(
            MatchRule::from_saved(&document),
            Err(RecordRefused::TooMany { .. })
        ));
    }
}
