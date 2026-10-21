//! The payload channel of §5: what a dependent declares, and who a send reaches.
//!
//! A dependent builds a channel scoped to one namespace and hands it payload
//! declarations ahead of time. Each declaration carries its own identifier and a
//! direction, and the direction alone decides which way the payload travels;
//! declaration order has no effect on the wire and is kept only so a dependent
//! reads back the sequence it wrote.
//!
//! Four send targets exist: every player, one named player, every player within
//! a radius of a point in a level with an optional exclusion, and the server.
//!
//! # The changes declared here
//!
//! - **A radius send with no running server answers rather than faulting.** The
//!   original reaches the running server through a process-wide slot which the
//!   accessor beside it would have reported absent for, so an unset slot faults
//!   at the send instead of yielding absent. [`Sends::resolve`] returns
//!   [`SendRefused::NoServer`], which is the same "nothing happened" the original
//!   intends and a caller can see.
//! - **A radius that is not a number is refused.** The original hands whatever it
//!   was given to the platform's distributor. A `NaN` radius makes every distance
//!   comparison false, so the send silently reaches nobody and looks like a
//!   delivery that happened.
//! - **A payload is an immutable value.** The original's reader constructs an
//!   empty instance from the payload's own factory and fills it from the buffer,
//!   so every payload is mutable and has an empty state. Nothing here has one.
//!
//! # Declarable only
//!
//! **Carrying a payload to the other end.** `ravel_sdk::network` publishes the
//! contract, the framing and the side rule and states plainly that nothing routes
//! a mod's packet yet. [`Channel::contract`] produces a real Ravel contract whose
//! name is this channel's derived identifier, so what Ravel would route on and
//! what a dependent declared are one string rather than two that can disagree.
//! [`Sends::resolve`] resolves a target into a [`Delivery`] and the carrying is
//! the host's.

use std::fmt;

use ravel_sdk::ModId;
use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{Direction, PacketContract, PacketId};

/// The most payloads one channel will declare.
///
/// The upgrade path is a larger constant. A dependent past it has a naming
/// problem rather than a capacity one.
pub const MAX_PAYLOADS: usize = 256;

/// One declared payload: what it is called and which way it travels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    id: PacketId,
    direction: Direction,
}

impl Declaration {
    /// Returns the payload's identifier.
    #[must_use]
    pub const fn id(&self) -> &PacketId {
        &self.id
    }

    /// Returns which way the payload travels.
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.direction
    }
}

/// Why a channel refused a declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelRefused {
    /// The path is not a legal content path.
    Path(ContentIdError),
    /// A payload is already declared under that path.
    Duplicate(String),
    /// The channel already holds [`MAX_PAYLOADS`].
    Full,
}

impl fmt::Display for ChannelRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(source) => write!(formatter, "{source}"),
            Self::Duplicate(path) => write!(formatter, "`{path}` is declared twice"),
            Self::Full => write!(
                formatter,
                "a channel declares at most {MAX_PAYLOADS} payloads"
            ),
        }
    }
}

impl std::error::Error for ChannelRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(source) => Some(source),
            Self::Duplicate(_) | Self::Full => None,
        }
    }
}

/// A channel scoped to one namespace, holding the payloads a dependent declared.
#[derive(Clone, Debug)]
pub struct Channel {
    namespace: ModId,
    version: u32,
    declarations: Vec<Declaration>,
}

impl Channel {
    /// An empty channel in one namespace, at one protocol version.
    #[must_use]
    pub const fn new(namespace: ModId, version: u32) -> Self {
        Self {
            namespace,
            version,
            declarations: Vec::new(),
        }
    }

    /// Returns the namespace every payload here is named in.
    #[must_use]
    pub const fn namespace(&self) -> &ModId {
        &self.namespace
    }

    /// Returns the protocol version every contract carries.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Declares one payload.
    ///
    /// # Errors
    ///
    /// Returns [`ChannelRefused::Path`] for an illegal path,
    /// [`ChannelRefused::Duplicate`] for a path already declared, and
    /// [`ChannelRefused::Full`] past [`MAX_PAYLOADS`].
    pub fn declare(
        &mut self,
        path: &str,
        direction: Direction,
    ) -> Result<PacketId, ChannelRefused> {
        let id = ContentId::new(self.namespace.clone(), path).map_err(ChannelRefused::Path)?;
        let id = PacketId::new(id);
        if self.declarations.iter().any(|held| held.id == id) {
            return Err(ChannelRefused::Duplicate(path.to_owned()));
        }
        if self.declarations.len() >= MAX_PAYLOADS {
            return Err(ChannelRefused::Full);
        }
        self.declarations.push(Declaration {
            id: id.clone(),
            direction,
        });
        Ok(id)
    }

    /// Returns every declaration, in declaration order.
    #[must_use]
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    /// Returns the declaration made under a path.
    #[must_use]
    pub fn declaration(&self, path: &str) -> Option<&Declaration> {
        self.declarations
            .iter()
            .find(|held| held.id.id().path() == path)
    }

    /// Returns the Ravel contract for a declared payload.
    ///
    /// The identifier the contract carries is the one this channel derived, so
    /// there is one string rather than two that can disagree.
    #[must_use]
    pub fn contract(&self, path: &str) -> Option<PacketContract> {
        self.declaration(path).map(|declaration| {
            PacketContract::new(declaration.id.clone(), declaration.direction, self.version)
        })
    }
}

/// A point a radius send is measured from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Point {
    /// A block position.
    Block(i32, i32, i32),
    /// A floating-point position.
    Precise(f64, f64, f64),
}

/// Who a send reaches.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Every player.
    Everyone,
    /// One named player.
    Player(String),
    /// Every player within a radius of a point in a level, with an optional
    /// exclusion.
    Within {
        /// The level the point is in.
        level: String,
        /// The point.
        centre: Point,
        /// The radius, in blocks.
        radius: f64,
        /// A player the send skips.
        except: Option<String>,
    },
    /// The server.
    Server,
}

/// Why a send did not happen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendRefused {
    /// A radius send was made with no server running.
    NoServer,
    /// A radius send named no level.
    NoLevel,
    /// A radius that is not a number, which would reach nobody while looking
    /// like a delivery.
    RadiusNotANumber,
    /// A named player with an empty name.
    NoPlayer,
    /// The payload travels the other way.
    WrongDirection {
        /// What the payload declared.
        declared: Direction,
    },
}

impl fmt::Display for SendRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoServer => formatter.write_str("no server is running"),
            Self::NoLevel => formatter.write_str("a radius send named no level"),
            Self::RadiusNotANumber => formatter.write_str("a radius that is not a number"),
            Self::NoPlayer => formatter.write_str("a send named no player"),
            Self::WrongDirection { declared } => {
                write!(formatter, "a {declared} payload was sent the other way")
            }
        }
    }
}

impl std::error::Error for SendRefused {}

/// What a resolved send hands the host.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    /// The payload being sent.
    pub id: PacketId,
    /// Who it goes to.
    pub target: Target,
}

/// The send helpers, and the running server they need.
#[derive(Clone, Debug, Default)]
pub struct Sends {
    server: Option<String>,
}

impl Sends {
    /// The helpers with no server running.
    #[must_use]
    pub const fn idle() -> Self {
        Self { server: None }
    }

    /// Records that a server is running.
    pub fn server_started(&mut self, name: &str) {
        self.server = Some(name.to_owned());
    }

    /// Records that the running server is gone.
    pub fn server_stopped(&mut self) {
        self.server = None;
    }

    /// Returns the running server, and absent when there is none.
    ///
    /// This is §2's accessor, and it answers absent on every path rather than on
    /// some of them.
    #[must_use]
    pub fn server(&self) -> Option<&str> {
        self.server.as_deref()
    }

    /// Resolves a send.
    ///
    /// # Errors
    ///
    /// Returns [`SendRefused::WrongDirection`] for a payload sent against its
    /// declared direction, [`SendRefused::NoPlayer`] for an unnamed player, and
    /// for a radius send [`SendRefused::NoServer`] with no server running,
    /// [`SendRefused::NoLevel`] for an unnamed level and
    /// [`SendRefused::RadiusNotANumber`] for a radius that is not a number.
    pub fn resolve(
        &self,
        declaration: &Declaration,
        target: Target,
    ) -> Result<Delivery, SendRefused> {
        let travels_to_server = matches!(target, Target::Server);
        let declared_to_server = declaration.direction == Direction::ToServer;
        if travels_to_server != declared_to_server {
            return Err(SendRefused::WrongDirection {
                declared: declaration.direction,
            });
        }
        match &target {
            Target::Everyone | Target::Server => {}
            Target::Player(name) => {
                if name.is_empty() {
                    return Err(SendRefused::NoPlayer);
                }
            }
            Target::Within {
                level,
                radius,
                except,
                ..
            } => {
                if self.server.is_none() {
                    return Err(SendRefused::NoServer);
                }
                if level.is_empty() {
                    return Err(SendRefused::NoLevel);
                }
                if radius.is_nan() {
                    return Err(SendRefused::RadiusNotANumber);
                }
                if except.as_ref().is_some_and(String::is_empty) {
                    return Err(SendRefused::NoPlayer);
                }
            }
        }
        Ok(Delivery {
            id: declaration.id.clone(),
            target,
        })
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::ModId;
    use ravel_sdk::network::Direction;

    use super::{Channel, ChannelRefused, MAX_PAYLOADS, Point, SendRefused, Sends, Target};

    fn namespace() -> ModId {
        "rvl_146_counterpart"
            .parse()
            .expect("the declared identity is a legal mod id")
    }

    fn channel() -> Channel {
        let mut channel = Channel::new(namespace(), 3);
        channel
            .declare("edit", Direction::ToServer)
            .expect("a legal path");
        channel
            .declare("report", Direction::ToClient)
            .expect("a legal path");
        channel
    }

    #[test]
    fn a_refused_path_exposes_its_validation_error() {
        use std::error::Error;

        let error = Channel::new(namespace(), 3)
            .declare("not a path", Direction::ToServer)
            .expect_err("an invalid content path");
        let source = error.source().expect("the path validation error");
        assert!(source.is::<ravel_sdk::content::ContentIdError>());
        assert_eq!(source.to_string(), error.to_string());
        assert!(
            ChannelRefused::Duplicate("edit".to_owned())
                .source()
                .is_none()
        );
        assert!(ChannelRefused::Full.source().is_none());
    }

    #[test]
    fn a_payload_is_named_in_the_channels_namespace_and_keeps_declaration_order() {
        let channel = channel();
        assert_eq!(
            channel
                .declarations()
                .iter()
                .map(|held| held.id().to_string())
                .collect::<Vec<_>>(),
            [
                "rvl_146_counterpart:edit".to_owned(),
                "rvl_146_counterpart:report".to_owned()
            ]
        );
    }

    #[test]
    fn the_direction_flag_alone_decides_which_way_a_payload_travels() {
        let channel = channel();
        assert_eq!(
            channel.declaration("edit").expect("declared").direction(),
            Direction::ToServer
        );
        assert_eq!(
            channel.declaration("report").expect("declared").direction(),
            Direction::ToClient
        );
    }

    #[test]
    fn the_ravel_contract_carries_the_derived_identifier_and_the_channels_version() {
        let channel = channel();
        let contract = channel.contract("report").expect("a declared payload");
        assert_eq!(contract.id().to_string(), "rvl_146_counterpart:report");
        assert_eq!(contract.direction(), Direction::ToClient);
        assert_eq!(contract.version(), 3);
        assert!(channel.contract("vanish").is_none());
    }

    #[test]
    fn a_channel_refuses_an_illegal_path_a_duplicate_and_a_full_channel() {
        let mut channel = channel();
        assert!(matches!(
            channel.declare("Not A Path", Direction::ToServer),
            Err(ChannelRefused::Path(_))
        ));
        assert_eq!(
            channel.declare("edit", Direction::ToClient),
            Err(ChannelRefused::Duplicate("edit".to_owned()))
        );
        for index in channel.declarations().len()..MAX_PAYLOADS {
            channel
                .declare(&format!("filler{index}"), Direction::ToServer)
                .expect("inside the ceiling");
        }
        assert_eq!(
            channel.declare("past", Direction::ToServer),
            Err(ChannelRefused::Full)
        );
    }

    #[test]
    fn a_radius_send_with_no_server_answers_rather_than_faulting() {
        let channel = channel();
        let report = channel.declaration("report").expect("declared");
        let sends = Sends::idle();
        assert_eq!(sends.server(), None);
        assert_eq!(
            sends.resolve(
                report,
                Target::Within {
                    level: "overworld".to_owned(),
                    centre: Point::Block(0, 64, 0),
                    radius: 16.0,
                    except: None,
                }
            ),
            Err(SendRefused::NoServer)
        );
    }

    #[test]
    fn every_target_resolves_once_a_server_is_running() {
        let channel = channel();
        let report = channel.declaration("report").expect("declared");
        let edit = channel.declaration("edit").expect("declared");
        let mut sends = Sends::idle();
        sends.server_started("the running server");
        assert_eq!(sends.server(), Some("the running server"));
        for target in [
            Target::Everyone,
            Target::Player("one".to_owned()),
            Target::Within {
                level: "overworld".to_owned(),
                centre: Point::Block(0, 64, 0),
                radius: 16.0,
                except: Some("two".to_owned()),
            },
            Target::Within {
                level: "overworld".to_owned(),
                centre: Point::Precise(0.5, 64.0, 0.5),
                radius: 16.0,
                except: None,
            },
        ] {
            let delivery = sends.resolve(report, target.clone()).expect("a live send");
            assert_eq!(delivery.id, *report.id());
            assert_eq!(delivery.target, target);
        }
        assert_eq!(
            sends
                .resolve(edit, Target::Server)
                .expect("a serverbound send")
                .target,
            Target::Server
        );
        sends.server_stopped();
        assert_eq!(sends.server(), None);
    }

    #[test]
    fn a_payload_sent_against_its_direction_is_refused() {
        let channel = channel();
        let report = channel.declaration("report").expect("declared");
        let edit = channel.declaration("edit").expect("declared");
        let sends = Sends::idle();
        assert_eq!(
            sends.resolve(report, Target::Server),
            Err(SendRefused::WrongDirection {
                declared: Direction::ToClient
            })
        );
        assert_eq!(
            sends.resolve(edit, Target::Everyone),
            Err(SendRefused::WrongDirection {
                declared: Direction::ToServer
            })
        );
    }

    #[test]
    fn a_radius_that_is_not_a_number_is_refused_rather_than_reaching_nobody_quietly() {
        let channel = channel();
        let report = channel.declaration("report").expect("declared");
        let mut sends = Sends::idle();
        sends.server_started("the running server");
        assert_eq!(
            sends.resolve(
                report,
                Target::Within {
                    level: "overworld".to_owned(),
                    centre: Point::Block(0, 64, 0),
                    radius: f64::NAN,
                    except: None,
                }
            ),
            Err(SendRefused::RadiusNotANumber)
        );
        assert!(
            sends
                .resolve(
                    report,
                    Target::Within {
                        level: "overworld".to_owned(),
                        centre: Point::Block(0, 64, 0),
                        radius: -1.0,
                        except: None,
                    }
                )
                .is_ok(),
            "a negative radius is a number and reaches nobody, which is the platform's answer"
        );
    }

    #[test]
    fn an_unnamed_level_and_an_unnamed_player_are_refused() {
        let channel = channel();
        let report = channel.declaration("report").expect("declared");
        let mut sends = Sends::idle();
        sends.server_started("the running server");
        assert_eq!(
            sends.resolve(
                report,
                Target::Within {
                    level: String::new(),
                    centre: Point::Block(0, 64, 0),
                    radius: 8.0,
                    except: None,
                }
            ),
            Err(SendRefused::NoLevel)
        );
        assert_eq!(
            sends.resolve(report, Target::Player(String::new())),
            Err(SendRefused::NoPlayer)
        );
        assert_eq!(
            sends.resolve(
                report,
                Target::Within {
                    level: "overworld".to_owned(),
                    centre: Point::Block(0, 64, 0),
                    radius: 8.0,
                    except: Some(String::new()),
                }
            ),
            Err(SendRefused::NoPlayer)
        );
    }
}
