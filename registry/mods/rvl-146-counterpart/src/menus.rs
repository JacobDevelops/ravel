//! The two generic action payloads of §5.1, and what happens when one arrives.
//!
//! A screen or a menu opts in by declaring named actions. The client-to-server
//! payload is looked up on the receiving player's open menu; the server-to-client
//! one on the client's open screen. [`Surface`] is which of the two a table is,
//! and a payload that arrived for the other one is refused rather than run.
//!
//! # The change declared here
//!
//! **An action declares its argument shape, and the shape is checked before the
//! handler runs.** In the original a handler is a callback taking the argument
//! vector, and the vector's accessor casts each stored value to whatever the
//! caller expects without checking, so a vector of the wrong arity or the wrong
//! types reaches the handler and fails inside it, on the server thread, with the
//! bytes already decoded. Here [`Actions::declare`] takes a [`Shape`] beside the
//! handler, [`Actions::dispatch`] checks arity and kinds against it, and a
//! mismatch is a [`Dispatch::Refused`] the handler never sees.
//!
//! What is deliberately kept is that a name nothing declared is ignored rather
//! than being an error: §5.1 reads it that way and it is not in §13's table. It
//! is answered as [`Dispatch::Unknown`] rather than passing silently, so a caller
//! that wants to notice can.

use std::collections::BTreeMap;
use std::fmt;

use crate::transport::{Action, Argument, ArgumentKind};

/// The most actions one table will hold.
///
/// A table is built by a dependent rather than by a peer, so this is a ceiling
/// on a mistake rather than on an attack. The upgrade path is a larger constant.
pub const MAX_ACTIONS: usize = 256;

/// Which of the two payloads a table answers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Surface {
    /// A menu, which the server resolves a client-sent action on.
    Menu,
    /// A screen, which the client resolves a server-sent action on.
    Screen,
}

impl Surface {
    /// Returns the surface's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Screen => "screen",
        }
    }
}

impl fmt::Display for Surface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// What one argument slot admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    /// One kind and no other.
    Exactly(ArgumentKind),
    /// One kind, or nothing in its place.
    Optional(ArgumentKind),
}

impl Slot {
    /// Returns whether an argument fills this slot.
    #[must_use]
    pub fn admits(self, argument: &Argument) -> bool {
        match self {
            Self::Exactly(kind) => argument.kind() == kind,
            Self::Optional(kind) => {
                argument.kind() == kind || argument.kind() == ArgumentKind::Absent
            }
        }
    }

    /// Returns the kind this slot is about.
    #[must_use]
    pub const fn kind(self) -> ArgumentKind {
        match self {
            Self::Exactly(kind) | Self::Optional(kind) => kind,
        }
    }
}

/// The argument shape an action declares.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Shape {
    slots: Vec<Slot>,
}

impl Shape {
    /// An action taking no arguments.
    #[must_use]
    pub const fn none() -> Self {
        Self { slots: Vec::new() }
    }

    /// An action taking exactly these kinds, in order.
    #[must_use]
    pub fn of(kinds: &[ArgumentKind]) -> Self {
        Self {
            slots: kinds.iter().copied().map(Slot::Exactly).collect(),
        }
    }

    /// An action taking these slots, in order.
    #[must_use]
    pub fn from_slots(slots: Vec<Slot>) -> Self {
        Self { slots }
    }

    /// Returns how many arguments this shape requires.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.slots.len()
    }

    /// Returns the slots, in order.
    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// Checks an arrived vector against this shape.
    ///
    /// # Errors
    ///
    /// Returns [`Mismatch::Arity`] when the counts differ and
    /// [`Mismatch::Kind`] for the first slot an argument does not fill.
    pub fn check(&self, arguments: &[Argument]) -> Result<(), Mismatch> {
        if arguments.len() != self.slots.len() {
            return Err(Mismatch::Arity {
                declared: self.slots.len(),
                arrived: arguments.len(),
            });
        }
        for (index, (slot, argument)) in self.slots.iter().zip(arguments).enumerate() {
            if !slot.admits(argument) {
                return Err(Mismatch::Kind {
                    index,
                    declared: slot.kind(),
                    arrived: argument.kind(),
                });
            }
        }
        Ok(())
    }
}

/// How an arrived vector disagrees with the shape its action declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mismatch {
    /// The vector is the wrong length.
    Arity {
        /// How many the action declared.
        declared: usize,
        /// How many arrived.
        arrived: usize,
    },
    /// An argument is the wrong kind.
    Kind {
        /// Which slot.
        index: usize,
        /// What the slot declared.
        declared: ArgumentKind,
        /// What arrived in it.
        arrived: ArgumentKind,
    },
    /// The payload was resolved on the other surface.
    WrongSurface {
        /// What the table is.
        declared: Surface,
        /// What the payload asked for.
        arrived: Surface,
    },
}

impl fmt::Display for Mismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity { declared, arrived } => {
                write!(formatter, "{arrived} arguments arrived for {declared}")
            }
            Self::Kind {
                index,
                declared,
                arrived,
            } => write!(formatter, "argument {index} is {arrived}, not {declared}"),
            Self::WrongSurface { declared, arrived } => {
                write!(formatter, "a {arrived} action reached a {declared}")
            }
        }
    }
}

impl std::error::Error for Mismatch {}

/// Why a table refused a declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclareRefused {
    /// An action with no name, which nothing could look up.
    EmptyName,
    /// A name already declared.
    Duplicate(String),
    /// The table already holds [`MAX_ACTIONS`].
    Full,
}

impl fmt::Display for DeclareRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => formatter.write_str("an action with no name names nothing"),
            Self::Duplicate(name) => write!(formatter, "`{name}` is declared twice"),
            Self::Full => write!(formatter, "a table holds at most {MAX_ACTIONS} actions"),
        }
    }
}

impl std::error::Error for DeclareRefused {}

/// What one action does when it arrives.
pub trait Handler {
    /// Runs the action. The arguments have already been checked against the
    /// shape the action declared, so an implementation matches on them rather
    /// than guarding.
    fn run(&mut self, arguments: &[Argument]);
}

impl<F: FnMut(&[Argument])> Handler for F {
    fn run(&mut self, arguments: &[Argument]) {
        self(arguments);
    }
}

/// What became of an arrived payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dispatch {
    /// The handler ran.
    Ran,
    /// Nothing is declared under that name. §5.1 ignores it, and this is that
    /// silence made answerable.
    Unknown,
    /// The payload disagreed with what the action declared, and the handler did
    /// not run.
    Refused(Mismatch),
}

/// The named actions one screen or menu answers.
pub struct Actions {
    surface: Surface,
    order: Vec<String>,
    table: BTreeMap<String, (Shape, Box<dyn Handler>)>,
}

impl fmt::Debug for Actions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Actions<'a> {
            surface: &'a Surface,
            declared: &'a [String],
        }
        impl fmt::Debug for Actions<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct("Actions")
                    .field("surface", &self.surface)
                    .field("declared", &self.declared)
                    .finish()
            }
        }
        fmt::Debug::fmt(
            &Actions {
                surface: &self.surface,
                declared: &self.order,
            },
            formatter,
        )
    }
}

impl Actions {
    /// An empty table for one surface.
    #[must_use]
    pub fn on(surface: Surface) -> Self {
        Self {
            surface,
            order: Vec::new(),
            table: BTreeMap::new(),
        }
    }

    /// Returns which surface this table answers.
    #[must_use]
    pub const fn surface(&self) -> Surface {
        self.surface
    }

    /// Declares one named action, its argument shape and its handler.
    ///
    /// # Errors
    ///
    /// Returns [`DeclareRefused::EmptyName`] for an unnamed action,
    /// [`DeclareRefused::Duplicate`] for a name already declared, and
    /// [`DeclareRefused::Full`] past [`MAX_ACTIONS`].
    pub fn declare(
        &mut self,
        name: &str,
        shape: Shape,
        handler: Box<dyn Handler>,
    ) -> Result<(), DeclareRefused> {
        if name.is_empty() {
            return Err(DeclareRefused::EmptyName);
        }
        if self.table.contains_key(name) {
            return Err(DeclareRefused::Duplicate(name.to_owned()));
        }
        if self.table.len() >= MAX_ACTIONS {
            return Err(DeclareRefused::Full);
        }
        self.order.push(name.to_owned());
        self.table.insert(name.to_owned(), (shape, handler));
        Ok(())
    }

    /// Returns every declared name, in declaration order.
    #[must_use]
    pub fn declared(&self) -> &[String] {
        &self.order
    }

    /// Returns the shape declared under a name.
    #[must_use]
    pub fn shape(&self, name: &str) -> Option<&Shape> {
        self.table.get(name).map(|(shape, _)| shape)
    }

    /// Resolves an arrived payload and runs it.
    ///
    /// The order is the one the original does not have: the surface, then the
    /// name, then the shape, and only then the handler.
    pub fn dispatch(&mut self, surface: Surface, action: &Action) -> Dispatch {
        if surface != self.surface {
            return Dispatch::Refused(Mismatch::WrongSurface {
                declared: self.surface,
                arrived: surface,
            });
        }
        let Some((shape, handler)) = self.table.get_mut(&action.name) else {
            return Dispatch::Unknown;
        };
        let empty: Vec<Argument> = Vec::new();
        let arguments = action.arguments.as_ref().unwrap_or(&empty);
        if let Err(mismatch) = shape.check(arguments) {
            Dispatch::Refused(mismatch)
        } else {
            handler.run(arguments);
            Dispatch::Ran
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{
        Actions, DeclareRefused, Dispatch, Handler, MAX_ACTIONS, Mismatch, Shape, Slot, Surface,
    };
    use crate::transport::{Action, Argument, ArgumentKind};

    /// A handler whose effect a test can observe.
    struct Recorder {
        seen: Rc<RefCell<Vec<String>>>,
    }

    impl Handler for Recorder {
        fn run(&mut self, arguments: &[Argument]) {
            let rendered = arguments
                .iter()
                .map(|argument| argument.kind().as_str().to_owned())
                .collect::<Vec<_>>()
                .join(",");
            self.seen.borrow_mut().push(rendered);
        }
    }

    fn table(seen: &Rc<RefCell<Vec<String>>>) -> Actions {
        let mut actions = Actions::on(Surface::Menu);
        actions
            .declare(
                "commit",
                Shape::of(&[ArgumentKind::Integer, ArgumentKind::Text]),
                Box::new(Recorder {
                    seen: Rc::clone(seen),
                }),
            )
            .expect("a fresh table admits one action");
        actions
    }

    #[test]
    fn a_declared_action_runs_and_its_effect_is_observable() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = table(&seen);
        let arrived = Action::carrying(
            "commit",
            vec![Argument::Integer(3), Argument::Text("north".to_owned())],
        );
        assert_eq!(actions.dispatch(Surface::Menu, &arrived), Dispatch::Ran);
        assert_eq!(*seen.borrow(), ["integer,text"]);
    }

    #[test]
    fn a_vector_of_the_wrong_arity_never_reaches_the_handler() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = table(&seen);
        let arrived = Action::carrying("commit", vec![Argument::Integer(3)]);
        assert_eq!(
            actions.dispatch(Surface::Menu, &arrived),
            Dispatch::Refused(Mismatch::Arity {
                declared: 2,
                arrived: 1
            })
        );
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn a_vector_of_the_wrong_kinds_never_reaches_the_handler() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = table(&seen);
        let arrived = Action::carrying(
            "commit",
            vec![Argument::Text("three".to_owned()), Argument::Integer(1)],
        );
        assert_eq!(
            actions.dispatch(Surface::Menu, &arrived),
            Dispatch::Refused(Mismatch::Kind {
                index: 0,
                declared: ArgumentKind::Integer,
                arrived: ArgumentKind::Text,
            })
        );
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn a_bare_payload_reaches_an_action_that_declared_no_arguments() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = Actions::on(Surface::Screen);
        actions
            .declare(
                "close",
                Shape::none(),
                Box::new(Recorder {
                    seen: Rc::clone(&seen),
                }),
            )
            .expect("a fresh table");
        assert_eq!(
            actions.dispatch(Surface::Screen, &Action::bare("close")),
            Dispatch::Ran
        );
        assert_eq!(*seen.borrow(), [""]);
    }

    #[test]
    fn a_name_nothing_declared_is_ignored_and_says_so() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = table(&seen);
        assert_eq!(
            actions.dispatch(Surface::Menu, &Action::bare("vanish")),
            Dispatch::Unknown
        );
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn a_payload_for_the_other_surface_is_refused_before_the_name_is_looked_up() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = table(&seen);
        let arrived = Action::carrying(
            "commit",
            vec![Argument::Integer(3), Argument::Text("north".to_owned())],
        );
        assert_eq!(
            actions.dispatch(Surface::Screen, &arrived),
            Dispatch::Refused(Mismatch::WrongSurface {
                declared: Surface::Menu,
                arrived: Surface::Screen,
            })
        );
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn an_optional_slot_admits_its_kind_and_nothing_in_its_place() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut actions = Actions::on(Surface::Menu);
        actions
            .declare(
                "select",
                Shape::from_slots(vec![Slot::Optional(ArgumentKind::Stack)]),
                Box::new(Recorder {
                    seen: Rc::clone(&seen),
                }),
            )
            .expect("a fresh table");
        for argument in [Argument::Stack(vec![1]), Argument::Absent] {
            assert_eq!(
                actions.dispatch(Surface::Menu, &Action::carrying("select", vec![argument])),
                Dispatch::Ran
            );
        }
        assert_eq!(*seen.borrow(), ["stack", "absent"]);
        assert_eq!(
            actions.dispatch(
                Surface::Menu,
                &Action::carrying("select", vec![Argument::Boolean(true)])
            ),
            Dispatch::Refused(Mismatch::Kind {
                index: 0,
                declared: ArgumentKind::Stack,
                arrived: ArgumentKind::Boolean,
            })
        );
    }

    #[test]
    fn a_table_refuses_an_unnamed_action_a_duplicate_and_a_full_table() {
        let mut actions = Actions::on(Surface::Menu);
        assert_eq!(
            actions.declare("", Shape::none(), Box::new(|_: &[Argument]| {})),
            Err(DeclareRefused::EmptyName)
        );
        actions
            .declare("one", Shape::none(), Box::new(|_: &[Argument]| {}))
            .expect("a fresh table");
        assert_eq!(
            actions.declare("one", Shape::none(), Box::new(|_: &[Argument]| {})),
            Err(DeclareRefused::Duplicate("one".to_owned()))
        );
        for index in 1..MAX_ACTIONS {
            actions
                .declare(
                    &format!("filler{index}"),
                    Shape::none(),
                    Box::new(|_: &[Argument]| {}),
                )
                .expect("inside the ceiling");
        }
        assert_eq!(
            actions.declare("past", Shape::none(), Box::new(|_: &[Argument]| {})),
            Err(DeclareRefused::Full)
        );
        assert_eq!(actions.declared().len(), MAX_ACTIONS);
    }

    #[test]
    fn a_table_reports_the_shape_it_declared() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let actions = table(&seen);
        assert_eq!(actions.surface(), Surface::Menu);
        assert_eq!(
            actions.shape("commit").map(Shape::arity),
            Some(2),
            "a dependent can read the shape it declared"
        );
        assert!(actions.shape("vanish").is_none());
    }
}
