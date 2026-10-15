//! The command transport dependents build on, from §7.
//!
//! A dependent declares named commands on a block entity and calls them from a
//! screen; this carries the call, runs it on the authoritative side and carries
//! any answer back. A global command is keyed by namespace and id and filed by
//! the side it runs on; a list command's element type and codec are keyed by the
//! bare name instead, so one name two namespaces both declare collides there and
//! nowhere else. One refusal rule survives that dependents branch on: an unknown
//! key answers false rather than raising.
//!
//! **Host-blocked.** Nothing Ravel publishes lets a mod discover another mod's
//! statically declared commands by inspecting its types, so the original's
//! annotation scan has no equivalent here. A dependent declares its commands
//! explicitly at its own declaration point and the guarantee that a command
//! exists before it is called comes from declaration order rather than from
//! discovery. What survives is the part a dependent observes: the shapes, the
//! exempt element set, the startup-time refusal of a list command with no
//! codec, and the return-false contract.

use std::collections::BTreeMap;
use std::fmt;

/// What a declared command does when it is called.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// Runs on the server against the block entity, the player and the
    /// arguments, and returns nothing.
    FireAndForget,
    /// Runs on the server, returns a bag of typed values, and a client handler
    /// receives them.
    WithResult,
    /// Runs on the server, returns a list of one element type, and a client
    /// handler receives the list.
    WithList,
}

/// The type of a list command's elements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ElementType {
    /// A signed integer.
    Integer,
    /// Text.
    Text,
    /// A position in the world.
    Position,
    /// A stack of items.
    ItemStack,
    /// A quantity of fluid.
    FluidStack,
    /// Anything else, named by the declaration.
    Other(String),
}

impl ElementType {
    /// The five types that need no codec because encoders are built in.
    pub const EXEMPT: [Self; 5] = [
        Self::Integer,
        Self::Text,
        Self::Position,
        Self::ItemStack,
        Self::FluidStack,
    ];

    /// Returns whether this type is one an encoder is built in for.
    #[must_use]
    pub fn is_exempt(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}

/// A value carried in a command's argument bag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Argument {
    /// A signed integer.
    Integer(i32),
    /// Text.
    Text(String),
    /// A truth value.
    Boolean(bool),
}

/// What running a command answered.
///
/// One arm per shape of §7, which is what makes [`Shape`] a promise about the
/// answer rather than a label: [`Commands::dispatch`] refuses a handler whose
/// answer does not match the shape its declaration published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Answer {
    /// A fire-and-forget command ran and answers nothing.
    Nothing,
    /// A command answering a bag of typed values.
    Values(Vec<Argument>),
    /// A command answering a list of one element type.
    List(Vec<Argument>),
}

impl Answer {
    /// Returns whether this answer is the one a shape promises.
    #[must_use]
    pub const fn matches(&self, shape: Shape) -> bool {
        matches!(
            (self, shape),
            (Self::Nothing, Shape::FireAndForget)
                | (Self::Values(_), Shape::WithResult)
                | (Self::List(_), Shape::WithList)
        )
    }
}

/// What a declared command runs when it is called.
///
/// The handler runs on the side the declaration was filed under, against the
/// argument bag the caller sent, and answers what its shape promises.
pub type Handler = Box<dyn Fn(&[Argument]) -> Answer + Send + Sync>;

/// One command a block entity declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    /// The command's own name, unique inside its namespace.
    pub name: String,
    /// What calling it does.
    pub shape: Shape,
    /// The element type, for a list command.
    pub element: Option<ElementType>,
    /// The codec that encodes an element the exempt set does not cover.
    pub codec: Option<String>,
}

impl Declaration {
    /// Declares a command that returns nothing.
    #[must_use]
    pub fn fire_and_forget(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            shape: Shape::FireAndForget,
            element: None,
            codec: None,
        }
    }

    /// Declares a command that returns a bag of typed values.
    #[must_use]
    pub fn with_result(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            shape: Shape::WithResult,
            element: None,
            codec: None,
        }
    }

    /// Declares a command that returns a list of one element type.
    #[must_use]
    pub fn with_list(name: &str, element: ElementType, codec: Option<&str>) -> Self {
        Self {
            name: name.to_owned(),
            shape: Shape::WithList,
            element: Some(element),
            codec: codec.map(str::to_owned),
        }
    }

    /// Checks the declaration is one this transport can carry.
    ///
    /// # Errors
    ///
    /// Returns [`DeclarationRefused`] for a list command with no element type,
    /// for a non-list command that names one, and for a list command whose
    /// element is outside the exempt set and which names no codec. That last is
    /// the case the original discovers by instantiating the codec its marking
    /// declares, whose default value is the codec *interface*: instantiating an
    /// interface fails and the surrounding catch raises. It happens when the
    /// type is first inspected, so a missing codec is a startup fault and not a
    /// runtime one, and it is checked here at declaration for the same reason.
    pub fn check(&self) -> Result<(), DeclarationRefused> {
        match (self.shape, &self.element) {
            (Shape::WithList, None) => Err(DeclarationRefused::NoElement {
                name: self.name.clone(),
            }),
            (Shape::FireAndForget | Shape::WithResult, Some(_)) => {
                Err(DeclarationRefused::UnwantedElement {
                    name: self.name.clone(),
                })
            }
            (Shape::WithList, Some(element)) => {
                if element.is_exempt() || self.codec.is_some() {
                    Ok(())
                } else {
                    Err(DeclarationRefused::NoCodec {
                        name: self.name.clone(),
                    })
                }
            }
            (Shape::FireAndForget | Shape::WithResult, None) => Ok(()),
        }
    }
}

/// Why a command declaration was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationRefused {
    /// A list command named no element type.
    NoElement {
        /// The command.
        name: String,
    },
    /// A command that returns no list named an element type.
    UnwantedElement {
        /// The command.
        name: String,
    },
    /// A list command's element is outside the exempt set and it named no codec.
    NoCodec {
        /// The command.
        name: String,
    },
}

impl fmt::Display for DeclarationRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoElement { name } => {
                write!(
                    formatter,
                    "`{name}` returns a list and names no element type"
                )
            }
            Self::UnwantedElement { name } => {
                write!(
                    formatter,
                    "`{name}` returns no list and names an element type"
                )
            }
            Self::NoCodec { name } => write!(
                formatter,
                "`{name}` returns a list of a type no encoder is built in for and names no codec"
            ),
        }
    }
}

impl std::error::Error for DeclarationRefused {}

/// The key a global command is filed under: an owning namespace and a name.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CommandKey {
    /// The namespace of the package that owns the command.
    pub namespace: String,
    /// The command's own name.
    pub name: String,
}

impl CommandKey {
    /// The most characters either half of a key may carry.
    pub const MAX_PART: usize = 64;

    /// Builds a key, refusing an empty or over-long half.
    ///
    /// A key arrives from the network: any client may send any global command
    /// id, so this is a trust boundary and the shape is checked before the key
    /// is used to look anything up.
    #[must_use]
    pub fn new(namespace: &str, name: &str) -> Option<Self> {
        let legal = |part: &str| {
            !part.is_empty()
                && part.len() <= Self::MAX_PART
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
        };
        if !legal(namespace) || !legal(name) {
            return None;
        }
        Some(Self {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl fmt::Display for CommandKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.namespace, self.name)
    }
}

/// Which side a global command runs on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Where {
    /// It runs on the server.
    Server,
    /// It runs on a client.
    Client,
}

/// The declared commands of §7, filed under the two key shapes it uses.
///
/// Global commands are keyed by namespace and id, kept apart by the side they
/// run on; a list command's element type and codec are keyed by the bare command
/// name, so a name two namespaces both declare is one key here and two among the
/// commands. The original replaces silently under either key; this reports the
/// replacement, which is the declared change, and keeps the return-false
/// contract for an unknown key, which is not, because dependents branch on it.
#[derive(Default)]
pub struct Commands {
    server: BTreeMap<CommandKey, Entry>,
    client: BTreeMap<CommandKey, Entry>,
    elements: BTreeMap<String, (ElementType, Option<String>)>,
}

struct Entry {
    declaration: Declaration,
    handler: Handler,
}

impl fmt::Debug for Commands {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Commands")
            .field("server", &self.server.keys().collect::<Vec<_>>())
            .field("client", &self.client.keys().collect::<Vec<_>>())
            .field("elements", &self.elements)
            .finish()
    }
}

/// What filing a command displaced, in either table it is filed in.
///
/// The original replaces silently under either key. Reporting both halves is
/// the declared change: two namespaces declaring one list-command name disagree
/// about the element codec, and the side table alone never said so.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Replaced {
    /// The command the key already held on that side.
    pub command: Option<Declaration>,
    /// The element type and codec the command name already held.
    pub element: Option<(ElementType, Option<String>)>,
}

impl Replaced {
    /// Returns whether this declaration displaced nothing at all.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.command.is_none() && self.element.is_none()
    }
}

impl Commands {
    /// Builds an empty set of tables.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            server: BTreeMap::new(),
            client: BTreeMap::new(),
            elements: BTreeMap::new(),
        }
    }

    /// Files a global command and what running it does, answering what it
    /// displaced in both tables.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Declaration::check`] refuses, at declaration rather
    /// than at the first call.
    pub fn declare(
        &mut self,
        side: Where,
        key: CommandKey,
        declaration: Declaration,
        handler: Handler,
    ) -> Result<Replaced, DeclarationRefused> {
        declaration.check()?;
        let element = match (declaration.shape, declaration.element.clone()) {
            (Shape::WithList, Some(element)) => self.elements.insert(
                declaration.name.clone(),
                (element, declaration.codec.clone()),
            ),
            (Shape::FireAndForget | Shape::WithResult, _) | (Shape::WithList, None) => None,
        };
        let table = match side {
            Where::Server => &mut self.server,
            Where::Client => &mut self.client,
        };
        let command = table
            .insert(
                key,
                Entry {
                    declaration,
                    handler,
                },
            )
            .map(|entry| entry.declaration);
        Ok(Replaced { command, element })
    }

    /// Returns the command a key names on one side.
    #[must_use]
    pub fn command(&self, side: Where, key: &CommandKey) -> Option<&Declaration> {
        self.entry(side, key).map(|entry| &entry.declaration)
    }

    fn entry(&self, side: Where, key: &CommandKey) -> Option<&Entry> {
        match side {
            Where::Server => self.server.get(key),
            Where::Client => self.client.get(key),
        }
    }

    /// Returns the element type and codec a list command's name is filed under.
    #[must_use]
    pub fn element(&self, name: &str) -> Option<&(ElementType, Option<String>)> {
        self.elements.get(name)
    }

    /// Runs the command a key names, answering nothing for a key nothing
    /// answers.
    ///
    /// An unknown key is not an error here, because dependents branch on the
    /// answer: `None` is the original's `false` and `Some` is its `true`. What
    /// *is* checked is the call itself: the argument count and the length of
    /// every text argument, both before the handler runs on the calling player's
    /// behalf, and the answer's shape afterwards.
    ///
    /// # Errors
    ///
    /// Returns [`CallRefused`] when the argument bag is outside what a call may
    /// carry, which is a security requirement this counterpart adds because any
    /// client may send any global command id and any bag with it, and when a
    /// handler answers something other than what its shape promised.
    pub fn dispatch(
        &self,
        side: Where,
        key: &CommandKey,
        arguments: &[Argument],
    ) -> Result<Option<Answer>, CallRefused> {
        check_arguments(arguments)?;
        let Some(entry) = self.entry(side, key) else {
            return Ok(None);
        };
        let answer = (entry.handler)(arguments);
        if !answer.matches(entry.declaration.shape) {
            return Err(CallRefused::WrongAnswer {
                name: entry.declaration.name.clone(),
            });
        }
        Ok(Some(answer))
    }

    /// Returns how many global commands one side holds.
    #[must_use]
    pub fn len(&self, side: Where) -> usize {
        match side {
            Where::Server => self.server.len(),
            Where::Client => self.client.len(),
        }
    }

    /// Returns whether one side holds no command at all.
    #[must_use]
    pub fn is_empty(&self, side: Where) -> bool {
        self.len(side) == 0
    }
}

/// The most arguments one call may carry.
pub const MAX_ARGUMENTS: usize = 64;

/// The most bytes one text argument may carry.
pub const MAX_ARGUMENT_TEXT: usize = 32_768;

/// Why a call was refused before it reached a handler, or its answer after.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallRefused {
    /// The bag carries more arguments than a call may.
    TooManyArguments {
        /// How many it carried.
        count: usize,
    },
    /// A text argument is longer than one may be.
    ArgumentTooLong {
        /// How long it was.
        length: usize,
    },
    /// An argument off the wire is tagged with a type this package does not
    /// publish.
    UnknownArgument {
        /// The tag the bag carried.
        tag: u32,
    },
    /// A handler answered something other than what its shape promised.
    WrongAnswer {
        /// The command that answered.
        name: String,
    },
}

impl fmt::Display for CallRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyArguments { count } => write!(
                formatter,
                "a call carries {count} arguments and {MAX_ARGUMENTS} is the most one may carry"
            ),
            Self::ArgumentTooLong { length } => write!(
                formatter,
                "a text argument carries {length} bytes and {MAX_ARGUMENT_TEXT} is the most one may carry"
            ),
            Self::UnknownArgument { tag } => write!(
                formatter,
                "`{tag}` is not one of the three argument types a call may carry"
            ),
            Self::WrongAnswer { name } => write!(
                formatter,
                "`{name}` answered something other than the shape it was declared with"
            ),
        }
    }
}

impl std::error::Error for CallRefused {}

/// Checks an argument bag against what a call may carry.
///
/// # Errors
///
/// Returns [`CallRefused`] for a bag with too many arguments or an over-long
/// text argument.
pub fn check_arguments(arguments: &[Argument]) -> Result<(), CallRefused> {
    if arguments.len() > MAX_ARGUMENTS {
        return Err(CallRefused::TooManyArguments {
            count: arguments.len(),
        });
    }
    for argument in arguments {
        if let Argument::Text(text) = argument
            && text.len() > MAX_ARGUMENT_TEXT
        {
            return Err(CallRefused::ArgumentTooLong { length: text.len() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> CommandKey {
        CommandKey::new("dependent", name).expect("a legal key")
    }

    fn silent() -> Handler {
        Box::new(|_| Answer::Nothing)
    }

    fn answering(values: Vec<Argument>) -> Handler {
        Box::new(move |_| Answer::Values(values.clone()))
    }

    #[test]
    fn the_five_exempt_element_types_need_no_codec_and_anything_else_does() {
        for element in ElementType::EXEMPT {
            assert!(element.is_exempt());
            Declaration::with_list("listing", element, None)
                .check()
                .expect("an exempt element needs no codec");
        }
        let other = ElementType::Other("something".to_owned());
        assert!(!other.is_exempt());
        assert_eq!(
            Declaration::with_list("listing", other.clone(), None).check(),
            Err(DeclarationRefused::NoCodec {
                name: "listing".to_owned()
            })
        );
        Declaration::with_list("listing", other, Some("a_codec"))
            .check()
            .expect("a named codec answers for a type no encoder is built in for");
    }

    #[test]
    fn a_missing_codec_is_a_declaration_time_fault_and_not_a_call_time_one() {
        let mut commands = Commands::new();
        let refused = commands.declare(
            Where::Server,
            key("listing"),
            Declaration::with_list("listing", ElementType::Other("x".to_owned()), None),
            silent(),
        );
        assert!(refused.is_err());
        assert!(
            commands.is_empty(Where::Server),
            "a refused declaration files nothing, so the call can never happen"
        );
    }

    #[test]
    fn a_shape_and_an_element_type_have_to_agree() {
        assert_eq!(
            Declaration {
                name: "listing".to_owned(),
                shape: Shape::WithList,
                element: None,
                codec: None,
            }
            .check(),
            Err(DeclarationRefused::NoElement {
                name: "listing".to_owned()
            })
        );
        assert_eq!(
            Declaration {
                name: "act".to_owned(),
                shape: Shape::FireAndForget,
                element: Some(ElementType::Integer),
                codec: None,
            }
            .check(),
            Err(DeclarationRefused::UnwantedElement {
                name: "act".to_owned()
            })
        );
        Declaration::fire_and_forget("act")
            .check()
            .expect("a legal shape");
        Declaration::with_result("ask")
            .check()
            .expect("a legal shape");
    }

    #[test]
    fn the_two_global_tables_are_kept_apart_by_side() {
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("act"),
                Declaration::fire_and_forget("act"),
                silent(),
            )
            .expect("a legal declaration");
        assert_eq!(commands.len(Where::Server), 1);
        assert_eq!(commands.len(Where::Client), 0);
        assert!(commands.command(Where::Client, &key("act")).is_none());
    }

    #[test]
    fn a_duplicate_key_replaces_and_says_what_it_replaced() {
        let mut commands = Commands::new();
        let first = commands
            .declare(
                Where::Server,
                key("act"),
                Declaration::fire_and_forget("act"),
                silent(),
            )
            .expect("a legal declaration");
        assert!(first.is_fresh(), "the first declaration replaces nothing");

        let second = commands
            .declare(
                Where::Server,
                key("act"),
                Declaration::with_result("act"),
                answering(Vec::new()),
            )
            .expect("a legal declaration");
        assert_eq!(
            second.command,
            Some(Declaration::fire_and_forget("act")),
            "the replacement is reported rather than silent"
        );
        assert_eq!(
            commands
                .command(Where::Server, &key("act"))
                .map(|held| held.shape),
            Some(Shape::WithResult)
        );
    }

    #[test]
    fn a_second_list_command_of_one_name_says_what_element_codec_it_displaced() {
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("listing"),
                Declaration::with_list("listing", ElementType::Position, None),
                Box::new(|_| Answer::List(Vec::new())),
            )
            .expect("a legal declaration");

        let second = commands
            .declare(
                Where::Server,
                CommandKey::new("elsewhere", "listing").expect("a legal key"),
                Declaration::with_list(
                    "listing",
                    ElementType::Other("something".to_owned()),
                    Some("a_codec"),
                ),
                Box::new(|_| Answer::List(Vec::new())),
            )
            .expect("a legal declaration");

        assert_eq!(
            second.command, None,
            "the two namespaces file under different keys"
        );
        assert_eq!(
            second.element,
            Some((ElementType::Position, None)),
            "and the element table they share reports what the second displaced"
        );
    }

    #[test]
    fn a_list_command_files_its_element_type_under_its_own_name() {
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("listing"),
                Declaration::with_list("listing", ElementType::Position, None),
                Box::new(|_| Answer::List(Vec::new())),
            )
            .expect("a legal declaration");
        assert_eq!(
            commands.element("listing"),
            Some(&(ElementType::Position, None))
        );
        assert_eq!(commands.element("nothing"), None);
    }

    #[test]
    fn an_unknown_key_answers_nothing_rather_than_raising() {
        let commands = Commands::new();
        assert_eq!(
            commands.dispatch(Where::Server, &key("nothing"), &[]),
            Ok(None)
        );

        let mut declared = Commands::new();
        declared
            .declare(
                Where::Server,
                key("act"),
                Declaration::fire_and_forget("act"),
                silent(),
            )
            .expect("a legal declaration");
        assert_eq!(
            declared.dispatch(Where::Server, &key("act"), &[]),
            Ok(Some(Answer::Nothing))
        );
        assert_eq!(
            declared.dispatch(Where::Client, &key("act"), &[]),
            Ok(None),
            "the side is part of the lookup"
        );
    }

    #[test]
    fn dispatching_runs_the_handler_the_declaration_was_filed_with() {
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&ran);
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("act"),
                Declaration::fire_and_forget("act"),
                Box::new(move |arguments| {
                    counted.fetch_add(arguments.len(), std::sync::atomic::Ordering::Relaxed);
                    Answer::Nothing
                }),
            )
            .expect("a legal declaration");

        let bag = [Argument::Integer(1), Argument::Boolean(true)];
        assert_eq!(
            commands.dispatch(Where::Server, &key("act"), &bag),
            Ok(Some(Answer::Nothing))
        );
        assert_eq!(
            ran.load(std::sync::atomic::Ordering::Relaxed),
            2,
            "the handler saw the bag the caller sent"
        );
    }

    #[test]
    fn a_command_with_a_result_carries_its_typed_values_back_to_the_caller() {
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("ask"),
                Declaration::with_result("ask"),
                Box::new(|arguments| {
                    Answer::Values(vec![Argument::Integer(
                        i32::try_from(arguments.len()).unwrap_or(i32::MAX),
                    )])
                }),
            )
            .expect("a legal declaration");
        assert_eq!(
            commands.dispatch(Where::Server, &key("ask"), &[Argument::Boolean(false)]),
            Ok(Some(Answer::Values(vec![Argument::Integer(1)])))
        );
    }

    #[test]
    fn a_handler_answering_something_its_shape_did_not_promise_is_refused() {
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("act"),
                Declaration::fire_and_forget("act"),
                answering(vec![Argument::Integer(1)]),
            )
            .expect("a legal declaration");
        assert_eq!(
            commands.dispatch(Where::Server, &key("act"), &[]),
            Err(CallRefused::WrongAnswer {
                name: "act".to_owned()
            })
        );

        assert!(Answer::Nothing.matches(Shape::FireAndForget));
        assert!(Answer::Values(Vec::new()).matches(Shape::WithResult));
        assert!(Answer::List(Vec::new()).matches(Shape::WithList));
        assert!(!Answer::List(Vec::new()).matches(Shape::WithResult));
    }

    #[test]
    fn a_key_off_the_network_is_checked_for_shape_before_it_looks_anything_up() {
        assert!(CommandKey::new("dependent", "act").is_some());
        assert!(CommandKey::new("dependent.two", "act.one").is_some());
        for (namespace, name) in [
            ("", "act"),
            ("dependent", ""),
            ("dependent ", "act"),
            ("dependent", "act/../other"),
            ("dependent", "act:other"),
        ] {
            assert!(
                CommandKey::new(namespace, name).is_none(),
                "`{namespace}` `{name}` is not a legal key"
            );
        }
        let long = "n".repeat(CommandKey::MAX_PART + 1);
        assert!(CommandKey::new(&long, "act").is_none());
        assert!(CommandKey::new("dependent", &long).is_none());
    }

    #[test]
    fn an_argument_bag_is_bounded_before_the_handler_acts_on_the_caller_behalf() {
        let refusing = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&refusing);
        let mut commands = Commands::new();
        commands
            .declare(
                Where::Server,
                key("act"),
                Declaration::fire_and_forget("act"),
                Box::new(move |_| {
                    counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Answer::Nothing
                }),
            )
            .expect("a legal declaration");

        let bag: Vec<Argument> = (0..=MAX_ARGUMENTS)
            .map(|number| Argument::Integer(i32::try_from(number).unwrap_or(i32::MAX)))
            .collect();
        assert_eq!(
            commands.dispatch(Where::Server, &key("act"), &bag),
            Err(CallRefused::TooManyArguments {
                count: MAX_ARGUMENTS + 1
            })
        );

        let long = Argument::Text("t".repeat(MAX_ARGUMENT_TEXT + 1));
        assert_eq!(
            commands.dispatch(Where::Server, &key("act"), std::slice::from_ref(&long)),
            Err(CallRefused::ArgumentTooLong {
                length: MAX_ARGUMENT_TEXT + 1
            })
        );
        assert_eq!(
            refusing.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "neither refused bag reached the handler"
        );

        check_arguments(&[
            Argument::Integer(1),
            Argument::Text("short".to_owned()),
            Argument::Boolean(true),
        ])
        .expect("an ordinary bag is carried");
    }
}
