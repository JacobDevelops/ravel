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

use ravel_sdk::content::ContentId;
use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
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
    pub charge: Option<(ContentId, f64)>,
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
    recipes_by_item: BTreeMap<ContentId, Vec<String>>,
    materials: BTreeSet<ContentId>,
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
        enumerates: &dyn Fn(&Ingredient) -> Vec<ContentId>,
    ) {
        for ingredient in ingredients {
            for entry in enumerates(ingredient) {
                let held = self.recipes_by_item.entry(entry).or_default();
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
    pub fn recipes_accepting(&self, entry: &ContentId) -> &[String] {
        self.recipes_by_item.get(entry).map_or(&[], Vec::as_slice)
    }

    /// Whether an item appears as a declared material in any ritual.
    ///
    /// This is the reservoir's whole insertion predicate, which is why a
    /// reservoir accepts nothing until the first data load.
    #[must_use]
    pub fn is_material(&self, entry: &ContentId) -> bool {
        self.materials.contains(entry)
    }

    /// How many items the index holds.
    #[must_use]
    pub fn indexed_item_count(&self) -> usize {
        self.recipes_by_item.len()
    }

    /// Whether the index holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.recipes_by_item.is_empty() && self.materials.is_empty()
    }
}

/// The colours a data pack declares for items.
///
/// Cleared and rebuilt on each reload, and consulted with a fallback to white.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Colours {
    declared: BTreeMap<ContentId, u32>,
}

impl Colours {
    /// Replaces every declared colour with a freshly loaded set.
    pub fn rebuild<I: IntoIterator<Item = (ContentId, u32)>>(&mut self, declared: I) {
        self.declared = declared.into_iter().collect();
    }

    /// The colour declared for an item, or white.
    #[must_use]
    pub fn of(&self, entry: &ContentId) -> u32 {
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

fn read_identity(reader: &mut PacketReader<'_>) -> Result<ContentId, WireError> {
    let text = reader.text()?.to_owned();
    text.parse().map_err(|_| WireError::ContentId { text })
}

fn contract(path: &str, direction: Direction) -> PacketContract {
    PacketContract::new(
        PacketId::new(
            ContentId::new(crate::Entry::declared_id().clone(), path)
                .expect("a packaged payload name is a legal content path"),
        ),
        direction,
        CHANNEL_VERSION,
    )
}

/// The unit payload that plays the experience-pickup sound on a client.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExperienceSound;

impl Packet for ExperienceSound {
    fn contract() -> PacketContract {
        contract("experience_sound", Direction::ToClient)
    }

    fn write(&self, _writer: &mut PacketWriter) {}

    fn read(_reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self)
    }
}

/// The payload carrying the ingredient cache to a client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CachePayload {
    /// The item, and the recipes that accept it.
    pub entries: Vec<(ContentId, Vec<String>)>,
    /// The items that appear as a declared ritual material.
    pub materials: Vec<ContentId>,
}

impl Packet for CachePayload {
    fn contract() -> PacketContract {
        contract("ingredient_cache", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.entries.len()).expect("entry count fits the wire length"));
        for (entry, recipes) in &self.entries {
            writer.text(&entry.to_string());
            writer.u32(u32::try_from(recipes.len()).expect("recipe count fits the wire length"));
            for recipe in recipes {
                writer.text(recipe);
            }
        }
        writer
            .u32(u32::try_from(self.materials.len()).expect("material count fits the wire length"));
        for material in &self.materials {
            writer.text(&material.to_string());
        }
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let count = reader.u32()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let entry = read_identity(reader)?;
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
            materials.push(read_identity(reader)?);
        }
        Ok(Self { entries, materials })
    }
}

/// The payload carrying the colour map to a client.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ColourPayload {
    /// The item, and the colour declared for it.
    pub entries: Vec<(ContentId, u32)>,
}

impl Packet for ColourPayload {
    fn contract() -> PacketContract {
        contract("colour_map", Direction::ToClient)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(u32::try_from(self.entries.len()).expect("colour count fits the wire length"));
        for (entry, colour) in &self.entries {
            writer.text(&entry.to_string());
            writer.u32(*colour);
        }
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        let count = reader.u32()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let entry = read_identity(reader)?;
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

impl Packet for OffsetNudge {
    fn contract() -> PacketContract {
        contract("offset_nudge", Direction::ToServer)
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(self.horizontal.cast_unsigned());
        writer.u32(self.vertical.cast_unsigned());
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
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

    fn ritual() -> EightStation {
        EightStation {
            id: "test:ritual".to_owned(),
            altar: Ingredient::Entry("a:altar".parse().expect("item id")),
            ring: vec![Ingredient::Entry("a:one".parse().expect("item id"))],
            materials: vec![Stack::new(
                "a:fuel".parse().expect("a legal item identity"),
                4,
            )],
            result: Stack::new("a:result".parse().expect("a legal item identity"), 1),
        }
    }

    fn enumerates(ingredient: &Ingredient) -> Vec<ContentId> {
        match ingredient {
            Ingredient::Entry(entry) => vec![entry.clone()],
            Ingredient::Tag(_) => vec!["a:tagged".parse().expect("item id")],
            Ingredient::TillingTool
            | Ingredient::ChargedContainer
            | Ingredient::CropComponent { .. } => Vec::new(),
        }
    }

    #[test]
    fn a_client_that_never_received_a_payload_accepts_nothing() {
        let cache = Cache::new();
        assert!(cache.is_empty());
        assert_eq!(
            cache
                .recipes_accepting(&"a:anything".parse().expect("item id"))
                .len(),
            0
        );
        assert!(!cache.is_material(&"a:fuel".parse().expect("item id")));
    }

    #[test]
    fn the_index_is_keyed_by_concrete_item_and_a_predicate_reaches_nothing() {
        let mut cache = Cache::new();
        cache.index(
            "test:recipe",
            &[
                Ingredient::Entry("a:one".parse().expect("item id")),
                Ingredient::ChargedContainer,
            ],
            &enumerates,
        );
        assert_eq!(
            cache.recipes_accepting(&"a:one".parse().expect("item id")),
            ["test:recipe"]
        );
        assert!(
            cache
                .recipes_accepting(&"a:container".parse().expect("item id"))
                .is_empty(),
            "a non-simple ingredient enumerates nothing to key on"
        );
        assert_eq!(cache.indexed_item_count(), 1);
    }

    #[test]
    fn one_recipe_is_recorded_once_under_an_item_it_names_twice() {
        let mut cache = Cache::new();
        let twice = [
            Ingredient::Entry("a:one".parse().expect("item id")),
            Ingredient::Entry("a:one".parse().expect("item id")),
        ];
        cache.index("test:recipe", &twice, &enumerates);
        assert_eq!(
            cache.recipes_accepting(&"a:one".parse().expect("item id")),
            ["test:recipe"]
        );
    }

    #[test]
    fn the_reservoir_predicate_is_empty_until_a_ritual_is_indexed() {
        let mut cache = Cache::new();
        assert!(!cache.is_material(&"a:fuel".parse().expect("item id")));
        cache.index_materials(&ritual());
        assert!(cache.is_material(&"a:fuel".parse().expect("item id")));
        assert!(!cache.is_material(&"a:dust".parse().expect("item id")));
    }

    #[test]
    fn the_colour_map_falls_back_to_white_and_is_rebuilt_whole() {
        let mut colours = Colours::default();
        assert!(colours.is_empty());
        assert_eq!(
            colours.of(&"a:one".parse().expect("item id")),
            FALLBACK_COLOUR
        );

        colours.rebuild([("a:one".parse().expect("item id"), 0x00ff_0000)]);
        assert_eq!(colours.of(&"a:one".parse().expect("item id")), 0x00ff_0000);
        assert_eq!(colours.len(), 1);

        colours.rebuild([("a:two".parse().expect("item id"), 1)]);
        assert_eq!(
            colours.of(&"a:one".parse().expect("item id")),
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
            ExperienceSound::contract(),
            CachePayload::contract(),
            ColourPayload::contract(),
            OffsetNudge::contract(),
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
    }

    #[test]
    fn cache_and_colour_payloads_refuse_invalid_item_identifiers() {
        let mut cache = PacketWriter::default();
        cache.u32(1);
        cache.text("not a qualified item");
        cache.u32(0);
        cache.u32(0);
        assert!(matches!(
            CachePayload::read(&mut PacketReader::new(&cache.finish())),
            Err(WireError::ContentId { .. })
        ));
        let mut colour = PacketWriter::default();
        colour.u32(1);
        colour.text("not a qualified item");
        colour.u32(0);
        assert!(matches!(
            ColourPayload::read(&mut PacketReader::new(&colour.finish())),
            Err(WireError::ContentId { .. })
        ));
        let mut material = PacketWriter::default();
        material.u32(0);
        material.u32(1);
        material.text("not a qualified item");
        assert!(matches!(
            CachePayload::read(&mut PacketReader::new(&material.finish())),
            Err(WireError::ContentId { .. })
        ));
    }

    #[test]
    fn the_cache_payload_round_trips_through_the_wire() {
        let sent = CachePayload {
            entries: vec![
                (
                    "a:one".parse().expect("item id"),
                    vec!["test:recipe".to_owned()],
                ),
                ("a:two".parse().expect("item id"), Vec::new()),
            ],
            materials: vec!["a:fuel".parse().expect("item id")],
        };
        let read = ravel_sdk::network::decode::<CachePayload>(&ravel_sdk::network::encode(&sent))
            .expect("the cache payload decodes");
        assert_eq!(read, sent);
    }

    #[test]
    fn the_colour_payload_and_the_unit_payload_round_trip() {
        let sent = ColourPayload {
            entries: vec![("a:one".parse().expect("item id"), 0x00ab_cdef)],
        };
        let read = ravel_sdk::network::decode::<ColourPayload>(&ravel_sdk::network::encode(&sent))
            .expect("the colour payload decodes");
        assert_eq!(read, sent);

        let unit = ravel_sdk::network::decode::<ExperienceSound>(&ravel_sdk::network::encode(
            &ExperienceSound,
        ))
        .expect("the unit payload decodes");
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
            let read =
                ravel_sdk::network::decode::<OffsetNudge>(&ravel_sdk::network::encode(&sent))
                    .expect("the nudge payload decodes");
            assert_eq!(read, sent);
        }
    }
}
