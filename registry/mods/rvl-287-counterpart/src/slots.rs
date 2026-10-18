//! Texture slots and index assignment, the specification's §10.
//!
//! Two schemes exist and they are not interchangeable. Most kinds hold one flat
//! map from slot number to texture and hand it straight to the atlas, so the
//! declaration's slot numbers *are* the runtime indices. The connected block
//! kind holds a separate texture set per face and assigns runtime indices by
//! walking the six faces in [`Face::ALL`] order, allocating a fresh sequential
//! index per entry. Its indices are meaningful only through the per-face map.
//!
//! Slot zero is the particle texture throughout, and a set with no slot zero
//! falls back to the missing-texture sprite rather than to nothing.
//!
//! # Bounds
//!
//! A declaration is untrusted input from a resource pack. [`MAX_SLOTS`] caps
//! what one texture set may hold and [`MAX_CELLS`] caps a tiled grid, so a
//! document declaring an enormous grid is refused where it is read rather than
//! allocated.

use std::collections::BTreeMap;
use std::fmt;

use ravel_sdk::JsonValue;
use ravel_sdk::content::ContentId;

use crate::conditions::qualify;
use crate::neighbourhood::Face;
use crate::quadrant::Quadrant;

/// The most entries one texture set may declare.
pub const MAX_SLOTS: usize = 32;

/// The most cells a tiled grid may declare.
pub const MAX_CELLS: usize = 256;

/// The slot number the particle texture is held under.
pub const PARTICLE_SLOT: u32 = 0;

/// The name a declaration spells the particle texture by.
pub const PARTICLE_KEY: &str = "dust";

/// The key a per-face declaration holds its shared fallback under.
pub const FALLBACK_KEY: &str = "otherwise";

/// The slot number cell zero of a tiled grid is held under.
pub const CELL_BASE: u32 = 16;

/// The key a grid declaration holds its width under.
pub const WIDTH_KEY: &str = "across";

/// The key a grid declaration holds its height under.
pub const HEIGHT_KEY: &str = "down";

/// The key a grid declaration holds its cells under.
pub const CELLS_KEY: &str = "cells";

/// The name of the sprite a missing texture falls back to.
pub const MISSING_TEXTURE: &str = "base:absent";

/// One texture, named the way a declaration names it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureId(ContentId);

impl TextureId {
    /// Reads a texture name, refusing one that is not a name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        qualify(name).map(Self)
    }

    /// Returns the sprite a missing texture falls back to.
    #[must_use]
    pub fn missing() -> Self {
        Self(
            MISSING_TEXTURE
                .parse()
                .expect("the missing texture is qualified"),
        )
    }

    /// Returns the qualified texture identity.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.0
    }
}

/// The four slots a column kind selects between, plus its cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColumnSlot {
    /// The column runs through this face in both directions.
    Continuation,
    /// The column ends at the positive end of its axis.
    PositiveEnd,
    /// The column ends at the negative end of its axis.
    NegativeEnd,
    /// The column has no neighbour along its axis.
    Standalone,
    /// The face the column's axis points through.
    Cap,
}

impl ColumnSlot {
    /// Every column slot, in the order this enum declares them.
    pub const ALL: [Self; 5] = [
        Self::Continuation,
        Self::PositiveEnd,
        Self::NegativeEnd,
        Self::Standalone,
        Self::Cap,
    ];

    /// Returns the slot number a texture map addresses this slot by.
    #[must_use]
    pub const fn slot_number(self) -> u32 {
        match self {
            Self::Continuation => 6,
            Self::PositiveEnd => 7,
            Self::NegativeEnd => 8,
            Self::Standalone => 9,
            Self::Cap => 10,
        }
    }

    /// Returns the name a declaration spells this slot by.
    #[must_use]
    pub const fn declared_name(self) -> &'static str {
        match self {
            Self::Continuation => "middle",
            Self::PositiveEnd => "upper",
            Self::NegativeEnd => "lower",
            Self::Standalone => "single",
            Self::Cap => "end",
        }
    }
}

/// The two extra slots a pane kind declares for its edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaneEdge {
    /// The edge drawn on a face the sheet's plane runs into.
    Rim,
    /// The edge drawn along the sheet's own long side.
    Flank,
}

impl PaneEdge {
    /// Every pane edge, in the order this enum declares them.
    pub const ALL: [Self; 2] = [Self::Rim, Self::Flank];

    /// Returns the slot number a texture map addresses this slot by.
    #[must_use]
    pub const fn slot_number(self) -> u32 {
        match self {
            Self::Rim => 11,
            Self::Flank => 12,
        }
    }

    /// Returns the name a declaration spells this slot by.
    #[must_use]
    pub const fn declared_name(self) -> &'static str {
        match self {
            Self::Rim => "rim",
            Self::Flank => "flank",
        }
    }
}

/// Any slot a quad may name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// The particle texture, slot zero.
    Particle,
    /// One of the five connection slots of §6.
    Quadrant(Quadrant),
    /// One of the five column slots.
    Column(ColumnSlot),
    /// One of the two pane edges.
    Edge(PaneEdge),
    /// One cell of a tiled grid.
    Cell(u32),
}

impl Slot {
    /// Returns the slot number a texture map addresses this slot by.
    #[must_use]
    pub const fn number(self) -> u32 {
        match self {
            Self::Particle => PARTICLE_SLOT,
            Self::Quadrant(quadrant) => quadrant.slot_number(),
            Self::Column(column) => column.slot_number(),
            Self::Edge(edge) => edge.slot_number(),
            Self::Cell(cell) => CELL_BASE.saturating_add(cell),
        }
    }

    /// Returns the slot a declaration spells by `name`, if any.
    ///
    /// Cells are addressed by number rather than by name, so no name reaches
    /// one.
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        if name == PARTICLE_KEY {
            return Some(Self::Particle);
        }
        if let Some(quadrant) = Quadrant::by_name(name) {
            return Some(Self::Quadrant(quadrant));
        }
        if let Some(column) = ColumnSlot::ALL
            .into_iter()
            .find(|slot| slot.declared_name() == name)
        {
            return Some(Self::Column(column));
        }
        PaneEdge::ALL
            .into_iter()
            .find(|edge| edge.declared_name() == name)
            .map(Self::Edge)
    }
}

/// Why a texture declaration was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotError {
    /// The declaration is not an object, or the whole shape is unreadable.
    Malformed,
    /// The declaration names a slot this kind does not have.
    UnknownSlot(String),
    /// A value under a slot is not a texture name.
    UnreadableTexture(String),
    /// The declaration holds more entries than [`MAX_SLOTS`].
    TooManySlots(usize),
    /// The grid holds more cells than [`MAX_CELLS`], or none at all.
    UnusableGrid(usize),
    /// The grid declares no texture for one of its cells.
    MissingCell(u32),
    /// The declaration contains more textures than its dimensions address.
    SurplusCells {
        /// Number of declared textures.
        declared: usize,
        /// Number of addressable cells.
        area: usize,
    },
}

impl fmt::Display for SlotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => {
                formatter.write_str("the texture declaration is not a readable object")
            }
            Self::UnknownSlot(slot) => write!(
                formatter,
                "the texture declaration names an unknown slot `{slot}`"
            ),
            Self::UnreadableTexture(slot) => {
                write!(formatter, "slot `{slot}` holds no readable texture name")
            }
            Self::TooManySlots(count) => write!(
                formatter,
                "the texture declaration holds {count} slots, exceeding {MAX_SLOTS}"
            ),
            Self::UnusableGrid(count) => write!(
                formatter,
                "a texture grid with {count} cells must hold between 1 and {MAX_CELLS}"
            ),
            Self::MissingCell(cell) => write!(
                formatter,
                "the texture grid declares no texture for cell {cell}"
            ),
            Self::SurplusCells { declared, area } => write!(
                formatter,
                "the texture grid declares {declared} textures for {area} cells"
            ),
        }
    }
}

impl std::error::Error for SlotError {}

/// One flat texture set: slot number to texture, handed straight to the atlas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlatSlots {
    textures: BTreeMap<u32, TextureId>,
}

impl FlatSlots {
    /// Builds an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a texture under a slot, replacing anything already there.
    #[must_use]
    pub fn with(mut self, slot: Slot, texture: TextureId) -> Self {
        self.textures.insert(slot.number(), texture);
        self
    }

    /// Returns the particle texture, or the missing sprite when none is set.
    #[must_use]
    pub fn particle(&self) -> TextureId {
        self.textures
            .get(&PARTICLE_SLOT)
            .cloned()
            .unwrap_or_else(TextureId::missing)
    }

    /// Returns the texture under `slot`, or the missing sprite.
    #[must_use]
    pub fn texture(&self, slot: Slot) -> TextureId {
        self.textures
            .get(&slot.number())
            .cloned()
            .unwrap_or_else(TextureId::missing)
    }

    /// Returns whether a texture is declared under `slot`.
    #[must_use]
    pub fn declares(&self, slot: Slot) -> bool {
        self.textures.contains_key(&slot.number())
    }

    /// Returns how many slots this set holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Returns whether this set holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// Gives each undeclared pane edge the particle texture.
    ///
    /// §10 says the two pane edges each default to the particle texture when a
    /// declaration does not name them. Filling them here rather than at lookup
    /// makes the defaulting visible in the parsed set.
    #[must_use]
    pub fn with_defaulted_edges(mut self) -> Self {
        let particle = self.particle();
        for edge in PaneEdge::ALL {
            self.textures
                .entry(Slot::Edge(edge).number())
                .or_insert_with(|| particle.clone());
        }
        self
    }

    /// Reads a flat texture object, admitting only the slots in `allowed`.
    ///
    /// # Errors
    ///
    /// Returns [`SlotError`] when the value is not an object, names a slot this
    /// kind does not have, holds a value that is not a texture name, or holds
    /// more than [`MAX_SLOTS`] entries.
    pub fn parse(value: &JsonValue, allowed: &[Slot]) -> Result<Self, SlotError> {
        let object = value.as_object().ok_or(SlotError::Malformed)?;
        if object.len() > MAX_SLOTS {
            return Err(SlotError::TooManySlots(object.len()));
        }
        let mut textures = BTreeMap::new();
        for (name, declared) in object {
            let slot = Slot::by_name(name).ok_or_else(|| SlotError::UnknownSlot(name.clone()))?;
            if slot != Slot::Particle && !allowed.contains(&slot) {
                return Err(SlotError::UnknownSlot(name.clone()));
            }
            let texture = declared
                .as_str()
                .and_then(TextureId::parse)
                .ok_or_else(|| SlotError::UnreadableTexture(name.clone()))?;
            textures.insert(slot.number(), texture);
        }
        Ok(Self { textures })
    }

    /// The five connection slots, which is what a connection kind admits.
    #[must_use]
    pub fn connection_slots() -> Vec<Slot> {
        Quadrant::ALL.into_iter().map(Slot::Quadrant).collect()
    }

    /// The five column slots.
    #[must_use]
    pub fn column_slots() -> Vec<Slot> {
        ColumnSlot::ALL.into_iter().map(Slot::Column).collect()
    }

    /// The five connection slots plus the two pane edges.
    #[must_use]
    pub fn pane_slots() -> Vec<Slot> {
        let mut slots = Self::connection_slots();
        slots.extend(PaneEdge::ALL.into_iter().map(Slot::Edge));
        slots
    }
}

/// A tiled grid: a width, a height and a texture for every cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    width: u32,
    height: u32,
    cells: Vec<TextureId>,
}

impl Grid {
    /// Builds a grid from its dimensions and its cells in row-major order.
    ///
    /// # Errors
    ///
    /// Returns [`SlotError::UnusableGrid`] when either dimension is zero or the
    /// cell count exceeds [`MAX_CELLS`], [`SlotError::MissingCell`] for a short
    /// cell list, and [`SlotError::SurplusCells`] for surplus textures.
    pub fn new(width: u32, height: u32, cells: Vec<TextureId>) -> Result<Self, SlotError> {
        Self::validate_shape(width, height, cells.len())?;
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    fn validate_shape(width: u32, height: u32, declared: usize) -> Result<(), SlotError> {
        let area = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(width, height)| width.checked_mul(height))
            .unwrap_or(0);
        if width == 0 || height == 0 || area == 0 || area > MAX_CELLS {
            return Err(SlotError::UnusableGrid(area));
        }
        if declared > area {
            return Err(SlotError::SurplusCells { declared, area });
        }
        if declared < area {
            let missing = u32::try_from(declared).expect("a missing index is below MAX_CELLS");
            return Err(SlotError::MissingCell(missing));
        }
        Ok(())
    }

    /// Returns the grid's width in cells.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the grid's height in cells.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the texture of the cell at `column` and `row`.
    ///
    /// Both are taken modulo the grid, so every pair addresses a declared cell.
    #[must_use]
    pub fn cell(&self, column: u32, row: u32) -> &TextureId {
        // `Grid::new` refuses a cell list that is not exactly `width * height`
        // long and `index` reduces both coordinates modulo the grid, so this
        // index is always inside the list.
        let index = usize::try_from(self.index(column, row)).expect("grid index fits in memory");
        &self.cells[index]
    }

    /// Reads a grid declaration.
    ///
    /// # Errors
    ///
    /// Returns [`SlotError::Malformed`] when the declaration is not an object
    /// carrying two dimensions and a cell list, and whatever [`Grid::new`]
    /// refuses otherwise.
    pub fn parse(value: &JsonValue) -> Result<Self, SlotError> {
        let dimension = |key: &str| {
            value
                .get(key)
                .and_then(JsonValue::as_u64)
                .and_then(|held| u32::try_from(held).ok())
        };
        let (Some(width), Some(height)) = (dimension(WIDTH_KEY), dimension(HEIGHT_KEY)) else {
            return Err(SlotError::Malformed);
        };
        let declared = value
            .get(CELLS_KEY)
            .and_then(JsonValue::as_array)
            .ok_or(SlotError::Malformed)?;
        Self::validate_shape(width, height, declared.len())?;
        let mut cells = Vec::with_capacity(declared.len());
        for (position, held) in declared.iter().enumerate() {
            let texture = held.as_str().and_then(TextureId::parse).ok_or_else(|| {
                SlotError::MissingCell(
                    u32::try_from(position).expect("a cell index is below MAX_CELLS"),
                )
            })?;
            cells.push(texture);
        }
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    /// Returns the texture at a cell index, or the missing sprite.
    #[must_use]
    pub fn at(&self, index: u32) -> TextureId {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.cells.get(index))
            .cloned()
            .unwrap_or_else(TextureId::missing)
    }

    /// Returns the cell index the pair reduces to.
    ///
    /// §12's defect 1 is that the original multiplies the row by the grid's
    /// *height*, which addresses wrong or undeclared cells on any grid whose
    /// width and height differ. The row is multiplied by the width here.
    #[must_use]
    pub fn index(&self, column: u32, row: u32) -> u32 {
        let column = column % self.width;
        let row = row % self.height;
        row.saturating_mul(self.width).saturating_add(column)
    }
}

/// The connected block kind's per-face texture sets, with indices assigned.
///
/// The sequential numbering is what makes an index meaningful only through the
/// per-face map: index four is whatever the fourth declared entry was, on
/// whichever face declared it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PerFaceSlots {
    sprites: Vec<TextureId>,
    per_face: [BTreeMap<u32, u32>; 6],
    sprite_of: Vec<u32>,
}

impl PerFaceSlots {
    /// Assigns runtime indices to six per-face texture sets.
    ///
    /// The faces are walked in [`Face::ALL`] order, a fresh sequential index is
    /// allocated per entry, and identical textures are deduplicated by texture
    /// identity before the indices are handed out, so one sprite may back
    /// several indices.
    #[must_use]
    pub fn assign(sets: [FlatSlots; 6]) -> Self {
        let mut sprites: Vec<TextureId> = Vec::new();
        let mut per_face: [BTreeMap<u32, u32>; 6] = std::array::from_fn(|_| BTreeMap::new());
        let mut sprite_of: Vec<u32> = Vec::new();

        for (position, set) in sets.into_iter().enumerate() {
            for (slot, texture) in set.textures {
                let sprite = sprites
                    .iter()
                    .position(|held| held == &texture)
                    .unwrap_or_else(|| {
                        sprites.push(texture);
                        sprites.len() - 1
                    });
                let index =
                    u32::try_from(sprite_of.len()).expect("runtime texture indices must fit u32");
                sprite_of.push(u32::try_from(sprite).expect("sprite indices must fit u32"));
                per_face[position].insert(slot, index);
            }
        }

        Self {
            sprites,
            per_face,
            sprite_of,
        }
    }

    /// Returns the runtime index of `slot` on `face`.
    ///
    /// A face that declared no texture set has an empty map, and every lookup
    /// against it falls back to index zero.
    #[must_use]
    pub fn index(&self, face: Face, slot: Slot) -> u32 {
        self.per_face[face.position()]
            .get(&slot.number())
            .copied()
            .unwrap_or(0)
    }

    /// Returns the sprite a runtime index reaches.
    #[must_use]
    pub fn sprite(&self, index: u32) -> TextureId {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.sprite_of.get(index))
            .and_then(|sprite| usize::try_from(*sprite).ok())
            .and_then(|sprite| self.sprites.get(sprite))
            .cloned()
            .unwrap_or_else(TextureId::missing)
    }

    /// Returns how many distinct sprites the six sets reach.
    #[must_use]
    pub fn sprite_count(&self) -> usize {
        self.sprites.len()
    }

    /// Returns how many runtime indices were handed out.
    #[must_use]
    pub fn index_count(&self) -> usize {
        self.sprite_of.len()
    }

    /// Reads a connected block declaration, which is accepted in three shapes.
    ///
    /// Two attempts are made in order. The first reads a single five-entry
    /// texture object and applies it to all six faces. The second walks the six
    /// faces, taking a face that is a single texture string as that texture,
    /// any other face as a five-entry object, and falling back to a shared
    /// object under [`FALLBACK_KEY`] for a face that declares none.
    ///
    /// A malformed first attempt falls through to the second rather than
    /// failing, which is what makes the second attempt cover two shapes.
    ///
    /// # Errors
    ///
    /// Returns the second attempt's [`SlotError`] when neither attempt reads
    /// the declaration, and the *first* attempt's when the second attempt read
    /// the declaration into six empty sets. That second case is the common
    /// typo: a shared object with one bad slot name or one bad value falls
    /// through to an attempt that cannot fail on it, because every face takes
    /// the absent fallback. §10 makes an empty per-face map legal, so the empty
    /// sets are not the fault; a refusal nothing reported is.
    pub fn parse(value: &JsonValue) -> Result<Self, SlotError> {
        let allowed = FlatSlots::connection_slots();
        let refused = match FlatSlots::parse(value, &allowed) {
            Ok(shared) => {
                let sets = [
                    shared.clone(),
                    shared.clone(),
                    shared.clone(),
                    shared.clone(),
                    shared.clone(),
                    shared,
                ];
                return Ok(Self::assign(sets));
            }
            Err(refused) => refused,
        };

        if !value.is_object() {
            return Err(SlotError::Malformed);
        }
        let fallback = match value.get(FALLBACK_KEY) {
            None => None,
            Some(declared) => Some(FlatSlots::parse(declared, &allowed)?),
        };
        let mut sets: Vec<FlatSlots> = Vec::with_capacity(6);
        for face in Face::ALL {
            let declared = value.get(face.declared_name());
            let set = match declared {
                None => fallback.clone().unwrap_or_default(),
                Some(declared) => match declared.as_str() {
                    Some(name) => {
                        let texture = TextureId::parse(name).ok_or_else(|| {
                            SlotError::UnreadableTexture(face.declared_name().to_owned())
                        })?;
                        let mut set = FlatSlots::new();
                        for quadrant in Quadrant::ALL {
                            set = set.with(Slot::Quadrant(quadrant), texture.clone());
                        }
                        set
                    }
                    None => FlatSlots::parse(declared, &allowed)?,
                },
            };
            sets.push(set);
        }
        let sets: [FlatSlots; 6] = sets.try_into().map_err(|_| SlotError::Malformed)?;
        let assigned = Self::assign(sets);
        if assigned.index_count() == 0 {
            return Err(refused);
        }
        Ok(assigned)
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::json;

    use super::{
        CELL_BASE, ColumnSlot, FlatSlots, Grid, MAX_CELLS, MAX_SLOTS, PaneEdge, PerFaceSlots, Slot,
        SlotError, TextureId,
    };
    use crate::neighbourhood::Face;
    use crate::quadrant::Quadrant;

    fn texture(name: &str) -> TextureId {
        TextureId::parse(name).expect("a legal texture name")
    }

    #[test]
    fn every_slot_family_has_its_own_numbers_and_its_own_names() {
        let mut numbers = vec![Slot::Particle.number()];
        let mut names = vec![super::PARTICLE_KEY];
        for quadrant in Quadrant::ALL {
            numbers.push(Slot::Quadrant(quadrant).number());
            names.push(quadrant.declared_name());
        }
        for column in ColumnSlot::ALL {
            numbers.push(Slot::Column(column).number());
            names.push(column.declared_name());
        }
        for edge in PaneEdge::ALL {
            numbers.push(Slot::Edge(edge).number());
            names.push(edge.declared_name());
        }
        let count = numbers.len();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), count, "the families share no slot number");
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "and no declared name");

        for name in names {
            assert!(Slot::by_name(name).is_some(), "{name}");
        }
        assert_eq!(Slot::by_name("nothing_spells_this"), None);
        assert!(Slot::Cell(0).number() >= CELL_BASE);
    }

    #[test]
    fn a_set_with_no_particle_falls_back_to_the_missing_sprite() {
        let empty = FlatSlots::new();
        assert_eq!(empty.particle(), TextureId::missing());
        assert_eq!(
            empty.texture(Slot::Quadrant(Quadrant::Isolated)),
            TextureId::missing()
        );

        let held = FlatSlots::new().with(Slot::Particle, texture("base:dust"));
        assert_eq!(held.particle(), texture("base:dust"));
        assert_eq!(texture("dust"), texture("base:dust"));
        assert_eq!(
            texture("dust").id(),
            &"base:dust".parse().expect("qualified texture")
        );
        assert!(TextureId::parse("bad texture").is_none());
    }

    #[test]
    fn an_undeclared_pane_edge_defaults_to_the_particle_texture() {
        let set = FlatSlots::new()
            .with(Slot::Particle, texture("base:dust"))
            .with(Slot::Edge(PaneEdge::Rim), texture("base:rim"))
            .with_defaulted_edges();
        assert_eq!(set.texture(Slot::Edge(PaneEdge::Rim)), texture("base:rim"));
        assert_eq!(
            set.texture(Slot::Edge(PaneEdge::Flank)),
            texture("base:dust"),
            "the edge nobody declared takes the particle texture"
        );
    }

    #[test]
    fn a_flat_declaration_admits_its_own_kinds_slots_and_refuses_another_kinds() {
        let value = json!({ "alone": "base:one", "notch": "base:two", "dust": "base:dust" });
        let set =
            FlatSlots::parse(&value, &FlatSlots::connection_slots()).expect("three legal slots");
        assert_eq!(set.len(), 3);
        assert_eq!(
            set.texture(Slot::Quadrant(Quadrant::Isolated)),
            texture("base:one")
        );

        let column = json!({ "middle": "base:one" });
        assert_eq!(
            FlatSlots::parse(&column, &FlatSlots::connection_slots()),
            Err(SlotError::UnknownSlot("middle".to_owned())),
            "a column slot is not a connection slot"
        );
        assert!(FlatSlots::parse(&column, &FlatSlots::column_slots()).is_ok());
    }

    #[test]
    fn a_texture_value_that_is_not_a_name_and_a_set_past_the_cap_are_both_refused() {
        assert_eq!(
            FlatSlots::parse(&json!({ "alone": 7 }), &FlatSlots::connection_slots()),
            Err(SlotError::UnreadableTexture("alone".to_owned()))
        );
        assert_eq!(
            FlatSlots::parse(
                &json!({ "alone": "not a name" }),
                &FlatSlots::connection_slots()
            ),
            Err(SlotError::UnreadableTexture("alone".to_owned()))
        );
        assert_eq!(
            FlatSlots::parse(&json!([]), &FlatSlots::connection_slots()),
            Err(SlotError::Malformed)
        );

        let value: ravel_sdk::JsonValue = (0..=MAX_SLOTS)
            .map(|index| {
                (
                    format!("slot{index}"),
                    ravel_sdk::JsonValue::from("base:one"),
                )
            })
            .collect();
        assert_eq!(
            FlatSlots::parse(&value, &FlatSlots::connection_slots()),
            Err(SlotError::TooManySlots(MAX_SLOTS + 1)),
            "the cap is read before any name is"
        );
    }

    #[test]
    fn a_grid_multiplies_the_row_by_the_width_rather_than_the_height() {
        // A three by two grid: three columns, two rows, six cells.
        let cells: Vec<TextureId> = (0..6).map(|n| texture(&format!("base:cell{n}"))).collect();
        let grid = Grid::new(3, 2, cells).expect("a legal grid");

        // Row one, column two is cell five under the corrected arithmetic. The
        // original multiplies the row by the height, which addresses cell four.
        assert_eq!(grid.index(2, 1), 5);
        assert_eq!(grid.cell(2, 1), &texture("base:cell5"));
        assert_eq!(grid.index(0, 0), 0);
        assert_eq!(grid.index(2, 0), 2);

        // Every pair addresses a declared cell, which is the other half of
        // defect 1: past the last row the original reaches slots nobody wrote.
        for column in 0..12 {
            for row in 0..12 {
                assert!(grid.index(column, row) < 6, "{column},{row}");
            }
        }
    }

    #[test]
    fn a_grid_that_cannot_be_used_is_refused_where_it_is_read() {
        let one = vec![texture("base:one")];
        assert_eq!(
            Grid::new(0, 2, one.clone()),
            Err(SlotError::UnusableGrid(0))
        );
        assert_eq!(
            Grid::new(2, 0, one.clone()),
            Err(SlotError::UnusableGrid(0))
        );
        assert_eq!(
            Grid::new(1024, 1024, one.clone()),
            Err(SlotError::UnusableGrid(1024 * 1024)),
            "a grid past the cap is refused rather than allocated"
        );
        assert!(matches!(
            Grid::new(2, 2, one),
            Err(SlotError::MissingCell(_))
        ));
        assert_eq!(
            Grid::new(2, 2, vec![texture("base:one"); 5]),
            Err(SlotError::SurplusCells {
                declared: 5,
                area: 4
            })
        );
        let full: Vec<TextureId> = (0..MAX_CELLS)
            .map(|n| texture(&format!("base:c{n}")))
            .collect();
        assert!(Grid::new(16, 16, full).is_ok(), "the cap itself is usable");
    }

    #[test]
    fn parsing_refuses_surplus_cells_before_decoding_their_textures() {
        let value = json!({
            "across": 2,
            "down": 2,
            "cells": ["base:one", "base:one", "base:one", "base:one", false]
        });
        assert_eq!(
            Grid::parse(&value),
            Err(SlotError::SurplusCells {
                declared: 5,
                area: 4
            })
        );
    }

    #[test]
    fn one_shared_texture_object_is_applied_to_all_six_faces() {
        let value = json!({
            "alone": "base:one",
            "first_side": "base:one",
            "second_side": "base:one",
            "notch": "base:one",
            "enclosed": "base:two",
        });
        let assigned = PerFaceSlots::parse(&value).expect("the first attempt reads this");

        assert_eq!(
            assigned.index_count(),
            30,
            "five entries on each of six faces"
        );
        assert_eq!(
            assigned.sprite_count(),
            2,
            "identical textures are deduplicated by identity before indices are handed out"
        );
        for face in Face::ALL {
            assert_eq!(
                assigned.sprite(assigned.index(face, Slot::Quadrant(Quadrant::Surrounded))),
                texture("base:two")
            );
        }
    }

    #[test]
    fn the_second_attempt_reads_a_face_that_is_a_string_and_one_that_is_an_object() {
        let value = json!({
            "up": "base:top",
            "down": { "alone": "base:bottom", "enclosed": "base:bottom" },
            "otherwise": { "alone": "base:side" },
        });
        let assigned = PerFaceSlots::parse(&value).expect("the second attempt reads this");

        assert_eq!(
            assigned.sprite(assigned.index(Face::Up, Slot::Quadrant(Quadrant::InnerCorner))),
            texture("base:top"),
            "a face that is a single string is that texture in every slot"
        );
        assert_eq!(
            assigned.sprite(assigned.index(Face::Down, Slot::Quadrant(Quadrant::Isolated))),
            texture("base:bottom")
        );
        assert_eq!(
            assigned.sprite(assigned.index(Face::North, Slot::Quadrant(Quadrant::Isolated))),
            texture("base:side"),
            "a face that declares none falls back to the shared object"
        );
    }

    #[test]
    fn a_face_with_no_texture_set_falls_back_to_index_zero_on_every_lookup() {
        let value = json!({ "up": { "alone": "base:top" } });
        let assigned = PerFaceSlots::parse(&value).expect("one face and no fallback");
        assert_eq!(assigned.index_count(), 1);
        assert_eq!(assigned.sprite(0), texture("base:top"));
        for face in Face::ALL {
            for quadrant in Quadrant::ALL {
                let index = assigned.index(face, Slot::Quadrant(quadrant));
                assert_eq!(index, 0, "{face:?} {quadrant}");
            }
        }
    }

    #[test]
    fn indices_are_sequential_across_the_faces_in_the_declared_face_order() {
        let value = json!({
            "down": { "alone": "base:a" },
            "up": { "alone": "base:b" },
            "north": { "alone": "base:c" },
        });
        let assigned = PerFaceSlots::parse(&value).expect("three faces");
        assert_eq!(
            assigned.index(Face::Down, Slot::Quadrant(Quadrant::Isolated)),
            0
        );
        assert_eq!(
            assigned.index(Face::Up, Slot::Quadrant(Quadrant::Isolated)),
            1
        );
        assert_eq!(
            assigned.index(Face::North, Slot::Quadrant(Quadrant::Isolated)),
            2
        );
        assert_eq!(assigned.sprite(1), texture("base:b"));
    }

    #[test]
    fn a_declaration_neither_attempt_reads_is_refused_with_a_syntax_error() {
        assert_eq!(
            PerFaceSlots::parse(&json!([1, 2])),
            Err(SlotError::Malformed)
        );
        assert!(PerFaceSlots::parse(&json!({ "up": 7 })).is_err());
        assert!(PerFaceSlots::parse(&json!({ "otherwise": { "middle": "base:one" } })).is_err());
    }

    #[test]
    fn a_shared_object_the_second_attempt_reads_as_nothing_reports_the_first_attempt() {
        // Each of these is a shared five-entry object with one thing wrong.
        // The second attempt cannot fail on any of them, because it finds no
        // face key and hands every face the absent fallback, so without the
        // first attempt's refusal the block would draw entirely in the
        // missing-texture sprite and nothing would say why.
        assert_eq!(
            PerFaceSlots::parse(&json!({ "alone": 7 })),
            Err(SlotError::UnreadableTexture("alone".to_owned()))
        );
        assert_eq!(
            PerFaceSlots::parse(&json!({ "nothing_names_this": "base:one" })),
            Err(SlotError::UnknownSlot("nothing_names_this".to_owned()))
        );
        assert_eq!(
            PerFaceSlots::parse(&json!({ "otherwise": {} })),
            Err(SlotError::UnknownSlot("otherwise".to_owned())),
            "a shared fallback that declares nothing is the same fault"
        );

        // §10 makes an empty per-face map legal, and an object that declares
        // nothing at all is read by the first attempt rather than refused by
        // it, so it stays accepted.
        let empty = PerFaceSlots::parse(&json!({})).expect("an empty set is legal");
        assert_eq!(empty.index_count(), 0);
        for face in Face::ALL {
            assert_eq!(empty.index(face, Slot::Quadrant(Quadrant::Isolated)), 0);
        }

        // One good face beside one absent one is still read: the second
        // attempt produced something, so there is nothing to report.
        let partial = PerFaceSlots::parse(&json!({ "up": { "alone": "base:one" } }))
            .expect("one declared face is a declaration");
        assert_eq!(partial.index_count(), 1);
    }
}
