//! What the world-acting commands act on.
//!
//! §5 of the specification recovers three commands from the language document:
//! one counts entities, one reclaims memory, and one removes entities in bulk
//! and refuses the case that would remove players. Two of the three need to know
//! what is in the world, and Ravel publishes no entity surface at the reviewed
//! revision, so the population the commands act on is held here and filled by
//! whoever knows: a host, or a dependent that has an entity list of its own.
//!
//! Everything the commands decide — the totals, the per-kind lines, the empty
//! case and the refusal — is decided over this value, so the rules are real and
//! testable without a running game. What is host-blocked is where the numbers
//! come from, and `NOTES.md` says so.
//!
//! Every input here arrives from outside this package, so every input is
//! bounded: the number of kinds, the count within a kind, and the length and
//! charset of a kind's name.

use std::collections::BTreeMap;
use std::fmt;

/// How many distinct kinds one population holds.
pub const MAX_KINDS: usize = 512;

/// How many of one kind a population counts.
///
/// A count arrives from outside and is added to rather than assigned, so it is
/// held below a ceiling rather than allowed to wrap or to grow without end.
pub const MAX_PER_KIND: u32 = 1_000_000;

/// How long a kind's name may be.
pub const MAX_KIND_LENGTH: usize = 64;

/// Why a population refused what it was given.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PopulationError {
    /// The name is empty, too long, or spells something a name may not.
    Name {
        /// The rejected spelling, quoted so a report can say what arrived.
        candidate: String,
    },
    /// The population already holds [`MAX_KINDS`] kinds.
    Full {
        /// The bound that was reached.
        bound: usize,
    },
}

impl fmt::Display for PopulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { candidate } => {
                write!(formatter, "`{candidate}` is not a legal kind name")
            }
            Self::Full { bound } => write!(formatter, "no more than {bound} kinds are counted"),
        }
    }
}

impl std::error::Error for PopulationError {}

/// One line of a removal report: a kind and how many of it went.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Line {
    /// The kind the line is about.
    pub kind: String,
    /// How many of it the removal took.
    pub count: u32,
}

/// What is in the world, as far as this package has been told.
#[derive(Clone, Debug, Default)]
pub struct Population {
    counted: BTreeMap<String, u32>,
    players: BTreeMap<String, bool>,
}

impl Population {
    /// Builds an empty population.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `count` of `kind`, saying whether that kind is a player.
    ///
    /// The count saturates at [`MAX_PER_KIND`] rather than wrapping, and a kind
    /// beyond [`MAX_KINDS`] is refused rather than recorded.
    ///
    /// # Errors
    ///
    /// Returns [`PopulationError::Name`] for a name outside the charset and length a
    /// name may have, and [`PopulationError::Full`] when the population already
    /// holds [`MAX_KINDS`] kinds. Nothing is recorded in either case.
    pub fn record(&mut self, kind: &str, count: u32, players: bool) -> Result<(), PopulationError> {
        if !is_legal_kind(kind) {
            return Err(PopulationError::Name {
                candidate: kind.to_owned(),
            });
        }
        if !self.counted.contains_key(kind) && self.counted.len() >= MAX_KINDS {
            return Err(PopulationError::Full { bound: MAX_KINDS });
        }
        let held = self.counted.entry(kind.to_owned()).or_insert(0);
        *held = held.saturating_add(count).min(MAX_PER_KIND);
        let marked = self.players.entry(kind.to_owned()).or_insert(players);
        *marked = *marked || players;
        Ok(())
    }

    /// Returns how many entities are counted in total.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.counted
            .values()
            .fold(0_u32, |running, count| running.saturating_add(*count))
    }

    /// Returns whether nothing is counted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// Returns how many kinds are counted.
    #[must_use]
    pub fn kinds(&self) -> usize {
        self.counted.len()
    }

    /// Returns how many of `kind` are counted.
    #[must_use]
    pub fn count_of(&self, kind: &str) -> u32 {
        self.counted.get(kind).copied().unwrap_or(0)
    }

    /// Returns whether `kind` is a kind a player is.
    #[must_use]
    pub fn is_player_kind(&self, kind: &str) -> bool {
        self.players.get(kind).copied().unwrap_or(false)
    }

    /// Returns whether any kind matched by `selector` is a player kind.
    ///
    /// This is what the removal command asks before it removes anything, and it
    /// is asked over the selection rather than over the whole population, so
    /// removing one kind is not refused because a player is standing elsewhere.
    #[must_use]
    pub fn selection_holds_players(&self, selector: &Selector) -> bool {
        self.counted
            .iter()
            .any(|(kind, count)| *count > 0 && selector.matches(kind) && self.is_player_kind(kind))
    }

    /// Returns one line per kind `selector` matches, in name order.
    #[must_use]
    pub fn lines(&self, selector: &Selector) -> Vec<Line> {
        self.counted
            .iter()
            .filter(|(kind, count)| **count > 0 && selector.matches(kind))
            .map(|(kind, count)| Line {
                kind: kind.clone(),
                count: *count,
            })
            .collect()
    }

    /// Takes every entity `selector` matches out of the population.
    ///
    /// Returns the lines that were taken. The population is changed by this
    /// call, which is what makes the removal command's effect observable. A kind
    /// that was taken is dropped rather than left counting zero, so a long-lived
    /// population that has been swept clean has room for a kind it has not seen
    /// before.
    pub fn remove(&mut self, selector: &Selector) -> Vec<Line> {
        let taken = self.lines(selector);
        for line in &taken {
            self.counted.remove(&line.kind);
            self.players.remove(&line.kind);
        }
        taken
    }
}

/// Which kinds a command was pointed at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selector {
    /// Every kind counted.
    Everything,
    /// One named kind.
    OfKind(String),
}

impl Selector {
    /// Reads a selector from the words a player typed.
    ///
    /// The literal `all` selects everything; any other single word selects that
    /// kind. Nothing else is a selector, so a mistyped command is refused rather
    /// than read as something the player did not ask for.
    ///
    /// # Errors
    ///
    /// Returns [`PopulationError::Name`] for a word outside the charset and length a
    /// kind name may have, quoting what arrived.
    pub fn parse(arguments: &[String]) -> Result<Self, PopulationError> {
        match arguments {
            [only] if only == "all" => Ok(Self::Everything),
            [only] if is_legal_kind(only) => Ok(Self::OfKind(only.clone())),
            _ => Err(PopulationError::Name {
                candidate: arguments.join(" "),
            }),
        }
    }

    /// Returns whether this selector points at `kind`.
    #[must_use]
    pub fn matches(&self, kind: &str) -> bool {
        match self {
            Self::Everything => true,
            Self::OfKind(named) => named == kind,
        }
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Everything => formatter.write_str("all"),
            Self::OfKind(named) => formatter.write_str(named),
        }
    }
}

fn is_legal_kind(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate.len() <= MAX_KIND_LENGTH
        && candidate.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '.' | '-' | '/' | ':')
        })
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_KIND_LENGTH, MAX_KINDS, MAX_PER_KIND, Population, PopulationError, Selector,
        is_legal_kind,
    };

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn a_population_totals_what_it_was_told() {
        let mut population = Population::new();
        population.record("cow", 4, false).expect("a legal kind");
        population.record("cow", 2, false).expect("a legal kind");
        population.record("bat", 1, false).expect("a legal kind");
        assert_eq!(population.count_of("cow"), 6);
        assert_eq!(population.total(), 7);
        assert_eq!(population.kinds(), 2);
        assert!(!population.is_empty());
    }

    #[test]
    fn a_count_saturates_at_the_ceiling_rather_than_wrapping() {
        let mut population = Population::new();
        population
            .record("cow", u32::MAX, false)
            .expect("a legal kind");
        population
            .record("cow", u32::MAX, false)
            .expect("a legal kind");
        assert_eq!(population.count_of("cow"), MAX_PER_KIND);
    }

    #[test]
    fn the_kind_bound_is_enforced() {
        let mut population = Population::new();
        for index in 0..MAX_KINDS + 5_000 {
            let outcome = population.record(&format!("kind_{index}"), 1, false);
            if index >= MAX_KINDS {
                assert!(matches!(outcome, Err(PopulationError::Full { .. })));
            } else {
                outcome.expect("a kind below the bound is recorded");
            }
        }
        assert_eq!(population.kinds(), MAX_KINDS);
    }

    #[test]
    fn a_name_outside_the_charset_or_the_length_is_refused_and_records_nothing() {
        let mut population = Population::new();
        for candidate in ["", "Cow", "co w", "cow!", &"c".repeat(MAX_KIND_LENGTH + 1)] {
            assert!(matches!(
                population.record(candidate, 1, false),
                Err(PopulationError::Name { .. })
            ));
        }
        assert_eq!(population.kinds(), 0);
        assert!(is_legal_kind("namespace:kind_one"));
    }

    #[test]
    fn removing_takes_the_selected_kinds_out_and_leaves_the_rest() {
        let mut population = Population::new();
        population.record("cow", 3, false).expect("a legal kind");
        population.record("bat", 2, false).expect("a legal kind");

        let taken = population.remove(&Selector::OfKind("cow".to_owned()));
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].count, 3);
        assert_eq!(population.count_of("cow"), 0);
        assert_eq!(population.count_of("bat"), 2);

        let rest = population.remove(&Selector::Everything);
        assert_eq!(
            rest.len(),
            1,
            "a kind already at zero is not reported again"
        );
        assert!(population.is_empty());
    }

    #[test]
    fn a_kind_taken_out_frees_its_place_under_the_bound() {
        let mut population = Population::new();
        for index in 0..MAX_KINDS {
            population
                .record(&format!("kind_{index}"), 1, false)
                .expect("a kind below the bound is recorded");
        }
        assert_eq!(population.kinds(), MAX_KINDS);

        population.remove(&Selector::Everything);
        assert!(population.is_empty());
        assert_eq!(
            population.kinds(),
            0,
            "a swept population that still counts its kinds refuses every later kind forever"
        );
        population
            .record("cow", 1, false)
            .expect("a swept population has room for a kind it has not seen");
        assert_eq!(population.count_of("cow"), 1);
    }

    #[test]
    fn a_player_kind_taken_out_is_not_remembered_as_a_player_kind() {
        let mut population = Population::new();
        population.record("zombie", 2, true).expect("a legal kind");
        population.remove(&Selector::OfKind("zombie".to_owned()));
        population.record("zombie", 2, false).expect("a legal kind");
        assert!(
            !population.is_player_kind("zombie"),
            "a kind that was taken out carries nothing of what it was"
        );
    }

    #[test]
    fn a_selection_holding_a_player_kind_is_recognised_and_one_that_does_not_is_not() {
        let mut population = Population::new();
        population.record("player", 1, true).expect("a legal kind");
        population.record("cow", 3, false).expect("a legal kind");

        assert!(population.selection_holds_players(&Selector::Everything));
        assert!(!population.selection_holds_players(&Selector::OfKind("cow".to_owned())));
        assert!(population.selection_holds_players(&Selector::OfKind("player".to_owned())));
    }

    #[test]
    fn a_selector_reads_the_two_forms_and_refuses_everything_else() {
        assert_eq!(
            Selector::parse(&words("all")).expect("the literal selects everything"),
            Selector::Everything
        );
        assert_eq!(
            Selector::parse(&words("cow")).expect("a legal kind selects that kind"),
            Selector::OfKind("cow".to_owned())
        );
        for line in ["", "cow bat", "Cow"] {
            assert!(matches!(
                Selector::parse(&words(line)),
                Err(PopulationError::Name { .. })
            ));
        }
    }
}
