//! The model kinds of the specification's §5, and the table they live in.
//!
//! Eight kind identifiers are registered, backed by seven distinct behaviours:
//! two of the eight names share one behaviour. All eight are registered here,
//! because dropping the duplicate breaks any pack that spells it.
//!
//! Registering a kind identifier twice is refused with an error rather than
//! silently overwriting, which is the refusal §2 says to keep.
//!
//! # Defect 6 of §12
//!
//! The original's kind table is an unordered hash map, so which kind claims a
//! contested identifier is unspecified by contract. [`Registry`] iterates in
//! *registration order*, [`Registry::names`] hands that order out, and
//! [`Registry::claim`] takes the first kind in it that holds a declaration. The
//! order is recorded rather than incidental: [`BUILT_IN`] is the order the
//! eight built-in kinds are registered in and a test pins it.
//!
//! # What is host-blocked here
//!
//! [`Registry::claim`] is the portable half of §4.3. Substituting the claimed
//! model into the game's model bakery, and the second lookup path that feeds a
//! kind's table from the bakery's own parsed documents, both need a model
//! surface Ravel does not publish. `NOTES.md` records them.

use std::collections::{BTreeMap, BTreeSet};

use ravel_sdk::JsonValue;

use crate::conditions::{Condition, Groups, Report, Vocabulary, qualify};
use crate::geometry::{
    Continuation, Quad, Sheet, connected_block, connected_carpet, connected_pane, pane_column,
    pane_sample, pane_sheet, rotating_column, tiled, upright_column,
};
use crate::lookup::{Catalogue, KIND_KEY, Lookup, MAX_DOCUMENTS};
use crate::neighbourhood::{
    Axis, Face, Neighbourhood, Neighbours, Offset, Probe, occluded, sample,
};
use crate::slots::{FlatSlots, Grid, PerFaceSlots, Slot, SlotError, TextureId};

/// The most kinds one table may hold.
pub const MAX_KINDS: usize = 64;

/// The key a declaration holds its texture set under.
pub const TEXTURES_KEY: &str = "textures";

/// The key a declaration holds its connection condition under.
pub const CONDITION_KEY: &str = "connects";

/// The key a declaration holds its render layer under.
pub const LAYER_KEY: &str = "layer";

/// The variant marker an item model carries.
///
/// §4.3 refuses a model identifier whose variant is the inventory one before
/// any lookup, so an item model is never substituted.
pub const INVENTORY_VARIANT: &str = "held";

/// The seven distinct behaviours the eight kinds are backed by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Behaviour {
    /// §9.1. Quadrant connection on a full cube.
    ConnectedBlock,
    /// §9.2. Quadrant connection on a thin horizontal slab.
    ConnectedSheet,
    /// §9.3. Quadrant connection on a pane, driven by state flags.
    ConnectedPane,
    /// §9.4. A texture grid tiled across world coordinates.
    LargeTiled,
    /// §9.5. Axis-aware continuation, rotated per face.
    RotatingColumn,
    /// §9.6. As §9.5, block identity, never rotated.
    UprightColumn,
    /// §9.7. Column continuation on a pane.
    PaneColumn,
}

impl Behaviour {
    /// Every behaviour, in the order this enum declares them.
    pub const ALL: [Self; 7] = [
        Self::ConnectedBlock,
        Self::ConnectedSheet,
        Self::ConnectedPane,
        Self::LargeTiled,
        Self::RotatingColumn,
        Self::UprightColumn,
        Self::PaneColumn,
    ];

    /// Returns whether this behaviour reads the condition language of §7.
    ///
    /// The pane kinds do not: a neighbour connects only if it is the same block
    /// with both of its in-plane perpendicular flags set, which is a rule of
    /// their own.
    #[must_use]
    pub const fn reads_condition(self) -> bool {
        !matches!(self, Self::ConnectedPane | Self::PaneColumn)
    }

    /// Returns the slots a declaration for this behaviour may name.
    #[must_use]
    pub fn slots(self) -> Vec<Slot> {
        match self {
            Self::ConnectedBlock | Self::ConnectedSheet => FlatSlots::connection_slots(),
            Self::ConnectedPane | Self::PaneColumn => FlatSlots::pane_slots(),
            Self::RotatingColumn | Self::UprightColumn => FlatSlots::column_slots(),
            Self::LargeTiled => Vec::new(),
        }
    }
}

/// The eight built-in kinds, in the order they are registered.
///
/// The fourth and fifth share one behaviour, which is the duplicate pair §5
/// counts.
pub const BUILT_IN: [(&str, Behaviour); 8] = [
    ("joined_cube", Behaviour::ConnectedBlock),
    ("joined_mat", Behaviour::ConnectedSheet),
    ("joined_sheet", Behaviour::ConnectedPane),
    ("spread_grid", Behaviour::LargeTiled),
    ("spread_tiles", Behaviour::LargeTiled),
    ("turning_post", Behaviour::RotatingColumn),
    ("standing_post", Behaviour::UprightColumn),
    ("sheet_post", Behaviour::PaneColumn),
];

/// The render layer a declaration may name, from a fixed four-name vocabulary.
///
/// §11's baking is host-blocked, so the layer is carried as data rather than
/// acted on. It is read and validated here because a declaration naming one the
/// game does not have is a pack fault worth reporting where it is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// The game's default when a declaration names none.
    #[default]
    Solid,
    /// Cut out at a hard threshold.
    Clipped,
    /// Cut out at a hard threshold, sampled finely.
    ClippedFine,
    /// Blended against what is behind it.
    Blended,
}

impl Layer {
    /// Every layer, in the order this enum declares them.
    pub const ALL: [Self; 4] = [Self::Solid, Self::Clipped, Self::ClippedFine, Self::Blended];

    /// Returns the name a declaration spells this layer by.
    #[must_use]
    pub const fn declared_name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Clipped => "clipped",
            Self::ClippedFine => "clipped_fine",
            Self::Blended => "blended",
        }
    }

    /// Returns the layer a declaration spells by `name`, if any.
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|layer| layer.declared_name() == name)
    }
}

/// Why a kind could not be registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KindError {
    /// A kind is already registered under that identifier.
    AlreadyRegistered(String),
    /// The identifier is not a legal kind name.
    Unreadable(String),
    /// The table already holds [`MAX_KINDS`].
    Full(usize),
}

/// Why a declaration could not be read into a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// The declaration carries no texture set.
    NoTextures,
    /// The texture set was refused.
    Textures(SlotError),
    /// The declaration names a layer the vocabulary does not have.
    UnknownLayer(String),
}

/// Everything one face's geometry may need to be decided.
///
/// Different kinds read different parts of it. Bundling them keeps every kind
/// behind one call, which is what lets [`Model::quads`] dispatch without the
/// caller knowing which kind it holds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    /// The probe result, including whether the face is culled.
    pub probe: Option<Probe>,
    /// The eight answers, used by the kind that never culls.
    pub neighbours: Neighbours,
    /// The block's own sheet flags and what sits in front of the face.
    pub sheet: Sheet,
    /// The block's own axis property, if it carries one.
    pub axis: Option<Axis>,
    /// Whether the column continues in each direction along its axis.
    pub continuation: Continuation,
    /// The block's absolute world position.
    pub position: Offset,
}

impl Context {
    /// Returns the probe, defaulting to a drawn face with these neighbours.
    #[must_use]
    pub fn probe(&self) -> Probe {
        self.probe.unwrap_or(Probe::Drawn(self.neighbours))
    }
}

/// The texture set a model holds, in whichever of the two schemes it uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Textures {
    /// One map from slot number to texture, handed straight to the atlas.
    Flat(Box<FlatSlots>),
    /// A separate set per face, with sequential runtime indices.
    PerFace(Box<PerFaceSlots>),
    /// A grid of cells addressed by world position.
    Tiled(Box<Grid>),
}

/// One parsed declaration, ready to answer for a face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    behaviour: Behaviour,
    textures: Textures,
    condition: Option<Condition>,
    layer: Layer,
    report: Report,
}

impl Model {
    /// Reads a declaration into a model.
    ///
    /// # Errors
    ///
    /// Returns [`ModelError`] when the declaration carries no texture set, when
    /// the texture set is refused, or when it names a layer the vocabulary does
    /// not have.
    pub fn parse(
        behaviour: Behaviour,
        declaration: &JsonValue,
        vocabulary: &Vocabulary,
    ) -> Result<Self, ModelError> {
        let declared = declaration
            .get(TEXTURES_KEY)
            .ok_or(ModelError::NoTextures)?;
        let textures = match behaviour {
            Behaviour::ConnectedBlock => Textures::PerFace(Box::new(
                PerFaceSlots::parse(declared).map_err(ModelError::Textures)?,
            )),
            Behaviour::LargeTiled => Textures::Tiled(Box::new(
                Grid::parse(declared).map_err(ModelError::Textures)?,
            )),
            Behaviour::ConnectedSheet
            | Behaviour::ConnectedPane
            | Behaviour::RotatingColumn
            | Behaviour::UprightColumn
            | Behaviour::PaneColumn => {
                let set =
                    FlatSlots::parse(declared, &behaviour.slots()).map_err(ModelError::Textures)?;
                let set = if behaviour.reads_condition() {
                    set
                } else {
                    set.with_defaulted_edges()
                };
                Textures::Flat(Box::new(set))
            }
        };

        let layer = match declaration.get(LAYER_KEY) {
            None => Layer::default(),
            Some(named) => {
                let named = named.as_str().unwrap_or_default();
                Layer::by_name(named).ok_or_else(|| ModelError::UnknownLayer(named.to_owned()))?
            }
        };

        // The pane kinds do not read the condition language at all, so a
        // declaration carrying one is not consulted rather than half applied.
        let (condition, report) = if behaviour.reads_condition() {
            let (condition, report) =
                Condition::from_declaration(declaration.get(CONDITION_KEY), vocabulary);
            (Some(condition), report)
        } else {
            (None, Report::default())
        };

        Ok(Self {
            behaviour,
            textures,
            condition,
            layer,
            report,
        })
    }

    /// Returns the behaviour this model draws with.
    #[must_use]
    pub const fn behaviour(&self) -> Behaviour {
        self.behaviour
    }

    /// Returns the connection condition, or `None` for a kind that reads none.
    #[must_use]
    pub const fn condition(&self) -> Option<&Condition> {
        self.condition.as_ref()
    }

    /// Returns the render layer the declaration named.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// Returns what reading the condition refused.
    #[must_use]
    pub const fn report(&self) -> Report {
        self.report
    }

    /// Returns the texture set.
    #[must_use]
    pub const fn textures(&self) -> &Textures {
        &self.textures
    }

    /// Reads one face's neighbourhood into the context this model draws from.
    ///
    /// This is where the two connection rules part. A kind that reads the
    /// condition language of §7 gets the eight samples of §8 put to its own
    /// condition, and the occlusion test beside them so that §9.2's kind can
    /// ignore it. A pane kind reads no condition at all: its neighbours come
    /// from §9.3's own rule through [`crate::geometry::pane_sample`], which is the
    /// only way a pane's `Neighbours` is ever answered.
    ///
    /// [`Context::position`], [`Context::axis`] and [`Context::continuation`]
    /// are left at their defaults, because a world position and a block's own
    /// axis property are not facts about a neighbourhood. A caller drawing a
    /// tiled or a column kind fills them.
    #[must_use]
    pub fn context(
        &self,
        face: Face,
        groups: &Groups,
        neighbourhood: &dyn Neighbourhood,
    ) -> Context {
        let Some(condition) = self.condition.as_ref() else {
            let own = neighbourhood.appearance(Offset::ZERO);
            return Context {
                neighbours: pane_sample(face, &own, neighbourhood),
                sheet: pane_sheet(face, &own, neighbourhood),
                ..Context::default()
            };
        };
        // The samples are taken whether or not the face is occluded: §9.2's
        // kind culls no face and still needs its eight answers.
        let neighbours = sample(face, condition, groups, neighbourhood);
        let probe = if occluded(face, neighbourhood) {
            Probe::Culled
        } else {
            Probe::Drawn(neighbours)
        };
        Context {
            probe: Some(probe),
            neighbours,
            ..Context::default()
        }
    }

    /// Returns the quads this model draws on `face`.
    #[must_use]
    pub fn quads(&self, face: Face, context: &Context) -> Vec<Quad> {
        match self.behaviour {
            Behaviour::ConnectedBlock => connected_block(context.probe()),
            Behaviour::ConnectedSheet => connected_carpet(face, context.neighbours),
            Behaviour::ConnectedPane => connected_pane(face, context.sheet, context.neighbours),
            Behaviour::LargeTiled => match &self.textures {
                Textures::Tiled(grid) => tiled(face, context.position, grid),
                Textures::Flat(_) | Textures::PerFace(_) => Vec::new(),
            },
            Behaviour::RotatingColumn => rotating_column(face, context.axis, context.continuation),
            Behaviour::UprightColumn => {
                upright_column(face, context.probe(), context.axis, context.continuation)
            }
            Behaviour::PaneColumn => pane_column(
                face,
                context.sheet,
                context.neighbours,
                context.continuation,
            ),
        }
    }

    /// Returns the texture a quad's slot reaches on `face`.
    #[must_use]
    pub fn texture(&self, face: Face, slot: Slot) -> TextureId {
        match &self.textures {
            Textures::Flat(set) => set.texture(slot),
            Textures::PerFace(assigned) => assigned.sprite(assigned.index(face, slot)),
            Textures::Tiled(grid) => match slot {
                Slot::Cell(cell) => grid.at(cell),
                Slot::Particle | Slot::Quadrant(_) | Slot::Column(_) | Slot::Edge(_) => {
                    TextureId::missing()
                }
            },
        }
    }
}

/// One registered kind and the declarations it holds.
#[derive(Clone, Debug, Default)]
struct Registered {
    behaviour: Option<Behaviour>,
    catalogue: Catalogue,
}

/// What a kind claimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim<'a> {
    /// The kind identifier that claimed it.
    pub kind: &'a str,
    /// That kind's behaviour.
    pub behaviour: Behaviour,
    /// The declaration it claimed with.
    pub document: &'a JsonValue,
}

/// The process-wide table of model kinds.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    order: Vec<String>,
    kinds: BTreeMap<String, Registered>,
    directory: BTreeSet<ravel_sdk::content::ContentId>,
}

impl Registry {
    /// Builds an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the table holding the eight built-in kinds of [`BUILT_IN`].
    ///
    /// # Errors
    ///
    /// Returns [`KindError`] only if [`BUILT_IN`] itself were malformed, which
    /// would be a packaging fault rather than input.
    pub fn built_in() -> Result<Self, KindError> {
        let mut registry = Self::new();
        for (name, behaviour) in BUILT_IN {
            registry.register(name, behaviour)?;
        }
        Ok(registry)
    }

    /// Registers a kind.
    ///
    /// # Errors
    ///
    /// Returns [`KindError::AlreadyRegistered`] rather than overwriting,
    /// [`KindError::Unreadable`] for a name that is not a legal path segment,
    /// and [`KindError::Full`] past [`MAX_KINDS`].
    pub fn register(&mut self, name: &str, behaviour: Behaviour) -> Result<(), KindError> {
        if qualify(name).is_none() || name.contains(':') {
            return Err(KindError::Unreadable(name.to_owned()));
        }
        if self.kinds.contains_key(name) {
            return Err(KindError::AlreadyRegistered(name.to_owned()));
        }
        if self.order.len() >= MAX_KINDS {
            return Err(KindError::Full(self.order.len()));
        }
        self.order.push(name.to_owned());
        self.kinds.insert(
            name.to_owned(),
            Registered {
                behaviour: Some(behaviour),
                catalogue: Catalogue::new(),
            },
        );
        Ok(())
    }

    /// Returns the kind identifiers in registration order.
    ///
    /// This is the deterministic order defect 6 asks for, handed out so a
    /// caller can record it rather than infer it.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(String::as_str)
    }

    /// Returns how many kinds are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Returns whether the table holds no kind.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Returns the behaviour registered under `name`, if any.
    #[must_use]
    pub fn behaviour(&self, name: &str) -> Option<Behaviour> {
        self.kinds.get(name).and_then(|held| held.behaviour)
    }

    /// Returns the declarations held for `name`.
    #[must_use]
    pub fn catalogue(&self, name: &str) -> Option<&Catalogue> {
        self.kinds.get(name).map(|held| &held.catalogue)
    }

    /// Returns the declarations held for `name`, for a host to fill.
    ///
    /// The second lookup path of §4.1 fills a kind's table from the model
    /// bakery's own parsed documents. That path is host-blocked, so the table
    /// is reachable rather than sealed: a host that gains the surface fills it
    /// here without this module changing.
    pub fn catalogue_mut(&mut self, name: &str) -> Option<&mut Catalogue> {
        self.kinds.get_mut(name).map(|held| &mut held.catalogue)
    }

    /// Rebuilds every kind's table from one reload's documents.
    ///
    /// A document is routed to the kind its own kind key names, and one naming
    /// no registered kind is dropped rather than held.
    pub fn reload<I>(&mut self, documents: I) -> usize
    where
        I: IntoIterator<Item = (String, JsonValue)>,
    {
        let mut sorted: BTreeMap<String, Vec<(String, JsonValue)>> = BTreeMap::new();
        let mut routed = 0usize;
        self.directory.clear();
        for (identifier, document) in documents {
            // The directory is what §4.1 short-circuits on, so an identifier
            // enters it whatever its document goes on to name — including a
            // kind nothing registers, which is exactly the case a per-kind
            // table cannot record.
            if let Some(qualified) = qualify(&identifier)
                && self.directory.len() < MAX_DOCUMENTS
            {
                self.directory.insert(qualified);
            }
            let Some(kind) = document.get(KIND_KEY).and_then(JsonValue::as_str) else {
                continue;
            };
            if !self.kinds.contains_key(kind) {
                continue;
            }
            sorted
                .entry(kind.to_owned())
                .or_default()
                .push((identifier, document));
            routed += 1;
        }
        for (name, held) in &mut self.kinds {
            let batch = sorted.remove(name).unwrap_or_default();
            held.catalogue.rebuild(batch);
        }
        routed
    }

    /// Offers a model identifier to every kind in registration order.
    ///
    /// The first kind holding a declaration for it claims it. An identifier
    /// whose variant is [`INVENTORY_VARIANT`] is refused before any lookup, so
    /// an item model is never claimed.
    #[must_use]
    pub fn claim(&self, identifier: &str) -> Option<Claim<'_>> {
        let (path, variant) = split_variant(identifier);
        if variant == Some(INVENTORY_VARIANT) {
            return None;
        }
        for name in &self.order {
            // A kind the order names and the table does not hold, or one
            // holding a table without a behaviour, is skipped rather than
            // ending the scan: the identifier is still offered to every later
            // kind, which is what §4.3 asks for.
            let Some(held) = self.kinds.get(name) else {
                continue;
            };
            let Some(behaviour) = held.behaviour else {
                continue;
            };
            if let Lookup::Found(document) = held.catalogue.find(path, name) {
                return Some(Claim {
                    kind: name.as_str(),
                    behaviour,
                    document,
                });
            }
        }
        None
    }

    /// Returns whether the second lookup path of §4.1 is reached for
    /// `identifier`.
    ///
    /// §4.1 short-circuits on the *directory* rather than on a kind: if the
    /// reload directory held a document under this identifier at all, that
    /// document's verdict is final and path two is never tried, even where it
    /// named a kind nothing registers. The per-kind tables cannot answer this
    /// between them — a document whose kind key names nothing registered is in
    /// none of them — so the directory is kept once, here, and this is the
    /// answer a host-blocked path two would ask.
    ///
    /// An identifier whose variant is [`INVENTORY_VARIANT`] is refused before
    /// any lookup by §4.3, so it reaches no path at all.
    #[must_use]
    pub fn falls_through(&self, identifier: &str) -> bool {
        let (path, variant) = split_variant(identifier);
        if variant == Some(INVENTORY_VARIANT) {
            return false;
        }
        qualify(path).is_some_and(|qualified| !self.directory.contains(&qualified))
    }

    /// Returns how many identifiers the last reload's directory held.
    #[must_use]
    pub fn directory_len(&self) -> usize {
        self.directory.len()
    }
}

/// Splits a model identifier into its path and its variant.
#[must_use]
pub fn split_variant(identifier: &str) -> (&str, Option<&str>) {
    match identifier.split_once('#') {
        None => (identifier, None),
        Some((path, variant)) => (path, Some(variant)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ravel_sdk::{JsonValue, json};

    use super::{
        BUILT_IN, Behaviour, Context, INVENTORY_VARIANT, KindError, Layer, MAX_KINDS, Model,
        ModelError, Registry, Textures, split_variant,
    };
    use crate::conditions::{BlockState, Condition, Groups, Vocabulary};
    use crate::geometry::{Continuation, Sheet, SheetFlags};
    use crate::neighbourhood::{Axis, Face, Neighbourhood, Neighbours, Offset, Probe};
    use crate::quadrant::Quadrant;
    use crate::slots::{ColumnSlot, Slot, TextureId};

    fn vocabulary() -> Vocabulary {
        Vocabulary::new().with_block("base:panel", ["lit"])
    }

    fn declaring(kind: &str, textures: JsonValue) -> JsonValue {
        let mut declaration = json!({ "shape": kind });
        declaration["textures"] = textures;
        declaration
    }

    fn five(texture: &str) -> JsonValue {
        json!({
            "alone": texture,
            "first_side": texture,
            "second_side": texture,
            "notch": texture,
            "enclosed": texture,
        })
    }

    #[test]
    fn eight_names_are_registered_and_they_are_backed_by_seven_behaviours() {
        let registry = Registry::built_in().expect("the built-in table is well formed");
        assert_eq!(registry.len(), 8);

        let mut behaviours: Vec<Behaviour> = BUILT_IN
            .into_iter()
            .map(|(_, behaviour)| behaviour)
            .collect();
        behaviours.sort_unstable();
        behaviours.dedup();
        assert_eq!(
            behaviours.len(),
            7,
            "two of the eight names share a behaviour"
        );
        assert_eq!(behaviours, Behaviour::ALL.to_vec());

        let mut names: Vec<&str> = BUILT_IN.into_iter().map(|(name, _)| name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "and the eight names are distinct");
    }

    #[test]
    fn the_duplicate_pair_is_registered_rather_than_dropped() {
        let registry = Registry::built_in().expect("well formed");
        let shared = registry
            .names()
            .filter(|name| registry.behaviour(name) == Some(Behaviour::LargeTiled))
            .count();
        assert_eq!(
            shared, 2,
            "dropping the duplicate breaks any pack that spells it"
        );
    }

    #[test]
    fn registering_an_identifier_twice_is_refused_rather_than_overwriting() {
        let mut registry = Registry::built_in().expect("well formed");
        assert_eq!(
            registry.register("joined_cube", Behaviour::LargeTiled),
            Err(KindError::AlreadyRegistered("joined_cube".to_owned()))
        );
        assert_eq!(
            registry.behaviour("joined_cube"),
            Some(Behaviour::ConnectedBlock),
            "and the first registration still stands"
        );

        assert_eq!(
            registry.register("not a name", Behaviour::LargeTiled),
            Err(KindError::Unreadable("not a name".to_owned()))
        );
        assert_eq!(
            registry.register("base:qualified", Behaviour::LargeTiled),
            Err(KindError::Unreadable("base:qualified".to_owned())),
            "a kind identifier is one segment, not a qualified name"
        );
    }

    #[test]
    fn the_table_is_capped_and_says_so_rather_than_growing() {
        let mut registry = Registry::new();
        for index in 0..MAX_KINDS {
            registry
                .register(&format!("kind{index}"), Behaviour::LargeTiled)
                .expect("under the cap");
        }
        assert_eq!(registry.len(), MAX_KINDS);
        assert_eq!(
            registry.register("one_more", Behaviour::LargeTiled),
            Err(KindError::Full(MAX_KINDS))
        );
        assert_eq!(registry.len(), MAX_KINDS);
    }

    #[test]
    fn the_iteration_order_is_registration_order_and_it_is_the_same_every_time() {
        let expected: Vec<&str> = BUILT_IN.into_iter().map(|(name, _)| name).collect();
        for _ in 0..4 {
            let registry = Registry::built_in().expect("well formed");
            assert_eq!(registry.names().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn a_contested_identifier_goes_to_the_first_kind_in_that_order() {
        let mut registry = Registry::built_in().expect("well formed");
        // Both tables hold a declaration for the same identifier, each valid
        // for its own kind. Under an unordered table the winner would be
        // whichever the map happened to yield first.
        for kind in ["standing_post", "joined_cube"] {
            registry
                .catalogue_mut(kind)
                .expect("a registered kind")
                .rebuild([("base:wall".to_owned(), declaring(kind, five("base:one")))]);
        }

        let claim = registry
            .claim("base:wall")
            .expect("one of the two claims it");
        assert_eq!(
            claim.kind, "joined_cube",
            "the earlier registration wins, every run"
        );
        assert_eq!(claim.behaviour, Behaviour::ConnectedBlock);
    }

    #[test]
    fn an_inventory_variant_is_refused_before_any_lookup() {
        let mut registry = Registry::built_in().expect("well formed");
        registry
            .catalogue_mut("joined_cube")
            .expect("registered")
            .rebuild([(
                "base:wall".to_owned(),
                declaring("joined_cube", five("base:one")),
            )]);

        assert!(registry.claim("base:wall").is_some());
        assert!(
            registry
                .claim(&format!("base:wall#{INVENTORY_VARIANT}"))
                .is_none(),
            "an item model is never claimed"
        );
        assert!(
            registry.claim("base:wall#standing").is_some(),
            "another variant is claimed normally"
        );
        assert_eq!(split_variant("base:wall"), ("base:wall", None));
        assert_eq!(split_variant("base:wall#held"), ("base:wall", Some("held")));
    }

    #[test]
    fn a_reload_routes_each_document_to_the_kind_its_own_key_names() {
        let mut registry = Registry::built_in().expect("well formed");
        let routed = registry.reload([
            (
                "base:wall".to_owned(),
                declaring("joined_cube", five("base:one")),
            ),
            (
                "base:rug".to_owned(),
                declaring("joined_mat", five("base:two")),
            ),
            (
                "base:stray".to_owned(),
                declaring("nothing_registers_this", five("base:three")),
            ),
        ]);
        assert_eq!(
            routed, 2,
            "the document naming no registered kind is dropped"
        );

        assert_eq!(
            registry.claim("base:wall").map(|c| c.kind),
            Some("joined_cube")
        );
        assert_eq!(
            registry.claim("base:rug").map(|c| c.kind),
            Some("joined_mat")
        );
        assert!(registry.claim("base:stray").is_none());

        // A reload replaces every kind's table, including one that received
        // nothing this time.
        registry.reload(std::iter::empty::<(String, JsonValue)>());
        assert!(registry.claim("base:wall").is_none());
        for name in BUILT_IN.into_iter().map(|(name, _)| name) {
            assert!(
                registry.catalogue(name).expect("registered").is_empty(),
                "{name} still holds documents after an empty reload"
            );
        }
    }

    #[test]
    fn a_claimed_declaration_is_read_into_a_model_that_draws_quads() {
        let mut registry = Registry::built_in().expect("well formed");
        registry.reload([(
            "base:wall".to_owned(),
            declaring("joined_cube", five("base:one")),
        )]);

        let claim = registry.claim("base:wall").expect("claimed");
        let model = Model::parse(claim.behaviour, claim.document, &vocabulary())
            .expect("the declaration reads");

        let quads = model.quads(Face::North, &Context::default());
        assert_eq!(quads.len(), 4, "the resolved kind actually draws");
        for quad in &quads {
            assert_eq!(
                model.texture(Face::North, quad.slot()),
                TextureId::parse("base:one").expect("legal"),
                "and the slot the quad names reaches the declared texture"
            );
        }
    }

    #[test]
    fn every_behaviour_draws_something_through_the_one_dispatch() {
        let vocabulary = vocabulary();
        let context = Context {
            probe: Some(Probe::Drawn(Neighbours::NONE)),
            neighbours: Neighbours::NONE,
            sheet: Sheet::new(SheetFlags::new([false, false], [true, true]), false),
            axis: Some(Axis::Y),
            continuation: Continuation::new(true, false),
            position: Offset::new(2, 3, 5),
        };

        for (name, behaviour) in BUILT_IN {
            let textures = match behaviour {
                Behaviour::LargeTiled => json!({
                    "across": 2,
                    "down": 3,
                    "cells": ["base:a", "base:b", "base:c", "base:d", "base:e", "base:f"],
                }),
                Behaviour::RotatingColumn | Behaviour::UprightColumn => json!({
                    "middle": "base:one",
                    "upper": "base:two",
                    "lower": "base:three",
                    "single": "base:four",
                    "end": "base:five",
                }),
                Behaviour::ConnectedBlock
                | Behaviour::ConnectedSheet
                | Behaviour::ConnectedPane
                | Behaviour::PaneColumn => five("base:one"),
            };
            let declaration = declaring(name, textures);
            let model =
                Model::parse(behaviour, &declaration, &vocabulary).expect("a legal declaration");

            let mut drawn = 0usize;
            for face in Face::ALL {
                for quad in model.quads(face, &context) {
                    assert!(quad.edges().is_usable(), "{name} {face:?}");
                    drawn += 1;
                }
            }
            assert!(drawn > 0, "{name} drew nothing on any face");
        }
    }

    /// A neighbourhood written out position by position.
    #[derive(Default)]
    struct Written {
        centre: BlockState,
        around: BTreeMap<Offset, BlockState>,
    }

    impl Neighbourhood for Written {
        fn appearance(&self, offset: Offset) -> BlockState {
            if offset == Offset::ZERO {
                return self.centre.clone();
            }
            self.around
                .get(&offset)
                .cloned()
                .unwrap_or_else(BlockState::nothing)
        }
    }

    fn surrounded(centre: BlockState, face: Face, neighbour: &BlockState) -> Written {
        let mut around = BTreeMap::new();
        for index in 0..4 {
            around.insert(face.orthogonal(index), neighbour.clone());
            around.insert(face.diagonal(index), neighbour.clone());
        }
        Written { centre, around }
    }

    fn pane_state(across: bool) -> BlockState {
        let along = if across { "false" } else { "true" };
        let perpendicular = if across { "true" } else { "false" };
        BlockState::new("base:pane")
            .with("north", along)
            .with("south", along)
            .with("west", perpendicular)
            .with("east", perpendicular)
    }

    #[test]
    fn a_pane_model_reads_its_neighbourhood_by_the_flag_rule_and_not_the_condition() {
        let vocabulary = vocabulary();
        let declaration = declaring("joined_sheet", five("base:one"));
        let pane = Model::parse(Behaviour::ConnectedPane, &declaration, &vocabulary)
            .expect("the declaration reads");
        let mut declaration = declaring("joined_cube", five("base:one"));
        declaration["connects"] = json!({ "test": "same_block" });
        let block = Model::parse(Behaviour::ConnectedBlock, &declaration, &vocabulary)
            .expect("the declaration reads");

        // Every neighbour is the same block, and every one of them runs into
        // the face rather than across it. §7's condition connects them all;
        // §9.3's rule connects none of them, and only the pane model may use
        // §9.3's rule.
        let own = pane_state(true);
        let written = surrounded(own.clone(), Face::North, &pane_state(false));
        let groups = Groups::new();

        let context = pane.context(Face::North, &groups, &written);
        assert_eq!(
            context.neighbours,
            Neighbours::NONE,
            "a neighbour running into the face does not connect"
        );
        assert_eq!(context.sheet.flags, SheetFlags::read(&own));

        let context = block.context(Face::North, &groups, &written);
        assert!(
            context.neighbours.all_connected(),
            "the same neighbourhood connects under §7's own condition"
        );
    }

    #[test]
    fn a_condition_reading_model_is_handed_its_samples_even_where_the_face_is_culled() {
        let vocabulary = vocabulary();
        let own = BlockState::new("base:mat");
        let mut written = surrounded(own.clone(), Face::Up, &own);
        written.around.insert(Face::Up.normal(), own);

        let groups = Groups::new();
        let carpet = Model::parse(
            Behaviour::ConnectedSheet,
            &declaring("joined_mat", five("base:one")),
            &vocabulary,
        )
        .expect("reads");
        let context = carpet.context(Face::Up, &groups, &written);
        assert_eq!(context.probe, Some(Probe::Culled), "the face is occluded");
        assert!(
            context.neighbours.all_connected(),
            "and §9.2's kind still has its eight answers, because it culls nothing"
        );
        assert_eq!(
            carpet.quads(Face::Up, &context).len(),
            1,
            "so the carpet draws the surrounded quad rather than four"
        );
        assert_eq!(
            Model::parse(
                Behaviour::ConnectedBlock,
                &declaring("joined_cube", five("base:one")),
                &vocabulary,
            )
            .expect("reads")
            .quads(Face::Up, &context)
            .len(),
            0,
            "while the kind that does cull draws nothing"
        );
    }

    #[test]
    fn the_directory_ends_the_lookup_even_where_the_document_names_no_kind_of_ours() {
        let mut registry = Registry::built_in().expect("well formed");
        registry.reload([
            (
                "base:wall".to_owned(),
                declaring("joined_cube", five("base:one")),
            ),
            (
                "base:orphan".to_owned(),
                declaring("nothing_registers_this", five("base:one")),
            ),
            (
                "base:kindless".to_owned(),
                json!({ "textures": five("base:one") }),
            ),
        ]);

        assert_eq!(registry.directory_len(), 3, "the directory holds all three");
        assert!(!registry.falls_through("base:wall"));
        assert!(
            !registry.falls_through("base:orphan"),
            "a document naming a kind nothing registers still ends the lookup"
        );
        assert!(
            !registry.falls_through("base:kindless"),
            "and so does one naming no kind at all"
        );
        assert!(
            registry
                .catalogue("joined_cube")
                .expect("a registered kind")
                .find("base:orphan", "joined_cube")
                .falls_through(),
            "no kind's own table holds it, which is why the directory is kept once"
        );
        assert!(
            registry.falls_through("base:nobody_shipped_this"),
            "only an identifier the directory never held reaches path two"
        );
        assert!(
            !registry.falls_through("base:wall#held"),
            "and §4.3 refuses an inventory variant before any path is reached"
        );

        // The directory is replaced by each reload, as the tables are.
        registry.reload([(
            "base:wall".to_owned(),
            declaring("joined_cube", five("base:one")),
        )]);
        assert_eq!(registry.directory_len(), 1);
        assert!(registry.falls_through("base:orphan"));
    }

    #[test]
    fn a_pane_declaration_is_never_asked_the_condition_language() {
        let vocabulary = vocabulary();
        let mut declaration = declaring("joined_sheet", five("base:one"));
        declaration["connects"] = json!({ "test": "same_block" });
        let model = Model::parse(Behaviour::ConnectedPane, &declaration, &vocabulary)
            .expect("the declaration reads");
        assert_eq!(
            model.condition(),
            None,
            "the pane kinds have a connection rule of their own"
        );
        assert!(model.report().is_clean());

        let mut declaration = declaring("joined_cube", five("base:one"));
        declaration["connects"] = json!({ "test": "same_block" });
        let model = Model::parse(Behaviour::ConnectedBlock, &declaration, &vocabulary)
            .expect("the declaration reads");
        assert_eq!(model.condition(), Some(&Condition::SameBlock));
    }

    #[test]
    fn an_absent_condition_is_state_identity_and_a_malformed_one_is_counted() {
        let vocabulary = vocabulary();
        let plain = declaring("joined_cube", five("base:one"));
        let model = Model::parse(Behaviour::ConnectedBlock, &plain, &vocabulary).expect("reads");
        assert_eq!(model.condition(), Some(&Condition::SameState));

        let mut broken = declaring("joined_cube", five("base:one"));
        broken["connects"] = json!({ "test": "named_group", "group": "base:nothing" });
        let model = Model::parse(Behaviour::ConnectedBlock, &broken, &vocabulary).expect("reads");
        assert_eq!(model.condition(), Some(&Condition::Never));
        assert_eq!(
            model.report().refused_forms,
            1,
            "and the refusal is counted"
        );

        let panel = BlockState::new("base:panel");
        assert!(
            !model
                .condition()
                .expect("a condition")
                .connects(&panel, &panel, &Groups::new()),
            "a malformed condition connects nothing rather than defaulting"
        );
    }

    #[test]
    fn a_declaration_with_no_textures_or_an_unknown_layer_is_refused() {
        let vocabulary = vocabulary();
        assert_eq!(
            Model::parse(
                Behaviour::ConnectedBlock,
                &json!({ "shape": "joined_cube" }),
                &vocabulary
            ),
            Err(ModelError::NoTextures)
        );

        let mut declaration = declaring("joined_cube", five("base:one"));
        declaration["layer"] = json!("nothing_names_this");
        assert_eq!(
            Model::parse(Behaviour::ConnectedBlock, &declaration, &vocabulary),
            Err(ModelError::UnknownLayer("nothing_names_this".to_owned()))
        );

        declaration["layer"] = json!("blended");
        let model = Model::parse(Behaviour::ConnectedBlock, &declaration, &vocabulary)
            .expect("a named layer reads");
        assert_eq!(model.layer(), Layer::Blended);
        assert_eq!(
            Layer::default(),
            Layer::Solid,
            "and the default is the game's"
        );
    }

    #[test]
    fn a_pane_models_two_edges_default_to_its_particle_texture() {
        let vocabulary = vocabulary();
        let mut textures = five("base:one");
        textures["dust"] = json!("base:dust");
        let declaration = declaring("joined_sheet", textures);
        let model =
            Model::parse(Behaviour::ConnectedPane, &declaration, &vocabulary).expect("reads");
        let Textures::Flat(set) = model.textures() else {
            panic!("a pane holds a flat set");
        };
        for edge in crate::slots::PaneEdge::ALL {
            assert_eq!(
                set.texture(Slot::Edge(edge)),
                TextureId::parse("base:dust").expect("legal")
            );
        }
    }

    #[test]
    fn a_column_model_reaches_its_own_slots_and_a_tiled_one_reaches_its_cells() {
        let vocabulary = vocabulary();
        let declaration = declaring(
            "standing_post",
            json!({ "middle": "base:one", "end": "base:two", "single": "base:three" }),
        );
        let model =
            Model::parse(Behaviour::UprightColumn, &declaration, &vocabulary).expect("reads");
        assert_eq!(
            model.texture(Face::Up, Slot::Column(ColumnSlot::Cap)),
            TextureId::parse("base:two").expect("legal")
        );
        assert_eq!(
            model.texture(Face::Up, Slot::Quadrant(Quadrant::Isolated)),
            TextureId::missing(),
            "a slot this kind does not have reaches the missing sprite"
        );

        let declaration = declaring(
            "spread_grid",
            json!({ "across": 2, "down": 2, "cells": ["base:a", "base:b", "base:c", "base:d"] }),
        );
        let model = Model::parse(Behaviour::LargeTiled, &declaration, &vocabulary).expect("reads");
        assert_eq!(
            model.texture(Face::Up, Slot::Cell(3)),
            TextureId::parse("base:d").expect("legal")
        );
    }
}
