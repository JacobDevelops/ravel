//! The connection condition language of the specification's §7.
//!
//! A declaration may carry an object saying which neighbouring block states
//! count as connected. This module is that object read into a predicate tree,
//! and the tree asked about a pair of states.
//!
//! Three things about it are load-bearing and are held to tests below.
//!
//! **The default and the malformed case are different answers.** No condition
//! key at all means exact state identity. A key whose value is not an object,
//! or an object the language does not understand, means never-connects.
//! Falling back to the default on a malformed document would connect blocks a
//! pack author asked not to connect.
//!
//! **Degenerate input has a stated answer for every form.** An empty list, a
//! single-element list, an exclusive-or of the wrong arity, a property value
//! that is not a string: each has a row in §7's table and a test here.
//!
//! **The named-tag form does not raise.** §12's defect 5 is that a missing or
//! unparseable tag name escapes declaration parsing in the original and fails
//! the whole model. Here it returns never-connects like every sibling form.
//!
//! # Bounds
//!
//! A condition arrives from a resource pack, which is untrusted input: a pack
//! author can nest one as deeply as a file allows. [`MAX_DEPTH`] and
//! [`MAX_ELEMENTS`] are enforced at parse and again at evaluation, and a
//! [`Report`] counts every refusal rather than discarding it, so a pack whose
//! condition was refused is diagnosable rather than silently inert.

use std::collections::{BTreeMap, BTreeSet};

use ravel_sdk::JsonValue;
use ravel_sdk::content::ContentId;

/// The deepest a parsed condition may nest.
///
/// A hand-written condition is a handful of levels. This is far above anything
/// a pack author writes and far below anything that threatens the stack. Raise
/// the number if a dependent ever needs more; do not remove the check.
pub const MAX_DEPTH: usize = 16;

/// The most elements a conjunction or a disjunction may list.
pub const MAX_ELEMENTS: usize = 64;

/// The namespace a bare name is read under.
pub const DEFAULT_NAMESPACE: &str = "base";

/// The key whose value names the form.
pub const FORM_KEY: &str = "test";

/// The key a list-shaped form holds its elements under.
pub const ELEMENTS_KEY: &str = "of";

/// The key the named-block form holds its block name under.
pub const BLOCK_KEY: &str = "block";

/// The key the named-block form holds its property constraints under.
pub const PROPERTIES_KEY: &str = "properties";

/// The key the named-tag form holds its tag name under.
pub const TAG_KEY: &str = "group";

/// One block state, as much of one as a connection decision needs.
///
/// The original compares the game's own interned property values by reference.
/// Comparing by value is the same answer by a safer route, so that is what this
/// does.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState {
    block: ContentId,
    properties: BTreeMap<String, String>,
}

impl BlockState {
    /// Builds a state of `block` carrying no property.
    #[must_use]
    pub fn new(block: ContentId) -> Self {
        Self {
            block,
            properties: BTreeMap::new(),
        }
    }

    /// Adds a property to this state, replacing any value already under it.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// Returns the block this state is of.
    #[must_use]
    pub const fn block(&self) -> &ContentId {
        &self.block
    }

    /// Returns the value of `name`, if this state carries that property.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    /// Returns whether the two states are of the same block.
    #[must_use]
    pub fn same_block(&self, other: &Self) -> bool {
        self.block == other.block
    }
}

/// What the parser knows about the blocks and groups a declaration may name.
///
/// The original resolves a name against the game's live registries. Ravel
/// publishes no registry view, so the vocabulary is a value handed in: the
/// host supplies the real one and a test supplies its own. Parsing is a pure
/// function of the vocabulary it is given either way.
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    blocks: BTreeMap<ContentId, BTreeSet<String>>,
    groups: BTreeSet<ContentId>,
}

impl Vocabulary {
    /// Builds a vocabulary that knows nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a block and the property names it carries.
    #[must_use]
    pub fn with_block<I, S>(mut self, name: &str, properties: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let Some(name) = qualify(name) else {
            return self;
        };
        self.blocks
            .entry(name)
            .or_default()
            .extend(properties.into_iter().map(Into::into));
        self
    }

    /// Declares a group of blocks a condition may name.
    #[must_use]
    pub fn with_group(mut self, name: &str) -> Self {
        if let Some(name) = qualify(name) {
            self.groups.insert(name);
        }
        self
    }

    /// Returns whether this vocabulary knows `block`.
    #[must_use]
    pub fn knows_block(&self, block: &ContentId) -> bool {
        self.blocks.contains_key(block)
    }

    /// Returns whether `block` carries a property called `property`.
    #[must_use]
    pub fn knows_property(&self, block: &ContentId, property: &str) -> bool {
        self.blocks
            .get(block)
            .is_some_and(|names| names.contains(property))
    }

    /// Returns whether this vocabulary knows `group`.
    #[must_use]
    pub fn knows_group(&self, group: &ContentId) -> bool {
        self.groups.contains(group)
    }
}

/// Which blocks belong to which named group, as a connection decision sees it.
#[derive(Clone, Debug, Default)]
pub struct Groups {
    members: BTreeMap<ContentId, BTreeSet<ContentId>>,
}

impl Groups {
    /// Builds an index in which no group has a member.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `block` is a member of `group`.
    #[must_use]
    pub fn with_member(mut self, group: &str, block: &str) -> Self {
        if let (Some(group), Some(block)) = (qualify(group), qualify(block)) {
            self.members.entry(group).or_default().insert(block);
        }
        self
    }

    /// Returns whether `block` is a member of `group`.
    #[must_use]
    pub fn holds(&self, group: &ContentId, block: &ContentId) -> bool {
        self.members
            .get(group)
            .is_some_and(|members| members.contains(block))
    }
}

/// What a parse refused, counted rather than discarded.
///
/// Every field is a refusal a pack author would otherwise never hear about. A
/// non-zero count is what a diagnostic reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Sub-conditions refused for nesting deeper than [`MAX_DEPTH`].
    pub too_deep: usize,
    /// Lists refused for holding more than [`MAX_ELEMENTS`] elements.
    pub too_many: usize,
    /// Property constraints dropped for naming an unknown property or a value
    /// that is not a string.
    pub dropped_properties: usize,
    /// List elements skipped for not being objects.
    pub skipped_elements: usize,
    /// Forms refused outright: an unknown form word, a malformed body, an
    /// unknown name, or an exclusive-or of the wrong arity.
    pub refused_forms: usize,
}

impl Report {
    /// Returns whether the parse refused nothing at all.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        *self == Self::default()
    }
}

/// A parsed connection condition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The neighbour's state is identical to the block's own.
    SameState,
    /// The block's own state is of the neighbour's block.
    SameBlock,
    /// The neighbour is that block, with the named properties matching.
    NamedBlock {
        /// The qualified block name.
        block: ContentId,
        /// The property constraints that survived parsing.
        properties: BTreeMap<String, String>,
    },
    /// The neighbour's block is a member of that group.
    NamedGroup(ContentId),
    /// The nested condition does not hold.
    Inverted(Box<Self>),
    /// Every listed condition holds.
    AllOf(Vec<Self>),
    /// Some listed condition holds.
    AnyOf(Vec<Self>),
    /// Exactly one of the two listed conditions holds.
    OneOfTwo(Box<Self>, Box<Self>),
    /// Nothing connects.
    ///
    /// Every degenerate input in §7's table lands here, and so does every form
    /// the language does not understand.
    Never,
}

impl Default for Condition {
    /// The default is exact state identity, which is what the *absence* of a
    /// condition key means. It is deliberately not [`Condition::Never`]: the
    /// two are the answers to different questions.
    fn default() -> Self {
        Self::SameState
    }
}

impl Condition {
    /// Reads the condition a declaration carries under its condition key.
    ///
    /// `None` is the key being absent, which is exact state identity. `Some`
    /// of anything that is not an object is never-connects, and so is `Some`
    /// of an object the language does not understand.
    #[must_use]
    pub fn from_declaration(value: Option<&JsonValue>, vocabulary: &Vocabulary) -> (Self, Report) {
        value.map_or_else(
            || (Self::SameState, Report::default()),
            |value| Self::parse(value, vocabulary),
        )
    }

    /// Reads one condition object.
    #[must_use]
    pub fn parse(value: &JsonValue, vocabulary: &Vocabulary) -> (Self, Report) {
        let mut report = Report::default();
        let parsed = parse_at(value, vocabulary, 0, &mut report);
        (parsed, report)
    }

    /// Answers whether a neighbour connects.
    ///
    /// `own` is the block's own appearance as seen from the sampled position
    /// and `neighbour` is the sample's appearance. The specification is
    /// explicit that both are appearances rather than raw states.
    #[must_use]
    pub fn connects(&self, own: &BlockState, neighbour: &BlockState, groups: &Groups) -> bool {
        self.connects_at(own, neighbour, groups, 0)
    }

    fn connects_at(
        &self,
        own: &BlockState,
        neighbour: &BlockState,
        groups: &Groups,
        depth: usize,
    ) -> bool {
        if depth >= MAX_DEPTH {
            return false;
        }
        let deeper = depth + 1;
        match self {
            Self::SameState => own == neighbour,
            Self::SameBlock => own.same_block(neighbour),
            Self::NamedBlock { block, properties } => {
                neighbour.block() == block
                    && properties
                        .iter()
                        .all(|(name, value)| neighbour.property(name) == Some(value.as_str()))
            }
            Self::NamedGroup(group) => groups.holds(group, neighbour.block()),
            Self::Inverted(inner) => !inner.connects_at(own, neighbour, groups, deeper),
            Self::AllOf(elements) => elements
                .iter()
                .all(|element| element.connects_at(own, neighbour, groups, deeper)),
            Self::AnyOf(elements) => elements
                .iter()
                .any(|element| element.connects_at(own, neighbour, groups, deeper)),
            Self::OneOfTwo(first, second) => {
                first.connects_at(own, neighbour, groups, deeper)
                    != second.connects_at(own, neighbour, groups, deeper)
            }
            Self::Never => false,
        }
    }
}

/// Reads a name into its qualified form, or refuses it.
///
/// A bare path takes [`DEFAULT_NAMESPACE`]. Anything carrying a character a
/// name may not hold, an empty half, or more than one separator, is refused.
#[must_use]
pub fn qualify(name: &str) -> Option<ContentId> {
    if name.contains(':') {
        name.parse().ok()
    } else {
        ContentId::new(DEFAULT_NAMESPACE.parse().ok()?, name).ok()
    }
}

fn parse_at(
    value: &JsonValue,
    vocabulary: &Vocabulary,
    depth: usize,
    report: &mut Report,
) -> Condition {
    if depth >= MAX_DEPTH {
        report.too_deep += 1;
        return Condition::Never;
    }
    if !value.is_object() {
        report.refused_forms += 1;
        return Condition::Never;
    }
    let Some(form) = value.get(FORM_KEY).and_then(JsonValue::as_str) else {
        report.refused_forms += 1;
        return Condition::Never;
    };
    match form {
        "same_state" => Condition::SameState,
        "same_block" => Condition::SameBlock,
        "named_block" => parse_named_block(value, vocabulary, report),
        "named_group" => parse_named_group(value, vocabulary, report),
        "inverted" => match value.get(ELEMENTS_KEY) {
            Some(body) if body.is_object() => {
                Condition::Inverted(Box::new(parse_at(body, vocabulary, depth + 1, report)))
            }
            _ => {
                report.refused_forms += 1;
                Condition::Never
            }
        },
        "all_of" => match parse_elements(value, vocabulary, depth, report) {
            Elements::Refused => Condition::Never,
            Elements::One(single) => *single,
            Elements::Many(elements) => Condition::AllOf(elements),
        },
        "any_of" => match parse_elements(value, vocabulary, depth, report) {
            Elements::Refused => Condition::Never,
            Elements::One(single) => *single,
            Elements::Many(elements) => Condition::AnyOf(elements),
        },
        "one_of_two" => match parse_elements(value, vocabulary, depth, report) {
            Elements::Many(elements) => {
                if let Ok([first, second]) = <[Condition; 2]>::try_from(elements) {
                    Condition::OneOfTwo(Box::new(first), Box::new(second))
                } else {
                    report.refused_forms += 1;
                    Condition::Never
                }
            }
            Elements::Refused => Condition::Never,
            Elements::One(_) => {
                report.refused_forms += 1;
                Condition::Never
            }
        },
        _ => {
            report.refused_forms += 1;
            Condition::Never
        }
    }
}

fn parse_named_block(value: &JsonValue, vocabulary: &Vocabulary, report: &mut Report) -> Condition {
    let named = value.get(BLOCK_KEY).and_then(JsonValue::as_str);
    let Some(block) = named
        .and_then(qualify)
        .filter(|block| vocabulary.knows_block(block))
    else {
        report.refused_forms += 1;
        return Condition::Never;
    };
    let mut properties = BTreeMap::new();
    if let Some(declared) = value.get(PROPERTIES_KEY) {
        let Some(declared) = declared.as_object() else {
            report.refused_forms += 1;
            return Condition::Never;
        };
        for (name, constraint) in declared {
            match constraint.as_str() {
                Some(constraint) if vocabulary.knows_property(&block, name) => {
                    properties.insert(name.clone(), constraint.to_owned());
                }
                // An unknown property name and a value that is not a string are
                // both dropped rather than refusing the form, which is §7's
                // stated answer for each.
                _ => report.dropped_properties += 1,
            }
        }
    }
    Condition::NamedBlock { block, properties }
}

fn parse_named_group(value: &JsonValue, vocabulary: &Vocabulary, report: &mut Report) -> Condition {
    let named = value.get(TAG_KEY).and_then(JsonValue::as_str);
    // Defect 5 of §12: the original raises out of declaration parsing here and
    // fails the whole model with it. Every sibling form answers never-connects,
    // so this one does too.
    if let Some(group) = named
        .and_then(qualify)
        .filter(|group| vocabulary.knows_group(group))
    {
        Condition::NamedGroup(group)
    } else {
        report.refused_forms += 1;
        Condition::Never
    }
}

enum Elements {
    Refused,
    One(Box<Condition>),
    Many(Vec<Condition>),
}

fn parse_elements(
    value: &JsonValue,
    vocabulary: &Vocabulary,
    depth: usize,
    report: &mut Report,
) -> Elements {
    let Some(declared) = value.get(ELEMENTS_KEY).and_then(JsonValue::as_array) else {
        report.refused_forms += 1;
        return Elements::Refused;
    };
    if declared.len() > MAX_ELEMENTS {
        report.too_many += 1;
        report.refused_forms += 1;
        return Elements::Refused;
    }
    // Non-object elements are skipped before the arity rules apply, so a
    // two-element list holding one non-object is a single-element list rather
    // than a two-element one.
    let mut parsed = Vec::new();
    for element in declared {
        if element.is_object() {
            parsed.push(parse_at(element, vocabulary, depth + 1, report));
        } else {
            report.skipped_elements += 1;
        }
    }
    match parsed.len() {
        0 => {
            report.refused_forms += 1;
            Elements::Refused
        }
        1 => Elements::One(Box::new(parsed.remove(0))),
        _ => Elements::Many(parsed),
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::json;

    use super::{
        BlockState, Condition, Groups, MAX_DEPTH, MAX_ELEMENTS, Report, Vocabulary, qualify,
    };

    fn vocabulary() -> Vocabulary {
        Vocabulary::new()
            .with_block("base:panel", ["facing", "lit"])
            .with_block("base:beam", ["axis"])
            .with_group("base:panels")
    }

    fn groups() -> Groups {
        Groups::new()
            .with_member("base:panels", "base:panel")
            .with_member("base:panels", "base:beam")
    }

    fn parse(value: &ravel_sdk::JsonValue) -> (Condition, Report) {
        Condition::parse(value, &vocabulary())
    }

    #[test]
    fn an_absent_condition_key_is_state_identity_and_a_malformed_one_is_never() {
        let (absent, report) = Condition::from_declaration(None, &vocabulary());
        assert_eq!(absent, Condition::SameState);
        assert!(report.is_clean());

        for malformed in [json!("same_state"), json!(7), json!([]), json!(null)] {
            let (parsed, report) = Condition::from_declaration(Some(&malformed), &vocabulary());
            assert_eq!(
                parsed,
                Condition::Never,
                "a condition value that is not an object never connects"
            );
            assert_eq!(report.refused_forms, 1);
        }
    }

    #[test]
    fn the_default_and_the_malformed_answer_differ_where_it_matters() {
        let panel = BlockState::new("base:panel".parse().expect("qualified block"));
        let groups = groups();
        assert!(
            Condition::SameState.connects(&panel, &panel, &groups),
            "the default connects a block to its own state"
        );
        assert!(
            !Condition::Never.connects(&panel, &panel, &groups),
            "a malformed condition connects nothing, which is the point of the asymmetry"
        );
    }

    #[test]
    fn an_unknown_or_absent_form_word_never_connects() {
        for value in [json!({ "test": "nothing_spells_this" }), json!({})] {
            let (parsed, report) = parse(&value);
            assert_eq!(parsed, Condition::Never);
            assert_eq!(report.refused_forms, 1);
        }
    }

    #[test]
    fn the_two_identity_forms_read_the_pair_they_are_given() {
        let groups = groups();
        let lit =
            BlockState::new("base:panel".parse().expect("qualified block")).with("lit", "true");
        let unlit =
            BlockState::new("base:panel".parse().expect("qualified block")).with("lit", "false");
        let beam = BlockState::new("base:beam".parse().expect("qualified block"));

        let (same_state, _) = parse(&json!({ "test": "same_state" }));
        assert!(same_state.connects(&lit, &lit.clone(), &groups));
        assert!(!same_state.connects(&lit, &unlit, &groups));

        let (same_block, _) = parse(&json!({ "test": "same_block" }));
        assert!(same_block.connects(&lit, &unlit, &groups));
        assert!(!same_block.connects(&lit, &beam, &groups));
    }

    #[test]
    fn a_named_block_matches_its_declared_properties_and_refuses_an_unknown_name() {
        let groups = groups();
        let (parsed, report) = parse(&json!({
            "test": "named_block",
            "block": "base:panel",
            "properties": { "lit": "true" },
        }));
        assert!(report.is_clean());
        assert!(parsed.connects(
            &BlockState::new("base:beam".parse().expect("qualified block")),
            &BlockState::new("base:panel".parse().expect("qualified block")).with("lit", "true"),
            &groups
        ));
        assert!(!parsed.connects(
            &BlockState::new("base:beam".parse().expect("qualified block")),
            &BlockState::new("base:panel".parse().expect("qualified block")).with("lit", "false"),
            &groups
        ));

        for unknown in ["base:nothing_declares_this", "not a name", ""] {
            let (parsed, report) = parse(&json!({ "test": "named_block", "block": unknown }));
            assert_eq!(parsed, Condition::Never, "{unknown} is not a block");
            assert_eq!(report.refused_forms, 1);
        }
    }

    #[test]
    fn a_property_constraint_that_cannot_apply_is_dropped_and_the_form_survives() {
        let (parsed, report) = parse(&json!({
            "test": "named_block",
            "block": "base:panel",
            "properties": { "lit": "true", "nothing_named_this": "x", "facing": 7 },
        }));
        assert_eq!(report.dropped_properties, 2);
        assert_eq!(report.refused_forms, 0);
        let Condition::NamedBlock { properties, .. } = &parsed else {
            panic!("the form survives its dropped constraints: {parsed:?}");
        };
        assert_eq!(properties.len(), 1);
        assert_eq!(properties.get("lit").map(String::as_str), Some("true"));
    }

    #[test]
    fn a_properties_value_that_is_not_an_object_refuses_the_whole_form() {
        let (parsed, report) = parse(&json!({
            "test": "named_block",
            "block": "base:panel",
            "properties": ["lit"],
        }));
        assert_eq!(parsed, Condition::Never);
        assert_eq!(report.refused_forms, 1);
    }

    #[test]
    fn a_named_group_that_cannot_be_read_returns_never_rather_than_raising() {
        let groups = groups();
        let (known, report) = parse(&json!({ "test": "named_group", "group": "base:panels" }));
        assert!(report.is_clean());
        assert!(known.connects(
            &BlockState::new("base:panel".parse().expect("qualified block")),
            &BlockState::new("base:beam".parse().expect("qualified block")),
            &groups
        ));

        // Defect 5 of §12. Each of these fails the whole model in the original.
        for broken in [
            json!({ "test": "named_group" }),
            json!({ "test": "named_group", "group": "not a name" }),
            json!({ "test": "named_group", "group": "base:nothing_declares_this" }),
            json!({ "test": "named_group", "group": 7 }),
        ] {
            let (parsed, report) = parse(&broken);
            assert_eq!(parsed, Condition::Never, "{broken}");
            assert_eq!(report.refused_forms, 1);
        }
    }

    #[test]
    fn an_inversion_with_a_body_that_is_not_an_object_is_never_rather_than_always() {
        let groups = groups();
        let panel = BlockState::new("base:panel".parse().expect("qualified block"));

        let (inverted, _) = parse(&json!({ "test": "inverted", "of": { "test": "same_state" } }));
        assert!(!inverted.connects(&panel, &panel, &groups));
        assert!(inverted.connects(
            &panel,
            &BlockState::new("base:beam".parse().expect("qualified block")),
            &groups,
        ));

        for broken in [
            json!({ "test": "inverted", "of": ["x"] }),
            json!({ "test": "inverted", "of": "same_state" }),
            json!({ "test": "inverted" }),
        ] {
            let (parsed, _) = parse(&broken);
            assert_eq!(
                parsed,
                Condition::Never,
                "a malformed inversion is never-connects, not the inversion of never-connects"
            );
            assert!(
                !parsed.connects(&panel, &panel, &groups),
                "which is the whole reason the distinction matters"
            );
        }
    }

    #[test]
    fn an_empty_list_is_never_and_a_single_element_list_is_that_element() {
        for word in ["all_of", "any_of"] {
            let (empty, report) = parse(&json!({ "test": word, "of": [] }));
            assert_eq!(empty, Condition::Never, "{word} of nothing");
            assert_eq!(report.refused_forms, 1);

            let (single, _) = parse(&json!({
                "test": word,
                "of": [{ "test": "same_block" }],
            }));
            assert_eq!(single, Condition::SameBlock, "{word} of one");

            let (absent, _) = parse(&json!({ "test": word }));
            assert_eq!(absent, Condition::Never, "{word} with no list at all");
        }
    }

    #[test]
    fn a_non_object_element_is_skipped_before_the_arity_rules_are_applied() {
        // Two elements, one of which is not an object, is a single-element list
        // rather than a two-element one, so the exclusive-or refuses it.
        let value = json!({
            "test": "one_of_two",
            "of": [{ "test": "same_block" }, "same_state"],
        });
        let (parsed, report) = parse(&value);
        assert_eq!(parsed, Condition::Never);
        assert_eq!(report.skipped_elements, 1);

        let value = json!({
            "test": "all_of",
            "of": [{ "test": "same_block" }, 7],
        });
        let (parsed, report) = parse(&value);
        assert_eq!(
            parsed,
            Condition::SameBlock,
            "and the same list is a single-element conjunction, which is its one element"
        );
        assert_eq!(report.skipped_elements, 1);
    }

    #[test]
    fn an_exclusive_or_of_any_arity_but_two_never_connects() {
        let groups = groups();
        let panel = BlockState::new("base:panel".parse().expect("qualified block"));
        let beam = BlockState::new("base:beam".parse().expect("qualified block"));

        let (two, _) = parse(&json!({
            "test": "one_of_two",
            "of": [{ "test": "same_state" }, { "test": "same_block" }],
        }));
        assert!(
            !two.connects(&panel, &panel, &groups),
            "both hold, so the two do not disagree"
        );
        assert!(
            two.connects(
                &BlockState::new("base:panel".parse().expect("qualified block"))
                    .with("lit", "true"),
                &panel,
                &groups
            ),
            "the block matches and the state does not, so exactly one holds"
        );
        assert!(!two.connects(&panel, &beam, &groups), "neither holds");

        for arity in [0usize, 1, 3, 4] {
            let elements: Vec<_> = (0..arity)
                .map(|_| json!({ "test": "same_block" }))
                .collect();
            let (parsed, report) = parse(&json!({ "test": "one_of_two", "of": elements }));
            assert_eq!(report.refused_forms, 1, "one refusal for arity {arity}");
            assert_eq!(
                parsed,
                Condition::Never,
                "an exclusive-or of {arity} never connects"
            );
        }
    }

    #[test]
    fn a_conjunction_and_a_disjunction_of_several_read_every_element() {
        let groups = groups();
        let panel = BlockState::new("base:panel".parse().expect("qualified block"));
        let beam = BlockState::new("base:beam".parse().expect("qualified block"));
        let elements = json!([
            { "test": "same_block" },
            { "test": "named_group", "group": "base:panels" },
            { "test": "named_block", "block": "base:panel" },
        ]);

        let (all, _) = parse(&json!({ "test": "all_of", "of": elements }));
        assert!(all.connects(&panel, &panel, &groups));
        assert!(
            !all.connects(&beam, &beam, &groups),
            "the last element fails"
        );

        let (any, _) = parse(&json!({
            "test": "any_of",
            "of": [{ "test": "same_state" }, { "test": "named_block", "block": "base:beam" }],
        }));
        assert!(any.connects(&panel, &beam, &groups), "the second holds");
        assert!(!any.connects(
            &panel,
            &BlockState::new("base:panel".parse().expect("qualified block")).with("lit", "true"),
            &groups
        ));
    }

    #[test]
    fn a_list_longer_than_the_cap_is_refused_rather_than_parsed() {
        let elements: Vec<_> = (0..=MAX_ELEMENTS)
            .map(|_| json!({ "test": "same_block" }))
            .collect();
        let (parsed, report) = parse(&json!({ "test": "all_of", "of": elements }));
        assert_eq!(parsed, Condition::Never);
        assert_eq!(report.too_many, 1);

        let elements: Vec<_> = (0..MAX_ELEMENTS)
            .map(|_| json!({ "test": "same_block" }))
            .collect();
        let (parsed, report) = parse(&json!({ "test": "all_of", "of": elements }));
        assert_eq!(report.too_many, 0, "the cap itself parses");
        assert!(matches!(parsed, Condition::AllOf(ref e) if e.len() == MAX_ELEMENTS));
    }

    #[test]
    fn a_condition_nested_past_the_depth_cap_is_refused_and_the_parse_returns_at_once() {
        // Deep enough that a parser without a cap would walk hundreds of
        // levels, and shallow enough that building and dropping the document
        // itself -- both recursive over the JSON value -- stay inside a test
        // thread's stack. The cap is what the test observes, not the depth.
        let mut value = json!({ "test": "same_block" });
        for _ in 0..512 {
            value = json!({ "test": "inverted", "of": value });
        }

        let (parsed, report) = parse(&value);

        assert!(report.too_deep >= 1, "the cap fired");
        let mut walked = &parsed;
        let mut depth = 0usize;
        while let Condition::Inverted(inner) = walked {
            walked = inner;
            depth += 1;
        }
        assert_eq!(depth, MAX_DEPTH, "the tree stops exactly at the cap");
        assert_eq!(*walked, Condition::Never);
    }

    #[test]
    fn evaluation_stops_at_the_cap_on_a_tree_that_was_not_built_by_the_parser() {
        // A `Condition` is public and constructible, so the bound is enforced
        // again where it is used. Without it this answers true; with it the
        // innermost element is never reached and the conjunction is false.
        let mut condition = Condition::SameBlock;
        for _ in 0..512 {
            condition = Condition::AllOf(vec![condition]);
        }
        let panel = BlockState::new("base:panel".parse().expect("qualified block"));

        let answer = condition.connects(&panel, &panel, &groups());

        assert!(!answer, "the cap decides, not the innermost element");
    }

    #[test]
    fn a_name_is_qualified_or_refused_and_never_half_read() {
        assert_eq!(
            qualify("panel").map(|id| id.to_string()),
            Some("base:panel".to_owned())
        );
        assert_eq!(
            qualify("other:panel").map(|id| id.to_string()),
            Some("other:panel".to_owned())
        );
        let panel = BlockState::new(qualify("panel").expect("a bare name is qualified"));
        let (named, report) = parse(&json!({ "test": "named_block", "block": "panel" }));
        assert!(report.is_clean());
        assert!(named.connects(&panel, &panel, &groups()));
        let (grouped, report) = parse(&json!({ "test": "named_group", "group": "panels" }));
        assert!(report.is_clean());
        assert!(grouped.connects(&panel, &panel, &groups()));
        for path in ["deep/panel", "panel.variant", "panel-variant"] {
            let id = qualify(path).expect("valid SDK content path");
            assert_eq!(id.namespace().as_str(), "base");
            assert_eq!(id.path(), path);
        }
        for bad in [
            "", ":", "a:", ":b", "Panel", "a:b:c", "a b", "a:b c", "a:../b", "/panel", "panel/",
        ] {
            assert_eq!(qualify(bad), None, "{bad:?} is not a name");
        }
    }
}
