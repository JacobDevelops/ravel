#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-180.
//!
//! The entry this stands in for is a shared library with no gameplay of its own.
//! Its specification was written from the compiled artifact's non-code documents
//! and the project's public landing page, and from nothing else: no source, no
//! decompiled tree, and no compiled entry was opened by its author or by this
//! one. What that method can support is what is built here, and §12 of the
//! specification is a long list of what it cannot.
//!
//! What is carried, in the order the specification supports it:
//!
//! - [`grouping`] — the thirty-four shared groups, aggregates resolving through
//!   leaves;
//! - [`presentation`] — the eighteen translated lines and the two read-only
//!   displays behind them;
//! - [`commands`] — the three commands, including the removal that refuses a
//!   selection holding players;
//! - [`population`] — the population those commands read and change;
//! - [`sprites`] — the twenty-one interface sprite names and the one atlas
//!   document that stitches them;
//! - [`transport`] — this package's own wire contract for a display readout;
//! - [`colour`] — the sixteen dyes the groups are cut along;
//! - [`dumps`] — the three documents under `generated/`, each rendered out of
//!   the functions above.
//!
//! `NOTES.md` beside this file says which sections became which of those, what
//! is declarable only, and what was changed on purpose. The one thing it says
//! that belongs here too: this package satisfies the group, language, sprite and
//! side tests, and it is not wire-compatible with any dependent of the original,
//! because §10 records that no channel identifier, payload identifier, direction
//! or wire format is recoverable from any document in the artifact. That is the
//! true state of the specification and this package does not soften it.

pub mod colour;
pub mod commands;
pub mod dumps;
pub mod grouping;
pub mod population;
pub mod presentation;
pub mod sprites;
pub mod transport;

use ravel_sdk::content::{ContentDeclaration, ContentKind};
use ravel_sdk::events::{CommandInvocation, GameEvent};
use ravel_sdk::network::PacketDelivery;
use ravel_sdk::prelude::*;
use ravel_sdk::sides::ClientOnly;

pub use crate::colour::Dye;
pub use crate::commands::{Command, Console, Refusal, Report};
pub use crate::grouping::{Aggregation, GroupingError, Resolution};
pub use crate::population::{Line, Population, PopulationError, Selector};
pub use crate::presentation::{Display, Readout};
pub use crate::transport::Outbox;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_180_counterpart";

const OPERATOR_ONLY: &str = "operator-only until invocations carry a permission level";

/// How many readouts a client holds before it stops holding more.
///
/// A readout arrives per tick from whatever is showing one, and a display only
/// ever shows the newest, so the client keeps a short window and drops the
/// oldest rather than growing a log of frames nobody reads.
pub const MAX_HELD_READOUTS: usize = 16;

/// What a client keeps because it is a client.
///
/// Only a client receives a client-bound readout, so only a client has one to
/// remember. On a server side this is never built rather than being built and
/// hidden.
#[derive(Clone, Debug, Default)]
pub struct ClientState {
    received: Vec<Readout>,
}

impl ClientState {
    /// Returns the readouts held, oldest first.
    #[must_use]
    pub fn received(&self) -> &[Readout] {
        &self.received
    }

    /// Returns the newest readout, if one has arrived.
    #[must_use]
    pub fn latest(&self) -> Option<&Readout> {
        self.received.last()
    }

    /// Holds one readout, dropping the oldest when the window is full.
    pub fn hold(&mut self, readout: Readout) {
        if self.received.len() >= MAX_HELD_READOUTS {
            self.received.remove(0);
        }
        self.received.push(readout);
    }
}

/// The shared library, as the loader drives it.
#[derive(Debug)]
pub struct Entry {
    side: Side,
    declaration: Result<ContentDeclaration, ModError>,
    aggregation: Aggregation,
    console: Console,
    outbox: Outbox,
    client: ClientOnly<ClientState>,
    entered: Vec<LifecycleState>,
    observed: Vec<GameEvent>,
}

impl Entry {
    /// Builds the library on `side`, holding its groups and no readout.
    ///
    /// The loader builds a mod infallibly, so a packaging failure here cannot be
    /// returned. It is kept rather than dropped: whatever went wrong is held as
    /// the declaration and is what [`Mod::register`] reports, so the cause
    /// reaches the loader's report instead of being replaced by a generic one.
    #[must_use]
    pub fn new(side: Side) -> Self {
        let (aggregation, declaration) = match Aggregation::conventional() {
            Ok(aggregation) => {
                let declaration = Self::declaration(&aggregation);
                (aggregation, declaration)
            }
            Err(source) => (Aggregation::default(), Err(ModError::from_display(source))),
        };
        Self {
            side,
            declaration,
            aggregation,
            console: Console::new(),
            outbox: Outbox::new(),
            client: ClientOnly::built_on(side, ClientState::default),
            entered: Vec::new(),
            observed: Vec::new(),
        }
    }

    /// Returns the side this library was built on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the nested groups this package contributes.
    #[must_use]
    pub const fn aggregation(&self) -> &Aggregation {
        &self.aggregation
    }

    /// Returns the groups for contribution.
    ///
    /// A dependent adding a coloured block to a leaf writes it here. The
    /// declaration Ravel receives is built from the resolution, so a
    /// contribution made after [`Entry::rebuild_declaration`] last ran is
    /// reached by running it again.
    pub const fn aggregation_mut(&mut self) -> &mut Aggregation {
        &mut self.aggregation
    }

    /// Returns what the three commands read and change.
    #[must_use]
    pub const fn console(&self) -> &Console {
        &self.console
    }

    /// Returns the population and the held notes for filling in.
    pub const fn console_mut(&mut self) -> &mut Console {
        &mut self.console
    }

    /// Returns the encoded readouts waiting to be handed over.
    #[must_use]
    pub const fn outbox(&self) -> &Outbox {
        &self.outbox
    }

    /// Returns what this client has been told, on a client.
    ///
    /// # Errors
    ///
    /// Returns [`ravel_sdk::sides::ClientOnlyError::WrongSide`] on a server side, where there is
    /// no client view because nothing built one.
    pub fn client(&self) -> Result<&ClientState, ravel_sdk::sides::ClientOnlyError> {
        self.client.get(self.side)
    }

    /// Returns the lifecycle steps this library has taken.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns the signals this library has observed.
    #[must_use]
    pub fn observed(&self) -> &[GameEvent] {
        &self.observed
    }

    /// Queues a readout for the client end.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on a side that may not send a client-bound
    /// packet, which is every client end.
    pub fn publish(&mut self, readout: &Readout) -> Result<bool, ModError> {
        self.outbox
            .queue(readout, self.side)
            .map_err(ModError::from_display)
    }

    /// Hands over the encoded readouts and empties the queue.
    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        self.outbox.take()
    }

    /// Rebuilds the declaration from the groups as they now stand.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a declared spelling is illegal, which
    /// for this package's constants would be a packaging failure.
    pub fn rebuild_declaration(&mut self) -> Result<(), ModError> {
        self.declaration = Ok(Self::declaration(&self.aggregation)?);
        Ok(())
    }

    /// Builds what this package declares to Ravel from `aggregation`.
    ///
    /// The thirty-four groups, each carrying what resolving it reached; the
    /// eighteen translated lines; the three command names; and the atlas
    /// document. No block, item or recipe: §1 counts this entry's contribution
    /// to a world as zero of each.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] when a spelling this package holds as a
    /// constant is illegal.
    pub fn declaration(aggregation: &Aggregation) -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());

        for root in commands::ROOTS {
            declaration
                .entry(ContentKind::Command, root)
                .map_err(ModError::from_display)?;
        }

        for group in grouping::all().map_err(ModError::from_display)? {
            let resolved = aggregation.resolve(&group);
            if resolved.too_deep {
                return Err(ModError::refused(format!(
                    "{group} names a group past the reference depth bound, so the members it \
                     declares would be short of what it holds"
                )));
            }
            for member in resolved.members {
                declaration
                    .tag(&group, &member)
                    .map_err(ModError::from_display)?;
            }
        }

        for (key, line) in presentation::LINES {
            declaration
                .translation(presentation::LOCALE, key, line)
                .map_err(ModError::from_display)?;
        }

        declaration
            .resource(
                sprites::atlas_path().map_err(ModError::from_display)?,
                sprites::atlas_document(),
            )
            .map_err(ModError::from_display)?;

        Ok(declaration)
    }
}

impl Default for Entry {
    fn default() -> Self {
        Self::new(Side::DedicatedServer)
    }
}

#[declare_mod(id = "rvl_180_counterpart")]
impl Mod for Entry {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        self.declaration
            .as_ref()
            .map(std::borrow::Cow::Borrowed)
            .map(Some)
            .map_err(Clone::clone)
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        self.observed.push(event);
        Ok(())
    }

    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let root = invocation.key.id().path();
        let command = Command::from_root(root).ok_or_else(|| {
            ModError::refused(format!("`{root}` is not a command this package claimed"))
        })?;
        if matches!(command, Command::Reclaim | Command::Sweep) {
            return Err(ModError::refused(OPERATOR_ONLY));
        }
        let report = self
            .console
            .run(command, invocation.side, &invocation.arguments)
            .map_err(ModError::from_display)?;
        for key in report.keys() {
            self.console.retain(key.to_string());
        }
        Ok(())
    }

    fn receive_packet(&mut self, delivery: &PacketDelivery) -> Result<(), ModError> {
        let readout = delivery.decode::<Readout>().map_err(ModError::from)?;
        self.client
            .get_mut(self.side)
            .map_err(ModError::from_display)?
            .hold(readout);
        Ok(())
    }
}

export_mod!(|side| Entry::new(side));

#[cfg(test)]
mod tests {
    use super::{__ravel_entry, ClientState, Entry, MAX_HELD_READOUTS, MOD_ID, OPERATOR_ONLY};
    use crate::colour::Dye;
    use crate::commands::{Command, Report};
    use crate::grouping::{self, TAG_COUNT};
    use crate::presentation::{self, Readout};
    use crate::sprites;
    use ravel_sdk::content::{ContentKind, RegistryKey};
    use ravel_sdk::events::{CommandInvocation, GameEvent};
    use ravel_sdk::network::PacketDelivery;
    use ravel_sdk::prelude::*;
    use ravel_sdk::tags::TagId;

    fn invocation(root: &str, side: Side, arguments: &[&str]) -> CommandInvocation {
        let key = RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Entry::declared_id().clone(), root).expect("a legal content path"),
        );
        CommandInvocation::new(
            key,
            side,
            arguments.iter().map(|word| (*word).to_owned()).collect(),
        )
    }

    #[test]
    fn the_declared_identity_is_the_one_the_package_carries() {
        assert_eq!(Entry::declared_id().as_str(), MOD_ID);
        assert_eq!(Entry::new(Side::Client).id().as_str(), MOD_ID);
    }

    #[test]
    fn the_declaration_carries_thirty_four_groups_eighteen_lines_and_the_atlas() {
        let shared = Entry::new(Side::DedicatedServer);
        let declaration = shared
            .content_declaration()
            .expect("the content declaration is valid")
            .expect("the package declares content");

        assert_eq!(declaration.tags().count(), TAG_COUNT);
        assert_eq!(
            declaration.locales().collect::<Vec<_>>(),
            [presentation::LOCALE]
        );
        for (key, line) in presentation::LINES {
            assert_eq!(
                declaration.translated(presentation::LOCALE, key),
                Some(line),
                "{key} is declared and not supplied"
            );
        }

        let generated = declaration.generate();
        let atlas = sprites::atlas_path().expect("a legal path");
        assert!(
            generated.file(&atlas).is_some(),
            "the atlas document is what stitches the sprite set"
        );
        for group in grouping::all().expect("the constants are legal") {
            assert!(
                generated.file(&group.generated_path()).is_some(),
                "{group} is declared and not generated"
            );
        }
    }

    #[test]
    fn each_aggregate_is_generated_holding_what_its_leaves_hold() {
        let mut shared = Entry::new(Side::DedicatedServer);
        let whole = grouping::aggregate(ContentKind::Block).expect("a legal aggregate");
        let part = grouping::leaf(ContentKind::Block, Dye::Red).expect("a legal leaf");
        let theirs = RegistryKey::new(
            ContentKind::Block,
            ContentId::new(
                "dependent".parse().expect("a legal namespace"),
                "red_padding",
            )
            .expect("a legal content path"),
        );

        shared
            .aggregation_mut()
            .add_member(&part, theirs)
            .expect("a block joins a block group");
        shared
            .rebuild_declaration()
            .expect("the declaration rebuilds");

        let declaration = shared
            .content_declaration()
            .expect("the content declaration is valid")
            .expect("the package declares content");
        assert_eq!(declaration.tag_members(&whole).len(), 17);
        assert_eq!(declaration.tag_members(&part).len(), 2);
    }

    #[test]
    fn a_player_cannot_remove_entities_without_a_reviewed_permission_level() {
        let mut shared = Entry::new(Side::DedicatedServer);
        shared
            .console_mut()
            .population_mut()
            .record("cow", 4, false)
            .expect("a legal kind");
        assert_eq!(
            shared.run_command(&invocation("entity_sweep", Side::DedicatedServer, &["cow"])),
            Err(ModError::refused(OPERATOR_ONLY))
        );
        assert_eq!(shared.console().population().count_of("cow"), 4);
        assert_eq!(shared.console().notes_held(), 0);
    }

    #[test]
    fn a_command_typed_on_a_client_is_refused_and_a_name_nobody_claimed_is_refused() {
        let mut shared = Entry::new(Side::Client);
        shared
            .console_mut()
            .population_mut()
            .record("cow", 4, false)
            .expect("a legal kind");

        assert!(
            shared
                .run_command(&invocation("entity_sweep", Side::Client, &["cow"]))
                .is_err()
        );
        assert_eq!(shared.console().population().count_of("cow"), 4);
        assert!(
            shared
                .run_command(&invocation("nothing_claimed", Side::DedicatedServer, &[]))
                .is_err()
        );
    }

    #[test]
    fn a_player_cannot_release_retained_state_without_a_reviewed_permission_level() {
        let mut shared = Entry::new(Side::IntegratedServer);
        assert!(shared.console_mut().retain("held".to_owned()));
        assert_eq!(
            shared.run_command(&invocation("reclaim_held", Side::IntegratedServer, &[])),
            Err(ModError::refused(OPERATOR_ONLY))
        );
        assert_eq!(shared.console().notes_held(), 1);
        assert_eq!(
            shared
                .console_mut()
                .run(Command::Reclaim, Side::IntegratedServer, &[])
                .expect("the underlying operation remains available"),
            Report::Reclaimed {
                before: 1,
                after: 0,
                released: 1,
            }
        );
    }

    #[test]
    fn a_count_the_host_drove_is_still_reported_through_a_retained_note() {
        let mut shared = Entry::new(Side::DedicatedServer);
        shared
            .run_command(&invocation("entity_tally", Side::DedicatedServer, &[]))
            .expect("a server runs the count");
        assert_eq!(
            shared.console().notes_held(),
            1,
            "only the reclamation is exempt from retaining its report"
        );
    }

    #[test]
    fn a_group_nested_past_the_depth_bound_refuses_the_declaration_rather_than_shipping_it_short() {
        let mut shared = Entry::new(Side::DedicatedServer);
        let part = grouping::leaf(ContentKind::Block, Dye::Red).expect("a legal leaf");
        let mut previous = part;
        for step in 0..=grouping::MAX_DEPTH {
            let next = TagId::new(
                ContentKind::Block,
                ContentId::new(
                    "c".parse().expect("a legal namespace"),
                    format!("deep/{step}"),
                )
                .expect("a legal content path"),
            )
            .expect("blocks are grouped");
            shared
                .aggregation_mut()
                .refer(&previous, next.clone())
                .expect("a group may name any group");
            previous = next;
        }

        assert!(
            shared.rebuild_declaration().is_err(),
            "a declaration short of what the groups hold is a packaging failure, not a document"
        );
    }

    #[test]
    fn a_server_publishes_a_readout_and_the_client_that_receives_it_holds_it() {
        let readout = Readout::fluid(4_000, 1_000, "water");

        let mut server = Entry::new(Side::DedicatedServer);
        assert!(server.publish(&readout).expect("a server may send one"));
        assert_eq!(server.take_outgoing().len(), 1);
        assert_eq!(server.outbox().pending(), 0);
        assert!(
            server.client().is_err(),
            "a server holds no client view at all"
        );

        let mut client = Entry::new(Side::Client);
        assert!(
            client.publish(&readout).is_err(),
            "a client may not send a client-bound readout"
        );
        let delivery = PacketDelivery::of(&readout, Side::Client).expect("a client receives one");
        client
            .receive_packet(&delivery)
            .expect("the client reads what it was sent");
        assert_eq!(
            client.client().expect("a client has a view").latest(),
            Some(&readout)
        );
    }

    #[test]
    fn what_a_client_holds_is_a_window_rather_than_a_log() {
        let mut view = ClientState::default();
        for stored in 0..u32::try_from(MAX_HELD_READOUTS * 4).expect("a small count") {
            view.hold(Readout::energy(1_000_000, stored));
        }
        assert_eq!(view.received().len(), MAX_HELD_READOUTS);
        assert_eq!(
            view.latest().map(Readout::stored),
            u32::try_from(MAX_HELD_READOUTS * 4 - 1).ok()
        );
    }

    #[test]
    fn the_library_runs_the_whole_lifecycle_and_records_what_it_observes() {
        let mut shared = Entry::new(Side::DedicatedServer);
        for state in LifecycleState::ALL {
            shared.enter(state).expect("the library accepts every step");
        }
        shared
            .observe(GameEvent::ServerTick)
            .expect("the library records what it observes");
        assert_eq!(shared.entered(), LifecycleState::ALL);
        assert_eq!(shared.observed(), [GameEvent::ServerTick]);
    }

    #[test]
    fn the_host_builds_this_package_on_either_side() {
        for side in [Side::Client, Side::DedicatedServer, Side::IntegratedServer] {
            assert_eq!(__ravel_entry(side).id().as_str(), MOD_ID);
        }
    }
}
