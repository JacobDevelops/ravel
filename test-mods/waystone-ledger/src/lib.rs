#![forbid(unsafe_code)]

//! A per-world ledger of named waystones, with content and a `/waystone` command.
//!
//! Version 1 stores bare points; version 2 adds a dimension through migration.

use ravel_sdk::JsonValue;
use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::content::{ContentDeclaration, ContentId, ContentKind, RegistryKey};
use ravel_sdk::events::{CommandInvocation, GameEvent};
use ravel_sdk::generate::GeneratedData;
use ravel_sdk::network::{
    Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError,
};
use ravel_sdk::resources::RelativePath;
use ravel_sdk::save::{
    MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataEnvelope, SavedDataVersion,
};
use ravel_sdk::sides::{ClientOnly, Side};
use ravel_sdk::tags::TagId;
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, declare_mod, json};

/// The one path this mod's block, item and recipe are all named after.
const WAYSTONE: &str = "waystone";

// Convention tags share the unowned `c` namespace.
const CONVENTION: &str = "c";

// Version 1 omitted dimensions; migrate its points into the overworld.
const ASSUMED_DIMENSION: &str = "minecraft:overworld";

const SETTINGS: &str = include_str!("../configuration.json");

const CAPACITY: &str = "ledger.capacity";
const OVERWRITE: &str = "ledger.overwrite";

// Embedded model bytes are covered by the source digest.
const MODEL: &[u8] = b"{\n  \"parent\": \"minecraft:block/lodestone\"\n}\n";

// Keep oldest-to-newest versions aligned with `package.json`.
const VERSIONS: [u32; 2] = [1, 2];

fn version(raw: u32) -> SavedDataVersion {
    SavedDataVersion::new(raw).expect("every declared version is non-zero")
}

/// One entry in the ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Waystone {
    /// The name a player gave it.
    pub name: String,
    /// Block coordinates.
    pub x: i64,
    pub y: i64,
    pub z: i64,
    /// The dimension it stands in.
    pub dimension: String,
}

impl Waystone {
    fn to_json(&self) -> JsonValue {
        json!({
            "name": self.name,
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "dimension": self.dimension,
        })
    }

    fn from_json(value: &JsonValue) -> Option<Self> {
        Some(Self {
            name: value.get("name")?.as_str()?.to_owned(),
            x: value.get("x")?.as_i64()?,
            y: value.get("y")?.as_i64()?,
            z: value.get("z")?.as_i64()?,
            dimension: value.get("dimension")?.as_str()?.to_owned(),
        })
    }
}

/// What `/waystone` did, so a caller can tell a refusal from a silence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// Nothing has been asked of the command yet.
    Idle,
    /// A new waystone was written.
    Recorded(String),
    /// A name the ledger already held was replaced.
    Replaced(String),
    /// A name the ledger already held was kept, because the settings in force do
    /// not permit overwriting.
    Kept(String),
    /// A waystone was removed.
    Forgotten(String),
    /// The ledger is full.
    Full,
    /// The arguments were not something this command understands. They are text a
    /// player typed, so this is ordinary rather than a failure.
    NotUnderstood,
}

/// What a client knows about the ledger.
///
/// A client never holds the ledger. It holds whatever the most recent
/// [`LedgerSummary`] packet told it, which is state the mod keeps *because* it is
/// on the client, and is therefore the thing `ClientOnly` exists for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientView {
    /// How many waystones the server last reported.
    pub count: u32,
    /// The name the server last reported as newest, empty when there is none.
    pub newest: String,
}

/// The summary a server sends a client so the client can say how many waystones
/// this world holds without ever holding the ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerSummary {
    /// How many waystones the ledger holds.
    pub count: u32,
    /// The most recently recorded name, empty when there is none.
    pub newest: String,
}

impl Packet for LedgerSummary {
    fn contract() -> PacketContract {
        PacketContract::new(
            PacketId::new(
                "waystone_ledger:summary"
                    .parse()
                    .expect("a legal content id"),
            ),
            Direction::ToClient,
            1,
        )
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.u32(self.count);
        writer.text(&self.newest);
    }

    fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
        Ok(Self {
            count: reader.u32()?,
            newest: reader.text()?.to_owned(),
        })
    }
}

/// Turns version 1's bare points into version 2's dimensioned waystones.
///
/// Version 1 named its list `points` and recorded no dimension. Every field a
/// version-1 world holds survives; the one version 2 added is filled with the
/// assumption stated at [`ASSUMED_DIMENSION`], because the alternative is
/// dropping the record.
fn to_two(payload: &JsonValue) -> Result<JsonValue, MigrationFailed> {
    let points = payload
        .get("points")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| MigrationFailed::new("version 1 held no `points` list"))?;

    let mut waystones = Vec::with_capacity(points.len());
    for point in points {
        let name = point
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| MigrationFailed::new("a version 1 point held no `name` string"))?;
        let mut coordinates = [0_i64; 3];
        for (axis, slot) in ["x", "y", "z"].iter().zip(&mut coordinates) {
            *slot = point
                .get(*axis)
                .and_then(JsonValue::as_i64)
                .ok_or_else(|| {
                    MigrationFailed::new(format!("a version 1 point held no `{axis}` coordinate"))
                })?;
        }
        waystones.push(
            Waystone {
                name: name.to_owned(),
                x: coordinates[0],
                y: coordinates[1],
                z: coordinates[2],
                dimension: ASSUMED_DIMENSION.to_owned(),
            }
            .to_json(),
        );
    }
    Ok(json!({ "waystones": waystones }))
}

/// A mod that records where a player has been.
///
/// # Examples
///
/// ```
/// use ravel_sdk::config::LayeredConfiguration;
/// use ravel_sdk::sides::Side;
/// use ravel_sdk::{Declared, Mod};
/// use waystone_ledger::WaystoneLedger;
///
/// let mut ledger = WaystoneLedger::new(Side::IntegratedServer);
/// assert_eq!(ledger.id().as_str(), "waystone_ledger");
///
/// let declaration = ledger
///     .configuration_declaration()?
///     .expect("this mod declares settings");
/// let resolved = LayeredConfiguration::resolve(declaration, None, None)?;
/// ledger.configure(&resolved)?;
/// assert_eq!(ledger.capacity(), 32);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaystoneLedger {
    side: Side,
    capacity: usize,
    overwrite: bool,
    waystones: Vec<Waystone>,
    outcome: Outcome,
    view: ClientOnly<ClientView>,
}

impl WaystoneLedger {
    /// Builds the mod, constructing its client view only on the client side.
    #[must_use]
    pub fn new(side: Side) -> Self {
        Self {
            side,
            capacity: 0,
            overwrite: false,
            waystones: Vec::new(),
            outcome: Outcome::Idle,
            view: ClientOnly::built_on(side, ClientView::default),
        }
    }

    /// Returns the side this instance was built for.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns how many waystones the settings in force allow.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns whether recording a name the ledger already holds replaces it.
    #[must_use]
    pub const fn overwrites(&self) -> bool {
        self.overwrite
    }

    /// Returns the ledger, in the order the names were first recorded.
    #[must_use]
    pub fn waystones(&self) -> &[Waystone] {
        &self.waystones
    }

    /// Returns what the last `/waystone` invocation did.
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// Returns what this mod knows on the client, or [`None`] on a server side,
    /// where there is nothing to know.
    #[must_use]
    pub fn client_view(&self) -> Option<&ClientView> {
        self.view.get(self.side).ok()
    }

    /// Applies a summary a server sent.
    ///
    /// On a server side there is no client view to apply it to, so nothing
    /// happens and the mod carries on: a summary reaching the wrong side is the
    /// loader's to refuse, not a reason for this mod to fail a world.
    pub fn apply_summary(&mut self, summary: &LedgerSummary) {
        if let Ok(view) = self.view.get_mut(self.side) {
            view.count = summary.count;
            view.newest.clone_from(&summary.newest);
        }
    }

    /// Returns the summary a server would send for the ledger it holds.
    #[must_use]
    pub fn summary(&self) -> LedgerSummary {
        LedgerSummary {
            count: u32::try_from(self.waystones.len()).unwrap_or(u32::MAX),
            newest: self
                .waystones
                .last()
                .map(|waystone| waystone.name.clone())
                .unwrap_or_default(),
        }
    }

    /// Returns the tag of `kind` called `waystones` in the shared namespace.
    ///
    /// # Errors
    ///
    /// Returns an error when the kind cannot be tagged or its identifier is invalid.
    pub fn waystones_tag(kind: ContentKind) -> Result<TagId, ModError> {
        let namespace: ModId = CONVENTION.parse().map_err(ModError::from_display)?;
        TagId::new(
            kind,
            ContentId::new(namespace, "waystones").map_err(ModError::from_display)?,
        )
        .map_err(ModError::from_display)
    }

    /// Returns the command this mod claims.
    ///
    /// # Panics
    ///
    /// Panics if `waystone` stops being a legal content path.
    #[must_use]
    pub fn command() -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Self::declared_id().clone(), WAYSTONE)
                .expect("`waystone` is a legal content path"),
        )
    }

    /// Returns the path the block model is supplied at.
    ///
    /// # Errors
    ///
    /// Returns an error if the derived package path is invalid.
    pub fn model_path() -> Result<RelativePath, ModError> {
        RelativePath::try_from(format!(
            "assets/{}/models/block/{WAYSTONE}.json",
            Self::declared_id()
        ))
        .map_err(ModError::from_display)
    }

    /// Declares everything this mod contributes.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the packaged declarations conflict or are invalid.
    pub fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());

        let block = declaration
            .entry(ContentKind::Block, WAYSTONE)
            .map_err(ModError::from_display)?;
        let item = declaration
            .entry(ContentKind::Item, WAYSTONE)
            .map_err(ModError::from_display)?;
        declaration
            .entry(ContentKind::Recipe, &format!("crafting/{WAYSTONE}"))
            .map_err(ModError::from_display)?;
        declaration
            .entry(ContentKind::Command, WAYSTONE)
            .map_err(ModError::from_display)?;

        declaration
            .tag(&Self::waystones_tag(ContentKind::Block)?, &block)
            .map_err(ModError::from_display)?;
        declaration
            .tag(&Self::waystones_tag(ContentKind::Item)?, &item)
            .map_err(ModError::from_display)?;

        for (locale, name) in [("en_us", "Waystone"), ("de_de", "Wegstein")] {
            for prefix in ["block", "item"] {
                declaration
                    .translation(
                        locale,
                        &format!("{prefix}.{}.{WAYSTONE}", Self::declared_id()),
                        name,
                    )
                    .map_err(ModError::from_display)?;
            }
        }

        declaration
            .resource(Self::model_path()?, MODEL.to_vec())
            .map_err(ModError::from_display)?;

        Ok(declaration)
    }

    /// Produces the pack this mod ships.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the packaged declarations conflict or are invalid.
    pub fn generated() -> Result<GeneratedData, ModError> {
        Ok(Self::declaration()?.generate())
    }

    /// Returns the versions this build reads.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the packaged versions are invalid.
    pub fn saved_versions() -> Result<SavedDataDeclaration, ModError> {
        SavedDataDeclaration::new(
            Self::declared_id().clone(),
            VERSIONS.into_iter().map(version),
        )
        .map_err(ModError::from_display)
    }

    /// Returns the step into every version above the oldest.
    #[must_use]
    pub fn migrations() -> MigrationChain {
        MigrationChain::new(Self::declared_id().clone()).reaching(version(2), to_two)
    }

    /// Records `waystone`, honouring the capacity and overwrite settings in force.
    pub fn record(&mut self, waystone: Waystone) -> &Outcome {
        let name = waystone.name.clone();
        if let Some(existing) = self
            .waystones
            .iter_mut()
            .find(|held| held.name == waystone.name)
        {
            self.outcome = if self.overwrite {
                *existing = waystone;
                Outcome::Replaced(name)
            } else {
                Outcome::Kept(name)
            };
        } else if self.waystones.len() >= self.capacity {
            self.outcome = Outcome::Full;
        } else {
            self.waystones.push(waystone);
            self.outcome = Outcome::Recorded(name);
        }
        &self.outcome
    }

    /// Removes the waystone called `name`, if the ledger holds one.
    pub fn forget(&mut self, name: &str) -> &Outcome {
        let before = self.waystones.len();
        self.waystones.retain(|held| held.name != name);
        self.outcome = if self.waystones.len() == before {
            Outcome::NotUnderstood
        } else {
            Outcome::Forgotten(name.to_owned())
        };
        &self.outcome
    }
}

// The manifest declares all three sides, and it means all three for different
// reasons. Every side has to register the block, the item and the recipe, because
// a client that never claimed `block:waystone_ledger:waystone` has no such block
// to render or name and would disagree with the server about what a chunk holds.
// The ledger itself only ever changes on a server side: `/waystone` is run there,
// and `open_saved_data`/`save_saved_data` are a world's, and a world belongs to
// the integrated or dedicated server. The client's share is the summary a server
// sends it, which is why the only client-only state here is `ClientView` and why
// the packet's direction is `ToClient`. Declaring `["client"]` alone would strand
// every world; declaring the two server sides alone would leave a client with an
// unnamed block. So: all three, with the asymmetry expressed in the code rather
// than in the manifest.
#[declare_mod(id = "waystone_ledger")]
impl Mod for WaystoneLedger {
    /// Accepts every step. Nothing this mod sets up belongs to one phase.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    /// Supplies the content claimed by default registration, model included.
    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        Self::declaration().map(std::borrow::Cow::Owned).map(Some)
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        ConfigurationDeclaration::parse(SETTINGS)
            .map(Some)
            .map_err(ModError::from_display)
    }

    /// Reads the settings in force.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared key has the wrong shape or
    /// its capacity cannot be represented on this platform.
    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        self.capacity = resolved
            .integer_in::<usize>(CAPACITY)
            .map_err(ModError::from_display)?;

        self.overwrite = resolved
            .boolean(OVERWRITE)
            .map_err(ModError::from_display)?;
        Ok(())
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        Self::saved_versions().map(Some)
    }

    fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
        Ok(Some(Self::migrations()))
    }

    /// Reads the ledger the loader has already migrated.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] for unreadable data, leaving state unchanged.
    fn open_saved_data(&mut self, envelope: Option<&SavedDataEnvelope>) -> Result<(), ModError> {
        let Some(envelope) = envelope else {
            self.waystones.clear();
            return Ok(());
        };
        let stored = envelope
            .payload()
            .get("waystones")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| ModError::refused("the stored ledger holds no waystone list"))?;

        let mut waystones = Vec::with_capacity(stored.len());
        for value in stored {
            waystones.push(Waystone::from_json(value).ok_or_else(|| {
                ModError::refused(
                    "a stored waystone is missing a name, a coordinate or a dimension",
                )
            })?);
        }
        self.waystones = waystones;
        Ok(())
    }

    /// Answers with the ledger at the version this build writes.
    ///
    /// # Errors
    ///
    /// Returns a refusal if the packaged versions are invalid.
    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        let waystones: Vec<JsonValue> = self.waystones.iter().map(Waystone::to_json).collect();
        Ok(Some(SavedDataEnvelope::current(
            Self::declared_id().clone(),
            Self::saved_versions()?.current(),
            json!({ "waystones": waystones }),
        )))
    }

    /// Ignores every event. The ledger changes when a player says so, not on a
    /// tick, so counting ticks here would be work no one asked for.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        let _ = event;
        Ok(())
    }

    /// Runs `/waystone`, recording unrecognized input as [`Outcome::NotUnderstood`].
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let arguments: Vec<&str> = invocation.arguments.iter().map(String::as_str).collect();
        match arguments.as_slice() {
            ["set", name, x, y, z] => {
                let parsed = (x.parse::<i64>(), y.parse::<i64>(), z.parse::<i64>());
                if let (Ok(x), Ok(y), Ok(z)) = parsed {
                    self.record(Waystone {
                        name: (*name).to_owned(),
                        x,
                        y,
                        z,
                        dimension: ASSUMED_DIMENSION.to_owned(),
                    });
                } else {
                    self.outcome = Outcome::NotUnderstood;
                }
            }
            ["forget", name] => {
                self.forget(name);
            }
            _ => self.outcome = Outcome::NotUnderstood,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ASSUMED_DIMENSION, CAPACITY, ClientView, LedgerSummary, OVERWRITE, Outcome, VERSIONS,
        Waystone, WaystoneLedger, version,
    };
    use ravel_sdk::config::{LayeredConfiguration, ValueDocument, ValueSource};
    use ravel_sdk::content::{
        ContentDeclarationError, ContentKind, Registrar, RegistrationRefused, RegistryKey,
    };
    use ravel_sdk::events::CommandInvocation;
    use ravel_sdk::generate::GeneratedFile;
    use ravel_sdk::network::{
        Direction, Packet, PacketContract, PacketId, PacketReader, PacketWriter, WireError, decode,
        encode,
    };
    use ravel_sdk::resources::RelativePath;
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan};
    use ravel_sdk::sides::Side;
    use ravel_sdk::{Declared, JsonValue, LifecycleState, Mod, ModError, ModId, json};

    /// A registrar that accepts every claim, standing in for the loader's table.
    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    /// A registrar that refuses claims of one kind as already owned.
    struct Contested {
        kind: ContentKind,
        owner: ModId,
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Contested {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            if key.kind() == self.kind {
                return Err(RegistrationRefused::Conflict {
                    key,
                    owner: self.owner.clone(),
                });
            }
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    fn configured(side: Side) -> WaystoneLedger {
        let mut ledger = WaystoneLedger::new(side);
        let declaration = ledger
            .configuration_declaration()
            .expect("valid configuration declaration")
            .expect("this mod declares settings");
        let resolved =
            LayeredConfiguration::resolve(declaration, None, None).expect("nothing to layer");
        ledger.configure(&resolved).expect("its own defaults");
        ledger
    }

    fn waystone(name: &str) -> Waystone {
        Waystone {
            name: name.to_owned(),
            x: 10,
            y: 64,
            z: -30,
            dimension: ASSUMED_DIMENSION.to_owned(),
        }
    }

    fn opened(at: u32, payload: &JsonValue) -> Result<WaystoneLedger, ModError> {
        let mut ledger = configured(Side::IntegratedServer);
        let plan = SavedDataPlan::open(&WaystoneLedger::saved_versions()?, Some(version(at)))
            .expect("a declared version");
        let payload = WaystoneLedger::migrations()
            .apply(&plan, payload)
            .map_err(ModError::from_display)?;
        let current = WaystoneLedger::saved_versions()?.current();
        let envelope =
            SavedDataEnvelope::current(WaystoneLedger::declared_id().clone(), current, payload);
        ledger.open_saved_data(Some(&envelope))?;
        Ok(ledger)
    }

    #[test]
    fn the_mod_reports_the_identity_it_declared() {
        let expected: ModId = "waystone_ledger".parse().expect("a legal mod id");
        assert_eq!(WaystoneLedger::declared_id(), &expected);
        assert_eq!(WaystoneLedger::new(Side::Client).id(), &expected);
    }

    #[test]
    fn every_lifecycle_step_is_accepted() {
        let mut ledger = WaystoneLedger::new(Side::DedicatedServer);
        for state in LifecycleState::ALL {
            ledger.enter(state).expect("nothing is refused");
        }
    }

    #[test]
    fn registration_claims_every_declared_entry_in_declaration_order() {
        let mut ledger = WaystoneLedger::new(Side::IntegratedServer);
        let mut registrar = Accepting::default();
        ledger.register(&mut registrar).expect("nothing conflicts");
        assert_eq!(
            registrar
                .claimed()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [
                "block:waystone_ledger:waystone",
                "item:waystone_ledger:waystone",
                "recipe:waystone_ledger:crafting/waystone",
                "command:waystone_ledger:waystone",
            ]
        );
        assert!(registrar.claimed().contains(&WaystoneLedger::command()));
    }

    #[test]
    fn a_contested_entry_fails_the_mod_and_stops_at_the_refusal() {
        for kind in [
            ContentKind::Block,
            ContentKind::Item,
            ContentKind::Recipe,
            ContentKind::Command,
        ] {
            let owner: ModId = "other".parse().expect("a legal mod id");
            let mut registrar = Contested {
                kind,
                owner: owner.clone(),
                claimed: Vec::new(),
            };
            let error = WaystoneLedger::new(Side::Client)
                .register(&mut registrar)
                .expect_err("one kind is taken");

            assert!(
                matches!(&error, ModError::Refused { detail, .. }
                    if detail.contains(owner.as_str()) && detail.contains(kind.as_str())),
                "{error}"
            );

            let refused_at = WaystoneLedger::declaration()
                .expect("valid declaration")
                .entries()
                .iter()
                .position(|key| key.kind() == kind)
                .expect("every kind is declared");
            assert_eq!(registrar.claimed().len(), refused_at);
        }
    }

    #[test]
    fn the_block_and_the_item_join_the_shared_tag_for_their_own_registry() {
        let declaration = WaystoneLedger::declaration().expect("valid declaration");
        for kind in [ContentKind::Block, ContentKind::Item] {
            let tag = WaystoneLedger::waystones_tag(kind).expect("valid tag");
            let members = declaration.tag_members(&tag);
            assert_eq!(members.len(), 1, "{tag}");
            assert_eq!(members[0].kind(), kind);
        }
    }

    #[test]
    fn a_block_cannot_join_the_item_tag() {
        let mut declaration = WaystoneLedger::declaration().expect("valid declaration");
        let block = declaration
            .entries()
            .iter()
            .find(|key| key.kind() == ContentKind::Block)
            .expect("a block is declared")
            .clone();
        let error = declaration
            .tag(
                &WaystoneLedger::waystones_tag(ContentKind::Item).expect("valid tag"),
                &block,
            )
            .expect_err("an item tag holds items");
        assert!(
            matches!(error, ContentDeclarationError::TagKind { .. }),
            "{error}"
        );
    }

    #[test]
    fn the_generated_pack_holds_the_derived_files_and_the_supplied_model() {
        let generated = WaystoneLedger::generated().expect("valid declaration");
        assert_eq!(
            generated.paths().collect::<Vec<_>>(),
            [
                "assets/waystone_ledger/blockstates/waystone.json",
                "assets/waystone_ledger/items/waystone.json",
                "assets/waystone_ledger/lang/de_de.json",
                "assets/waystone_ledger/lang/en_us.json",
                "assets/waystone_ledger/models/block/waystone.json",
                "assets/waystone_ledger/models/item/waystone.json",
                "data/c/tags/block/waystones.json",
                "data/c/tags/item/waystones.json",
                "data/waystone_ledger/loot_table/blocks/waystone.json",
            ]
        );
        assert_eq!(
            generated
                .file(&WaystoneLedger::model_path().expect("valid model path"))
                .map(GeneratedFile::bytes),
            Some(super::MODEL)
        );

        let english = RelativePath::try_from("assets/waystone_ledger/lang/en_us.json".to_owned())
            .expect("a legal path");
        assert_eq!(
            std::str::from_utf8(
                generated
                    .file(&english)
                    .expect("English is declared")
                    .bytes()
            )
            .expect("generated JSON is UTF-8"),
            "{\n  \"block.waystone_ledger.waystone\": \"Waystone\",\n  \
             \"item.waystone_ledger.waystone\": \"Waystone\"\n}\n"
        );
    }

    #[test]
    fn generating_the_pack_twice_produces_the_same_bytes() {
        assert_eq!(
            WaystoneLedger::generated().expect("valid declaration"),
            WaystoneLedger::generated().expect("valid declaration")
        );
    }

    #[test]
    fn the_authors_defaults_are_what_a_mod_with_no_pack_and_no_player_file_reads() {
        let ledger = configured(Side::IntegratedServer);
        assert_eq!(ledger.capacity(), 32);
        assert!(ledger.overwrites());
    }

    #[test]
    fn a_pack_may_state_both_settings_and_the_mod_uses_them() {
        let pack = ValueDocument::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "waystone_ledger",
              "values": { "ledger.capacity": 2, "ledger.overwrite": false }
            }"#,
        )
        .expect("a legal override document");

        let mut ledger = WaystoneLedger::new(Side::DedicatedServer);
        let declaration = ledger
            .configuration_declaration()
            .expect("valid configuration declaration")
            .expect("this mod declares settings");
        let resolved = LayeredConfiguration::resolve(declaration, Some(&pack), None)
            .expect("the pack states declared keys");
        ledger.configure(&resolved).expect("legal values");

        assert_eq!(ledger.capacity(), 2);
        assert!(!ledger.overwrites());
        for key in [CAPACITY, OVERWRITE] {
            assert_eq!(resolved.source(key), Ok(ValueSource::Pack));
        }
    }

    #[test]
    fn the_ledger_refuses_to_grow_past_the_capacity_in_force() {
        let pack = ValueDocument::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "waystone_ledger",
              "values": { "ledger.capacity": 2 }
            }"#,
        )
        .expect("a legal override document");
        let mut ledger = WaystoneLedger::new(Side::IntegratedServer);
        let declaration = ledger
            .configuration_declaration()
            .expect("valid configuration declaration")
            .expect("this mod declares settings");
        let resolved =
            LayeredConfiguration::resolve(declaration, Some(&pack), None).expect("a declared key");
        ledger.configure(&resolved).expect("a legal value");

        ledger.record(waystone("home"));
        ledger.record(waystone("mine"));
        assert_eq!(ledger.record(waystone("farm")), &Outcome::Full);
        assert_eq!(ledger.waystones().len(), 2);
    }

    #[test]
    fn overwriting_is_the_settings_decision_rather_than_the_mods() {
        let mut permissive = configured(Side::IntegratedServer);
        permissive.record(waystone("home"));
        let outcome = permissive.record(Waystone {
            y: 12,
            ..waystone("home")
        });
        assert_eq!(outcome, &Outcome::Replaced("home".to_owned()));
        assert_eq!(permissive.waystones()[0].y, 12);

        let pack = ValueDocument::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "waystone_ledger",
              "values": { "ledger.overwrite": false }
            }"#,
        )
        .expect("a legal override document");
        let mut strict = WaystoneLedger::new(Side::IntegratedServer);
        let declaration = strict
            .configuration_declaration()
            .expect("valid configuration declaration")
            .expect("this mod declares settings");
        let resolved =
            LayeredConfiguration::resolve(declaration, Some(&pack), None).expect("a declared key");
        strict.configure(&resolved).expect("a legal value");

        strict.record(waystone("home"));
        let outcome = strict.record(Waystone {
            y: 12,
            ..waystone("home")
        });
        assert_eq!(outcome, &Outcome::Kept("home".to_owned()));
        assert_eq!(strict.waystones()[0].y, 64);
    }

    #[test]
    fn the_chain_covers_every_version_the_declaration_admits() {
        WaystoneLedger::migrations()
            .check_covers(&WaystoneLedger::saved_versions().expect("valid declaration"))
            .expect("every declared version above the oldest has a step");
        assert_eq!(
            WaystoneLedger::saved_versions()
                .expect("valid declaration")
                .current(),
            version(*VERSIONS.last().expect("`VERSIONS` is not empty"))
        );
    }

    #[test]
    fn a_version_one_world_keeps_every_point_and_gains_the_dimension_it_lacked() {
        let ledger = opened(
            1,
            &json!({ "points": [
                { "name": "home", "x": 10, "y": 64, "z": -30 },
                { "name": "mine", "x": -400, "y": 12, "z": 88 }
            ]}),
        )
        .expect("the step runs");

        assert_eq!(ledger.waystones().len(), 2);
        assert_eq!(ledger.waystones()[1].name, "mine");
        assert_eq!(ledger.waystones()[1].x, -400);
        assert!(
            ledger
                .waystones()
                .iter()
                .all(|held| held.dimension == ASSUMED_DIMENSION)
        );
    }

    #[test]
    fn a_version_one_shape_the_step_cannot_read_refuses_and_leaves_the_ledger_empty() {
        let refusal =
            opened(1, &json!({ "waystones": [] })).expect_err("version 1 named its list `points`");
        assert!(refusal.to_string().contains("points"), "{refusal}");

        let partial = opened(
            1,
            &json!({ "points": [{ "name": "home", "x": 1, "y": 2 }] }),
        )
        .expect_err("a point with no `z` is not a point");
        assert!(partial.to_string().contains('z'), "{partial}");
    }

    #[test]
    fn what_this_mod_writes_is_what_it_reads_back() {
        let mut ledger = configured(Side::IntegratedServer);
        ledger.record(waystone("home"));
        ledger.record(waystone("mine"));
        let envelope = ledger
            .save_saved_data()
            .expect("this mod owns world data")
            .expect("and produces an envelope");
        assert_eq!(envelope.data_version(), version(2));

        let mut reopened = configured(Side::IntegratedServer);
        reopened
            .open_saved_data(Some(&envelope))
            .expect("its own shape");
        assert_eq!(reopened.waystones(), ledger.waystones());
    }

    #[test]
    fn a_world_holding_nothing_this_mod_owns_initializes_fresh() {
        let mut ledger = configured(Side::IntegratedServer);
        ledger.record(waystone("stale"));
        ledger
            .open_saved_data(None)
            .expect("absence is not a refusal");
        assert_eq!(ledger.waystones(), []);
    }

    #[test]
    fn client_only_state_is_not_built_on_a_server_side() {
        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            let mut ledger = configured(side);
            assert!(ledger.client_view().is_none(), "{side:?}");
            // Applying a summary on a server side changes nothing and refuses
            // nothing, because there is nothing there to change.
            ledger.apply_summary(&LedgerSummary {
                count: 3,
                newest: "home".to_owned(),
            });
            assert!(ledger.client_view().is_none(), "{side:?}");
        }
    }

    #[test]
    fn a_client_learns_what_the_summary_carried_and_nothing_more() {
        let mut ledger = configured(Side::Client);
        assert_eq!(ledger.client_view(), Some(&ClientView::default()));
        ledger.apply_summary(&LedgerSummary {
            count: 2,
            newest: "mine".to_owned(),
        });
        assert_eq!(
            ledger.client_view(),
            Some(&ClientView {
                count: 2,
                newest: "mine".to_owned(),
            })
        );
        // The client learned a count, not the ledger.
        assert_eq!(ledger.waystones(), []);
    }

    #[test]
    fn the_summary_a_server_would_send_describes_the_ledger_it_holds() {
        let mut ledger = configured(Side::DedicatedServer);
        assert_eq!(
            ledger.summary(),
            LedgerSummary {
                count: 0,
                newest: String::new()
            }
        );
        ledger.record(waystone("home"));
        ledger.record(waystone("mine"));
        assert_eq!(
            ledger.summary(),
            LedgerSummary {
                count: 2,
                newest: "mine".to_owned()
            }
        );
    }

    #[test]
    fn the_summary_packet_round_trips_and_is_refused_every_way_it_can_be_wrong() {
        let sent = LedgerSummary {
            count: 2,
            newest: "mine".to_owned(),
        };
        let bytes = encode(&sent);
        assert_eq!(
            decode::<LedgerSummary>(&bytes).expect("its own bytes"),
            sent
        );

        for length in 0..bytes.len() {
            assert!(
                decode::<LedgerSummary>(&bytes[..length]).is_err(),
                "a buffer of {length} bytes is not this packet"
            );
        }

        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            decode::<LedgerSummary>(&trailing),
            Err(WireError::TrailingBytes { .. })
        ));

        assert_eq!(
            LedgerSummary::contract(),
            PacketContract::new(
                PacketId::new(
                    "waystone_ledger:summary"
                        .parse()
                        .expect("a legal content id")
                ),
                Direction::ToClient,
                1,
            )
        );
    }

    /// The same fields under a later contract version are not this packet.
    #[derive(Debug, Eq, PartialEq)]
    struct LaterSummary {
        count: u32,
        newest: String,
    }

    impl Packet for LaterSummary {
        fn contract() -> PacketContract {
            PacketContract::new(
                PacketId::new(
                    "waystone_ledger:summary"
                        .parse()
                        .expect("a legal content id"),
                ),
                Direction::ToClient,
                2,
            )
        }

        fn write(&self, writer: &mut PacketWriter) {
            writer.u32(self.count);
            writer.text(&self.newest);
        }

        fn read(reader: &mut PacketReader<'_>) -> Result<Self, WireError> {
            Ok(Self {
                count: reader.u32()?,
                newest: reader.text()?.to_owned(),
            })
        }
    }

    #[test]
    fn a_later_contract_version_is_refused_rather_than_read_as_this_one() {
        let bytes = encode(&LaterSummary {
            count: 2,
            newest: "mine".to_owned(),
        });
        assert!(
            matches!(
                decode::<LedgerSummary>(&bytes),
                Err(WireError::Version { .. })
            ),
            "a version-2 buffer must not decode as version 1"
        );
    }

    #[test]
    fn the_command_writes_the_ledger_and_a_typo_leaves_it_alone() {
        let mut ledger = configured(Side::IntegratedServer);
        let run = |ledger: &mut WaystoneLedger, arguments: &[&str]| {
            ledger
                .run_command(&CommandInvocation::new(
                    WaystoneLedger::command(),
                    Side::IntegratedServer,
                    arguments.iter().map(|word| (*word).to_owned()).collect(),
                ))
                .expect("a command never fails this mod");
        };

        run(&mut ledger, &["set", "home", "10", "64", "-30"]);
        assert_eq!(ledger.outcome(), &Outcome::Recorded("home".to_owned()));
        assert_eq!(ledger.waystones(), [waystone("home")]);

        run(&mut ledger, &["set", "home", "ten", "64", "-30"]);
        assert_eq!(ledger.outcome(), &Outcome::NotUnderstood);
        assert_eq!(ledger.waystones().len(), 1, "a typo wrote nothing");

        run(&mut ledger, &["teleport", "home"]);
        assert_eq!(ledger.outcome(), &Outcome::NotUnderstood);

        run(&mut ledger, &["forget", "home"]);
        assert_eq!(ledger.outcome(), &Outcome::Forgotten("home".to_owned()));
        assert_eq!(ledger.waystones(), []);
    }

    /// The guide is explicit that *nothing* checks that a package's `id` and its
    /// `#[declare_mod(id = …)]` agree, and that a mismatch namespaces every piece
    /// of this mod's content under a name the loader never heard of. Since no
    /// tooling recomputes it, this package recomputes it for itself — along with
    /// the same two-statements-of-one-fact problem for `savedDataVersions`.
    ///
    /// The manifest is read from disk rather than through `include_str!` on
    /// purpose: `package.json` records the digest of `src/lib.rs`, so embedding
    /// the manifest in this file would make each one's contents depend on the
    /// other's.
    #[test]
    fn the_manifest_and_the_code_state_the_same_identity_and_the_same_versions() {
        let manifest: JsonValue =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/package.json"))
                .expect("the package manifest sits beside `Cargo.toml`")
                .parse()
                .expect("the manifest is legal JSON");

        assert_eq!(
            manifest["id"].as_str(),
            Some(WaystoneLedger::declared_id().as_str()),
            "the manifest id and `#[declare_mod]` are two statements of one fact"
        );
        assert_eq!(
            manifest["savedDataVersions"]
                .as_array()
                .expect("this mod owns world data and says so")
                .iter()
                .map(
                    |value| u32::try_from(value.as_u64().expect("a version is a count"))
                        .expect("a version fits")
                )
                .collect::<Vec<_>>(),
            VERSIONS,
            "`savedDataVersions` and `saved_data_declaration` are two statements of one fact"
        );
        assert_eq!(
            manifest["configurationSchemas"][0].as_str(),
            Some("configuration.json"),
            "the settings document this mod compiles in is the one the package declares"
        );
    }

    #[test]
    fn no_refusal_this_mod_writes_names_a_path_or_a_secret() {
        let refusal = opened(1, &json!({ "note": "nothing" })).expect_err("no `points` list");
        let detail = refusal.to_string();
        assert!(!detail.contains('/'), "{detail}");
        assert!(!detail.contains('\\'), "{detail}");
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(|side| WaystoneLedger::new(side));
