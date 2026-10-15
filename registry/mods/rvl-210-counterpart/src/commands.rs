//! Section fifteen: one command tree with seven subcommands.
//!
//! Five of the seven are the entry's own. The settings and version subcommands
//! are inherited, and every dependent gets them without asking. That is the part
//! a dependent observes, because it means a dependent's own tree already answers
//! two names it never declared.
//!
//! The specification records the permission levels as unread, so none is
//! asserted here. `NOTES.md` says so rather than a level being invented.

use core::fmt;

/// Reads or writes a runtime-changeable setting.
pub const SETTINGS: &str = "settings";
/// Reports the entry's version.
pub const VERSION: &str = "version";
/// Sends a diagnostic round-trip message and reports the timing.
pub const DIAGNOSTIC: &str = "diagnostic";
/// Sets a target on fire.
pub const IGNITE: &str = "ignite";
/// Forces a resource reload on the client.
pub const RELOAD: &str = "reload";
/// Writes the registries out.
pub const DUMP: &str = "dump";
/// Opens the demonstration manual.
pub const MANUAL: &str = "manual";

/// Whether a dependent has to declare a subcommand to have it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Origin {
    /// Every dependent's tree carries it without declaring it.
    Inherited,
    /// This entry's own.
    Own,
}

/// The seven subcommands and whether each is inherited.
pub const SUBCOMMANDS: [(&str, Origin); 7] = [
    (SETTINGS, Origin::Inherited),
    (VERSION, Origin::Inherited),
    (DIAGNOSTIC, Origin::Own),
    (IGNITE, Origin::Own),
    (RELOAD, Origin::Own),
    (DUMP, Origin::Own),
    (MANUAL, Origin::Own),
];

/// What a dependent's own tree carries without declaring it.
#[must_use]
pub fn inherited() -> Vec<&'static str> {
    SUBCOMMANDS
        .iter()
        .filter(|(_, origin)| *origin != Origin::Own)
        .map(|(name, _)| *name)
        .collect()
}

/// This entry's own five.
#[must_use]
pub fn own() -> Vec<&'static str> {
    SUBCOMMANDS
        .iter()
        .filter(|(_, origin)| *origin == Origin::Own)
        .map(|(name, _)| *name)
        .collect()
}

/// What the tree refuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refused {
    /// The tree carries no subcommand of that name.
    NoSuchSubcommand {
        /// The name asked for.
        name: String,
    },
    /// The subcommand was given the wrong number of arguments.
    WrongArity {
        /// The subcommand.
        name: &'static str,
        /// How many it takes at most.
        takes: usize,
        /// How many it was given.
        given: usize,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchSubcommand { name } => {
                write!(formatter, "this tree carries no subcommand called {name}")
            }
            Self::WrongArity { name, takes, given } => write!(
                formatter,
                "{name} takes at most {takes} arguments and was given {given}"
            ),
        }
    }
}

/// The most arguments each subcommand takes.
///
/// The settings subcommand takes a property name and optionally a value, so it
/// is the only one taking two. Everything else takes at most one.
#[must_use]
pub fn arity(name: &str) -> Option<usize> {
    match name {
        SETTINGS => Some(2),
        DIAGNOSTIC | IGNITE | DUMP | MANUAL => Some(1),
        VERSION | RELOAD => Some(0),
        _ => None,
    }
}

/// Resolves one invocation against the tree.
///
/// # Errors
///
/// Returns [`Refused`] when the tree carries no such subcommand or when it was
/// given more arguments than it takes.
pub fn resolve(name: &str, arguments: &[String]) -> Result<&'static str, Refused> {
    let Some((held, _)) = SUBCOMMANDS.iter().find(|(held, _)| *held == name) else {
        return Err(Refused::NoSuchSubcommand {
            name: name.to_owned(),
        });
    };
    let takes = arity(held).unwrap_or(0);
    if arguments.len() > takes {
        return Err(Refused::WrongArity {
            name: held,
            takes,
            given: arguments.len(),
        });
    }
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_carries_seven_subcommands_named_once_each() {
        assert_eq!(SUBCOMMANDS.len(), 7);
        let mut names: Vec<&str> = SUBCOMMANDS.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn five_are_this_entry_s_own_and_two_are_inherited() {
        assert_eq!(own().len(), 5);
        assert_eq!(inherited(), vec![SETTINGS, VERSION]);
        assert_eq!(
            SUBCOMMANDS
                .iter()
                .filter(|(_, origin)| *origin == Origin::Inherited)
                .count(),
            2
        );
    }

    #[test]
    fn every_subcommand_resolves_and_a_name_the_tree_lacks_is_refused() {
        for (name, _) in SUBCOMMANDS {
            assert_eq!(resolve(name, &[]), Ok(name));
        }
        assert_eq!(
            resolve("never", &[]),
            Err(Refused::NoSuchSubcommand {
                name: "never".to_owned()
            })
        );
    }

    #[test]
    fn a_subcommand_given_more_arguments_than_it_takes_is_refused() {
        let two = vec!["one".to_owned(), "two".to_owned()];
        assert_eq!(resolve(SETTINGS, &two), Ok(SETTINGS));
        assert_eq!(
            resolve(VERSION, &two),
            Err(Refused::WrongArity {
                name: VERSION,
                takes: 0,
                given: 2,
            })
        );
        assert_eq!(
            resolve(DUMP, &two),
            Err(Refused::WrongArity {
                name: DUMP,
                takes: 1,
                given: 2,
            })
        );
    }

    #[test]
    fn only_the_settings_subcommand_takes_a_property_and_a_value() {
        assert_eq!(arity(SETTINGS), Some(2));
        for (name, _) in SUBCOMMANDS {
            if name != SETTINGS {
                assert!(arity(name).is_some_and(|takes| takes <= 1), "{name}");
            }
        }
        assert_eq!(arity("never"), None);
    }
}
