//! The service that treats a run of touching blocks as one thing, from §11.
//!
//! Each participating block carries a group id and this owns the groups. Ids
//! come from a counter that only increases, and the counter is saved beside the
//! groups so ids stay unique across a restart. There is exactly one place an id
//! is reused, and it is the removal path below.
//!
//! Three things §11 asks a counterpart to settle are settled here and recorded
//! in `NOTES.md`: the compatibility test is called in **one fixed order**, the
//! re-stamp after a many-neighbour merge covers **only the groups that merged**,
//! and both walks are iterative and bounded rather than recursive and open.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use ravel_sdk::{JsonValue, json};

/// A group id.
pub type GroupId = i32;

/// The id a block carries while a removal is recomputing around it.
pub const UNSTAMPED: GroupId = -1;

/// The most positions one walk will visit.
///
/// The original's walks are recursive over neighbours with no depth bound, so a
/// sufficiently large group exhausts the stack. These are iterative, which
/// removes the stack limit, and bounded, which puts a stated one in its place.
pub const MAX_REGION: usize = 1 << 16;

/// A block position.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position {
    /// The east-west coordinate.
    pub x: i32,
    /// The vertical coordinate.
    pub y: i32,
    /// The north-south coordinate.
    pub z: i32,
}

impl Position {
    /// Builds a position.
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns the six face neighbours, in a fixed order.
    ///
    /// The order is what decides which of several new groups reuses the removed
    /// block's id, so it is part of the behaviour rather than an iteration
    /// detail. A coordinate at the edge of the width saturates rather than
    /// wrapping to the far side of the world.
    #[must_use]
    pub const fn neighbours(self) -> [Self; 6] {
        [
            Self::new(self.x, self.y.saturating_sub(1), self.z),
            Self::new(self.x, self.y.saturating_add(1), self.z),
            Self::new(self.x, self.y, self.z.saturating_sub(1)),
            Self::new(self.x, self.y, self.z.saturating_add(1)),
            Self::new(self.x.saturating_sub(1), self.y, self.z),
            Self::new(self.x.saturating_add(1), self.y, self.z),
        ]
    }
}

/// What placing a block did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Placement {
    /// Nothing touched it, or nothing it touched would take it.
    Alone {
        /// The fresh group it kept.
        group: GroupId,
    },
    /// Exactly one group touched it and took it.
    Joined {
        /// The group it joined.
        group: GroupId,
    },
    /// Several groups touched it and the compatible ones merged into its own.
    Merged {
        /// The group everything was re-stamped with.
        group: GroupId,
        /// The neighbouring groups that merged, in ascending order.
        absorbed: Vec<GroupId>,
        /// The neighbouring groups the test refused.
        refused: Vec<GroupId>,
    },
}

/// The groups, the counter, and the stamp every participating block carries.
#[derive(Clone, Debug, Default)]
pub struct Grouping {
    stamps: BTreeMap<Position, GroupId>,
    last_issued: GroupId,
}

impl Grouping {
    /// Builds a grouping holding nothing and having issued nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stamps: BTreeMap::new(),
            last_issued: 0,
        }
    }

    /// Returns the group a position carries.
    #[must_use]
    pub fn group_of(&self, position: Position) -> Option<GroupId> {
        self.stamps.get(&position).copied()
    }

    /// Returns the last id issued.
    #[must_use]
    pub const fn last_issued(&self) -> GroupId {
        self.last_issued
    }

    /// Returns the positions a group holds, in position order.
    #[must_use]
    pub fn members(&self, group: GroupId) -> Vec<Position> {
        self.stamps
            .iter()
            .filter(|(_, held)| **held == group)
            .map(|(&position, _)| position)
            .collect()
    }

    /// Returns every group that holds a position, in ascending order.
    #[must_use]
    pub fn groups(&self) -> Vec<GroupId> {
        let held: BTreeSet<GroupId> = self.stamps.values().copied().collect();
        held.into_iter().collect()
    }

    /// Issues the next id.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::Exhausted`] once the counter reaches the end
    /// of its width. It only ever increases, so it has an end, and wrapping
    /// would hand two live groups one id.
    pub fn issue(&mut self) -> Result<GroupId, GroupingRefused> {
        let next = self
            .last_issued
            .checked_add(1)
            .ok_or(GroupingRefused::Exhausted)?;
        self.last_issued = next;
        Ok(next)
    }

    /// Places a block: a fresh group, then the merge over its six neighbours.
    ///
    /// `accepts` is the dependent's compatibility test and is called in one
    /// fixed order — **this block's group first, the neighbour's second** — in
    /// both the one-neighbour and the many-neighbour case. The original calls it
    /// one way round in the first and the other way round in the second, and
    /// nothing in it requires the test to be symmetric, so a test that is not
    /// answers differently depending on how many neighbours there happen to be.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::Occupied`] for a position that already carries
    /// a stamp, [`GroupingRefused::Exhausted`] when no id is left to issue, and
    /// [`GroupingRefused::TooLarge`] when the re-stamp would walk further than
    /// [`MAX_REGION`].
    pub fn place(
        &mut self,
        position: Position,
        accepts: &dyn Fn(GroupId, GroupId) -> bool,
    ) -> Result<Placement, GroupingRefused> {
        if self.stamps.contains_key(&position) {
            return Err(GroupingRefused::Occupied { position });
        }

        let mut adjacent: Vec<GroupId> = position
            .neighbours()
            .into_iter()
            .filter_map(|neighbour| self.group_of(neighbour))
            .filter(|&group| group != UNSTAMPED)
            .collect();
        adjacent.sort_unstable();
        adjacent.dedup();

        let own = self.issue()?;

        match adjacent.len() {
            0 => {
                self.stamps.insert(position, own);
                Ok(Placement::Alone { group: own })
            }
            1 => {
                let neighbour = adjacent[0];
                if accepts(own, neighbour) {
                    self.stamps.insert(position, neighbour);
                    Ok(Placement::Joined { group: neighbour })
                } else {
                    self.stamps.insert(position, own);
                    Ok(Placement::Alone { group: own })
                }
            }
            _ => {
                let (absorbed, refused): (Vec<GroupId>, Vec<GroupId>) = adjacent
                    .into_iter()
                    .partition(|&neighbour| accepts(own, neighbour));
                let merging: BTreeSet<GroupId> = absorbed.iter().copied().collect();

                // Every walk runs before a single stamp is written, so a walk
                // that refuses leaves the world exactly as it found it. The
                // block's own fresh group holds nothing but this position, so
                // walking out of each merging neighbour reaches the same region
                // walking out of the block would.
                let mut region = BTreeSet::from([position]);
                for neighbour in position.neighbours() {
                    if self
                        .group_of(neighbour)
                        .is_some_and(|group| merging.contains(&group))
                    {
                        region.extend(self.walk(neighbour, &merging, Some(position))?);
                    }
                }
                if region.len() > MAX_REGION {
                    return Err(GroupingRefused::TooLarge { start: position });
                }

                for member in region {
                    self.stamps.insert(member, own);
                }
                Ok(Placement::Merged {
                    group: own,
                    absorbed,
                    refused,
                })
            }
        }
    }

    /// Clears the region around a gap, which is the first pass of a removal.
    ///
    /// Every block reachable through each neighbour's own id is stamped
    /// [`UNSTAMPED`]. Between this and [`Grouping::reassign`] every touched
    /// block reads as belonging to no group at all, which is a state anything
    /// inspecting membership during a removal can see.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::TooLarge`] when a walk would go further than
    /// [`MAX_REGION`].
    pub fn clear_around(&mut self, gap: Position) -> Result<(), GroupingRefused> {
        let region = self.region_around(gap)?;
        for member in region {
            self.stamps.insert(member, UNSTAMPED);
        }
        Ok(())
    }

    /// Every position a removal at `gap` clears, walked and bounded.
    ///
    /// Nothing is stamped here. A walk that refuses therefore leaves the world
    /// as it was rather than half-cleared, which is the whole reason the passes
    /// compute before they commit.
    fn region_around(&self, gap: Position) -> Result<BTreeSet<Position>, GroupingRefused> {
        let mut region = BTreeSet::new();
        for neighbour in gap.neighbours() {
            let Some(group) = self.group_of(neighbour) else {
                continue;
            };
            if group == UNSTAMPED {
                continue;
            }
            region.extend(self.walk(neighbour, &BTreeSet::from([group]), Some(gap))?);
        }
        if region.len() > MAX_REGION {
            return Err(GroupingRefused::TooLarge { start: gap });
        }
        Ok(region)
    }

    /// Reassigns the cleared region, which is the second pass of a removal.
    ///
    /// Each neighbour still unstamped starts a fresh group over everything
    /// reachable from it. The **first** such group reuses `reused`, which is the
    /// removed block's own id, and each later one takes a newly issued id. This
    /// is the only place an id is ever reused, and a reused id afterwards names
    /// a different set of blocks, so an id is not a stable handle across a
    /// removal.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::Exhausted`] when no id is left to issue and
    /// [`GroupingRefused::TooLarge`] when a walk would go too far.
    pub fn reassign(
        &mut self,
        gap: Position,
        reused: GroupId,
    ) -> Result<Vec<(GroupId, Vec<Position>)>, GroupingRefused> {
        let mut cleared = BTreeSet::new();
        for neighbour in gap.neighbours() {
            if self.group_of(neighbour) != Some(UNSTAMPED) {
                continue;
            }
            cleared.extend(self.walk(neighbour, &BTreeSet::from([UNSTAMPED]), Some(gap))?);
        }
        if cleared.len() > MAX_REGION {
            return Err(GroupingRefused::TooLarge { start: gap });
        }
        self.commit(gap, &cleared, reused)
    }

    /// Removes a block and recomputes membership around the gap it left.
    ///
    /// The result is the list of pairs the dependent's fixer owns: an id, and
    /// the positions that now carry it. Dividing the original group's contents
    /// among them is the dependent's business and not this service's.
    ///
    /// Both passes are computed before either is committed, so a removal that
    /// refuses leaves every stamp as it was. The intermediate state
    /// [`Grouping::clear_around`] produces is reachable only by driving the two
    /// passes by hand.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::Absent`] for a position carrying no stamp, and
    /// whatever the two passes refuse.
    pub fn remove(
        &mut self,
        position: Position,
    ) -> Result<Vec<(GroupId, Vec<Position>)>, GroupingRefused> {
        let own = *self
            .stamps
            .get(&position)
            .ok_or(GroupingRefused::Absent { position })?;
        let cleared = self.region_around(position)?;
        let divided = self.commit(position, &cleared, own)?;
        self.stamps.remove(&position);
        Ok(divided)
    }

    /// Divides `cleared` into the regions reachable from each neighbour of
    /// `gap`, issues every id, and only then stamps.
    fn commit(
        &mut self,
        gap: Position,
        cleared: &BTreeSet<Position>,
        reused: GroupId,
    ) -> Result<Vec<(GroupId, Vec<Position>)>, GroupingRefused> {
        let regions = divided(gap, cleared);
        let mut ids = Vec::with_capacity(regions.len());
        for index in 0..regions.len() {
            ids.push(if index == 0 { reused } else { self.issue()? });
        }

        let mut divided = Vec::with_capacity(regions.len());
        for (id, region) in ids.into_iter().zip(regions) {
            let members: Vec<Position> = region.into_iter().collect();
            for &member in &members {
                self.stamps.insert(member, id);
            }
            divided.push((id, members));
        }
        Ok(divided)
    }

    /// Walks out from `start` through every position carrying one of `through`.
    ///
    /// `skip` is treated as carrying no group at all, which is how a removal
    /// walks around the gap it is about to leave without having to stamp
    /// anything first.
    fn walk(
        &self,
        start: Position,
        through: &BTreeSet<GroupId>,
        skip: Option<Position>,
    ) -> Result<BTreeSet<Position>, GroupingRefused> {
        let reaches = |position: Position| {
            skip != Some(position)
                && self
                    .group_of(position)
                    .is_some_and(|group| through.contains(&group))
        };
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        if reaches(start) {
            queue.push_back(start);
            visited.insert(start);
        }
        while let Some(here) = queue.pop_front() {
            if visited.len() > MAX_REGION {
                return Err(GroupingRefused::TooLarge { start });
            }
            for neighbour in here.neighbours() {
                if visited.contains(&neighbour) {
                    continue;
                }
                if reaches(neighbour) {
                    visited.insert(neighbour);
                    queue.push_back(neighbour);
                }
            }
        }
        Ok(visited)
    }

    /// Writes the groups and the counter in their saved form.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::Saved`] when any block still carries
    /// [`UNSTAMPED`], which is the state between the two passes of a removal.
    /// [`Grouping::from_saved`] refuses that id, so writing it would produce a
    /// save this module could not read back.
    pub fn to_saved(&self) -> Result<JsonValue, GroupingRefused> {
        if self.stamps.values().any(|&group| group == UNSTAMPED) {
            return Err(GroupingRefused::Saved { field: "id" });
        }
        let mut groups = Vec::new();
        for group in self.groups() {
            let members: Vec<JsonValue> = self
                .members(group)
                .into_iter()
                .map(|position| json!([position.x, position.y, position.z]))
                .collect();
            groups.push(json!({ "id": group, "members": members }));
        }
        Ok(json!({ "lastIssued": self.last_issued, "groups": groups }))
    }

    /// Reads the groups and the counter back.
    ///
    /// # Errors
    ///
    /// Returns [`GroupingRefused::Saved`] when a field is missing or the wrong
    /// shape, when a position is not three numbers, or when the saved counter is
    /// below an id the save itself uses. That last check is what keeps the "only
    /// increases, never reuses" rule true across a restart: a counter behind the
    /// groups would reissue a live id.
    pub fn from_saved(saved: &JsonValue) -> Result<Self, GroupingRefused> {
        let last_issued = saved
            .get("lastIssued")
            .and_then(JsonValue::as_i64)
            .and_then(|value| GroupId::try_from(value).ok())
            .ok_or(GroupingRefused::Saved {
                field: "lastIssued",
            })?;
        let groups = saved
            .get("groups")
            .and_then(JsonValue::as_array)
            .ok_or(GroupingRefused::Saved { field: "groups" })?;

        let mut stamps = BTreeMap::new();
        for group in groups {
            let id = group
                .get("id")
                .and_then(JsonValue::as_i64)
                .and_then(|value| GroupId::try_from(value).ok())
                .ok_or(GroupingRefused::Saved { field: "id" })?;
            if id > last_issued || id == UNSTAMPED {
                return Err(GroupingRefused::Saved { field: "id" });
            }
            let members = group
                .get("members")
                .and_then(JsonValue::as_array)
                .ok_or(GroupingRefused::Saved { field: "members" })?;
            if members.len() > MAX_REGION {
                return Err(GroupingRefused::Saved { field: "members" });
            }
            for member in members {
                let coordinates = member
                    .as_array()
                    .filter(|written| written.len() == 3)
                    .ok_or(GroupingRefused::Saved { field: "members" })?;
                let mut read = [0_i32; 3];
                for (slot, written) in read.iter_mut().zip(coordinates) {
                    *slot = written
                        .as_i64()
                        .and_then(|value| i32::try_from(value).ok())
                        .ok_or(GroupingRefused::Saved { field: "members" })?;
                }
                if stamps
                    .insert(Position::new(read[0], read[1], read[2]), id)
                    .is_some()
                {
                    return Err(GroupingRefused::Saved { field: "members" });
                }
            }
        }
        Ok(Self {
            stamps,
            last_issued,
        })
    }
}

/// Divides `cleared` into the regions reachable from each neighbour of `gap`.
///
/// What touches what is decided by membership of `cleared` alone, so this reads
/// no stamp and writes none: the caller commits the answer or discards it. The
/// neighbour order of [`Position::neighbours`] decides which region comes
/// first, which is what decides which one reuses the removed block's id.
fn divided(gap: Position, cleared: &BTreeSet<Position>) -> Vec<BTreeSet<Position>> {
    let mut regions = Vec::new();
    let mut seen: BTreeSet<Position> = BTreeSet::new();
    for neighbour in gap.neighbours() {
        if !cleared.contains(&neighbour) || seen.contains(&neighbour) {
            continue;
        }
        let mut region = BTreeSet::from([neighbour]);
        let mut queue = VecDeque::from([neighbour]);
        while let Some(here) = queue.pop_front() {
            for next in here.neighbours() {
                if cleared.contains(&next) && region.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        seen.extend(region.iter().copied());
        regions.push(region);
    }
    regions
}

/// Why the grouping service refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupingRefused {
    /// The position already carries a stamp.
    Occupied {
        /// Where.
        position: Position,
    },
    /// The position carries no stamp, so there is nothing to remove.
    Absent {
        /// Where.
        position: Position,
    },
    /// A walk would go further than [`MAX_REGION`].
    TooLarge {
        /// Where the walk started.
        start: Position,
    },
    /// Every id in the width has been issued.
    Exhausted,
    /// A saved field is missing or is not what it should be.
    Saved {
        /// Which field.
        field: &'static str,
    },
}

impl fmt::Display for GroupingRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Occupied { position } => {
                write!(formatter, "{position:?} already carries a stamp")
            }
            Self::Absent { position } => write!(formatter, "{position:?} carries no stamp"),
            Self::TooLarge { start } => write!(
                formatter,
                "the group reachable from {start:?} is larger than {MAX_REGION} positions"
            ),
            Self::Exhausted => formatter.write_str("every group id in the width has been issued"),
            Self::Saved { field } => {
                write!(
                    formatter,
                    "the saved `{field}` is missing or is not what it should be"
                )
            }
        }
    }
}

impl std::error::Error for GroupingRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    fn everything(_: GroupId, _: GroupId) -> bool {
        true
    }

    fn nothing(_: GroupId, _: GroupId) -> bool {
        false
    }

    fn line(length: i32) -> (Grouping, Vec<Position>) {
        let mut grouping = Grouping::new();
        let mut placed = Vec::new();
        for x in 0..length {
            let position = Position::new(x, 0, 0);
            grouping
                .place(position, &everything)
                .expect("a legal placement");
            placed.push(position);
        }
        (grouping, placed)
    }

    #[test]
    fn ids_only_increase_and_are_never_reused_by_placing() {
        let mut grouping = Grouping::new();
        let mut issued = Vec::new();
        for x in 0..5 {
            grouping
                .place(Position::new(x * 4, 0, 0), &everything)
                .expect("a legal placement");
            issued.push(grouping.last_issued());
        }
        assert_eq!(issued, [1, 2, 3, 4, 5]);
        assert_eq!(grouping.groups(), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_block_touching_nothing_keeps_the_fresh_group_it_was_stamped_with() {
        let mut grouping = Grouping::new();
        let placement = grouping
            .place(Position::new(0, 0, 0), &everything)
            .expect("a legal placement");
        assert_eq!(placement, Placement::Alone { group: 1 });
        assert_eq!(grouping.group_of(Position::new(0, 0, 0)), Some(1));
    }

    #[test]
    fn one_accepting_neighbour_takes_the_block_and_one_refusing_neighbour_does_not() {
        let mut joining = Grouping::new();
        joining
            .place(Position::new(0, 0, 0), &everything)
            .expect("legal");
        let placement = joining
            .place(Position::new(1, 0, 0), &everything)
            .expect("legal");
        assert_eq!(placement, Placement::Joined { group: 1 });
        assert_eq!(joining.group_of(Position::new(1, 0, 0)), Some(1));

        let mut refusing = Grouping::new();
        refusing
            .place(Position::new(0, 0, 0), &everything)
            .expect("legal");
        let placement = refusing
            .place(Position::new(1, 0, 0), &nothing)
            .expect("legal");
        assert_eq!(placement, Placement::Alone { group: 2 });
        assert_eq!(refusing.group_of(Position::new(1, 0, 0)), Some(2));
    }

    #[test]
    fn the_compatibility_test_is_called_in_one_fixed_order_however_many_neighbours_there_are() {
        let seen = std::cell::RefCell::new(Vec::new());
        let recording = |own: GroupId, neighbour: GroupId| {
            seen.borrow_mut().push((own, neighbour));
            true
        };

        let mut grouping = Grouping::new();
        grouping
            .place(Position::new(0, 0, 0), &recording)
            .expect("legal");
        grouping
            .place(Position::new(2, 0, 0), &recording)
            .expect("legal");
        // One neighbour: the block is 3, the neighbour is 2.
        grouping
            .place(Position::new(3, 0, 0), &recording)
            .expect("legal");
        // Several neighbours: the block is 4, the neighbours are 1 and 2.
        grouping
            .place(Position::new(1, 0, 0), &recording)
            .expect("legal");

        let calls = seen.borrow().clone();
        assert_eq!(calls[0], (3, 2), "one neighbour: own group first");
        assert!(
            calls[1..].iter().all(|&(own, _)| own == 4),
            "several neighbours: own group first there too"
        );
    }

    #[test]
    fn several_neighbours_merge_into_the_new_block_group_and_the_refused_ones_stay_out() {
        let mut grouping = Grouping::new();
        for x in [0, 2] {
            grouping
                .place(Position::new(x, 0, 0), &everything)
                .expect("legal");
        }
        let placement = grouping
            .place(Position::new(1, 0, 0), &|_, neighbour| neighbour == 1)
            .expect("legal");

        match placement {
            Placement::Merged {
                group,
                absorbed,
                refused,
            } => {
                assert_eq!(group, 3);
                assert_eq!(absorbed, [1]);
                assert_eq!(refused, [2]);
            }
            other @ (Placement::Alone { .. } | Placement::Joined { .. }) => {
                panic!("several neighbours merge: {other:?}")
            }
        }

        assert_eq!(grouping.group_of(Position::new(0, 0, 0)), Some(3));
        assert_eq!(grouping.group_of(Position::new(1, 0, 0)), Some(3));
        assert_eq!(
            grouping.group_of(Position::new(2, 0, 0)),
            Some(2),
            "a refused neighbour is not absorbed anyway"
        );
    }

    #[test]
    fn the_re_stamp_reaches_the_whole_of_every_group_that_merged() {
        let mut grouping = Grouping::new();
        for x in [0, -1, -2] {
            grouping
                .place(Position::new(x, 0, 0), &everything)
                .expect("legal");
        }
        for x in [2, 3, 4] {
            grouping
                .place(Position::new(x, 0, 0), &everything)
                .expect("legal");
        }
        grouping
            .place(Position::new(1, 0, 0), &everything)
            .expect("legal");

        assert_eq!(grouping.groups().len(), 1, "one group holds everything");
        for x in -2..=4 {
            assert_eq!(
                grouping.group_of(Position::new(x, 0, 0)),
                grouping.group_of(Position::new(0, 0, 0))
            );
        }
    }

    #[test]
    fn placing_where_a_block_already_is_is_refused() {
        let mut grouping = Grouping::new();
        let position = Position::new(0, 0, 0);
        grouping.place(position, &everything).expect("legal");
        assert_eq!(
            grouping.place(position, &everything),
            Err(GroupingRefused::Occupied { position })
        );
    }

    #[test]
    fn between_the_two_passes_of_a_removal_every_touched_block_reads_as_no_group() {
        let (mut grouping, placed) = line(5);
        let gap = placed[2];
        assert!(grouping.stamps.remove(&gap).is_some());
        grouping.clear_around(gap).expect("a walk inside the bound");

        for position in [placed[0], placed[1], placed[3], placed[4]] {
            assert_eq!(grouping.group_of(position), Some(UNSTAMPED));
        }
    }

    #[test]
    fn removing_a_block_divides_the_group_and_the_first_new_one_reuses_the_removed_id() {
        let (mut grouping, placed) = line(5);
        let original = grouping.group_of(placed[2]).expect("a stamped block");
        let issued_before = grouping.last_issued();

        let divided = grouping.remove(placed[2]).expect("a legal removal");
        assert_eq!(divided.len(), 2);
        assert_eq!(divided[0].0, original, "the first new group reuses the id");
        assert_ne!(divided[1].0, original);
        assert_eq!(divided[1].0, issued_before + 1);

        assert_eq!(divided[0].1, vec![placed[0], placed[1]]);
        assert_eq!(divided[1].1, vec![placed[3], placed[4]]);
        assert_eq!(grouping.group_of(placed[2]), None);
    }

    #[test]
    fn a_reused_id_afterwards_names_a_different_set_of_blocks() {
        let (mut grouping, placed) = line(5);
        let original = grouping.group_of(placed[2]).expect("a stamped block");
        let before = grouping.members(original);
        assert_eq!(before.len(), 5);

        grouping.remove(placed[2]).expect("a legal removal");
        let after = grouping.members(original);
        assert_eq!(after, vec![placed[0], placed[1]]);
        assert_ne!(
            before, after,
            "the id is not a stable handle across a removal"
        );
    }

    #[test]
    fn removing_an_end_block_leaves_one_group_still_carrying_the_removed_id() {
        let (mut grouping, placed) = line(3);
        let original = grouping.group_of(placed[0]).expect("a stamped block");
        let divided = grouping.remove(placed[0]).expect("a legal removal");
        assert_eq!(divided.len(), 1);
        assert_eq!(divided[0].0, original);
        assert_eq!(divided[0].1, vec![placed[1], placed[2]]);
    }

    #[test]
    fn removing_the_only_block_leaves_nothing_to_reassign() {
        let mut grouping = Grouping::new();
        let position = Position::new(0, 0, 0);
        grouping.place(position, &everything).expect("legal");
        assert_eq!(grouping.remove(position).ok(), Some(Vec::new()));
        assert_eq!(grouping.groups(), [0_i32; 0]);
    }

    #[test]
    fn removing_a_block_that_is_not_there_is_refused() {
        let mut grouping = Grouping::new();
        let position = Position::new(9, 9, 9);
        assert_eq!(
            grouping.remove(position),
            Err(GroupingRefused::Absent { position })
        );
    }

    #[test]
    fn a_walk_larger_than_the_bound_is_refused_rather_than_run() {
        let mut grouping = Grouping::new();
        grouping.last_issued = 1;
        for x in 0..=i32::try_from(MAX_REGION).expect("the bound fits") + 1 {
            grouping.stamps.insert(Position::new(x, 0, 0), 1);
        }
        assert!(matches!(
            grouping.clear_around(Position::new(-1, 0, 0)),
            Err(GroupingRefused::TooLarge { .. })
        ));
    }

    #[test]
    fn the_counter_has_an_end_rather_than_wrapping_onto_a_live_id() {
        let mut grouping = Grouping::new();
        grouping.last_issued = GroupId::MAX;
        assert_eq!(grouping.issue(), Err(GroupingRefused::Exhausted));
    }

    #[test]
    fn a_save_carries_every_group_and_the_last_id_issued_and_reads_back_whole() {
        let (grouping, placed) = line(4);
        let saved = grouping
            .to_saved()
            .expect("every block carries a real group");
        assert_eq!(saved["lastIssued"], grouping.last_issued());

        let read = Grouping::from_saved(&saved).expect("what was written reads back");
        assert_eq!(read.last_issued(), grouping.last_issued());
        for position in placed {
            assert_eq!(read.group_of(position), grouping.group_of(position));
        }
    }

    #[test]
    fn a_save_taken_between_the_two_passes_of_a_removal_is_refused_at_the_write() {
        let (mut grouping, placed) = line(5);
        let gap = placed[2];
        assert!(grouping.stamps.remove(&gap).is_some());
        grouping.clear_around(gap).expect("a walk inside the bound");

        assert_eq!(
            grouping.to_saved().err(),
            Some(GroupingRefused::Saved { field: "id" }),
            "the writer refuses what the reader could not read back"
        );
    }

    #[test]
    fn one_position_filed_under_two_group_ids_is_refused_rather_than_taking_the_last() {
        let saved = json!({
            "lastIssued": 2,
            "groups": [
                { "id": 1, "members": [[0, 0, 0]] },
                { "id": 2, "members": [[0, 0, 0]] }
            ]
        });
        assert_eq!(
            Grouping::from_saved(&saved).err(),
            Some(GroupingRefused::Saved { field: "members" })
        );
    }

    #[test]
    fn a_merge_that_walks_too_far_leaves_every_stamp_as_it_was() {
        let mut grouping = Grouping::new();
        grouping.last_issued = 2;
        let reach = i32::try_from(MAX_REGION).expect("the bound fits") + 2;
        for x in 1..=reach {
            grouping.stamps.insert(Position::new(x, 0, 0), 1);
        }
        grouping.stamps.insert(Position::new(-1, 0, 0), 2);

        let before = grouping.stamps.clone();
        let position = Position::new(0, 0, 0);
        assert!(matches!(
            grouping.place(position, &|_, _| true),
            Err(GroupingRefused::TooLarge { .. })
        ));
        assert_eq!(
            grouping.stamps, before,
            "a refused merge stamps nothing, so the retry is not refused as occupied"
        );
        assert_eq!(grouping.group_of(position), None);
    }

    #[test]
    fn a_removal_that_walks_too_far_leaves_no_block_reading_as_no_group() {
        let mut grouping = Grouping::new();
        grouping.last_issued = 1;
        let reach = i32::try_from(MAX_REGION).expect("the bound fits") + 2;
        for x in 0..=reach {
            grouping.stamps.insert(Position::new(x, 0, 0), 1);
        }
        grouping.stamps.insert(Position::new(-1, 0, 0), 1);

        let before = grouping.stamps.clone();
        assert!(matches!(
            grouping.remove(Position::new(0, 0, 0)),
            Err(GroupingRefused::TooLarge { .. })
        ));
        assert_eq!(
            grouping.stamps, before,
            "no part of the world is left stamped as belonging to nothing"
        );
        assert!(
            grouping.stamps.values().all(|&group| group != UNSTAMPED),
            "and none of it is left unstamped with no path back"
        );
    }

    #[test]
    fn a_reassignment_with_no_id_left_to_issue_stamps_nothing() {
        let (mut grouping, placed) = line(5);
        let gap = placed[2];
        let own = grouping.group_of(gap).expect("a stamped block");
        grouping.last_issued = GroupId::MAX;

        let before = grouping.stamps.clone();
        assert_eq!(grouping.remove(gap), Err(GroupingRefused::Exhausted));
        assert_eq!(
            grouping.stamps, before,
            "the two halves of the divided group keep the id they had"
        );
        assert_eq!(grouping.group_of(placed[4]), Some(own));
    }

    #[test]
    fn a_save_whose_counter_is_behind_its_own_groups_is_refused() {
        let saved = json!({
            "lastIssued": 1,
            "groups": [{ "id": 4, "members": [[0, 0, 0]] }]
        });
        assert_eq!(
            Grouping::from_saved(&saved).err(),
            Some(GroupingRefused::Saved { field: "id" })
        );
    }

    #[test]
    fn a_malformed_save_is_refused_field_by_field_rather_than_partly_read() {
        for saved in [
            json!({ "groups": [] }),
            json!({ "lastIssued": 1 }),
            json!({ "lastIssued": 1, "groups": {} }),
            json!({ "lastIssued": 1, "groups": [{ "members": [] }] }),
            json!({ "lastIssued": 1, "groups": [{ "id": 1 }] }),
            json!({ "lastIssued": 1, "groups": [{ "id": 1, "members": [[0, 0]] }] }),
            json!({ "lastIssued": 1, "groups": [{ "id": 1, "members": [[0, 0, 1e18]] }] }),
            json!({ "lastIssued": 1, "groups": [{ "id": -1, "members": [[0, 0, 0]] }] }),
        ] {
            assert!(
                Grouping::from_saved(&saved).is_err(),
                "{saved} must not read back"
            );
        }
    }
}
