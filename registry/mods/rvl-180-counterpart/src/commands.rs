//! The three commands, and what each one does when it runs.
//!
//! §5 of the specification recovers three commands from their message keys: an
//! entity count, a memory reclamation that reports a reading before and after a
//! collection it performs, and a bulk entity removal that reports a total, a
//! line per kind, an empty case, and a distinct refusal for a selection that
//! would remove players. The refusal is a behaviour a counterpart keeps.
//!
//! §5 also records that the side and the permission level of each are unknown,
//! and says the safe decision for the second and third is server-side with an
//! operator requirement. This package takes that decision and extends it to the
//! first, because counting what is loaded reads world state as much as removing
//! it does. The side is enforced here. The permission level is declared and not
//! enforced: a [`ravel_sdk::events::CommandInvocation`] carries the side, the
//! key and the words a player typed, and names no invoker and no level, so there
//! is nothing to compare a level against. `NOTES.md` lists that under what is
//! declarable only.

use std::fmt;

use ravel_sdk::Side;

use crate::census::{Census, CensusError, Line, Selector};

/// The root literal of the entity count.
pub const TALLY: &str = "entity_tally";

/// The root literal of the memory reclamation.
pub const RECLAIM: &str = "reclaim_held";

/// The root literal of the bulk removal.
pub const SWEEP: &str = "entity_sweep";

/// Every root literal this package claims, in the order §5 numbers them.
pub const ROOTS: [&str; 3] = [TALLY, RECLAIM, SWEEP];

/// The permission level each command is declared to need.
///
/// Declared, not enforced: see this module's own documentation and `NOTES.md`.
/// Two is the level the base game gives an operator command that changes a
/// running world.
pub const REQUIRED_LEVEL: u8 = 2;

/// How many notes the console holds before it stops holding more.
///
/// The removal command writes one note per kind it removed, so a script running
/// it in a loop is what would grow this without a bound. Reaching the bound
/// stops the growth; [`Console::reclaim`] is what empties it again.
pub const MAX_RETAINED: usize = 1024;

/// One of the three commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    /// Reports how many entities are loaded, or that none is.
    Tally,
    /// Releases what this package is holding and reports the two readings.
    Reclaim,
    /// Removes the selected entities, refusing a selection holding players.
    Sweep,
}

impl Command {
    /// Every command, in the order §5 numbers them.
    pub const ALL: [Self; 3] = [Self::Tally, Self::Reclaim, Self::Sweep];

    /// Returns the root literal this command is typed as.
    #[must_use]
    pub const fn root(self) -> &'static str {
        match self {
            Self::Tally => TALLY,
            Self::Reclaim => RECLAIM,
            Self::Sweep => SWEEP,
        }
    }

    /// Returns the command a root literal names, if it names one.
    #[must_use]
    pub fn from_root(root: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.root() == root)
    }

    /// Returns the permission level this command is declared to need.
    #[must_use]
    pub const fn required_level(self) -> u8 {
        REQUIRED_LEVEL
    }

    /// Returns whether this command runs on `side`.
    ///
    /// Every one of the three acts on or reads a running world, so a client is
    /// not where any of them belongs.
    #[must_use]
    pub const fn runs_on(self, side: Side) -> bool {
        side.is_server()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.root())
    }
}

/// What running a command produced, named by the line it is reported through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Report {
    /// How many entities are loaded.
    Tally {
        /// The total counted.
        total: u32,
    },
    /// Nothing is loaded.
    TallyNone,
    /// What was held before and after the reclamation, and how much went.
    Reclaimed {
        /// The reading before.
        before: usize,
        /// The reading after.
        after: usize,
        /// The difference between the two.
        released: usize,
    },
    /// What the removal took.
    Swept {
        /// The total removed.
        total: u32,
        /// One line per kind, in name order.
        lines: Vec<Line>,
    },
    /// The selection matched nothing, so nothing was removed.
    SweptNone,
    /// The selection held players, so nothing was removed.
    SweptRefusedPlayers,
}

impl Report {
    /// Returns the translated lines this report is printed through.
    ///
    /// Every one of them is a key [`crate::presentation::LINES`] supplies, which
    /// is what ties the eighteen lines to the three commands rather than leaving
    /// them a list nothing reads.
    #[must_use]
    pub const fn keys(&self) -> &'static [&'static str] {
        match self {
            Self::Tally { .. } => &["command.tally.total"],
            Self::TallyNone => &["command.tally.none"],
            Self::Reclaimed { .. } => &[
                "command.reclaim.before",
                "command.reclaim.after",
                "command.reclaim.done",
            ],
            Self::Swept { .. } => &["command.sweep.total", "command.sweep.kind"],
            Self::SweptNone => &["command.sweep.none"],
            Self::SweptRefusedPlayers => &["command.sweep.refused"],
        }
    }
}

/// Why a command did not run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The command was typed on a side it does not run on.
    WrongSide {
        /// The command that refused.
        command: Command,
        /// The side it was typed on.
        side: Side,
    },
    /// The words after the root literal are not a selection.
    Argument(CensusError),
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSide { command, side } => {
                write!(formatter, "{command} does not run on {}", side.name())
            }
            Self::Argument(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// What the three commands read and change.
///
/// The population is what a host or a dependent fills in; the notes are what
/// this package accumulates and what the reclamation releases.
#[derive(Clone, Debug, Default)]
pub struct Console {
    census: Census,
    retained: Vec<String>,
    dropped: usize,
}

impl Console {
    /// Builds a console over an empty population holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns what is in the world, as far as this package has been told.
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// Returns the population for filling in.
    pub const fn census_mut(&mut self) -> &mut Census {
        &mut self.census
    }

    /// Returns how many notes are held.
    #[must_use]
    pub fn retained(&self) -> usize {
        self.retained.len()
    }

    /// Returns how many notes the bound has dropped.
    ///
    /// Counted rather than silent, which is the rule [`crate::Outbox::dropped`]
    /// follows for the same reason: a bound that discards without a count is a
    /// bound nobody can see was reached.
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    /// Holds one note, if there is room for it.
    ///
    /// Returns whether it was held. A note beyond [`MAX_RETAINED`] is dropped
    /// rather than held, so a command run in a loop cannot grow this without
    /// end, and the drop is counted in [`Console::dropped`].
    pub fn retain(&mut self, note: String) -> bool {
        if self.retained.len() >= MAX_RETAINED {
            self.dropped += 1;
            return false;
        }
        self.retained.push(note);
        true
    }

    /// Releases everything held and reports the reading either side of it.
    ///
    /// This is the collection §5's second command performs. There is no process
    /// heap a mod may collect from here — Ravel publishes no such surface and a
    /// Rust mod has no collector to run — so what is released is what this
    /// package itself retains, and the two readings are counts of that.
    /// `NOTES.md` declares the difference.
    pub fn reclaim(&mut self) -> Report {
        let before = self.retained.len();
        self.retained.clear();
        self.retained.shrink_to_fit();
        let after = self.retained.len();
        Report::Reclaimed {
            before,
            after,
            released: before - after,
        }
    }

    /// Runs one command and returns what it produced.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::WrongSide`] on a side the command does not run on, and
    /// [`Refusal::Argument`] when the words after the root literal are not a
    /// selection. Nothing is read or changed in either case.
    pub fn run(
        &mut self,
        command: Command,
        side: Side,
        arguments: &[String],
    ) -> Result<Report, Refusal> {
        if !command.runs_on(side) {
            return Err(Refusal::WrongSide { command, side });
        }
        match command {
            Command::Tally => Ok(self.tally()),
            Command::Reclaim => Ok(self.reclaim()),
            Command::Sweep => self.sweep(arguments),
        }
    }

    fn tally(&self) -> Report {
        let total = self.census.total();
        if total == 0 {
            Report::TallyNone
        } else {
            Report::Tally { total }
        }
    }

    fn sweep(&mut self, arguments: &[String]) -> Result<Report, Refusal> {
        let selector = Selector::parse(arguments).map_err(Refusal::Argument)?;
        if self.census.selection_holds_players(&selector) {
            return Ok(Report::SweptRefusedPlayers);
        }
        let lines = self.census.remove(&selector);
        if lines.is_empty() {
            return Ok(Report::SweptNone);
        }
        let total = lines
            .iter()
            .fold(0_u32, |running, line| running.saturating_add(line.count));
        for line in &lines {
            self.retain(format!("{}: {}", line.kind, line.count));
        }
        Ok(Report::Swept { total, lines })
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, Console, MAX_RETAINED, REQUIRED_LEVEL, ROOTS, Refusal, Report};
    use crate::census::Line;
    use crate::presentation::text;
    use ravel_sdk::Side;
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    fn populated() -> Console {
        let mut console = Console::new();
        console
            .census_mut()
            .record("cow", 3, false)
            .expect("a legal kind");
        console
            .census_mut()
            .record("bat", 2, false)
            .expect("a legal kind");
        console
            .census_mut()
            .record("player", 1, true)
            .expect("a legal kind");
        console
    }

    #[test]
    fn three_root_literals_are_claimed_and_each_names_one_command() {
        assert_eq!(ROOTS.len(), 3);
        let distinct: BTreeSet<&str> = ROOTS.iter().copied().collect();
        assert_eq!(distinct.len(), 3);
        for root in ROOTS {
            assert_eq!(
                Command::from_root(root).map(Command::root),
                Some(root),
                "{root} has to name the command it is the literal of"
            );
        }
        assert!(Command::from_root("nothing_claims_this").is_none());
    }

    #[test]
    fn every_report_a_command_produces_prints_through_a_supplied_line() {
        let mut console = populated();
        let mut produced = vec![
            console.tally(),
            Report::TallyNone,
            console.reclaim(),
            Report::SweptNone,
            Report::SweptRefusedPlayers,
        ];
        produced.push(
            console
                .run(Command::Sweep, Side::DedicatedServer, &words("cow"))
                .expect("a kind holding no player is removed"),
        );
        for report in &produced {
            assert_ne!(report.keys().len(), 0);
            for key in report.keys() {
                assert!(text(key).is_some(), "{key} is reported and not supplied");
            }
        }
    }

    #[test]
    fn the_count_reports_a_total_when_there_is_one_and_the_empty_case_when_there_is_not() {
        let mut console = populated();
        assert_eq!(
            console.run(Command::Tally, Side::IntegratedServer, &[]),
            Ok(Report::Tally { total: 6 })
        );

        let mut empty = Console::new();
        assert_eq!(
            empty.run(Command::Tally, Side::IntegratedServer, &[]),
            Ok(Report::TallyNone)
        );
    }

    #[test]
    fn the_removal_reports_a_total_and_a_line_for_each_kind_it_took() {
        let mut console = Console::new();
        console
            .census_mut()
            .record("cow", 3, false)
            .expect("a legal kind");
        console
            .census_mut()
            .record("bat", 2, false)
            .expect("a legal kind");

        let report = console
            .run(Command::Sweep, Side::DedicatedServer, &words("all"))
            .expect("a selection is read");
        match report {
            Report::Swept { total, lines } => {
                assert_eq!(total, 5);
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0].kind, "bat");
                assert_eq!(lines[1].kind, "cow");
            }
            other => panic!("the removal reported {other:?}"),
        }
        assert!(console.census().is_empty());
    }

    #[test]
    fn a_wide_selection_holding_a_player_refuses_whole_rather_than_removing_the_rest() {
        let mut console = populated();
        let before = console.census().total();
        assert_eq!(
            console.run(Command::Sweep, Side::DedicatedServer, &words("all")),
            Ok(Report::SweptRefusedPlayers)
        );
        assert_eq!(
            console.census().total(),
            before,
            "declining the player case means declining it, not removing everything else"
        );
    }

    #[test]
    fn a_selection_holding_players_is_refused_and_removes_nothing() {
        let mut console = populated();
        console
            .census_mut()
            .record("player", 0, true)
            .expect("a legal kind");
        let before = console.census().total();
        assert_eq!(
            console.run(Command::Sweep, Side::DedicatedServer, &words("player")),
            Ok(Report::SweptRefusedPlayers)
        );
        assert_eq!(
            console.census().total(),
            before,
            "a refusal that removed something would be an execution"
        );
        assert_eq!(console.census().count_of("player"), 1);
    }

    #[test]
    fn a_selection_matching_nothing_reports_the_empty_case() {
        let mut console = populated();
        assert_eq!(
            console.run(Command::Sweep, Side::DedicatedServer, &words("squid")),
            Ok(Report::SweptNone)
        );
    }

    #[test]
    fn words_that_are_not_a_selection_refuse_the_command_and_change_nothing() {
        let mut console = populated();
        let before = console.census().total();
        assert!(matches!(
            console.run(Command::Sweep, Side::DedicatedServer, &words("cow bat")),
            Err(Refusal::Argument(_))
        ));
        assert_eq!(console.census().total(), before);
    }

    #[test]
    fn every_command_refuses_a_client_and_runs_on_both_server_sides() {
        let mut console = populated();
        for command in Command::ALL {
            assert!(!command.runs_on(Side::Client));
            assert!(command.runs_on(Side::IntegratedServer));
            assert!(command.runs_on(Side::DedicatedServer));
            assert_eq!(command.required_level(), REQUIRED_LEVEL);
            assert!(matches!(
                console.run(command, Side::Client, &words("all")),
                Err(Refusal::WrongSide { .. })
            ));
        }
        assert_eq!(
            console.census().total(),
            6,
            "a command refused for its side did nothing"
        );
    }

    #[test]
    fn the_reclamation_reports_the_two_readings_and_empties_what_it_read() {
        let mut console = Console::new();
        for index in 0..8 {
            assert!(console.retain(format!("note {index}")));
        }
        assert_eq!(console.retained(), 8);

        let report = console
            .run(Command::Reclaim, Side::DedicatedServer, &[])
            .expect("the reclamation runs on a server");
        assert_eq!(
            report,
            Report::Reclaimed {
                before: 8,
                after: 0,
                released: 8,
            }
        );
        assert_eq!(console.retained(), 0);
    }

    #[test]
    fn what_the_console_holds_is_bounded_and_the_reclamation_is_what_empties_it() {
        let started = Instant::now();
        let mut console = Console::new();
        for index in 0..MAX_RETAINED * 4 {
            let held = console.retain(format!("note {index}"));
            assert_eq!(held, index < MAX_RETAINED);
        }
        assert_eq!(console.retained(), MAX_RETAINED);
        assert!(started.elapsed() < Duration::from_secs(1));

        console.reclaim();
        assert_eq!(console.retained(), 0);
        assert!(console.retain("room again".to_owned()));
    }

    #[test]
    fn a_removal_run_in_a_loop_cannot_grow_what_is_held_past_the_bound() {
        let started = Instant::now();
        let mut console = Console::new();
        for round in 0..MAX_RETAINED * 2 {
            console
                .census_mut()
                .record("cow", 1, false)
                .expect("a legal kind");
            console
                .run(Command::Sweep, Side::DedicatedServer, &words("cow"))
                .unwrap_or_else(|refusal| panic!("round {round} refused: {refusal}"));
        }
        assert_eq!(console.retained(), MAX_RETAINED);
        assert_eq!(
            console.dropped(),
            MAX_RETAINED,
            "a note the bound discarded is counted rather than disappearing"
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_removal_after_the_bound_is_still_a_removal() {
        let mut console = Console::new();
        for index in 0..MAX_RETAINED {
            assert!(console.retain(format!("note {index}")));
        }
        console
            .census_mut()
            .record("cow", 3, false)
            .expect("a legal kind");

        let report = console
            .run(Command::Sweep, Side::DedicatedServer, &words("cow"))
            .expect("a kind holding no player is removed");
        assert_eq!(
            report,
            Report::Swept {
                total: 3,
                lines: vec![Line {
                    kind: "cow".to_owned(),
                    count: 3,
                }],
            }
        );
        assert_eq!(console.census().count_of("cow"), 0);
        assert_eq!(console.dropped(), 1);
    }
}
