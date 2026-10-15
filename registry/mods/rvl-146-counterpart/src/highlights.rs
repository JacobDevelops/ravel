//! The block highlights of §9: what is highlighted, until when, and what stops.
//!
//! An entry holds a block position, an optional face, a level key, an expiry, a
//! box, a colour and an optional predicate consulted each frame to decide whether
//! to draw. Adding an entry equal to one already held does nothing.
//!
//! # Host-blocked
//!
//! **The drawing.** This draws in world space during a render stage, and
//! `ravel_sdk::render_stages` publishes the stage name and the ordering among
//! mods while stating plainly that nothing draws at any stage. [`declare_stage`]
//! is what this package can say about where its drawing would go;
//! [`Highlights::visible`] decides what would be drawn and the drawing is the
//! host's. Nothing here is stubbed: every rule about which entries survive, which
//! are shown and when they stop is real and tested.
//!
//! # The three changes declared here, which are §13's
//!
//! - **One structure, keyed by everything its equality compares.** The original
//!   pairs a set with an expiry-ordered queue, expires the queue's head, and
//!   removes from the set whatever entry compares *equal to* that head. Its
//!   equality treats an entry with no face as equal to an entry with any face at
//!   the same position, so expiry can evict a live entry and leave an expired one
//!   drawing forever. There is one structure here and its key is the level, the
//!   position and the face, so the two cannot drift.
//! - **Expiry runs on a call rather than on a frame.** In the original nothing
//!   expires while the overlay is not rendering, and nothing is cleared when the
//!   player leaves a world, so entries survive into the next world until their
//!   stamp passes and the level test merely hides them. [`Highlights::expire`] is
//!   callable from a tick, [`Highlights::visible`] expires nothing, and
//!   [`Highlights::on_disconnect`] empties the whole set.
//! - **Time is monotonic and a lifetime is a duration.** The original compares
//!   against absolute wall-clock time, so a clock adjustment lengthens or
//!   shortens every pending highlight. [`Moment`] is a count from a monotonic
//!   source the caller supplies and an entry is added with a duration.

use std::collections::BTreeMap;
use std::fmt;

use ravel_sdk::render_stages::{RenderStage, StageDeclaration};
use ravel_sdk::{ModId, Side};

use crate::colour::Colour;

/// The most highlights held at once.
///
/// Entries arrive from a dependent on a client, and one that adds a highlight
/// per frame and never lets one expire is the case this ceiling is about. The
/// upgrade path is a larger constant.
pub const MAX_ENTRIES: usize = 4096;

/// A reading of a monotonic clock, in milliseconds.
///
/// Which clock is the caller's. What matters is that it does not go backwards
/// when the machine's wall clock is adjusted, which is the defect this replaces.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Moment(pub u64);

impl Moment {
    /// Returns this moment advanced by a number of milliseconds, saturating
    /// rather than wrapping.
    #[must_use]
    pub const fn after(self, millis: u64) -> Self {
        Self(self.0.saturating_add(millis))
    }
}

/// Which face of a block an entry is about.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Face {
    /// The face below.
    Down,
    /// The face above.
    Up,
    /// The four sides, by their index around the block.
    Side(u8),
}

/// Where an entry is: a level and a block position.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Place {
    /// The level the block is in.
    pub level: String,
    /// The block position.
    pub position: (i32, i32, i32),
}

/// What an entry is keyed by.
///
/// Everything the original's equality claims to compare, so nothing can be equal
/// to an entry it is not.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Key {
    /// Where.
    pub place: Place,
    /// Which face, when the entry is about one.
    pub face: Option<Face>,
}

/// The box an entry draws, in the block's own space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// The low corner.
    pub low: (f32, f32, f32),
    /// The high corner.
    pub high: (f32, f32, f32),
}

impl Bounds {
    /// The whole block.
    pub const BLOCK: Self = Self {
        low: (0.0, 0.0, 0.0),
        high: (1.0, 1.0, 1.0),
    };
}

/// One highlight.
#[derive(Clone, Debug)]
pub struct Entry {
    key: Key,
    expires: Moment,
    bounds: Bounds,
    colour: Colour,
    predicate: Option<fn() -> bool>,
}

impl Entry {
    /// Returns what this entry is keyed by.
    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }

    /// Returns when this entry stops.
    #[must_use]
    pub const fn expires(&self) -> Moment {
        self.expires
    }

    /// Returns the box.
    #[must_use]
    pub const fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Returns the colour.
    #[must_use]
    pub const fn colour(&self) -> Colour {
        self.colour
    }

    /// Returns whether the predicate, if there is one, allows drawing now.
    #[must_use]
    pub fn allowed(&self) -> bool {
        self.predicate.is_none_or(|predicate| predicate())
    }
}

/// Why a highlight was not added.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddRefused {
    /// An entry with this key is already held.
    AlreadyHeld,
    /// A highlight with no level, which nothing could ever draw.
    NoLevel,
    /// A lifetime of nothing, which would expire before it drew.
    NoLifetime,
    /// The set already holds [`MAX_ENTRIES`].
    Full,
}

impl fmt::Display for AddRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyHeld => formatter.write_str("an equal highlight is already held"),
            Self::NoLevel => formatter.write_str("a highlight named no level"),
            Self::NoLifetime => formatter.write_str("a highlight with no lifetime"),
            Self::Full => write!(formatter, "at most {MAX_ENTRIES} highlights are held"),
        }
    }
}

impl std::error::Error for AddRefused {}

/// Every highlight the client is holding.
#[derive(Clone, Debug, Default)]
pub struct Highlights {
    entries: BTreeMap<Key, Entry>,
}

impl Highlights {
    /// An empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Returns how many highlights are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds a highlight that stops `lifetime` milliseconds after `now`.
    ///
    /// # Errors
    ///
    /// Returns [`AddRefused::AlreadyHeld`] when an entry with this key is held,
    /// [`AddRefused::NoLevel`] for an unnamed level, [`AddRefused::NoLifetime`]
    /// for a lifetime of nothing, and [`AddRefused::Full`] past [`MAX_ENTRIES`].
    pub fn add(
        &mut self,
        key: Key,
        now: Moment,
        lifetime: u64,
        bounds: Bounds,
        colour: Colour,
        predicate: Option<fn() -> bool>,
    ) -> Result<(), AddRefused> {
        if key.place.level.is_empty() {
            return Err(AddRefused::NoLevel);
        }
        if lifetime == 0 {
            return Err(AddRefused::NoLifetime);
        }
        if self.entries.contains_key(&key) {
            return Err(AddRefused::AlreadyHeld);
        }
        if self.entries.len() >= MAX_ENTRIES {
            return Err(AddRefused::Full);
        }
        self.entries.insert(
            key.clone(),
            Entry {
                key,
                expires: now.after(lifetime),
                bounds,
                colour,
                predicate,
            },
        );
        Ok(())
    }

    /// Drops every entry whose time has passed, and returns how many went.
    ///
    /// This is callable from a tick and does not need a frame, which is the
    /// difference between an entry that stops and one that stops when somebody
    /// happens to be looking.
    pub fn expire(&mut self, now: Moment) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, entry| entry.expires > now);
        before - self.entries.len()
    }

    /// Returns what would be drawn in one level, in key order.
    ///
    /// Expires nothing: an entry whose time has passed is not shown, and
    /// removing it is [`Highlights::expire`]'s job on its own schedule.
    #[must_use]
    pub fn visible(&self, level: &str, now: Moment) -> Vec<&Entry> {
        self.entries
            .values()
            .filter(|entry| {
                entry.key.place.level == level && entry.expires > now && entry.allowed()
            })
            .collect()
    }

    /// Returns the entry held under a key.
    #[must_use]
    pub fn get(&self, key: &Key) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Forgets everything, which is what leaving a world does.
    ///
    /// **Declarable only in its trigger.** Ravel publishes no disconnect event,
    /// so the caller is the host's; the rule is here.
    pub fn on_disconnect(&mut self) {
        self.entries.clear();
    }
}

/// Declares where this package's drawing would go.
///
/// The stage and the ordering are real and Ravel decides them; the drawing at
/// the stage is what nothing publishes yet.
#[must_use]
pub fn declare_stage(owner: &ModId) -> StageDeclaration {
    let mut stages = StageDeclaration::default();
    stages.contribute(owner, RenderStage::AfterTerrain.as_str());
    stages
}

/// Returns whether the stage this package would draw at happens on a side.
#[must_use]
pub fn draws_on(side: Side) -> bool {
    RenderStage::AfterTerrain.happens_on(side)
}

#[cfg(test)]
mod tests {
    use ravel_sdk::{ModId, Side};

    use super::{
        AddRefused, Bounds, Face, Highlights, Key, MAX_ENTRIES, Moment, Place, declare_stage,
        draws_on,
    };
    use crate::colour::Colour;

    fn place(level: &str, x: i32) -> Place {
        Place {
            level: level.to_owned(),
            position: (x, 64, 0),
        }
    }

    fn key(level: &str, x: i32, face: Option<Face>) -> Key {
        Key {
            place: place(level, x),
            face,
        }
    }

    fn add(highlights: &mut Highlights, key: Key, lifetime: u64) -> Result<(), AddRefused> {
        highlights.add(key, Moment(0), lifetime, Bounds::BLOCK, Colour::WHITE, None)
    }

    #[test]
    fn an_entry_is_shown_in_its_own_level_until_its_time_passes() {
        let mut highlights = Highlights::empty();
        add(&mut highlights, key("overworld", 0, None), 100).expect("a fresh set");
        assert_eq!(highlights.visible("overworld", Moment(99)).len(), 1);
        assert!(highlights.visible("the_nether", Moment(0)).is_empty());
        assert!(highlights.visible("overworld", Moment(100)).is_empty());
    }

    #[test]
    fn an_entry_with_no_face_and_one_with_a_face_are_two_entries_at_one_position() {
        let mut highlights = Highlights::empty();
        add(&mut highlights, key("overworld", 0, None), 100).expect("a fresh set");
        add(&mut highlights, key("overworld", 0, Some(Face::Up)), 400).expect("a second key");
        assert_eq!(highlights.len(), 2);
        // The short one expires and the long one is untouched, which is the
        // drift the original's two structures produce.
        assert_eq!(highlights.expire(Moment(200)), 1);
        assert_eq!(highlights.len(), 1);
        assert!(
            highlights
                .get(&key("overworld", 0, Some(Face::Up)))
                .is_some(),
            "expiry removed the entry whose time came and no other"
        );
    }

    #[test]
    fn adding_an_entry_that_is_already_held_does_nothing() {
        let mut highlights = Highlights::empty();
        add(&mut highlights, key("overworld", 0, Some(Face::Down)), 100).expect("a fresh set");
        assert_eq!(
            add(&mut highlights, key("overworld", 0, Some(Face::Down)), 900),
            Err(AddRefused::AlreadyHeld)
        );
        assert_eq!(
            highlights
                .get(&key("overworld", 0, Some(Face::Down)))
                .map(super::Entry::expires),
            Some(Moment(100)),
            "the held entry keeps its own expiry"
        );
    }

    #[test]
    fn expiry_runs_without_a_frame_and_drawing_expires_nothing() {
        let mut highlights = Highlights::empty();
        add(&mut highlights, key("overworld", 0, None), 100).expect("a fresh set");
        assert!(highlights.visible("overworld", Moment(500)).is_empty());
        assert_eq!(highlights.len(), 1, "drawing removes nothing");
        assert_eq!(highlights.expire(Moment(500)), 1);
        assert!(highlights.is_empty());
    }

    #[test]
    fn leaving_a_world_forgets_every_highlight_rather_than_hiding_it() {
        let mut highlights = Highlights::empty();
        add(&mut highlights, key("overworld", 0, None), 10_000).expect("a fresh set");
        add(&mut highlights, key("overworld", 1, None), 10_000).expect("a second key");
        highlights.on_disconnect();
        assert!(highlights.is_empty());
        assert!(highlights.visible("overworld", Moment(0)).is_empty());
    }

    #[test]
    fn a_lifetime_is_a_duration_from_a_monotonic_reading() {
        let mut highlights = Highlights::empty();
        highlights
            .add(
                key("overworld", 0, None),
                Moment(1_000),
                50,
                Bounds::BLOCK,
                Colour::WHITE,
                None,
            )
            .expect("a fresh set");
        assert_eq!(
            highlights
                .get(&key("overworld", 0, None))
                .map(super::Entry::expires),
            Some(Moment(1_050))
        );
        assert_eq!(Moment(u64::MAX).after(10), Moment(u64::MAX));
    }

    #[test]
    fn a_predicate_that_refuses_hides_the_entry_and_keeps_it() {
        fn never() -> bool {
            false
        }
        let mut highlights = Highlights::empty();
        highlights
            .add(
                key("overworld", 0, None),
                Moment(0),
                100,
                Bounds::BLOCK,
                Colour::WHITE,
                Some(never),
            )
            .expect("a fresh set");
        assert!(highlights.visible("overworld", Moment(0)).is_empty());
        assert_eq!(highlights.len(), 1);
        assert!(
            !highlights
                .get(&key("overworld", 0, None))
                .expect("held")
                .allowed()
        );
    }

    #[test]
    fn an_unnamed_level_a_lifetime_of_nothing_and_a_full_set_are_refused() {
        let mut highlights = Highlights::empty();
        assert_eq!(
            add(&mut highlights, key("", 0, None), 10),
            Err(AddRefused::NoLevel)
        );
        assert_eq!(
            add(&mut highlights, key("overworld", 0, None), 0),
            Err(AddRefused::NoLifetime)
        );
        for x in 0..MAX_ENTRIES {
            add(
                &mut highlights,
                key(
                    "overworld",
                    i32::try_from(x).expect("inside the ceiling"),
                    None,
                ),
                10,
            )
            .expect("inside the ceiling");
        }
        assert_eq!(
            add(&mut highlights, key("overworld", -1, None), 10),
            Err(AddRefused::Full)
        );
        assert_eq!(highlights.len(), MAX_ENTRIES);
    }

    #[test]
    fn an_entry_carries_the_box_and_the_colour_it_was_added_with() {
        let mut highlights = Highlights::empty();
        highlights
            .add(
                key("overworld", 0, None),
                Moment(0),
                100,
                Bounds::BLOCK,
                Colour::from_bytes(1, 2, 3, 4),
                None,
            )
            .expect("a fresh set");
        let entry = highlights.get(&key("overworld", 0, None)).expect("held");
        assert_eq!(entry.bounds(), Bounds::BLOCK);
        assert_eq!(entry.colour(), Colour::from_bytes(1, 2, 3, 4));
        assert_eq!(entry.key(), &key("overworld", 0, None));
    }

    #[test]
    fn the_stage_this_would_draw_at_is_declared_and_is_the_clients_alone() {
        let owner: ModId = "rvl_146_counterpart"
            .parse()
            .expect("the declared identity is a legal mod id");
        let order = declare_stage(&owner)
            .resolve()
            .expect("one contribution has no cycle");
        assert_eq!(order.at("after_terrain"), [owner]);
        assert!(draws_on(Side::Client));
        assert!(!draws_on(Side::DedicatedServer));
        assert!(!draws_on(Side::IntegratedServer));
    }
}
