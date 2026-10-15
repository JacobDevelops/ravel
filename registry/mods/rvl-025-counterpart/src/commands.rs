//! The two commands and the two key bindings of §14, and the one place this
//! entry extends the shared transport rather than merely using it.
//!
//! Both commands are registered through the shared library's command transport
//! and both operate on the held item. Both registrations, and the reply encoding
//! below, are deferred to the phase that runs after registration and after
//! configuration is resolved, because §2 says they need every registry populated
//! and a partially-populated view is what running them earlier gets.
//!
//! §14 records that the second command reads the player's active hand in a way
//! that can be unset and that the original discards the result of that read on
//! the line before using it. That is a leftover rather than a behaviour, so the
//! hand is read once here and an unset one refuses.
//!
//! §18 excludes the shared library's own transport behaviour from this
//! specification, so what this entry owns is the registration — which commands
//! it files, on which side, what a reply's elements are, and the bound each
//! element is held to — and the work each command does when it is run.
//! [`Registration`] is the first, and [`evaluate`] and [`cycle`] are the second:
//! both are reached from the entry's own command hook, so a command name that is
//! claimed at the barrier is a command that changes something when a player types
//! it.

use std::fmt;

use crate::data::Stack;
use crate::shapes::{Pattern, ShapedRecipe};
use crate::storage::{Carrier, Hand, Step};

/// The namespace both commands are filed under.
pub const NAMESPACE: &str = "rvl_025_counterpart";

/// The command that evaluates the pattern card in the player's hand.
pub const EVALUATE: &str = "evaluate_pattern";

/// The command that advances or retreats the selection of a held item.
pub const CYCLE: &str = "cycle_selection";

/// The reply this entry teaches the shared transport to encode.
///
/// A list of text, which is the one registration §13 says this entry adds to the
/// shared transport at common setup.
pub const TEXT_LIST_REPLY: &str = "text_list";

/// The most characters one element of that reply may carry.
///
/// §13 states the bound and it is one below the shared library's own text
/// ceiling, so the narrower of the two is the one enforced.
pub const MAX_REPLY_TEXT: usize = 32_767;

/// The category both key bindings live in.
pub const BINDING_CATEGORY: &str = "rvl_025_counterpart";

/// The binding that advances the selection.
pub const BINDING_FORWARD: &str = "cycle_forward";

/// The binding that retreats it.
pub const BINDING_BACKWARD: &str = "cycle_backward";

/// The key the forward binding defaults to.
pub const DEFAULT_FORWARD_KEY: &str = "RightBracket";

/// The key the backward binding defaults to.
pub const DEFAULT_BACKWARD_KEY: &str = "LeftBracket";

/// One client-only key binding.
///
/// Both bindings are in an in-game-only conflict context and both do the same
/// thing: send the cycle command with the direction set. Two further bindings
/// exist upstream only as commented-out lines, which is where §5's two dead
/// labels come from; neither the bindings nor the labels are shipped here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    /// The binding's own name inside its category.
    pub name: &'static str,
    /// The key it defaults to.
    pub default_key: &'static str,
    /// The direction it sends.
    pub sends: Step,
}

/// The two bindings this entry registers, and no others.
pub const BINDINGS: [Binding; 2] = [
    Binding {
        name: BINDING_FORWARD,
        default_key: DEFAULT_FORWARD_KEY,
        sends: Step::Forward,
    },
    Binding {
        name: BINDING_BACKWARD,
        default_key: DEFAULT_BACKWARD_KEY,
        sends: Step::Backward,
    },
];

/// What one registration files with the transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Files {
    /// A command that runs on the server and returns nothing.
    Command,
    /// An encoding for a reply that is a list of one element type.
    ReplyEncoding {
        /// What one element of the list is.
        element: Element,
    },
}

/// The element type a list reply carries.
///
/// One value, because this entry teaches the transport exactly one encoding.
/// The set is the shared library's and is not restated; this names the member
/// this entry uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Element {
    /// Text, bounded by [`MAX_REPLY_TEXT`].
    Text,
}

/// One thing this entry hands the transport at the deferred phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registration {
    /// The namespace the transport files it under.
    pub namespace: &'static str,
    /// Its own name inside that namespace.
    pub name: &'static str,
    /// Whether it is a command or a reply encoding.
    pub files: Files,
    /// Whether the authoritative side runs it.
    pub on_server: bool,
}

/// Everything this entry files with the transport, in the order it files it.
///
/// Both commands run on the server, which is what "server-directed" means here:
/// a client sends the call and the authoritative side answers it. The third
/// entry is the one place §13 says this entry extends the shared transport
/// rather than merely using it.
pub const REGISTRATIONS: [Registration; 3] = [
    Registration {
        namespace: NAMESPACE,
        name: EVALUATE,
        files: Files::Command,
        on_server: true,
    },
    Registration {
        namespace: NAMESPACE,
        name: CYCLE,
        files: Files::Command,
        on_server: true,
    },
    Registration {
        namespace: NAMESPACE,
        name: TEXT_LIST_REPLY,
        files: Files::ReplyEncoding {
            element: Element::Text,
        },
        on_server: true,
    },
];

/// Bounds one element of the text-list reply.
///
/// # Errors
///
/// Returns [`ReplyRefused`] for an element longer than [`MAX_REPLY_TEXT`],
/// carrying the length and the ceiling, because a reply is built from what a
/// command found and a command can find a very long name.
pub fn check_reply(elements: &[String]) -> Result<(), ReplyRefused> {
    for element in elements {
        if element.len() > MAX_REPLY_TEXT {
            return Err(ReplyRefused {
                length: element.len(),
                limit: MAX_REPLY_TEXT,
            });
        }
    }
    Ok(())
}

/// An element of a text-list reply that is longer than one may be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplyRefused {
    /// How long it was.
    pub length: usize,
    /// The most it may be.
    pub limit: usize,
}

impl fmt::Display for ReplyRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a reply element carries {} characters and {} is the most one may carry",
            self.length, self.limit
        )
    }
}

impl std::error::Error for ReplyRefused {}

/// The names the two commands are typed under, in the order they are filed.
pub const NAMES: [&str; 2] = [EVALUATE, CYCLE];

/// Returns the registration one typed name files, or nothing.
#[must_use]
pub fn resolve(name: &str) -> Option<&'static Registration> {
    REGISTRATIONS
        .iter()
        .find(|filed| filed.name == name && filed.files == Files::Command)
}

/// Reads the cycle command's single argument.
///
/// §14 says the command takes one boolean for the direction. The argument is
/// text a player typed and nothing before this entry interprets it, so anything
/// but the two words is refused rather than read as one of them.
///
/// # Errors
///
/// Returns [`ArgumentRefused`] for a call carrying anything other than exactly
/// one of the two words.
pub fn direction(arguments: &[String]) -> Result<Step, ArgumentRefused> {
    let [only] = arguments else {
        return Err(ArgumentRefused::Count {
            given: arguments.len(),
        });
    };
    match only.as_str() {
        "true" => Ok(Step::Forward),
        "false" => Ok(Step::Backward),
        _ => Err(ArgumentRefused::NotATruthValue),
    }
}

/// Why a command's arguments were refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentRefused {
    /// The call did not carry exactly one argument.
    Count {
        /// How many it carried.
        given: usize,
    },
    /// The one argument is neither of the two words.
    NotATruthValue,
}

impl fmt::Display for ArgumentRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { given } => write!(
                formatter,
                "the direction is one argument and the call carried {given}"
            ),
            Self::NotATruthValue => {
                formatter.write_str("the direction is either `true` or `false`")
            }
        }
    }
}

impl std::error::Error for ArgumentRefused {}

/// What the evaluate command did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Evaluated {
    /// A recipe matched and the card's output position now holds its result.
    Matched,
    /// Nothing matched, and the output position is empty.
    Unmatched,
    /// The player is not holding a pattern card.
    NotACard,
}

/// Runs the evaluate command against the pattern card in the player's hand.
///
/// §9 owns what evaluation is; this is the command reaching it, which is what
/// makes the name a player types do the thing the name says.
#[must_use]
pub fn evaluate(held: Option<&mut Pattern>, recipes: &[ShapedRecipe]) -> Evaluated {
    let Some(card) = held else {
        return Evaluated::NotACard;
    };
    if card.evaluate(recipes) {
        Evaluated::Matched
    } else {
        Evaluated::Unmatched
    }
}

/// What the cycle command is holding when it runs.
#[derive(Debug)]
pub enum Cyclable<'held> {
    /// This entry's own carrier, whose selection the step moves (§10).
    Carrier(&'held mut Carrier),
    /// A dependent's item that declares itself cyclable (§15).
    ///
    /// §16 says this entry ships no item of the kind, so the step is carried to
    /// the dependent rather than applied here; the selection a dependent's item
    /// holds is that item's.
    Declared(&'held Stack),
}

/// What the cycle command did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cycled {
    /// The held item took the step.
    Stepped {
        /// Which direction it took.
        step: Step,
        /// Whether the selection actually moved.
        ///
        /// A carrier with one occupied slot selects that slot and reports that
        /// nothing moved; a carrier with none reports the same. A dependent's
        /// item reports nothing, because its selection is its own.
        moved: bool,
        /// Which slot is selected now, for a carrier.
        selected: Option<usize>,
    },
    /// The player has no active hand, so nothing was held to step.
    NoHand,
    /// The held item does not declare itself cyclable.
    NotCyclable,
}

/// Runs the cycle command against whatever the player is holding.
///
/// The active hand is read once. §14 records the original reading it in a way
/// that can be unset and discarding the result on the line before using it; an
/// unset hand refuses here instead.
#[must_use]
pub fn cycle(hand: Option<Hand>, held: Option<Cyclable<'_>>, step: Step) -> Cycled {
    if hand.is_none() {
        return Cycled::NoHand;
    }
    match held {
        Some(Cyclable::Carrier(carrier)) => {
            let moved = carrier.cycle(step);
            Cycled::Stepped {
                step,
                moved,
                selected: Some(carrier.selected()),
            }
        }
        Some(Cyclable::Declared(stack)) if stack.cyclable && !stack.is_empty() => Cycled::Stepped {
            step,
            moved: false,
            selected: None,
        },
        Some(Cyclable::Declared(_)) | None => Cycled::NotCyclable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_commands_and_one_reply_encoding_are_filed_and_all_three_on_the_server() {
        assert_eq!(REGISTRATIONS.len(), 3);
        assert!(REGISTRATIONS.iter().all(|filed| filed.on_server));
        assert!(
            REGISTRATIONS
                .iter()
                .all(|filed| filed.namespace == NAMESPACE)
        );

        let commands: Vec<&str> = REGISTRATIONS
            .iter()
            .filter(|filed| filed.files == Files::Command)
            .map(|filed| filed.name)
            .collect();
        assert_eq!(commands, [EVALUATE, CYCLE]);
    }

    #[test]
    fn the_one_reply_encoding_names_a_text_element() {
        let encodings: Vec<Files> = REGISTRATIONS
            .iter()
            .map(|filed| filed.files)
            .filter(|files| matches!(files, Files::ReplyEncoding { .. }))
            .collect();
        assert_eq!(
            encodings,
            [Files::ReplyEncoding {
                element: Element::Text
            }],
            "one registration, which is the one place this entry extends the transport"
        );
    }

    #[test]
    fn nothing_is_filed_twice_under_one_name() {
        let mut names: Vec<&str> = REGISTRATIONS.iter().map(|filed| filed.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), REGISTRATIONS.len());
    }

    #[test]
    fn a_reply_element_past_the_declared_bound_is_refused() {
        let ceiling = "a".repeat(MAX_REPLY_TEXT);
        check_reply(std::slice::from_ref(&ceiling)).expect("the bound itself is legal");

        let over = "a".repeat(MAX_REPLY_TEXT + 1);
        assert_eq!(
            check_reply(&[ceiling, over]),
            Err(ReplyRefused {
                length: MAX_REPLY_TEXT + 1,
                limit: MAX_REPLY_TEXT,
            })
        );
    }

    fn installable(identity: &str) -> Stack {
        let mut stack = Stack::of(identity).expect("a legal identity");
        stack.carrier_ready = true;
        stack
    }

    #[test]
    fn the_cycle_command_refuses_an_unset_hand_rather_than_using_the_read() {
        let mut cyclable = Stack::of("counterpart:device").expect("a legal identity");
        cyclable.cyclable = true;

        assert_eq!(
            cycle(None, Some(Cyclable::Declared(&cyclable)), Step::Forward),
            Cycled::NoHand,
            "an unset hand refuses instead of the read being discarded"
        );
    }

    #[test]
    fn the_cycle_command_steps_only_an_item_that_declares_itself_cyclable() {
        let mut cyclable = Stack::of("counterpart:device").expect("a legal identity");
        cyclable.cyclable = true;
        let plain = Stack::of("counterpart:plain").expect("a legal identity");

        assert_eq!(
            cycle(
                Some(Hand::Main),
                Some(Cyclable::Declared(&cyclable)),
                Step::Backward
            ),
            Cycled::Stepped {
                step: Step::Backward,
                moved: false,
                selected: None,
            }
        );
        assert_eq!(
            cycle(
                Some(Hand::Main),
                Some(Cyclable::Declared(&plain)),
                Step::Forward
            ),
            Cycled::NotCyclable
        );
        assert_eq!(
            cycle(Some(Hand::Off), None, Step::Forward),
            Cycled::NotCyclable
        );
    }

    #[test]
    fn the_cycle_command_moves_the_carriers_selection_rather_than_only_reporting_one() {
        let mut carrier = Carrier::new();
        carrier
            .insert(1, installable("counterpart:one"))
            .expect("it declares carrier support");
        carrier
            .insert(4, installable("counterpart:four"))
            .expect("it declares carrier support");

        assert_eq!(
            cycle(
                Some(Hand::Main),
                Some(Cyclable::Carrier(&mut carrier)),
                Step::Forward
            ),
            Cycled::Stepped {
                step: Step::Forward,
                moved: true,
                selected: Some(1),
            }
        );
        assert_eq!(carrier.selected(), 1, "the command drove the carrier");

        assert_eq!(
            cycle(
                Some(Hand::Main),
                Some(Cyclable::Carrier(&mut carrier)),
                Step::Backward
            ),
            Cycled::Stepped {
                step: Step::Backward,
                moved: true,
                selected: Some(4),
            }
        );
        assert_eq!(carrier.selected(), 4);
    }

    #[test]
    fn the_direction_argument_is_one_of_two_words_and_nothing_else() {
        assert_eq!(direction(&["true".to_owned()]), Ok(Step::Forward));
        assert_eq!(direction(&["false".to_owned()]), Ok(Step::Backward));
        assert_eq!(direction(&[]), Err(ArgumentRefused::Count { given: 0 }));
        assert_eq!(
            direction(&["true".to_owned(), "false".to_owned()]),
            Err(ArgumentRefused::Count { given: 2 })
        );
        assert_eq!(
            direction(&["yes".to_owned()]),
            Err(ArgumentRefused::NotATruthValue),
            "text a player typed is not read as one of the two words because it is nearby"
        );
    }

    #[test]
    fn both_typed_names_resolve_to_a_command_and_the_reply_encoding_does_not() {
        for name in NAMES {
            assert_eq!(
                resolve(name).expect("both names are filed").files,
                Files::Command
            );
        }
        assert!(resolve(TEXT_LIST_REPLY).is_none());
        assert!(resolve("absent").is_none());
    }

    #[test]
    fn two_key_bindings_are_shipped_and_they_send_opposite_directions() {
        assert_eq!(BINDINGS.len(), 2);
        assert_eq!(BINDINGS[0].sends, Step::Forward);
        assert_eq!(BINDINGS[1].sends, Step::Backward);
        assert_ne!(BINDINGS[0].default_key, BINDINGS[1].default_key);
        assert_ne!(BINDINGS[0].name, BINDINGS[1].name);
    }
}
