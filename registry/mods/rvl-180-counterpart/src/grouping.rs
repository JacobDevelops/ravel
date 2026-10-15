//! The thirty-four shared groups, and the rule that an aggregate resolves
//! through its leaves.
//!
//! §4 of the specification is the one part of this entry that is observed rather
//! than inferred: two aggregates naming sixteen colour sub-tags each, thirty-two
//! leaves, and a requirement that the aggregate resolve *through* the leaves so
//! that a third party adding a coloured block to a leaf is picked up by the
//! aggregate.
//!
//! Ravel's own tag model has no tag-that-names-a-tag: a
//! [`ravel_sdk::content::ContentDeclaration`] puts registry keys in a group and
//! nothing else. So the nesting lives here, in [`Aggregation`], and what reaches
//! Ravel is what [`Aggregation::resolve`] computed. `NOTES.md` states the ceiling
//! that leaves: a contribution made after the declaration was built is picked up
//! by rebuilding the declaration, not by the generated document.
//!
//! Resolution is the one recursive path in this package, so it carries a depth
//! bound and a visited set. A group that names itself terminates.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use ravel_sdk::ModIdError;
use ravel_sdk::content::{ContentId, ContentIdError, ContentKind, RegistryKey};
use ravel_sdk::tags::{TagError, TagId};

use crate::colour::Dye;

/// The conventional namespace groups several mods contribute to.
pub const SHARED_NAMESPACE: &str = "c";

/// The namespace the base game's own blocks and items are named in.
pub const BASE_NAMESPACE: &str = "minecraft";

/// The path of the aggregate, in each of the two registries it exists in.
pub const AGGREGATE: &str = "wools";

/// How many groups this package contributes, counting both registries.
pub const TAG_COUNT: usize = 34;

/// The two registries the groups exist in.
pub const GROUPED_KINDS: [ContentKind; 2] = [ContentKind::Block, ContentKind::Item];

/// How many references deep [`Aggregation::resolve`] will follow.
///
/// The shape this package declares is two deep. The bound is above that so a
/// dependent may nest its own groups under a leaf, and finite so that a chain
/// of references cannot be walked without end. Raising it is a number to change;
/// removing it is not.
pub const MAX_DEPTH: usize = 8;

/// How many members one group may hold.
pub const MAX_MEMBERS: usize = 4096;

/// How many other groups one group may name.
pub const MAX_REFERENCES: usize = 64;

/// How many distinct groups one aggregation may hold.
pub const MAX_GROUPS: usize = 512;

/// Why a group could not be named or contributed to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupingError {
    /// A namespace this package spells is not a legal identity.
    Namespace(ModIdError),
    /// A path this package spells is not a legal content path.
    Path(ContentIdError),
    /// The registry has no groups.
    Untaggable(TagError),
    /// The member belongs to a registry the group does not group.
    WrongRegistry {
        /// The group that refused it.
        tag: TagId,
        /// The member it refused.
        member: RegistryKey,
    },
    /// A declared bound was reached, and nothing was recorded.
    Full {
        /// What was being added when the bound was reached.
        what: &'static str,
        /// The bound that was reached.
        bound: usize,
    },
}

impl fmt::Display for GroupingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Namespace(source) => write!(formatter, "the namespace is illegal: {source}"),
            Self::Path(source) => write!(formatter, "the path is illegal: {source}"),
            Self::Untaggable(source) => write!(formatter, "the registry has no groups: {source}"),
            Self::WrongRegistry { tag, member } => {
                write!(formatter, "{tag} does not group {member}")
            }
            Self::Full { what, bound } => {
                write!(formatter, "no more than {bound} {what} are held")
            }
        }
    }
}

impl std::error::Error for GroupingError {}

impl From<ModIdError> for GroupingError {
    fn from(source: ModIdError) -> Self {
        Self::Namespace(source)
    }
}

impl From<ContentIdError> for GroupingError {
    fn from(source: ContentIdError) -> Self {
        Self::Path(source)
    }
}

impl From<TagError> for GroupingError {
    fn from(source: TagError) -> Self {
        Self::Untaggable(source)
    }
}

fn shared(path: &str) -> Result<ContentId, GroupingError> {
    Ok(ContentId::new(SHARED_NAMESPACE.parse()?, path)?)
}

/// Names the aggregate group of `kind`.
///
/// # Errors
///
/// Returns [`GroupingError`] when `kind` has no groups, which for the two kinds
/// this package uses cannot happen.
pub fn aggregate(kind: ContentKind) -> Result<TagId, GroupingError> {
    Ok(TagId::new(kind, shared(AGGREGATE)?)?)
}

/// Names the leaf group of `kind` for one dye.
///
/// # Errors
///
/// Returns [`GroupingError`] when `kind` has no groups.
pub fn leaf(kind: ContentKind, dye: Dye) -> Result<TagId, GroupingError> {
    Ok(TagId::new(
        kind,
        shared(&format!("{AGGREGATE}/{}", dye.as_str()))?,
    )?)
}

/// Every group this package contributes, aggregates first.
///
/// # Errors
///
/// Returns [`GroupingError`] when a spelling this package holds as a constant is
/// illegal, which would be a packaging failure.
pub fn all() -> Result<Vec<TagId>, GroupingError> {
    let mut groups = Vec::with_capacity(TAG_COUNT);
    for kind in GROUPED_KINDS {
        groups.push(aggregate(kind)?);
    }
    for kind in GROUPED_KINDS {
        for dye in Dye::ALL {
            groups.push(leaf(kind, dye)?);
        }
    }
    Ok(groups)
}

/// Names the base game's entry a leaf is inferred to hold.
///
/// **Inferred, and marked as such.** §4 records that the thirty-two leaves were
/// listed and not read, and says a counterpart should read its own sixteen
/// against the artifact's rather than assume the obvious membership. This
/// package could not read them, so it ships the obvious membership: the base
/// game's dyed wool of that colour, in the registry the group groups.
///
/// # Errors
///
/// Returns [`GroupingError`] when the derived spelling is illegal, which for
/// these constants would be a packaging failure.
pub fn conventional_member(kind: ContentKind, dye: Dye) -> Result<RegistryKey, GroupingError> {
    Ok(RegistryKey::new(
        kind,
        ContentId::new(BASE_NAMESPACE.parse()?, format!("{}_wool", dye.as_str()))?,
    ))
}

/// What a group holds and what other groups it names.
///
/// This is the nested model §4 requires and Ravel's tag model does not have.
#[derive(Clone, Debug, Default)]
pub struct Aggregation {
    members: BTreeMap<TagId, Vec<RegistryKey>>,
    references: BTreeMap<TagId, Vec<TagId>>,
}

/// What resolving one group produced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Resolution {
    /// Every member reached, deduplicated and in name order.
    pub members: Vec<RegistryKey>,
    /// Whether a reference was left unfollowed because it sat below
    /// [`MAX_DEPTH`].
    pub too_deep: bool,
    /// How many references named a group already reached, so were not followed
    /// a second time.
    pub revisited: usize,
}

impl Aggregation {
    /// Builds the thirty-four groups this package contributes.
    ///
    /// Each leaf holds the inferred base-game member; each aggregate names its
    /// sixteen leaves and holds no member of its own, which is what makes a
    /// later contribution to a leaf reach the aggregate.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingError`] when a constant spelling is illegal.
    pub fn conventional() -> Result<Self, GroupingError> {
        let mut aggregation = Self::default();
        for kind in GROUPED_KINDS {
            let whole = aggregate(kind)?;
            for dye in Dye::ALL {
                let part = leaf(kind, dye)?;
                aggregation.add_member(&part, conventional_member(kind, dye)?)?;
                aggregation.refer(&whole, part)?;
            }
        }
        Ok(aggregation)
    }

    /// Puts `member` in `tag`.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingError::WrongRegistry`] when the group does not group
    /// the member's registry, and [`GroupingError::Full`] when a bound is
    /// reached. Nothing is recorded in either case.
    pub fn add_member(&mut self, tag: &TagId, member: RegistryKey) -> Result<(), GroupingError> {
        if !tag.admits(&member) {
            return Err(GroupingError::WrongRegistry {
                tag: tag.clone(),
                member,
            });
        }
        self.reserve(tag)?;
        let held = self.members.entry(tag.clone()).or_default();
        if held.len() >= MAX_MEMBERS {
            return Err(GroupingError::Full {
                what: "members of one group",
                bound: MAX_MEMBERS,
            });
        }
        if !held.contains(&member) {
            held.push(member);
        }
        Ok(())
    }

    /// Makes `tag` name `referenced`, so resolving the first reaches the second.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingError::Full`] when a bound is reached, and records
    /// nothing.
    pub fn refer(&mut self, tag: &TagId, referenced: TagId) -> Result<(), GroupingError> {
        self.reserve(tag)?;
        let named = self.references.entry(tag.clone()).or_default();
        if named.len() >= MAX_REFERENCES {
            return Err(GroupingError::Full {
                what: "references from one group",
                bound: MAX_REFERENCES,
            });
        }
        if !named.contains(&referenced) {
            named.push(referenced);
        }
        Ok(())
    }

    /// Returns the members recorded directly against `tag`.
    #[must_use]
    pub fn direct_members(&self, tag: &TagId) -> &[RegistryKey] {
        self.members.get(tag).map_or(&[], Vec::as_slice)
    }

    /// Returns the groups `tag` names.
    #[must_use]
    pub fn references(&self, tag: &TagId) -> &[TagId] {
        self.references.get(tag).map_or(&[], Vec::as_slice)
    }

    /// Returns every group this aggregation knows about, in name order.
    pub fn groups(&self) -> impl Iterator<Item = &TagId> + '_ {
        let mut seen: BTreeSet<&TagId> = self.members.keys().collect();
        seen.extend(self.references.keys());
        seen.into_iter()
    }

    /// Resolves `tag` through every group it reaches.
    ///
    /// The walk is bounded twice over: a group already reached is not entered
    /// again, so a cycle terminates, and a reference below [`MAX_DEPTH`] is left
    /// unfollowed and reported rather than followed.
    ///
    /// The walk is breadth-first, and that is load-bearing rather than a taste.
    /// A group is entered at the shallowest depth any path reaches it by, so a
    /// group that some long chain also names is not pruned at the bound while a
    /// short path to it goes unwalked. Walking depth-first dropped members that
    /// sat one reference away.
    #[must_use]
    pub fn resolve(&self, tag: &TagId) -> Resolution {
        let mut resolution = Resolution::default();
        let mut reached: BTreeSet<TagId> = BTreeSet::new();
        let mut collected: BTreeSet<RegistryKey> = BTreeSet::new();
        let mut pending: VecDeque<(TagId, usize)> = VecDeque::from([(tag.clone(), 0)]);

        while let Some((current, depth)) = pending.pop_front() {
            if !reached.insert(current.clone()) {
                resolution.revisited += 1;
                continue;
            }
            collected.extend(self.direct_members(&current).iter().cloned());
            if depth >= MAX_DEPTH {
                if !self.references(&current).is_empty() {
                    resolution.too_deep = true;
                }
                continue;
            }
            for referenced in self.references(&current) {
                pending.push_back((referenced.clone(), depth + 1));
            }
        }

        resolution.members = collected.into_iter().collect();
        resolution
    }

    fn reserve(&self, tag: &TagId) -> Result<(), GroupingError> {
        if self.members.contains_key(tag) || self.references.contains_key(tag) {
            return Ok(());
        }
        if self.groups().count() >= MAX_GROUPS {
            return Err(GroupingError::Full {
                what: "groups in one aggregation",
                bound: MAX_GROUPS,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AGGREGATE, Aggregation, GROUPED_KINDS, GroupingError, MAX_DEPTH, MAX_GROUPS, MAX_MEMBERS,
        MAX_REFERENCES, TAG_COUNT, aggregate, all, conventional_member, leaf,
    };
    use crate::colour::Dye;
    use ravel_sdk::content::{ContentId, ContentKind, RegistryKey};
    use ravel_sdk::tags::TagId;
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    fn other(path: &str) -> TagId {
        TagId::new(
            ContentKind::Block,
            ContentId::new("c".parse().expect("a legal namespace"), path)
                .expect("a legal content path"),
        )
        .expect("blocks are grouped")
    }

    #[test]
    fn thirty_four_groups_are_named_and_each_is_named_once() {
        let groups = all().expect("the constants spell legal groups");
        assert_eq!(groups.len(), TAG_COUNT);
        let distinct: BTreeSet<String> = groups.iter().map(TagId::to_string).collect();
        assert_eq!(distinct.len(), TAG_COUNT);
    }

    #[test]
    fn the_two_aggregates_sit_in_the_shared_namespace_under_one_path() {
        for kind in GROUPED_KINDS {
            let whole = aggregate(kind).expect("a legal aggregate");
            assert_eq!(whole.id().namespace().as_str(), "c");
            assert_eq!(whole.id().path(), AGGREGATE);
            assert_eq!(whole.kind(), kind);
        }
    }

    #[test]
    fn each_leaf_sits_under_the_aggregate_path_and_names_its_dye() {
        for kind in GROUPED_KINDS {
            for dye in Dye::ALL {
                let part = leaf(kind, dye).expect("a legal leaf");
                assert_eq!(part.id().path(), format!("{AGGREGATE}/{dye}"));
            }
        }
    }

    #[test]
    fn an_aggregate_resolves_through_its_leaves_and_holds_nothing_itself() {
        let aggregation = Aggregation::conventional().expect("the constants are legal");
        for kind in GROUPED_KINDS {
            let whole = aggregate(kind).expect("a legal aggregate");
            assert_eq!(
                aggregation.direct_members(&whole).len(),
                0,
                "an aggregate that lists blocks directly cannot pick up a later leaf member"
            );
            assert_eq!(aggregation.references(&whole).len(), 16);
            let resolved = aggregation.resolve(&whole);
            assert_eq!(resolved.members.len(), 16);
            assert!(!resolved.too_deep);
        }
    }

    #[test]
    fn a_third_party_member_added_to_a_leaf_reaches_the_aggregate() {
        let mut aggregation = Aggregation::conventional().expect("the constants are legal");
        let whole = aggregate(ContentKind::Block).expect("a legal aggregate");
        let part = leaf(ContentKind::Block, Dye::Cyan).expect("a legal leaf");
        let theirs = RegistryKey::new(
            ContentKind::Block,
            ContentId::new(
                "dependent".parse().expect("a legal namespace"),
                "cyan_padding",
            )
            .expect("a legal content path"),
        );

        assert_eq!(aggregation.resolve(&whole).members.len(), 16);
        aggregation
            .add_member(&part, theirs.clone())
            .expect("a block joins a block group");
        let resolved = aggregation.resolve(&whole);
        assert_eq!(resolved.members.len(), 17);
        assert!(resolved.members.contains(&theirs));
    }

    #[test]
    fn the_inferred_leaf_member_is_the_base_games_dyed_wool() {
        let member = conventional_member(ContentKind::Item, Dye::LightGray)
            .expect("the constants are legal");
        assert_eq!(member.id().namespace().as_str(), "minecraft");
        assert_eq!(member.id().path(), "light_gray_wool");
        assert_eq!(member.kind(), ContentKind::Item);
    }

    #[test]
    fn a_group_that_names_itself_terminates_rather_than_walking_forever() {
        let mut aggregation = Aggregation::default();
        let loop_head = other("looping");
        aggregation
            .refer(&loop_head, loop_head.clone())
            .expect("a group may name any group");
        aggregation
            .add_member(
                &loop_head,
                RegistryKey::new(
                    ContentKind::Block,
                    ContentId::new("dependent".parse().expect("a legal namespace"), "one")
                        .expect("a legal content path"),
                ),
            )
            .expect("a block joins a block group");

        let started = Instant::now();
        let resolved = aggregation.resolve(&loop_head);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(resolved.members.len(), 1);
        assert_eq!(resolved.revisited, 1);
    }

    #[test]
    fn a_chain_longer_than_the_bound_is_reported_rather_than_followed() {
        let mut aggregation = Aggregation::default();
        let chain: Vec<TagId> = (0..=MAX_DEPTH + 2)
            .map(|step| other(&format!("chain/{step}")))
            .collect();
        for step in 0..chain.len() - 1 {
            aggregation
                .refer(&chain[step], chain[step + 1].clone())
                .expect("a group may name any group");
        }
        let deepest = chain.last().expect("the chain is not empty");
        aggregation
            .add_member(
                deepest,
                RegistryKey::new(
                    ContentKind::Block,
                    ContentId::new("dependent".parse().expect("a legal namespace"), "deep")
                        .expect("a legal content path"),
                ),
            )
            .expect("a block joins a block group");

        let resolved = aggregation.resolve(&chain[0]);
        assert!(resolved.too_deep, "the bound has to be observable");
        assert_eq!(
            resolved.members.len(),
            0,
            "the member below the bound is not reached"
        );
    }

    #[test]
    fn a_member_one_reference_away_survives_a_long_chain_that_also_names_it() {
        let mut aggregation = Aggregation::default();
        let root = other("root");
        let middle = other("middle");
        let held = other("held");
        let member = RegistryKey::new(
            ContentKind::Block,
            ContentId::new("dependent".parse().expect("a legal namespace"), "near")
                .expect("a legal content path"),
        );

        aggregation
            .add_member(&held, member.clone())
            .expect("a block joins a block group");
        aggregation
            .refer(&middle, held)
            .expect("a group may name any group");
        aggregation
            .refer(&root, middle.clone())
            .expect("a group may name any group");

        let chain: Vec<TagId> = (0..MAX_DEPTH - 1)
            .map(|step| other(&format!("long/{step}")))
            .collect();
        aggregation
            .refer(&root, chain[0].clone())
            .expect("a group may name any group");
        for step in 0..chain.len() - 1 {
            aggregation
                .refer(&chain[step], chain[step + 1].clone())
                .expect("a group may name any group");
        }
        aggregation
            .refer(chain.last().expect("the chain is not empty"), middle)
            .expect("a group may name any group");

        let resolved = aggregation.resolve(&root);
        assert_eq!(
            resolved.members,
            vec![member],
            "a group two references from the root is not the depth bound's to prune, \
             however deep some other path also reaches it"
        );
        assert!(
            !resolved.too_deep,
            "every reference here sits inside the bound by its shortest path"
        );
        assert_eq!(resolved.revisited, 1, "the long path reaches nothing new");
    }

    #[test]
    fn a_group_refuses_a_member_of_another_registry() {
        let mut aggregation = Aggregation::default();
        let blocks = other("mixed");
        let item = RegistryKey::new(
            ContentKind::Item,
            ContentId::new("dependent".parse().expect("a legal namespace"), "one")
                .expect("a legal content path"),
        );
        assert!(matches!(
            aggregation.add_member(&blocks, item),
            Err(GroupingError::WrongRegistry { .. })
        ));
        assert_eq!(aggregation.direct_members(&blocks).len(), 0);
    }

    #[test]
    fn the_member_reference_and_group_bounds_are_all_enforced() {
        let started = Instant::now();

        let mut members = Aggregation::default();
        let target = other("bounded");
        for index in 0..MAX_MEMBERS + 32 {
            let key = RegistryKey::new(
                ContentKind::Block,
                ContentId::new(
                    "dependent".parse().expect("a legal namespace"),
                    format!("filler_{index}"),
                )
                .expect("a legal content path"),
            );
            let outcome = members.add_member(&target, key);
            if index >= MAX_MEMBERS {
                assert!(matches!(outcome, Err(GroupingError::Full { .. })));
            } else {
                outcome.expect("a member below the bound is recorded");
            }
        }
        assert_eq!(members.direct_members(&target).len(), MAX_MEMBERS);

        let mut references = Aggregation::default();
        for index in 0..MAX_REFERENCES + 8 {
            let outcome = references.refer(&target, other(&format!("named/{index}")));
            if index >= MAX_REFERENCES {
                assert!(matches!(outcome, Err(GroupingError::Full { .. })));
            } else {
                outcome.expect("a reference below the bound is recorded");
            }
        }
        assert_eq!(references.references(&target).len(), MAX_REFERENCES);

        let mut groups = Aggregation::default();
        for index in 0..MAX_GROUPS + 8 {
            let key = RegistryKey::new(
                ContentKind::Block,
                ContentId::new("dependent".parse().expect("a legal namespace"), "one")
                    .expect("a legal content path"),
            );
            let outcome = groups.add_member(&other(&format!("group/{index}")), key);
            if index >= MAX_GROUPS {
                assert!(matches!(outcome, Err(GroupingError::Full { .. })));
            } else {
                outcome.expect("a group below the bound is recorded");
            }
        }
        assert_eq!(groups.groups().count(), MAX_GROUPS);

        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
