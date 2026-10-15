//! The item-level data kinds, the four payloads, and the index everything that
//! answers "may this item go in here" consults.
//!
//! The cache is the piece with real consequences. It is keyed by concrete item,
//! so an ingredient that answers a predicate rather than enumerating a set is
//! reachable only through the items it does enumerate. On a client the cache is
//! only ever what a payload delivered, and a client that never received one
//! accepts nothing.
//!
//! Ravel publishes payload declaration, so the four contracts here are real.
//! What is not reachable is the pair of signals that drive them: nothing public
//! says "a server data load finished" or "the tags were updated".

use std::collections::{BTreeMap, BTreeSet};

use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{
    Direction, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
};
use ravel_sdk::prelude::*;

use crate::recipes::{EightStation, Ingredient};

/// The version the channel is declared at.
pub const CHANNEL_VERSION: u32 = 1;

/// The cap on the experience an item-level data value holds.
pub const EXPERIENCE_CAP: u32 = 1200;

/// The colour a lookup falls back to.
pub const FALLBACK_COLOUR: u32 = 0x00ff_ffff;

/// The five item-level data kinds an item carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemData {
    /// Whether the wetting tool is running.
    pub running: bool,
    /// The slotted modifiers on a tool or an armour piece.
    pub slotted: Vec<crate::machines::Slotted>,
    /// The stored experience, capped.
    pub experience: u32,
    /// The charge the container holds, as a type and an amount.
    pub charge: Option<(String, f64)>,
    /// The area offset, horizontal and vertical.
    pub offset: (i32, i32),
}

impl ItemData {
    /// Adds experience, holding the cap.
    pub const fn add_experience(&mut self, amount: u32) {
        self.experience = self.experience.saturating_add(amount);
        if self.experience > EXPERIENCE_CAP {
            self.experience = EXPERIENCE_CAP;
        }
    }

    /// Removes experience, never below nothing.
    pub const fn remove_experience(&mut self, amount: u32) {
        self.experience = self.experience.saturating_sub(amount);
    }

    /// Applies a nudge to the area offset.
    ///
    /// The nudge is added and the result clamped to plus or minus the largest
    /// range any area modifier on the item declares, so a client cannot nudge
    /// an offset past what the item can reach.
    pub fn nudge(&mut self, horizontal: i32, vertical: i32, largest: i32) {
        let largest = largest.abs();
        self.offset.0 = (self.offset.0 + horizontal).clamp(-largest, largest);
        self.offset.1 = (self.offset.1 + vertical).clamp(-largest, largest);
    }

    /// Clears the area offset, as removing a modifier does.
    pub const fn clear_offset(&mut self) {
        self.offset = (0, 0);
    }
}

/// The index from an item to what accepts it.
///
/// Rebuilt on the server for a server data load alone, then pushed to a joining
/// player or to every player on a reload. A client-side tag update rebuilds
/// nothing, so a single-player world relies on the same payload path a
/// dedicated one does.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cache {
    accepts: BTreeMap<String, Vec<String>>,
    materials: BTreeSet<String>,
}

impl Cache {
    /// An empty cache, which is what a client holds before a payload arrives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Indexes one recipe's ingredients under the items they enumerate.
    ///
    /// `enumerates` answers the entries a simple ingredient names. A non-simple
    /// ingredient enumerates nothing, which is exactly why it is unreachable
    /// from a machine's sided insertion predicate.
    pub fn index(
        &mut self,
        recipe: &str,
        ingredients: &[Ingredient],
        enumerates: &dyn Fn(&Ingredient) -> Vec<String>,
    ) {
        for ingredient in ingredients {
            for entry in enumerates(ingredient) {
                let held = self.accepts.entry(entry).or_default();
                if !held.iter().any(|name| name == recipe) {
                    held.push(recipe.to_owned());
                }
            }
        }
    }

    /// Records the declared materials of an eight-station ritual.
    pub fn index_materials(&mut self, recipe: &EightStation) {
        for material in &recipe.materials {
            self.materials.insert(material.entry.clone());
        }
    }

    /// The recipes that accept an item.
    #[must_use]
    pub fn accepts(&self, entry: &str) -> &[String] {
        self.accepts.get(entry).map_or(&[], Vec::as_slice)
    }

    /// Whether an item appears as a declared material in any ritual.
    ///
    /// This is the reservoir's whole insertion predicate, which is why a
    /// reservoir accepts nothing until the first data load.
    #[must_use]
    pub fn is_material(&self, entry: &str) -> bool {
        self.materials.contains(entry)
    }

    /// How many items the index holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.accepts.len()
    }

    /// Whether the index holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accepts.is_empty() && self.materials.is_empty()
    }
}

/// The colours a data pack declares for items.
///
/// Cleared and rebuilt on each reload, and consulted with a fallback to white.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Colours {
    declared: BTreeMap<String, u32>,
}

impl Colours {
    /// Replaces every declared colour with a freshly loaded set.
    pub fn rebuild<I: IntoIterator<Item = (String, u32)>>(&mut self, declared: I) {
        self.declared = declared.into_iter().collect();
    }

    /// The colour declared for an item, or white.
    #[must_use]
    pub fn of(&self, entry: &str) -> u32 {
        self.declared.get(entry).copied().unwrap_or(FALLBACK_COLOUR)
    }

    /// How many colours are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.declared.len()
    }

    /// Whether nothing is declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declared.is_empty()
    }
}

/// The contract one of this entry's payloads travels under.
///
/// Fallible rather than assumed: a payload name is a content path and nothing
/// here decides on the package's behalf that a constant is a legal one.
///
/// # Errors
///
/// Returns [`ContentIdError`] when the name is not a legal content path.
pub fn contract(name: &str, direction: Direction) -> Result<PacketContract, ContentIdError> {
    let id = ContentId::new(crate::Entry::declared_id().clone(), name)?;
    Ok(PacketContract::new(
        PacketId::new(id),
        direction,
        CHANNEL_VERSION,
    ))
}

/// Writes a payload body to bytes.
pub fn to_bytes(write: &dyn Fn(&mut PacketWriter)) -> Vec<u8> {
    let mut writer = PacketWriter::default();
    write(&mut writer);
    writer.finish()
}

/// The unit payload that plays the experience-pickup sound on a client.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExperienceSound;

impl ExperienceSound {
    /// The contract this payload travels under.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] when the declared name is not a legal path.
    pub fn contract() -> Result<PacketContract, ContentIdError> {
        contract("experience_sound", Direction::ToClient)
    }

    /// Writes the body, which carries nothing.
    pub const fn write(&self, _writer: &mut PacketWriter) {}

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell this payload.
    pub const fn read(_reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self)
    }
}

/// The payload carrying the ingredient cache to a client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CachePayload {
    /// The item, and the recipes that accept it.
    pub entries: Vec<(String, Vec<String>)>,
    /// The items that appear as a declared ritual material.
    pub materials: Vec<String>,
}

impl CachePayload {
    /// The contract this payload travels under.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] when the declared name is not a legal path.
    pub fn contract() -> Result<PacketContract, ContentIdError> {
        contract("ingredient_cache", Direction::ToClient)
    }

    /// Writes the body.
    pub fn write(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.entries.len()).unwrap_or(u32::MAX));
        for (entry, recipes) in &self.entries {
            writer.text(entry);
            writer.u32(u32::try_from(recipes.len()).unwrap_or(u32::MAX));
            for recipe in recipes {
                writer.text(recipe);
            }
        }
        writer.u32(u32::try_from(self.materials.len()).unwrap_or(u32::MAX));
        for material in &self.materials {
            writer.text(material);
        }
    }

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell this payload.
    pub fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let count = reader.u32()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let entry = reader.text()?.to_owned();
            let held = reader.u32()?;
            let mut recipes = Vec::new();
            for _ in 0..held {
                recipes.push(reader.text()?.to_owned());
            }
            entries.push((entry, recipes));
        }
        let count = reader.u32()?;
        let mut materials = Vec::new();
        for _ in 0..count {
            materials.push(reader.text()?.to_owned());
        }
        Ok(Self { entries, materials })
    }
}

/// The payload carrying the colour map to a client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ColourPayload {
    /// The item, and the colour declared for it.
    pub entries: Vec<(String, u32)>,
}

impl ColourPayload {
    /// The contract this payload travels under.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] when the declared name is not a legal path.
    pub fn contract() -> Result<PacketContract, ContentIdError> {
        contract("colour_map", Direction::ToClient)
    }

    /// Writes the body.
    pub fn write(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.entries.len()).unwrap_or(u32::MAX));
        for (entry, colour) in &self.entries {
            writer.text(entry);
            writer.u32(*colour);
        }
    }

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell this payload.
    pub fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let count = reader.u32()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let entry = reader.text()?.to_owned();
            entries.push((entry, reader.u32()?));
        }
        Ok(Self { entries })
    }
}

/// The nudge a client sends for the held tool's area offset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OffsetNudge {
    /// The horizontal step.
    pub horizontal: i32,
    /// The vertical step.
    pub vertical: i32,
}

impl OffsetNudge {
    /// The contract this payload travels under.
    ///
    /// # Errors
    ///
    /// Returns [`ContentIdError`] when the declared name is not a legal path.
    pub fn contract() -> Result<PacketContract, ContentIdError> {
        contract("offset_nudge", Direction::ToServer)
    }

    /// Writes the body.
    pub fn write(&self, writer: &mut PacketWriter) {
        writer.u32(self.horizontal.cast_unsigned());
        writer.u32(self.vertical.cast_unsigned());
    }

    /// Reads the body.
    ///
    /// # Errors
    ///
    /// Returns [`WireError`] when the bytes do not spell this payload.
    pub fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            horizontal: reader.u32()?.cast_signed(),
            vertical: reader.u32()?.cast_signed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvl_042_counterpart::Stack;

    fn round_trip<T, W, R>(sent: &T, write: W, read: R) -> T
    where
        W: Fn(&T, &mut PacketWriter),
        R: Fn(&mut PacketReader<'_>) -> Result<T, WireError>,
    {
        let bytes = to_bytes(&|writer| write(sent, writer));
        let mut reader = PacketReader::new(&bytes);
        let value = read(&mut reader).expect("what was written reads back");
        reader.finish().expect("nothing is left over");
        value
    }

    fn ritual() -> EightStation {
        EightStation {
            id: "test:ritual".to_owned(),
            altar: Ingredient::Entry("a:altar".to_owned()),
            ring: vec![Ingredient::Entry("a:one".to_owned())],
            materials: vec![Stack::new("a:fuel", 4)],
            result: Stack::new("a:result", 1),
        }
    }

    fn enumerates(ingredient: &Ingredient) -> Vec<String> {
        match ingredient {
            Ingredient::Entry(entry) => vec![entry.clone()],
            Ingredient::Tag(_) => vec!["a:tagged".to_owned()],
            Ingredient::TillingTool
            | Ingredient::ChargedContainer
            | Ingredient::CropComponent { .. } => Vec::new(),
        }
    }

    #[test]
    fn a_client_that_never_received_a_payload_accepts_nothing() {
        let cache = Cache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.accepts("a:anything").len(), 0);
        assert!(!cache.is_material("a:fuel"));
    }

    #[test]
    fn the_index_is_keyed_by_concrete_item_and_a_predicate_reaches_nothing() {
        let mut cache = Cache::new();
        cache.index(
            "test:recipe",
            &[
                Ingredient::Entry("a:one".to_owned()),
                Ingredient::ChargedContainer,
            ],
            &enumerates,
        );
        assert_eq!(cache.accepts("a:one"), ["test:recipe"]);
        assert!(
            cache.accepts("a:container").is_empty(),
            "a non-simple ingredient enumerates nothing to key on"
        );
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn one_recipe_is_recorded_once_under_an_item_it_names_twice() {
        let mut cache = Cache::new();
        let twice = [
            Ingredient::Entry("a:one".to_owned()),
            Ingredient::Entry("a:one".to_owned()),
        ];
        cache.index("test:recipe", &twice, &enumerates);
        assert_eq!(cache.accepts("a:one"), ["test:recipe"]);
    }

    #[test]
    fn the_reservoir_predicate_is_empty_until_a_ritual_is_indexed() {
        let mut cache = Cache::new();
        assert!(!cache.is_material("a:fuel"));
        cache.index_materials(&ritual());
        assert!(cache.is_material("a:fuel"));
        assert!(!cache.is_material("a:dust"));
    }

    #[test]
    fn the_colour_map_falls_back_to_white_and_is_rebuilt_whole() {
        let mut colours = Colours::default();
        assert!(colours.is_empty());
        assert_eq!(colours.of("a:one"), FALLBACK_COLOUR);

        colours.rebuild([("a:one".to_owned(), 0x00ff_0000)]);
        assert_eq!(colours.of("a:one"), 0x00ff_0000);
        assert_eq!(colours.len(), 1);

        colours.rebuild([("a:two".to_owned(), 1)]);
        assert_eq!(
            colours.of("a:one"),
            FALLBACK_COLOUR,
            "a reload clears what it does not redeclare"
        );
    }

    #[test]
    fn experience_holds_its_cap_and_never_falls_below_nothing() {
        let mut data = ItemData::default();
        data.add_experience(1000);
        data.add_experience(1000);
        assert_eq!(data.experience, EXPERIENCE_CAP);
        data.remove_experience(200);
        assert_eq!(data.experience, 1000);
        data.remove_experience(5000);
        assert_eq!(data.experience, 0);
    }

    #[test]
    fn a_nudge_is_added_and_clamped_to_the_largest_declared_range() {
        let mut data = ItemData::default();
        data.nudge(2, 1, 3);
        assert_eq!(data.offset, (2, 1));
        data.nudge(5, 5, 3);
        assert_eq!(data.offset, (3, 3));
        data.nudge(-100, -100, 3);
        assert_eq!(data.offset, (-3, -3));
        data.nudge(1, 1, 0);
        assert_eq!(
            data.offset,
            (0, 0),
            "an item with no area modifier reaches nothing"
        );
    }

    #[test]
    fn removing_a_modifier_clears_the_offset() {
        let mut data = ItemData::default();
        data.nudge(2, 2, 4);
        data.clear_offset();
        assert_eq!(data.offset, (0, 0));
    }

    #[test]
    fn the_four_payloads_are_declared_on_one_channel_at_one_version() {
        let contracts = [
            ExperienceSound::contract().expect("a legal payload name"),
            CachePayload::contract().expect("a legal payload name"),
            ColourPayload::contract().expect("a legal payload name"),
            OffsetNudge::contract().expect("a legal payload name"),
        ];
        for held in &contracts {
            assert_eq!(held.version(), CHANNEL_VERSION);
            assert_eq!(held.id().owner().as_str(), crate::MOD_ID);
        }
        let directions: Vec<Direction> = contracts.iter().map(PacketContract::direction).collect();
        assert_eq!(
            directions,
            [
                Direction::ToClient,
                Direction::ToClient,
                Direction::ToClient,
                Direction::ToServer,
            ]
        );
        assert!(contract("", Direction::ToClient).is_err());
    }

    #[test]
    fn the_cache_payload_round_trips_through_the_wire() {
        let sent = CachePayload {
            entries: vec![
                ("a:one".to_owned(), vec!["test:recipe".to_owned()]),
                ("a:two".to_owned(), Vec::new()),
            ],
            materials: vec!["a:fuel".to_owned()],
        };
        let read = round_trip(&sent, CachePayload::write, CachePayload::read);
        assert_eq!(read, sent);
    }

    #[test]
    fn the_colour_payload_and_the_unit_payload_round_trip() {
        let sent = ColourPayload {
            entries: vec![("a:one".to_owned(), 0x00ab_cdef)],
        };
        let read = round_trip(&sent, ColourPayload::write, ColourPayload::read);
        assert_eq!(read, sent);

        let unit = round_trip(
            &ExperienceSound,
            ExperienceSound::write,
            ExperienceSound::read,
        );
        assert_eq!(unit, ExperienceSound);
    }

    #[test]
    fn a_nudge_round_trips_in_both_directions_of_sign() {
        for sent in [
            OffsetNudge {
                horizontal: 1,
                vertical: -1,
            },
            OffsetNudge {
                horizontal: -7,
                vertical: 7,
            },
        ] {
            let read = round_trip(&sent, OffsetNudge::write, OffsetNudge::read);
            assert_eq!(read, sent);
        }
    }
}
