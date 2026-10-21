//! The published interface surface of §15, which is what the dependents of §16
//! actually depend on.
//!
//! §15 is explicit that this is a design problem rather than a port: the upstream
//! declarations are written against the host's own types — stacks, levels,
//! positions, render buffers, codecs — so a counterpart cannot mirror them
//! shape for shape and publishes the same *capabilities* against Ravel's types
//! instead. What is carried here is the part that is an obligation rather than a
//! signature: what a dependent supplies, what this entry promises to do with it,
//! what is refused, and in which window.
//!
//! Two of the groups describe subsystems this entry does not implement
//! anywhere. They are published so that a consumer can compile against them
//! without depending on the mod that owns them, and [`Group::implemented`] says
//! which those are rather than leaving a reader to infer it from an empty
//! module.

use std::collections::BTreeMap;
use std::fmt;

use ravel_sdk::ModId;
use ravel_sdk::content::ContentId;

use crate::data::{Placement, Stack};
use crate::forms::{Pattern, PatternRefused, small_grid_positions};

mod primitives;

pub use primitives::{Argument, Effect, Instruction, Parameter, ParameterKind, Program};

/// The obligations §15 publishes, each one named for what it does.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Group {
    /// A dependent's module carried by the owning mod's screen.
    DisplayModules,
    /// A dependent's channel kind, ticked and persisted by a controller.
    Cabling,
    /// A dependent's instruction or function, run by a processor.
    ProgrammableMachines,
    /// Reading a dimension and shaping its generation.
    Dimensions,
    /// Versioned inventories and capped extraction across a system.
    BulkItems,
    /// The two sided capabilities a readout reads through.
    Readouts,
    /// Declaring an item carrier-compatible or cyclable.
    HandHeld,
    /// A dependent's block entity feeding the heads-up overlay.
    Overlay,
    /// A grid of stacks pushed into a card from a recipe browser.
    RecipeHandOff,
    /// Naming, creating and reaching relocation receivers.
    Relocation,
}

impl Group {
    /// Every group, in the order §15 states them.
    pub const ALL: [Self; 10] = [
        Self::DisplayModules,
        Self::Cabling,
        Self::ProgrammableMachines,
        Self::Dimensions,
        Self::BulkItems,
        Self::Readouts,
        Self::HandHeld,
        Self::Overlay,
        Self::RecipeHandOff,
        Self::Relocation,
    ];

    /// Returns whether anything in the entry implements this group's subsystem.
    ///
    /// Two do not, anywhere in the tree. They exist so a consumer can compile
    /// against them without depending on the mod that owns the subsystem, and
    /// §15 says so in as many words.
    #[must_use]
    pub const fn implemented(self) -> bool {
        !matches!(self, Self::Cabling | Self::ProgrammableMachines)
    }

    /// Returns whether the group's obligations can be exercised inside this
    /// package alone.
    ///
    /// The hand-held group cannot: this entry ships no carrier-compatible and no
    /// cyclable item of its own, so both obligations need a dependent or a
    /// fixture. §15 calls that a gap rather than a rule and so does this.
    #[must_use]
    pub const fn testable_alone(self) -> bool {
        !matches!(self, Self::HandHeld)
    }
}

impl fmt::Display for Group {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::DisplayModules => "display modules",
            Self::Cabling => "cabling network",
            Self::ProgrammableMachines => "programmable machines",
            Self::Dimensions => "dimension services",
            Self::BulkItems => "bulk item access",
            Self::Readouts => "readouts",
            Self::HandHeld => "hand-held declarations",
            Self::Overlay => "overlay feed",
            Self::RecipeHandOff => "recipe hand-off",
            Self::Relocation => "player relocation",
        };
        formatter.write_str(name)
    }
}

/// The five built-in kinds a display module's data may be.
///
/// A module whose data is none of these is refused until the dependent registers
/// a factory for it under its own namespace, which is §15's one stated refusal
/// for this group.
pub const BUILT_IN_DATA: [&str; 5] = ["amount", "text", "flag", "position", "stack"];

/// One display module a dependent supplies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    /// The namespace of the mod that supplies it.
    pub owner: ModId,
    /// The module's own name.
    pub name: String,
    /// What the module's data is.
    pub data: String,
    /// What running it costs the owning screen each tick.
    pub energy_per_tick: u32,
    /// Whether the module wants clicks forwarded to it.
    pub takes_clicks: bool,
}

/// One channel kind a dependent supplies to the cabling group.
///
/// Published and not implemented: nothing in this entry ticks a channel or
/// resolves a consumer. Registration enforces the window and consistency between
/// the identifier's namespace and the self-asserted owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelKind {
    /// The kind's identifier.
    pub identifier: ContentId,
    /// The namespace of the mod that supplies it.
    pub owner: ModId,
}

/// What a dimension read answers with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DimensionReport {
    /// Who owns the dimension.
    pub owner: ModId,
    /// How much energy it holds.
    pub energy: i64,
    /// How much it can hold.
    pub capacity: i64,
    /// How many probes are in it.
    pub probes: u32,
}

/// What one extraction from a bulk inventory produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Extracted {
    /// How many came out.
    pub count: u32,
    /// Whether the request was routed rather than taken directly.
    pub routed: bool,
}

/// The registration window and the tables the eight groups fill.
#[derive(Clone, Debug, Default)]
pub struct Surface {
    closed: bool,
    modules: Vec<Module>,
    data_factories: Vec<(ModId, ContentId)>,
    channels: Vec<ChannelKind>,
    instructions: primitives::Runtime,
    dimensions: BTreeMap<ContentId, DimensionReport>,
    rules: Vec<(String, String)>,
    overlay: Option<Overlay>,
    receivers: BTreeMap<String, Vec<(Placement, u32, String)>>,
}

impl Surface {
    /// Builds an open surface holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Closes the registration window.
    ///
    /// Every group's registrations happen at startup, and a registration after
    /// the window is refused rather than quietly accepted, which is the same
    /// rule Ravel's own registration barrier keeps.
    pub const fn close(&mut self) {
        self.closed = true;
    }

    /// Returns whether the window is still open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        !self.closed
    }

    fn window(&self) -> Result<(), SurfaceRefused> {
        if self.closed {
            return Err(SurfaceRefused::Closed);
        }
        Ok(())
    }

    /// Registers a factory for a data kind that is not one of the five built in.
    ///
    /// The namespace must match the supplied owner. This checks consistency,
    /// not caller authentication: the owner is self-asserted.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::Closed`] after the window and
    /// [`SurfaceRefused::ForeignNamespace`] for a kind outside the registrant's
    /// own namespace.
    pub fn register_data_factory(
        &mut self,
        owner: &ModId,
        kind: &ContentId,
    ) -> Result<(), SurfaceRefused> {
        self.window()?;
        if kind.namespace() != owner {
            return Err(SurfaceRefused::ForeignNamespace);
        }
        self.data_factories
            .push((owner.to_owned(), kind.to_owned()));
        Ok(())
    }

    /// Registers a display module.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::Closed`] after the window and
    /// [`SurfaceRefused::UnknownData`] for a module whose data is neither one of
    /// [`BUILT_IN_DATA`] nor a kind a factory has been registered for.
    pub fn register_module(&mut self, module: Module) -> Result<(), SurfaceRefused> {
        self.window()?;
        let known = BUILT_IN_DATA.contains(&module.data.as_str())
            || module
                .data
                .parse::<ContentId>()
                .is_ok_and(|data| self.data_factories.iter().any(|(_, kind)| kind == &data));
        if !known {
            return Err(SurfaceRefused::UnknownData { data: module.data });
        }
        self.modules.push(module);
        Ok(())
    }

    /// Returns the modules registered, in registration order.
    #[must_use]
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    /// Returns what one screen update costs across every registered module.
    #[must_use]
    pub fn module_cost(&self) -> u64 {
        self.modules
            .iter()
            .map(|module| u64::from(module.energy_per_tick))
            .sum()
    }

    /// Registers a channel kind.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::Closed`] after the window and
    /// [`SurfaceRefused::ForeignNamespace`] when the identifier and owner differ.
    pub fn register_channel(&mut self, channel: ChannelKind) -> Result<(), SurfaceRefused> {
        self.window()?;
        if channel.identifier.namespace() != &channel.owner {
            return Err(SurfaceRefused::ForeignNamespace);
        }
        self.channels.push(channel);
        Ok(())
    }

    /// Returns the channel kinds registered.
    #[must_use]
    pub fn channels(&self) -> &[ChannelKind] {
        &self.channels
    }

    /// Registers an instruction.
    ///
    /// A body step naming a parameter the declaration does not have is refused
    /// here rather than at the call: the declaration is what a dependent hands
    /// over at startup, so it is where a mismatch between the two is a startup
    /// fault instead of a runtime one.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::Closed`] after the window and
    /// [`SurfaceRefused::BadBody`] for a step naming a parameter outside the
    /// declared list.
    pub fn register_instruction(&mut self, instruction: Instruction) -> Result<(), SurfaceRefused> {
        self.window()?;
        self.instructions.register(instruction)
    }

    /// Runs an instruction against a program.
    ///
    /// Every parameter is evaluated to its declared kind before the body runs,
    /// which is what makes a body's own reads total. Two refusals are §15's:
    /// an unregistered instruction, and a parameter that resolved to nothing
    /// where the body asked for the form that may not be absent. A parameter
    /// that resolved to the wrong kind is refused here too, because "evaluated
    /// to the declared type" is otherwise a claim nothing checks.
    ///
    /// The body then runs against the program: §15 says the processor runs it
    /// against a program holding a last value, an optional craft ticket and a
    /// delay the body can set, and all three are written here.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::UnknownInstruction`],
    /// [`SurfaceRefused::AbsentParameter`], [`SurfaceRefused::WrongParameter`],
    /// [`SurfaceRefused::ParameterCount`] and [`SurfaceRefused::WrongOutput`].
    pub fn run(
        &self,
        program: &mut Program,
        name: &str,
        arguments: &[Argument],
    ) -> Result<(), SurfaceRefused> {
        self.instructions.run(program, name, arguments)
    }

    /// Publishes a dimension a dependent may read.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::Closed`] after the window and
    /// [`SurfaceRefused::ForeignNamespace`] when the identifier and owner differ.
    pub fn publish_dimension(
        &mut self,
        identifier: &ContentId,
        report: DimensionReport,
    ) -> Result<(), SurfaceRefused> {
        self.window()?;
        if identifier.namespace() != &report.owner {
            return Err(SurfaceRefused::ForeignNamespace);
        }
        self.dimensions.insert(identifier.to_owned(), report);
        Ok(())
    }

    /// Reads a dimension, answering with nothing for an identifier the owning
    /// mod did not create.
    ///
    /// Declared server-side only: a client asks the server rather than reading
    /// one of its own.
    #[must_use]
    pub fn read_dimension(&self, identifier: &ContentId) -> Option<&DimensionReport> {
        self.dimensions.get(identifier)
    }

    /// Inserts a generation rule for a dependent.
    ///
    /// §15 states the ordering in two clauses: a rule goes ahead of every rule
    /// added later, and behind that dependent's own earlier ones. The first
    /// clause already fixes the answer — rules apply in the order they were
    /// added — and the second is implied by it rather than adding anything, so
    /// this appends and nothing here reorders. Both clauses are held to the test
    /// below anyway, because a reader who expected the second to mean something
    /// separate should be able to see which reading is in force.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::Closed`] after the window. Rule order is the
    /// whole of what this obligation promises, and a rule accepted after the
    /// barrier changes that order for everything already registered.
    pub fn insert_rule(&mut self, owner: &str, rule: &str) -> Result<(), SurfaceRefused> {
        self.window()?;
        self.rules.push((owner.to_owned(), rule.to_owned()));
        Ok(())
    }

    /// Returns the generation rules in the order they would be applied.
    #[must_use]
    pub fn rules(&self) -> Vec<&str> {
        self.rules.iter().map(|(_, rule)| rule.as_str()).collect()
    }

    /// Takes one heads-up overlay feed from a dependent's block entity.
    ///
    /// A dependent supplies an orientation, an above-air test, a log and an
    /// update stamp, and the overlay is drawn client-side from the last feed. A feed older than the
    /// one already held is refused rather than applied: the stamp is what says
    /// which of two feeds is current, and applying an older one would draw a
    /// stale log over a fresh one.
    ///
    /// The log is bounded, because it is a list a dependent grows and the
    /// overlay is redrawn every frame from it.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::StaleFeed`] for a stamp at or below the one
    /// held, and [`SurfaceRefused::OverlayLog`] for a log past
    /// [`MAX_OVERLAY_LINES`].
    pub fn feed_overlay(&mut self, overlay: Overlay) -> Result<(), SurfaceRefused> {
        if overlay.log.len() > MAX_OVERLAY_LINES {
            return Err(SurfaceRefused::OverlayLog {
                lines: overlay.log.len(),
                limit: MAX_OVERLAY_LINES,
            });
        }
        if let Some(held) = &self.overlay
            && overlay.stamp <= held.stamp
        {
            return Err(SurfaceRefused::StaleFeed {
                held: held.stamp,
                offered: overlay.stamp,
            });
        }
        self.overlay = Some(overlay);
        Ok(())
    }

    /// Returns the overlay a client draws, where one has been fed.
    #[must_use]
    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    /// Accepts a grid of stacks pushed from a recipe browser.
    ///
    /// The server side of §17's transfer. The browser collects the first ingredient of each of ten slot views: the
    /// first is the result it is showing and the other nine are its ingredients,
    /// which go into the first three columns of the first three rows.
    ///
    /// The result is answered rather than written into the card's output
    /// position, because §9 makes that position one evaluation fills and never a
    /// player. `NOTES.md` records that divergence.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::PushedGrid`] for a grid that is not exactly
    /// [`BROWSER_GRID`] long, and [`SurfaceRefused::Pattern`] when a position
    /// refuses the stack it was handed.
    pub fn push_browser_grid(
        &self,
        pushed: &[Option<Stack>],
    ) -> Result<PushedGrid, SurfaceRefused> {
        if pushed.len() != BROWSER_GRID {
            return Err(SurfaceRefused::PushedGrid {
                given: pushed.len(),
                expected: BROWSER_GRID,
            });
        }
        let mut pattern = Pattern::new();
        for (position, stack) in small_grid_positions().into_iter().zip(&pushed[1..]) {
            pattern
                .set_input(position, stack.clone())
                .map_err(SurfaceRefused::Pattern)?;
        }
        Ok(PushedGrid {
            showing: pushed[0].clone(),
            pattern,
        })
    }

    /// Creates a relocation receiver at a stated power.
    ///
    /// Relocation is server-side in whole. A receiver is named, placed and
    /// powered, and the four reads below are what a dependent asks of the set.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::ReceiverTaken`] when something already stands
    /// at that placement, because two receivers at one position have no answer
    /// to [`Surface::receiver_name`].
    pub fn create_receiver(
        &mut self,
        at: Placement,
        power: u32,
        name: &str,
    ) -> Result<(), SurfaceRefused> {
        let dimension = self.receivers.entry(at.dimension.clone()).or_default();
        if dimension.iter().any(|(placed, _, _)| placed.at == at.at) {
            return Err(SurfaceRefused::ReceiverTaken { at: at.at });
        }
        dimension.push((at, power, name.to_owned()));
        Ok(())
    }

    /// Answers a receiver's name at a level and a position, or nothing.
    ///
    /// Nothing is the answer for a position that holds no receiver, which is
    /// this interface's own stated shape: it answers a name or nothing rather
    /// than refusing.
    #[must_use]
    pub fn receiver_name(&self, at: &Placement) -> Option<&str> {
        self.receivers
            .get(&at.dimension)?
            .iter()
            .find(|(placed, _, _)| placed.at == at.at)
            .map(|(_, _, name)| name.as_str())
    }

    /// Reads a receiver's name out of its item form.
    ///
    /// The item carries the name as a component, so a receiver held in an
    /// inventory answers the same question its placed form does.
    #[must_use]
    pub fn receiver_name_in_item(stack: &Stack) -> Option<&str> {
        if stack.is_empty() {
            return None;
        }
        stack.components.get(RECEIVER_NAME).map(String::as_str)
    }

    /// Moves a player to a dimension and a position.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceRefused::NoReceiver`] for a destination that holds no
    /// receiver. A move to a position nothing receives is what would put a
    /// player somewhere no dependent can get them back from.
    pub fn move_player(&self, player: &str, to: &Placement) -> Result<Moved, SurfaceRefused> {
        let power = self
            .receivers
            .get(&to.dimension)
            .and_then(|dimension| dimension.iter().find(|(placed, _, _)| placed.at == to.at))
            .map(|(_, power, _)| *power)
            .ok_or(SurfaceRefused::NoReceiver { at: to.at })?;
        Ok(Moved {
            player: player.to_owned(),
            to: to.clone(),
            power,
        })
    }

    /// Drops every destination in one dimension, answering how many went.
    #[must_use]
    pub fn drop_destinations(&mut self, dimension: &str) -> usize {
        self.receivers
            .remove(dimension)
            .map_or(0, |dropped| dropped.len())
    }

    /// Returns how many receivers stand in one dimension.
    #[must_use]
    pub fn receivers(&self, dimension: &str) -> usize {
        self.receivers.get(dimension).map_or(0, Vec::len)
    }
}

/// The most lines one overlay feed carries.
///
/// A dependent grows the log and the overlay is drawn from it every frame, so
/// the ceiling is a declared number to raise rather than a rule to rewrite.
pub const MAX_OVERLAY_LINES: usize = 64;

/// How many slot views §17's browser transfer collects.
///
/// One result and the nine positions of a plain grid.
pub const BROWSER_GRID: usize = 1 + SMALL_GRID;

/// How many ingredient positions a plain grid holds.
const SMALL_GRID: usize = 9;

/// The component a receiver's item form carries its name under.
pub const RECEIVER_NAME: &str = "receiver_name";

/// What a dependent's block entity feeds the heads-up overlay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Overlay {
    /// Which way the block faces.
    pub orientation: String,
    /// Whether the block has air above it, which is what decides where the
    /// overlay is drawn.
    pub above_air: bool,
    /// The lines the overlay draws.
    pub log: Vec<String>,
    /// Which feed this is, so a stale one can be told from a fresh one.
    pub stamp: u64,
}

/// What a browser push became.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushedGrid {
    /// The result the browser was showing.
    pub showing: Option<Stack>,
    /// The card the nine ingredients were written into.
    pub pattern: Pattern,
}

/// One player relocation, as the interface answers it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Moved {
    /// Who moved.
    pub player: String,
    /// Where they went.
    pub to: Placement,
    /// The power the receiver was created at.
    pub power: u32,
}

/// A bulk inventory a dependent supplies.
///
/// The version is what lets a reader skip work: it increases on every change and
/// never decreases, so a reader comparing it against what it last saw can tell
/// whether anything moved without walking the contents.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BulkInventory {
    version: u64,
    held: u32,
    powered: bool,
}

impl BulkInventory {
    /// Builds an inventory holding `held` items, powered or not.
    #[must_use]
    pub const fn new(held: u32, powered: bool) -> Self {
        Self {
            version: 0,
            held,
            powered,
        }
    }

    /// Returns the version.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Returns how many items the whole system holds.
    ///
    /// This may exceed a stack, which is why it is a count rather than a stack.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.held
    }

    /// Takes up to `amount` items, capped by the item's own stack limit.
    ///
    /// A request the providing block cannot power answers with an empty
    /// extraction rather than an error, which is §15's stated refusal for this
    /// group. Anything that moves raises the version.
    pub fn extract(&mut self, amount: u32, stack_limit: u32, routable: bool) -> Extracted {
        if !self.powered {
            return Extracted {
                count: 0,
                routed: routable,
            };
        }
        let taken = amount.min(stack_limit).min(self.held);
        if taken > 0 {
            self.held -= taken;
            self.version = self.version.saturating_add(1);
        }
        Extracted {
            count: taken,
            routed: routable,
        }
    }
}

/// Why the surface refused something.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceRefused {
    /// The registration window has closed.
    Closed,
    /// A data kind was registered outside the registrant's own namespace.
    ForeignNamespace,
    /// A module's data is neither built in nor registered.
    UnknownData {
        /// What the module declared.
        data: String,
    },
    /// Nothing is registered under that instruction name.
    UnknownInstruction {
        /// The name that was called.
        name: String,
    },
    /// A parameter resolved to nothing where the body asked for a value.
    AbsentParameter {
        /// Which parameter.
        index: usize,
    },
    /// A parameter resolved to a kind other than the declared one.
    WrongParameter {
        /// Which parameter.
        index: usize,
    },
    /// A call carried a different number of arguments than the declaration has.
    ParameterCount {
        /// How many the instruction declares.
        declared: usize,
        /// How many the call carried.
        given: usize,
    },
    /// A body step names a parameter the declaration does not have.
    BadBody {
        /// The index the step named.
        index: usize,
    },
    /// A body produced a value of a kind the instruction does not declare.
    WrongOutput {
        /// What the instruction declares it produces.
        declared: ParameterKind,
    },
    /// An overlay feed is not newer than the one already held.
    StaleFeed {
        /// The stamp the surface holds.
        held: u64,
        /// The stamp the feed carried.
        offered: u64,
    },
    /// An overlay feed's log is longer than one may be.
    OverlayLog {
        /// How many lines it carried.
        lines: usize,
        /// The most it may carry.
        limit: usize,
    },
    /// A browser push is not the shape the transfer collects.
    PushedGrid {
        /// How many slot views it carried.
        given: usize,
        /// How many it must carry.
        expected: usize,
    },
    /// A pushed grid's stack was refused by the position it was written to.
    Pattern(PatternRefused),
    /// Something already stands at that placement.
    ReceiverTaken {
        /// The position that is taken.
        at: (i32, i32, i32),
    },
    /// Nothing receives at that destination.
    NoReceiver {
        /// The position nothing receives at.
        at: (i32, i32, i32),
    },
}

impl fmt::Display for SurfaceRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("the registration window has closed"),
            Self::ForeignNamespace => {
                formatter.write_str("a data kind may only be registered in its owner's namespace")
            }
            Self::UnknownData { data } => {
                write!(
                    formatter,
                    "no factory is registered for the data kind {data}"
                )
            }
            Self::UnknownInstruction { name } => {
                write!(formatter, "nothing is registered under {name}")
            }
            Self::AbsentParameter { index } => write!(
                formatter,
                "parameter {index} resolved to nothing and the body asked for a value"
            ),
            Self::WrongParameter { index } => {
                write!(formatter, "parameter {index} resolved to the wrong kind")
            }
            Self::ParameterCount { declared, given } => write!(
                formatter,
                "the instruction declares {declared} parameters and the call carried {given}"
            ),
            Self::BadBody { index } => write!(
                formatter,
                "a body step names parameter {index}, which the declaration does not have"
            ),
            Self::WrongOutput { declared } => write!(
                formatter,
                "the body produced a value the instruction does not declare it produces, {declared:?}"
            ),
            Self::StaleFeed { held, offered } => write!(
                formatter,
                "the overlay holds feed {held} and {offered} is not newer than it"
            ),
            Self::OverlayLog { lines, limit } => write!(
                formatter,
                "an overlay log carries {lines} lines and {limit} is the most one may carry"
            ),
            Self::PushedGrid { given, expected } => write!(
                formatter,
                "a browser push carries {given} slot views and the transfer collects {expected}"
            ),
            Self::Pattern(refused) => write!(formatter, "{refused}"),
            Self::ReceiverTaken { at } => {
                write!(formatter, "a receiver already stands at {at:?}")
            }
            Self::NoReceiver { at } => write!(formatter, "nothing receives at {at:?}"),
        }
    }
}

impl std::error::Error for SurfaceRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pattern(refused) => Some(refused),
            Self::Closed
            | Self::ForeignNamespace
            | Self::UnknownData { .. }
            | Self::UnknownInstruction { .. }
            | Self::AbsentParameter { .. }
            | Self::WrongParameter { .. }
            | Self::ParameterCount { .. }
            | Self::BadBody { .. }
            | Self::WrongOutput { .. }
            | Self::StaleFeed { .. }
            | Self::OverlayLog { .. }
            | Self::PushedGrid { .. }
            | Self::ReceiverTaken { .. }
            | Self::NoReceiver { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_errors_preserve_the_complete_record_source_chain() {
        use crate::data::RecordRefused;
        use std::error::Error;

        let record = RecordRefused::Shape { record: None };
        let refused = SurfaceRefused::Pattern(PatternRefused::Record(record.clone()));
        let pattern = refused.source().expect("the wrapped pattern error");
        assert!(pattern.downcast_ref::<PatternRefused>().is_some());
        assert_eq!(
            pattern
                .source()
                .and_then(|source| source.downcast_ref::<RecordRefused>()),
            Some(&record)
        );
        assert!(SurfaceRefused::Closed.source().is_none());
    }

    fn module(data: &str) -> Module {
        Module {
            owner: "dependent".parse().expect("a legal owner"),
            name: "gauge".to_owned(),
            data: data.to_owned(),
            energy_per_tick: 4,
            takes_clicks: true,
        }
    }

    fn instruction() -> Instruction {
        Instruction {
            name: "hold".to_owned(),
            parameters: vec![
                Parameter {
                    kind: ParameterKind::Amount,
                    required: true,
                },
                Parameter {
                    kind: ParameterKind::Text,
                    required: false,
                },
            ],
            output: ParameterKind::Text,
            categories: vec!["general".to_owned()],
            body: vec![Effect::Produce { index: 1 }],
        }
    }

    #[test]
    fn two_of_the_groups_describe_subsystems_nothing_here_implements() {
        let unimplemented: Vec<Group> = Group::ALL
            .into_iter()
            .filter(|group| !group.implemented())
            .collect();
        assert_eq!(
            unimplemented,
            [Group::Cabling, Group::ProgrammableMachines],
            "they are published so a consumer can compile against them"
        );
    }

    #[test]
    fn one_group_cannot_be_exercised_inside_this_package_alone() {
        let needs_a_dependent: Vec<Group> = Group::ALL
            .into_iter()
            .filter(|group| !group.testable_alone())
            .collect();
        assert_eq!(needs_a_dependent, [Group::HandHeld]);
    }

    #[test]
    fn a_module_whose_data_is_built_in_is_accepted_and_an_unknown_one_is_not() {
        let mut surface = Surface::new();
        surface
            .register_module(module("amount"))
            .expect("amount is one of the five");
        assert_eq!(
            surface.register_module(module("dependent:curve")),
            Err(SurfaceRefused::UnknownData {
                data: "dependent:curve".to_owned()
            })
        );
        assert_eq!(surface.modules().len(), 1);
    }

    #[test]
    fn a_registered_factory_makes_the_dependents_own_data_kind_admissible() {
        let mut surface = Surface::new();
        surface
            .register_data_factory(
                &"dependent".parse().expect("a legal owner"),
                &"dependent:curve".parse().expect("a legal kind"),
            )
            .expect("its own namespace");
        surface
            .register_module(module("dependent:curve"))
            .expect("a factory is registered for it");
        assert_eq!(surface.modules().len(), 1);
    }

    #[test]
    fn a_dependent_cannot_register_a_data_kind_in_another_mods_namespace() {
        let mut surface = Surface::new();
        assert_eq!(
            surface.register_data_factory(
                &"dependent".parse().expect("a legal owner"),
                &"elsewhere:curve".parse().expect("a legal kind"),
            ),
            Err(SurfaceRefused::ForeignNamespace)
        );
    }

    #[test]
    fn nothing_registers_after_the_window_closes() {
        let mut surface = Surface::new();
        assert!(surface.is_open());
        surface.close();
        assert!(!surface.is_open());

        assert_eq!(
            surface.register_module(module("amount")),
            Err(SurfaceRefused::Closed)
        );
        assert_eq!(
            surface.register_channel(ChannelKind {
                identifier: "dependent:power".parse().expect("a legal identifier"),
                owner: "dependent".parse().expect("a legal owner"),
            }),
            Err(SurfaceRefused::Closed)
        );
        assert_eq!(
            surface.register_instruction(instruction()),
            Err(SurfaceRefused::Closed)
        );
        assert_eq!(
            surface.register_data_factory(
                &"dependent".parse().expect("a legal owner"),
                &"dependent:curve".parse().expect("a legal kind"),
            ),
            Err(SurfaceRefused::Closed)
        );
        assert_eq!(
            surface.publish_dimension(
                &"dependent:deep".parse().expect("a legal identifier"),
                DimensionReport {
                    owner: "dependent".parse().expect("a legal owner"),
                    energy: 0,
                    capacity: 0,
                    probes: 0,
                }
            ),
            Err(SurfaceRefused::Closed),
            "a dimension published after the barrier is one nobody who read the set sees"
        );
        assert_eq!(
            surface.insert_rule("dependent", "late"),
            Err(SurfaceRefused::Closed),
            "rule order is the whole of what the obligation promises"
        );
    }

    #[test]
    fn a_screens_cost_is_the_sum_of_what_its_modules_state() {
        let mut surface = Surface::new();
        for _ in 0..3 {
            surface
                .register_module(module("amount"))
                .expect("amount is built in");
        }
        assert_eq!(surface.module_cost(), 12);
    }

    #[test]
    fn every_parameter_is_evaluated_to_its_declared_kind_before_the_body_runs() {
        let mut surface = Surface::new();
        surface
            .register_instruction(instruction())
            .expect("the window is open");
        let mut program = Program::default();

        surface
            .run(
                &mut program,
                "hold",
                &[Argument::Amount(3), Argument::Text("note".to_owned())],
            )
            .expect("both parameters resolve to their declared kinds");
        assert_eq!(program.last, Some(Argument::Text("note".to_owned())));

        assert_eq!(
            surface.run(
                &mut program,
                "hold",
                &[Argument::Text("wrong".to_owned()), Argument::Absent]
            ),
            Err(SurfaceRefused::WrongParameter { index: 0 })
        );
    }

    #[test]
    fn a_parameter_that_resolved_to_nothing_refuses_only_where_it_was_required() {
        let mut surface = Surface::new();
        surface
            .register_instruction(instruction())
            .expect("the window is open");
        let mut program = Program::default();

        assert_eq!(
            surface.run(&mut program, "hold", &[Argument::Absent, Argument::Absent]),
            Err(SurfaceRefused::AbsentParameter { index: 0 })
        );
        surface
            .run(
                &mut program,
                "hold",
                &[Argument::Amount(1), Argument::Absent],
            )
            .expect("the second parameter did not ask for the non-absent form");
    }

    #[test]
    fn an_unregistered_instruction_and_a_miscounted_call_are_both_refused() {
        let mut surface = Surface::new();
        surface
            .register_instruction(instruction())
            .expect("the window is open");
        let mut program = Program::default();

        assert_eq!(
            surface.run(&mut program, "absent", &[]),
            Err(SurfaceRefused::UnknownInstruction {
                name: "absent".to_owned()
            })
        );
        assert_eq!(
            surface.run(&mut program, "hold", &[Argument::Amount(1)]),
            Err(SurfaceRefused::ParameterCount {
                declared: 2,
                given: 1
            })
        );
    }

    #[test]
    fn a_dimension_the_owning_mod_did_not_create_reads_as_nothing() {
        let mut surface = Surface::new();
        surface
            .publish_dimension(
                &"dependent:deep".parse().expect("a legal identifier"),
                DimensionReport {
                    owner: "dependent".parse().expect("a legal owner"),
                    energy: 12,
                    capacity: 100,
                    probes: 3,
                },
            )
            .expect("the window is open");

        assert_eq!(
            surface
                .read_dimension(&"dependent:deep".parse().expect("a legal identifier"))
                .expect("it was published")
                .probes,
            3
        );
        assert!(
            surface
                .read_dimension(&"dependent:absent".parse().expect("a legal identifier"))
                .is_none()
        );
    }

    #[test]
    fn a_rule_goes_ahead_of_later_rules_and_behind_its_owners_earlier_ones() {
        let mut surface = Surface::new();
        for (owner, rule) in [
            ("first", "first_one"),
            ("second", "second_one"),
            ("first", "first_two"),
            ("second", "second_two"),
        ] {
            surface
                .insert_rule(owner, rule)
                .expect("the window is open");
        }

        assert_eq!(
            surface.rules(),
            ["first_one", "second_one", "first_two", "second_two"],
            "a rule sits ahead of every rule added later, which already puts each \
             dependent's own rules in the order it added them"
        );
    }

    fn carrier_ready(identity: &str) -> Stack {
        Stack::of(identity).expect("a legal identity")
    }

    fn placement(at: (i32, i32, i32)) -> Placement {
        Placement::new("counterpart:deep", at).expect("a legal dimension")
    }

    #[test]
    fn a_body_writes_the_program_rather_than_the_last_argument() {
        let mut surface = Surface::new();
        let mut suspending = instruction();
        suspending.name = "wait".to_owned();
        suspending.body = vec![
            Effect::Produce { index: 1 },
            Effect::Await { index: 1 },
            Effect::Suspend { ticks: 7 },
        ];
        surface
            .register_instruction(suspending)
            .expect("the window is open");
        let mut program = Program::default();

        surface
            .run(
                &mut program,
                "wait",
                &[Argument::Amount(3), Argument::Text("craft".to_owned())],
            )
            .expect("both parameters resolve");
        assert_eq!(program.last, Some(Argument::Text("craft".to_owned())));
        assert_eq!(program.ticket, Some("craft".to_owned()));
        assert_eq!(program.delay, 7);
    }

    #[test]
    fn a_body_producing_a_kind_the_instruction_does_not_declare_is_refused() {
        let mut surface = Surface::new();
        let mut lying = instruction();
        lying.name = "lie".to_owned();
        lying.body = vec![Effect::Produce { index: 0 }];
        surface
            .register_instruction(lying)
            .expect("the window is open");
        let mut program = Program::default();

        assert_eq!(
            surface.run(
                &mut program,
                "lie",
                &[Argument::Amount(3), Argument::Text("note".to_owned())]
            ),
            Err(SurfaceRefused::WrongOutput {
                declared: ParameterKind::Text
            })
        );
        assert_eq!(program.last, None, "a refused run writes nothing");
    }

    #[test]
    fn a_body_step_naming_a_parameter_that_is_not_declared_is_a_startup_fault() {
        let mut surface = Surface::new();
        let mut wrong = instruction();
        wrong.body = vec![Effect::Produce { index: 2 }];
        assert_eq!(
            surface.register_instruction(wrong),
            Err(SurfaceRefused::BadBody { index: 2 })
        );
    }

    #[test]
    fn the_overlay_takes_the_newest_feed_and_refuses_an_older_one() {
        let mut surface = Surface::new();
        let feed = |stamp: u64, line: &str| Overlay {
            orientation: "north".to_owned(),
            above_air: true,
            log: vec![line.to_owned()],
            stamp,
        };

        surface
            .feed_overlay(feed(1, "first"))
            .expect("nothing was held");
        surface
            .feed_overlay(feed(2, "second"))
            .expect("two is newer than one");
        assert_eq!(surface.overlay().expect("a feed was taken").log, ["second"]);

        assert_eq!(
            surface.feed_overlay(feed(2, "again")),
            Err(SurfaceRefused::StaleFeed {
                held: 2,
                offered: 2
            })
        );
        assert_eq!(
            surface.overlay().expect("the held feed stands").log,
            ["second"],
            "a refused feed does not draw over the one that was current"
        );
    }

    #[test]
    fn an_overlay_log_past_its_ceiling_is_refused_before_it_is_held() {
        let mut surface = Surface::new();
        let overlong = Overlay {
            orientation: "north".to_owned(),
            above_air: false,
            log: vec!["line".to_owned(); MAX_OVERLAY_LINES + 1],
            stamp: 1,
        };
        assert_eq!(
            surface.feed_overlay(overlong),
            Err(SurfaceRefused::OverlayLog {
                lines: MAX_OVERLAY_LINES + 1,
                limit: MAX_OVERLAY_LINES,
            })
        );
        assert!(surface.overlay().is_none());
    }

    #[test]
    fn a_browser_push_writes_its_nine_ingredients_into_the_sub_grid() {
        let surface = Surface::new();
        let mut pushed: Vec<Option<Stack>> = Vec::with_capacity(BROWSER_GRID);
        pushed.push(Some(carrier_ready("counterpart:result")));
        for index in 0..SMALL_GRID {
            pushed.push(Some(carrier_ready(&format!("counterpart:in{index}"))));
        }

        let written = surface
            .push_browser_grid(&pushed)
            .expect("ten slot views is what the transfer collects");
        assert_eq!(
            written
                .showing
                .expect("the browser was showing a result")
                .identity
                .to_string(),
            "counterpart:result"
        );
        assert!(
            written.pattern.fits_small_grid(),
            "the nine go into the first three columns of the first three rows"
        );
        assert_eq!(
            written
                .pattern
                .small_grid_ingredients()
                .into_iter()
                .flatten()
                .count(),
            SMALL_GRID
        );
        assert!(
            written.pattern.output().is_none(),
            "the output position is one evaluation fills and never a push"
        );
    }

    #[test]
    fn a_browser_push_of_the_wrong_shape_is_refused() {
        let surface = Surface::new();
        assert_eq!(
            surface.push_browser_grid(&[None; 0]),
            Err(SurfaceRefused::PushedGrid {
                given: 0,
                expected: BROWSER_GRID
            })
        );
    }

    #[test]
    fn relocation_answers_a_name_creates_a_receiver_moves_a_player_and_drops_a_dimension() {
        let mut surface = Surface::new();
        let here = placement((4, 64, 9));
        assert!(surface.receiver_name(&here).is_none());

        surface
            .create_receiver(here.clone(), 500, "landing")
            .expect("nothing stands there yet");
        assert_eq!(surface.receiver_name(&here), Some("landing"));
        assert_eq!(
            surface.create_receiver(here.clone(), 500, "second"),
            Err(SurfaceRefused::ReceiverTaken { at: (4, 64, 9) })
        );

        let moved = surface
            .move_player("player", &here)
            .expect("something receives there");
        assert_eq!(moved.power, 500);
        assert_eq!(moved.to, here);

        assert_eq!(surface.drop_destinations("counterpart:deep"), 1);
        assert_eq!(surface.receivers("counterpart:deep"), 0);
        assert_eq!(
            surface.move_player("player", &here),
            Err(SurfaceRefused::NoReceiver { at: (4, 64, 9) }),
            "a move to a position nothing receives has no way back"
        );
    }

    #[test]
    fn a_receivers_item_form_carries_the_same_name_its_placed_form_answers() {
        let mut held = carrier_ready("counterpart:receiver");
        assert!(Surface::receiver_name_in_item(&held).is_none());

        held.components
            .insert(RECEIVER_NAME.to_owned(), "landing".to_owned());
        assert_eq!(Surface::receiver_name_in_item(&held), Some("landing"));

        held.count = 0;
        assert!(
            Surface::receiver_name_in_item(&held).is_none(),
            "an absent stack names no receiver"
        );
    }

    #[test]
    fn an_extraction_is_capped_by_the_amount_asked_for_and_by_the_stack_limit() {
        let mut inventory = BulkInventory::new(1_000, true);

        assert_eq!(
            inventory.extract(200, 64, false),
            Extracted {
                count: 64,
                routed: false
            }
        );
        assert_eq!(
            inventory.extract(10, 64, true),
            Extracted {
                count: 10,
                routed: true
            }
        );
        assert_eq!(
            inventory.count(),
            926,
            "the count across the system may exceed a stack"
        );
    }

    #[test]
    fn a_block_that_cannot_be_powered_answers_empty_rather_than_erroring() {
        let mut inventory = BulkInventory::new(1_000, false);
        assert_eq!(
            inventory.extract(64, 64, true),
            Extracted {
                count: 0,
                routed: true
            }
        );
        assert_eq!(inventory.count(), 1_000);
        assert_eq!(inventory.version(), 0);
    }

    #[test]
    fn the_version_rises_when_something_moves_and_stands_still_when_nothing_does() {
        let mut inventory = BulkInventory::new(5, true);
        assert_eq!(inventory.version(), 0);

        inventory.extract(5, 64, false);
        assert_eq!(inventory.version(), 1);

        inventory.extract(5, 64, false);
        assert_eq!(
            inventory.version(),
            1,
            "a reader that sees the same number can skip its work"
        );
    }
}
