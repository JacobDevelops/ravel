#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-287.
//!
//! This is a client-side rendering library with no gameplay of its own. It
//! declares no block, no item, no recipe, no tag, no loot and no world data,
//! and a player never encounters it directly. What its dependents observe is
//! one facility: a way to say, in a block's own state-to-model document, "draw
//! this block with a neighbour-aware model of kind K", and have a model built
//! for every position the block occupies.
//!
//! `NOTES.md` beside this file says which sections of the specification are
//! carried here and which are host-blocked because Ravel publishes no model
//! surface.
//!
//! What a dependent reaches for:
//!
//! - [`quadrant`] — §6, three booleans to one of five texture slots;
//! - [`conditions`] — §7, the connection condition language;
//! - [`neighbourhood`] — §8, the eight-sample probe and the face basis;
//! - [`geometry`] — §9, the seven per-kind geometries;
//! - [`slots`] — §10, the two texture slot schemes;
//! - [`lookup`] — §4.1 path one, the reload table;
//! - [`kinds`] — §5, the eight kinds and the table they live in.
//!
//! # Side
//!
//! **This entry does nothing whatsoever on a dedicated server.** The whole of
//! its state is a [`ClientOnly`] value, so on a server it is not merely hidden
//! but never built: no kind is registered and no reload is listened to. Two of
//! the three inbound edges are declared client-only and match that; the third
//! is declared required on both sides, so the counterpart still loads and
//! resolves on a server while contributing nothing there. Loading and doing
//! nothing is the correct server behaviour, not a gap.

pub mod conditions;
pub mod dumps;
pub mod geometry;
pub mod kinds;
pub mod lookup;
pub mod neighbourhood;
pub mod quadrant;
pub mod slots;

use ravel_sdk::events::GameEvent;
use ravel_sdk::prelude::*;
use ravel_sdk::render_stages::{RenderStage, StageDeclaration, StageOrder, StageRefused};
use ravel_sdk::sides::{ClientOnly, ClientOnlyError};
use ravel_sdk::{JsonValue, ModId};

pub use crate::conditions::{BlockState, Condition, Groups, Vocabulary};
pub use crate::geometry::{Continuation, Quad, Sheet, SheetFlags};
pub use crate::kinds::{Behaviour, Claim, Context, KindError, Layer, Model, ModelError, Registry};
pub use crate::lookup::{Catalogue, Lookup};
pub use crate::neighbourhood::{Axis, Face, Neighbourhood, Neighbours, Offset, Probe};
pub use crate::quadrant::Quadrant;
pub use crate::slots::{Slot, TextureId};

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_287_counterpart";

/// The most documents one reload may be handed before it is drained.
///
/// The documents a host hands over come from resource packs, so the queue is
/// untrusted input and is bounded rather than allowed to grow between reloads.
pub const MAX_PENDING: usize = 8_192;

/// The stage this library's drawing belongs at.
///
/// §11's baking is host-blocked, so nothing draws. Declaring the stage is what
/// a mod can say today about where its drawing would go, and it is the one
/// half of §11 that is decidable without a frame.
pub const STAGE: RenderStage = RenderStage::AfterTerrain;

/// What asking for one block's model answered.
///
/// [`Resolved::Unclaimed`] and [`Resolved::Refused`] are both "no model", and
/// keeping them apart is what makes a refused pack diagnosable rather than
/// silently inert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// No registered kind holds a declaration under that identifier.
    Unclaimed,
    /// A kind claimed the identifier and its declaration read into a model.
    Claimed(Box<Model>),
    /// A kind claimed the identifier and its declaration was refused.
    Refused {
        /// The kind identifier that claimed it.
        kind: String,
        /// Why the declaration could not be read.
        error: ModelError,
    },
}

impl Resolved {
    /// Returns the model, if a declaration read into one.
    #[must_use]
    pub fn model(&self) -> Option<&Model> {
        match self {
            Self::Claimed(model) => Some(model),
            Self::Unclaimed | Self::Refused { .. } => None,
        }
    }

    /// Returns why a claimed declaration was refused, if one was.
    #[must_use]
    pub const fn refusal(&self) -> Option<&ModelError> {
        match self {
            Self::Refused { error, .. } => Some(error),
            Self::Unclaimed | Self::Claimed(_) => None,
        }
    }
}

/// What this library holds on a client, and holds nowhere else.
#[derive(Clone, Debug, Default)]
pub struct ClientState {
    registry: Registry,
    vocabulary: Vocabulary,
    pending: Vec<(String, JsonValue)>,
    reloads: usize,
    refused: usize,
}

impl ClientState {
    /// Returns the kind table.
    #[must_use]
    pub const fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Returns the kind table for a host to fill a second lookup path into.
    pub const fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }

    /// Returns what the parser knows about blocks and groups.
    #[must_use]
    pub const fn vocabulary(&self) -> &Vocabulary {
        &self.vocabulary
    }

    /// Returns that vocabulary for the host to fill from its own registries.
    pub const fn vocabulary_mut(&mut self) -> &mut Vocabulary {
        &mut self.vocabulary
    }

    /// Returns how many reloads have been taken.
    #[must_use]
    pub const fn reloads_taken(&self) -> usize {
        self.reloads
    }

    /// Returns how many supplied documents were refused for the queue cap.
    #[must_use]
    pub const fn refused_documents(&self) -> usize {
        self.refused
    }

    /// Returns how many documents are waiting for the next reload.
    #[must_use]
    pub fn pending_documents(&self) -> usize {
        self.pending.len()
    }
}

/// The rendering library, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    side: Side,
    client: ClientOnly<ClientState>,
    fault: Option<KindError>,
    entered: Vec<LifecycleState>,
    observed: usize,
    last_observed: Option<GameEvent>,
}

impl Entry {
    /// Builds the library for the side it is running on.
    ///
    /// On a client the eight built-in kinds are registered here, which is where
    /// §2 puts them: at construction, before anything else. On a server the
    /// closure never runs, so nothing is registered and nothing is listened to.
    #[must_use]
    pub fn new(side: Side) -> Self {
        // A refusal here would be a packaging fault rather than input, and a
        // library that quietly held no kind would answer nothing to every
        // lookup for the life of the process. It is kept and raised at the
        // first lifecycle step instead, which is the seam the loader watches.
        let mut fault = None;
        let client = ClientOnly::built_on(side, || {
            let registry = match Registry::built_in() {
                Ok(registry) => registry,
                Err(refused) => {
                    fault = Some(refused);
                    Registry::default()
                }
            };
            ClientState {
                registry,
                ..ClientState::default()
            }
        });
        Self {
            side,
            client,
            fault,
            entered: Vec::new(),
            observed: 0,
            last_observed: None,
        }
    }

    /// Returns the side this library was built for.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the client-side state, on the side that has one.
    ///
    /// # Errors
    ///
    /// Returns [`ClientOnlyError::WrongSide`] on every server side, where there is no
    /// client-side state to return.
    pub fn client(&self) -> Result<&ClientState, ClientOnlyError> {
        self.client.get(self.side)
    }

    /// Returns the client-side state for writing, on the side that has one.
    ///
    /// # Errors
    ///
    /// The same as [`Entry::client`].
    pub fn client_mut(&mut self) -> Result<&mut ClientState, ClientOnlyError> {
        self.client.get_mut(self.side)
    }

    /// Returns whether this library holds any client-side state at all.
    ///
    /// This is the constructor's answer rather than the accessor's, so a caller
    /// can tell "never built" from "built and not readable from here".
    #[must_use]
    pub const fn runs_here(&self) -> bool {
        self.client.is_present()
    }

    /// Hands one declaration document to the next reload.
    ///
    /// The walk over loaded resource packs that produces these pairs needs a
    /// pack-reading surface Ravel does not publish, so the host supplies them
    /// and this holds them until [`GameEvent::ResourcesReloaded`] drains them.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on every server side.
    pub fn supply(&mut self, identifier: &str, document: JsonValue) -> Result<bool, ModError> {
        let client = self.client_mut().map_err(ModError::from_display)?;
        if client.pending.len() >= MAX_PENDING {
            client.refused += 1;
            return Ok(false);
        }
        client.pending.push((identifier.to_owned(), document));
        Ok(true)
    }

    /// Rebuilds every kind's table from what has been supplied since the last
    /// reload, and empties the queue.
    ///
    /// Returns how many documents were routed to a registered kind.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on every server side.
    pub fn reload(&mut self) -> Result<usize, ModError> {
        let client = self.client_mut().map_err(ModError::from_display)?;
        let supplied = std::mem::take(&mut client.pending);
        let routed = client.registry.reload(supplied);
        client.reloads += 1;
        Ok(routed)
    }

    /// Reads the model a block's declaration asks for, if a kind claims it.
    ///
    /// This is the whole facility a dependent observes, end to end: a kind
    /// identifier resolved through the table of §5, a declaration found through
    /// path one of §4.1, and a model of §9 and §10 built from it. Substituting
    /// that model into the game's model bakery is the host-blocked half.
    ///
    /// The answer has three states rather than two, because "nobody shipped a
    /// declaration for this" and "a declaration was shipped and refused" are
    /// different facts and §10 rejects the second with a syntax error. A
    /// counterpart that answered both with nothing would leave a pack author
    /// whose declaration names an unknown layer, an unreadable texture or an
    /// unknown slot with silence and an untextured block.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on every server side.
    pub fn model(&self, identifier: &str) -> Result<Resolved, ModError> {
        let client = self.client().map_err(ModError::from_display)?;
        let Some(claim) = client.registry.claim(identifier) else {
            return Ok(Resolved::Unclaimed);
        };
        match Model::parse(claim.behaviour, claim.document, &client.vocabulary) {
            Ok(model) => Ok(Resolved::Claimed(Box::new(model))),
            Err(error) => Ok(Resolved::Refused {
                kind: claim.kind.to_owned(),
                error,
            }),
        }
    }

    /// Declares where this library's drawing would go.
    ///
    /// # Errors
    ///
    /// Returns [`StageRefused`] if this declaration has a contributor-order conflict.
    pub fn stage_order(id: &ModId) -> Result<StageOrder, StageRefused> {
        let mut stages = StageDeclaration::default();
        stages.contribute(id, STAGE);
        stages.resolve()
    }

    /// Returns the lifecycle steps this library has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns how many signals this library has observed.
    ///
    /// A count and the last signal rather than the list: a client tick arrives
    /// every tick, so a list of them is the one accumulator in this package
    /// that nothing drains and nothing bounds.
    #[must_use]
    pub const fn events_observed(&self) -> usize {
        self.observed
    }

    /// Returns the last signal this library observed.
    #[must_use]
    pub const fn last_observed(&self) -> Option<GameEvent> {
        self.last_observed
    }
}

#[declare_mod(id = "rvl_287_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if let Some(fault) = &self.fault {
            return Err(ModError::refused(format!(
                "the built-in kind table was refused: {fault}"
            )));
        }
        self.entered.push(state);
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.observed = self.observed.saturating_add(1);
        self.last_observed = Some(event);
        if event == GameEvent::ResourcesReloaded && self.runs_here() {
            self.reload()?;
        }
        Ok(())
    }
}

export_mod!(|side| Entry::new(side));

#[cfg(test)]
mod tests {
    use ravel_sdk::json;

    use super::{Entry, Face, GameEvent, MAX_PENDING, MOD_ID, ModelError, Resolved, STAGE};
    use crate::kinds::{BUILT_IN, Context};
    use ravel_sdk::prelude::*;

    fn wall() -> ravel_sdk::JsonValue {
        json!({
            "shape": "joined_cube",
            "textures": {
                "alone": "base:one",
                "first_side": "base:one",
                "second_side": "base:one",
                "notch": "base:one",
                "enclosed": "base:two",
            },
        })
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_carries() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(Entry::new(Side::Client).id().as_str(), MOD_ID);
    }

    #[test]
    fn a_client_registers_the_eight_kinds_at_construction() {
        let library = Entry::new(Side::Client);
        assert!(library.runs_here());
        let client = library.client().expect("a client has client state");
        assert_eq!(client.registry().len(), BUILT_IN.len());
        assert_eq!(
            client.registry().names().collect::<Vec<_>>(),
            BUILT_IN
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_dedicated_server_registers_nothing_and_listens_to_nothing() {
        for side in [Side::DedicatedServer, Side::IntegratedServer] {
            let mut library = Entry::new(side);
            assert!(!library.runs_here(), "{side:?} builds no client state");
            assert!(library.client().is_err(), "{side:?}");
            assert!(library.supply("base:wall", wall()).is_err(), "{side:?}");
            assert!(library.reload().is_err(), "{side:?}");
            assert!(library.model("base:wall").is_err(), "{side:?}");

            // The lifecycle still runs and the signal is still observed: the
            // library loads and resolves on a server and contributes nothing.
            for state in LifecycleState::ALL {
                library
                    .enter(state)
                    .expect("every step is taken on every side");
            }
            library
                .observe(GameEvent::ResourcesReloaded)
                .expect("a reload on a server does nothing rather than failing");
            assert_eq!(library.entered(), LifecycleState::ALL, "{side:?}");
            assert_eq!(library.events_observed(), 1, "{side:?}");
            assert_eq!(
                library.last_observed(),
                Some(GameEvent::ResourcesReloaded),
                "{side:?}"
            );
        }
    }

    #[test]
    fn a_supplied_declaration_reaches_a_model_only_after_a_reload() {
        let mut library = Entry::new(Side::Client);
        assert!(
            library
                .supply("base:wall", wall())
                .expect("a client accepts one")
        );
        assert_eq!(
            library.client().expect("client").pending_documents(),
            1,
            "and holds it until a reload"
        );
        assert_eq!(
            library.model("base:wall").expect("client"),
            Resolved::Unclaimed,
            "nothing is claimable before the reload"
        );

        library
            .observe(GameEvent::ResourcesReloaded)
            .expect("the reload runs");

        let resolved = library.model("base:wall").expect("client");
        let model = resolved.model().expect("the kind claims it");
        let quads = model.quads(Face::North, &Context::default());
        assert_eq!(quads.len(), 4, "and the claimed kind draws");
        assert_eq!(library.client().expect("client").reloads_taken(), 1);
    }

    #[test]
    fn a_reload_drains_the_queue_so_the_same_document_is_not_taken_twice() {
        let mut library = Entry::new(Side::Client);
        library.supply("base:wall", wall()).expect("accepted");
        library
            .observe(GameEvent::ResourcesReloaded)
            .expect("the reload runs");
        assert_eq!(library.client().expect("client").pending_documents(), 0);
        assert!(
            library
                .model("base:wall")
                .expect("client")
                .model()
                .is_some()
        );

        // A reload that is handed nothing empties the table, which is what
        // makes disabling a pack take effect.
        library
            .observe(GameEvent::ResourcesReloaded)
            .expect("the reload runs");
        assert_eq!(
            library.model("base:wall").expect("client"),
            Resolved::Unclaimed,
            "the queue is drained rather than replayed"
        );
        assert_eq!(library.client().expect("client").reloads_taken(), 2);
    }

    #[test]
    fn the_supply_queue_is_capped_rather_than_grown_between_reloads() {
        let mut library = Entry::new(Side::Client);
        let mut accepted = 0usize;
        for index in 0..MAX_PENDING + 64 {
            if library
                .supply(&format!("base:d{index}"), wall())
                .expect("a client accepts or refuses")
            {
                accepted += 1;
            }
        }
        assert_eq!(accepted, MAX_PENDING);
        let client = library.client().expect("client");
        assert_eq!(client.pending_documents(), MAX_PENDING);
        assert_eq!(
            client.refused_documents(),
            64,
            "and the refusals are counted"
        );

        library
            .observe(GameEvent::ResourcesReloaded)
            .expect("the reload runs");
        assert_eq!(library.client().expect("client").pending_documents(), 0);
        assert!(
            library.supply("base:after", wall()).expect("client"),
            "and the queue takes documents again once it is drained"
        );
    }

    #[test]
    fn a_refused_declaration_is_answered_as_refused_rather_than_as_absent() {
        let mut library = Entry::new(Side::Client);
        let mut named_nothing = wall();
        named_nothing["layer"] = json!("nothing_names_this");
        library
            .supply("base:refused", named_nothing)
            .expect("accepted");
        let mut no_textures = wall();
        no_textures["textures"] = json!(null);
        library
            .supply("base:untextured", no_textures)
            .expect("accepted");
        library
            .observe(GameEvent::ResourcesReloaded)
            .expect("the reload runs");

        let refused = library.model("base:refused").expect("client");
        assert_eq!(
            refused,
            Resolved::Refused {
                kind: "joined_cube".to_owned(),
                error: ModelError::UnknownLayer("nothing_names_this".to_owned()),
            },
            "a declaration §10 rejects says which kind claimed it and why it was rejected"
        );
        assert!(refused.model().is_none());

        let untextured = library.model("base:untextured").expect("client");
        assert!(matches!(
            untextured.refusal(),
            Some(ModelError::Textures(_) | ModelError::NoTextures)
        ));

        assert_eq!(
            library.model("base:nobody_shipped_this").expect("client"),
            Resolved::Unclaimed,
            "and an identifier nobody shipped is a different answer from a refusal"
        );
    }

    #[test]
    fn an_unrelated_signal_does_not_rebuild_the_table() {
        let mut library = Entry::new(Side::Client);
        library.supply("base:wall", wall()).expect("accepted");
        for event in [GameEvent::ClientTick, GameEvent::ServerTick] {
            library.observe(event).expect("observed");
        }
        assert_eq!(library.client().expect("client").reloads_taken(), 0);
        assert_eq!(library.client().expect("client").pending_documents(), 1);
        assert_eq!(
            library.events_observed(),
            2,
            "both signals were still observed"
        );
        assert_eq!(library.last_observed(), Some(GameEvent::ServerTick));
    }

    #[test]
    fn this_library_declares_no_content_and_no_configuration() {
        let library = Entry::new(Side::Client);
        assert!(
            library
                .content_declaration()
                .expect("valid absent content")
                .is_none(),
            "it declares no block, item, recipe, tag, loot or world data"
        );
        assert!(
            library
                .configuration_declaration()
                .expect("valid absent configuration")
                .is_none()
        );
        assert!(
            library
                .saved_data_declaration()
                .expect("valid absent saved data")
                .is_none()
        );
    }

    #[test]
    fn the_stage_this_library_would_draw_at_is_a_published_one() {
        let order = Entry::stage_order(Entry::declared_id()).expect("a published stage");
        assert_eq!(order.at(STAGE), [Entry::declared_id().clone()]);
    }

    #[test]
    fn a_host_fills_the_vocabulary_the_condition_parser_reads() {
        let mut library = Entry::new(Side::Client);
        {
            let client = library.client_mut().expect("client");
            let taken = std::mem::take(client.vocabulary_mut());
            *client.vocabulary_mut() = taken.with_block("base:panel", ["lit"]);
        }
        let mut declaration = wall();
        declaration["connects"] = json!({ "test": "named_block", "block": "base:panel" });
        library.supply("base:wall", declaration).expect("accepted");
        library
            .observe(GameEvent::ResourcesReloaded)
            .expect("the reload runs");

        let resolved = library.model("base:wall").expect("client");
        let model = resolved.model().expect("claimed");
        assert!(
            matches!(
                model.condition(),
                Some(crate::conditions::Condition::NamedBlock { .. })
            ),
            "the block the host declared is a name the parser knows"
        );
    }

    #[test]
    fn public_refusals_are_readable_errors_and_preserve_their_cause() {
        use std::error::Error;

        let kind = crate::kinds::KindError::AlreadyRegistered("connected".to_owned());
        assert_eq!(
            kind.to_string(),
            "the kind `connected` is already registered"
        );
        let slot = crate::slots::SlotError::MissingCell(3);
        let model = ModelError::Textures(slot);
        assert_eq!(
            model.to_string(),
            "the texture set was refused: the texture grid declares no texture for cell 3"
        );
        assert_eq!(
            model.source().map(ToString::to_string),
            Some("the texture grid declares no texture for cell 3".to_owned())
        );
    }

    #[test]
    fn client_accessors_report_the_actual_server_side() {
        for side in [Side::DedicatedServer, Side::IntegratedServer] {
            let mut entry = Entry::new(side);
            assert_eq!(
                entry.client().expect_err("no client state").side(),
                Some(side)
            );
            assert_eq!(
                entry.client_mut().expect_err("no client state").side(),
                Some(side)
            );
        }
    }
}
