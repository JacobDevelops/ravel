//! Networking channels and the packet kinds that travel on them, from §5.
//!
//! A dependent creates a channel from a base identifier and an integer protocol
//! version, optionally marking the channel optional.
//!
//! **Two derived identifiers are the wire contract.** The channel's effective
//! identifier is the base identifier with a version-derived suffix appended, and
//! that suffixed string is also what the channel is registered under. Each
//! packet kind's payload identifier is the effective channel identifier with a
//! further suffix built from the packet's own identifier. Two peers agree on a
//! packet only if they derive the same string, so §14 makes both spellings
//! compatibility-required and neither is invented here: [`Suffixes::declared`]
//! reads them from build-time inputs and [`Suffixes::placeholder`] carries this
//! package's own, which agree with no original.
//!
//! # The change declared here
//!
//! **The distance-limited send compares against the square.** The original
//! compares a *squared* distance against the caller's unsquared argument, so a
//! caller asking for sixteen blocks reaches players within four. Every existing
//! caller has silently been compensating or getting a smaller radius than it
//! asked for; which is unknown, because no caller was read.
//! [`Target::WithinDistance`] takes a plain distance and is compared against its
//! square.
//!
//! # Host-blocked
//!
//! The process-wide list the original collects channels into, so they can all be
//! registered at one loader event, has no Ravel equivalent and needs none. Each
//! contract here is declared through Ravel's own networking declaration and the
//! list is dropped.

use num_traits::ToPrimitive;
use ravel_sdk::content::{ContentId, ContentIdError};
use ravel_sdk::network::{Direction, PacketContract, PacketId};
use ravel_sdk::{JsonValue, ModId};

/// The version-derived suffix pattern's placeholder.
pub const VERSION_MARK: &str = "{version}";

/// The packet-derived suffix pattern's placeholder.
pub const PACKET_MARK: &str = "{packet}";

/// Why the suffixes could not be built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuffixesUnavailable {
    /// The build supplied nothing.
    NotSupplied,
    /// A pattern is empty.
    Empty,
    /// A pattern does not carry the placeholder it derives from.
    NoPlaceholder,
}

impl std::fmt::Display for SuffixesUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupplied => write!(formatter, "the build supplied no channel suffixes"),
            Self::Empty => write!(formatter, "a channel suffix pattern is empty"),
            Self::NoPlaceholder => {
                write!(formatter, "a channel suffix pattern derives from nothing")
            }
        }
    }
}

impl std::error::Error for SuffixesUnavailable {}

/// The two suffix patterns that decide what two peers agree on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suffixes {
    channel: String,
    payload: String,
}

impl Default for Suffixes {
    fn default() -> Self {
        Self::placeholder()
    }
}

impl Suffixes {
    /// The patterns the tests pin the derivation **shape** with.
    ///
    /// These are this package's own. A build that means to talk to an original
    /// supplies the original's, and a build that does not can only talk to
    /// itself, which is what a placeholder means here.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            channel: format!("-v{VERSION_MARK}"),
            payload: format!("/{PACKET_MARK}"),
        }
    }

    /// Reads the two patterns from build-time inputs.
    ///
    /// # Errors
    ///
    /// Returns [`SuffixesUnavailable::NotSupplied`] when the build supplied
    /// nothing, and what [`Suffixes::parse`] returns otherwise.
    pub fn declared() -> Result<Self, SuffixesUnavailable> {
        let channel = option_env!("RVL_060_COUNTERPART_CHANNEL_SUFFIX")
            .ok_or(SuffixesUnavailable::NotSupplied)?;
        let payload = option_env!("RVL_060_COUNTERPART_PAYLOAD_SUFFIX")
            .ok_or(SuffixesUnavailable::NotSupplied)?;
        Self::parse(channel, payload)
    }

    /// Reads the two patterns.
    ///
    /// # Errors
    ///
    /// Returns [`SuffixesUnavailable::Empty`] for an empty pattern and
    /// [`SuffixesUnavailable::NoPlaceholder`] for one that does not carry the
    /// placeholder it derives from. A suffix that ignores its input would make
    /// every version, or every packet, spell the same identifier.
    pub fn parse(channel: &str, payload: &str) -> Result<Self, SuffixesUnavailable> {
        if channel.is_empty() || payload.is_empty() {
            return Err(SuffixesUnavailable::Empty);
        }
        if !channel.contains(VERSION_MARK) || !payload.contains(PACKET_MARK) {
            return Err(SuffixesUnavailable::NoPlaceholder);
        }
        Ok(Self {
            channel: channel.to_owned(),
            payload: payload.to_owned(),
        })
    }

    /// Returns the suffix a version derives.
    #[must_use]
    pub fn for_version(&self, version: u32) -> String {
        self.channel.replace(VERSION_MARK, &version.to_string())
    }

    /// Returns the suffix a packet's own identifier derives.
    #[must_use]
    pub fn for_packet(&self, packet: &str) -> String {
        self.payload.replace(PACKET_MARK, packet)
    }
}

/// The most packet kinds one channel will declare.
pub const MAX_KINDS: usize = 256;

/// The most bytes a packet body may carry.
///
/// A body arrives from a peer, so its announced length is not the reader's to
/// trust. The upgrade path is a larger constant, or a streaming reader.
pub const MAX_BODY_BYTES: usize = 1 << 20;

/// One packet kind's body, in the three shapes §5 offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Body {
    /// A kind carrying no data at all. Its write produces nothing and its read
    /// constructs a fresh instance, which is symmetric and safe.
    Dataless,
    /// A kind described by a codec, which here is an open document.
    Document(JsonValue),
    /// A kind described by the game's own stream codec, which here is bytes the
    /// dependent writes and reads itself.
    Stream(Vec<u8>),
}

impl Body {
    /// Returns which of the three shapes this is.
    #[must_use]
    pub const fn shape(&self) -> Shape {
        match self {
            Self::Dataless => Shape::Dataless,
            Self::Document(_) => Shape::Document,
            Self::Stream(_) => Shape::Stream,
        }
    }

    /// Returns whether this body is inside [`MAX_BODY_BYTES`].
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        match self {
            Self::Dataless => true,
            Self::Document(value) => value.to_string().len() <= MAX_BODY_BYTES,
            Self::Stream(bytes) => bytes.len() <= MAX_BODY_BYTES,
        }
    }
}

/// The three body shapes, without a body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Shape {
    /// [`Body::Dataless`].
    Dataless,
    /// [`Body::Document`].
    Document,
    /// [`Body::Stream`].
    Stream,
}

/// One packet kind a channel carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Kind {
    name: String,
    direction: Direction,
    shape: Shape,
}

impl Kind {
    /// Declares a kind travelling one way.
    ///
    /// A kind is clientbound or serverbound and never both, and declaring the
    /// direction is what decides which registration the loader gets.
    #[must_use]
    pub fn new(name: &str, direction: Direction, shape: Shape) -> Self {
        Self {
            name: name.to_owned(),
            direction,
            shape,
        }
    }

    /// Returns the kind's own identifier.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns which way this kind travels.
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    /// Returns the body shape this kind carries.
    #[must_use]
    pub const fn shape(&self) -> Shape {
        self.shape
    }

    /// Returns whether this kind's handler takes the sending player.
    ///
    /// A serverbound kind's handler yields an action taking the sender; a
    /// clientbound kind's yields a deferred action and has no sender to take.
    #[must_use]
    pub fn handler_takes_sender(&self) -> bool {
        self.direction == Direction::ToServer
    }
}

/// Why a channel refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelError {
    /// The channel already declares [`MAX_KINDS`] kinds.
    Full(usize),
    /// A kind with this name is already declared.
    Duplicate(String),
    /// The identifier the derivation produced is not a legal content path.
    Identity(String),
    /// A body is larger than [`MAX_BODY_BYTES`].
    BodyTooLarge,
    /// A body's shape is not the one the kind declared.
    WrongShape {
        /// What the kind declared.
        declared: Shape,
        /// What arrived.
        found: Shape,
    },
    /// A packet was sent the way its kind does not travel.
    WrongDirection {
        /// What the kind declared.
        declared: Direction,
    },
    /// No kind is declared under this name.
    UnknownKind(String),
}

impl std::fmt::Display for ChannelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(ceiling) => write!(formatter, "a channel declares {ceiling} kinds already"),
            Self::Duplicate(name) => write!(formatter, "`{name}` is declared twice"),
            Self::Identity(detail) => write!(formatter, "{detail}"),
            Self::BodyTooLarge => write!(formatter, "a packet body is past the ceiling"),
            Self::WrongShape { declared, found } => {
                write!(formatter, "a {declared:?} kind was given a {found:?} body")
            }
            Self::WrongDirection { declared } => {
                write!(formatter, "a {declared} kind was sent the other way")
            }
            Self::UnknownKind(name) => write!(formatter, "`{name}` names no declared kind"),
        }
    }
}

impl std::error::Error for ChannelError {}

/// A channel: a base identifier, a protocol version and its packet kinds.
#[derive(Clone, Debug)]
pub struct Channel {
    owner: ModId,
    base: String,
    version: u32,
    optional: bool,
    suffixes: Suffixes,
    kinds: Vec<Kind>,
}

impl Channel {
    /// Creates a channel.
    #[must_use]
    pub fn new(owner: ModId, base: &str, version: u32, optional: bool, suffixes: Suffixes) -> Self {
        Self {
            owner,
            base: base.to_owned(),
            version,
            optional,
            suffixes,
            kinds: Vec::new(),
        }
    }

    /// Returns the effective identifier, which is also what the channel is
    /// registered under.
    #[must_use]
    pub fn effective_id(&self) -> String {
        format!("{}{}", self.base, self.suffixes.for_version(self.version))
    }

    /// Returns a packet kind's payload identifier.
    #[must_use]
    pub fn payload_id(&self, packet: &str) -> String {
        format!(
            "{}{}",
            self.effective_id(),
            self.suffixes.for_packet(packet)
        )
    }

    /// Returns whether a peer without this channel may still connect.
    #[must_use]
    pub const fn is_optional(&self) -> bool {
        self.optional
    }

    /// Returns the declared protocol version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Declares a packet kind.
    ///
    /// # Errors
    ///
    /// Returns [`ChannelError::Full`] at [`MAX_KINDS`] and
    /// [`ChannelError::Duplicate`] for a name already declared.
    pub fn declare(&mut self, kind: Kind) -> Result<(), ChannelError> {
        if self.kinds.len() >= MAX_KINDS {
            return Err(ChannelError::Full(MAX_KINDS));
        }
        if self.kinds.iter().any(|held| held.name() == kind.name()) {
            return Err(ChannelError::Duplicate(kind.name().to_owned()));
        }
        self.kinds.push(kind);
        Ok(())
    }

    /// Returns every declared kind, in declaration order.
    #[must_use]
    pub fn kinds(&self) -> &[Kind] {
        &self.kinds
    }

    /// Returns the kind declared under a name.
    #[must_use]
    pub fn kind(&self, name: &str) -> Option<&Kind> {
        self.kinds.iter().find(|kind| kind.name() == name)
    }

    /// Returns the Ravel contract for a declared kind.
    ///
    /// The contract's own name is the derived payload identifier itself, not a
    /// repaired spelling of it, so what Ravel routes on and what a peer derives
    /// are one string rather than two that can disagree. A suffix pattern whose
    /// derivation Ravel cannot express is [`ChannelError::Identity`] here, at
    /// packaging time, rather than a silent rename that two different suffix
    /// patterns could collide into.
    ///
    /// # Errors
    ///
    /// Returns [`ChannelError::UnknownKind`] for a name nothing declared and
    /// [`ChannelError::Identity`] when the derived identifier is not a legal
    /// content path, which is a packaging failure rather than a runtime one.
    pub fn contract(&self, name: &str) -> Result<PacketContract, ChannelError> {
        let kind = self
            .kind(name)
            .ok_or_else(|| ChannelError::UnknownKind(name.to_owned()))?;
        let id = ContentId::new(self.owner.clone(), self.payload_id(name))
            .map_err(|error: ContentIdError| ChannelError::Identity(error.to_string()))?;
        Ok(PacketContract::new(
            PacketId::new(id),
            kind.direction(),
            self.version,
        ))
    }

    /// Decides who a send reaches.
    ///
    /// An optional channel refuses, silently, to send to a peer whose connection
    /// does not carry it. A non-optional channel does not check. The check is
    /// channel-level rather than packet-level, which is what the original does
    /// and says.
    ///
    /// # Errors
    ///
    /// Returns [`ChannelError::UnknownKind`] for a name nothing declared,
    /// [`ChannelError::WrongShape`] for a body that is not the shape the kind
    /// declared, [`ChannelError::WrongDirection`] for a clientbound kind sent to
    /// the server or the reverse, and [`ChannelError::BodyTooLarge`] for a body
    /// past [`MAX_BODY_BYTES`].
    pub fn recipients<'a>(
        &self,
        name: &str,
        body: &Body,
        target: &Target,
        peers: &'a [Peer],
    ) -> Result<Vec<&'a Peer>, ChannelError> {
        let kind = self
            .kind(name)
            .ok_or_else(|| ChannelError::UnknownKind(name.to_owned()))?;
        if kind.shape() != body.shape() {
            return Err(ChannelError::WrongShape {
                declared: kind.shape(),
                found: body.shape(),
            });
        }
        if kind.direction() != Direction::ToClient {
            return Err(ChannelError::WrongDirection {
                declared: kind.direction(),
            });
        }
        if !body.is_bounded() {
            return Err(ChannelError::BodyTooLarge);
        }
        Ok(peers
            .iter()
            .filter(|peer| target.reaches(peer))
            .filter(|peer| !self.optional || peer.carries_channel)
            .collect())
    }
}

/// Where a peer is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    /// The east-west coordinate.
    pub x: f64,
    /// The vertical coordinate.
    pub y: f64,
    /// The north-south coordinate.
    pub z: f64,
}

impl Position {
    /// Builds a position.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Returns the squared distance to another position.
    ///
    /// Squared, because that is what a distance comparison should be compared
    /// against and taking a root to compare is arithmetic nobody needs.
    #[must_use]
    pub fn squared_distance(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx.mul_add(dx, dy.mul_add(dy, dz * dz))
    }

    /// Returns the chunk this position sits in.
    #[must_use]
    pub fn chunk(self) -> (i64, i64) {
        // Preserve zero for NaN and saturation at the signed coordinate bounds.
        let chunk = |coordinate: f64| {
            coordinate
                .floor()
                .to_i64()
                .unwrap_or_else(|| {
                    if coordinate.is_nan() {
                        0
                    } else if coordinate.is_sign_negative() {
                        i64::MIN
                    } else {
                        i64::MAX
                    }
                })
                .div_euclid(16)
        };
        (chunk(self.x), chunk(self.z))
    }
}

/// One connected peer.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    /// What the peer is called.
    pub name: String,
    /// Which world the peer is in.
    pub world: String,
    /// Where the peer is.
    pub position: Position,
    /// Whether the peer's connection carries this channel.
    pub carries_channel: bool,
}

/// Who a send is addressed to.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// One peer, by name.
    One(String),
    /// A named collection of peers.
    Many(Vec<String>),
    /// Everyone on the server.
    Everyone,
    /// Everyone in one world.
    InWorld(String),
    /// Everyone tracking the chunk a position sits in.
    TrackingChunk {
        /// The world the position is in.
        world: String,
        /// The position whose chunk is tracked.
        position: Position,
    },
    /// Everyone within a plain distance of a position.
    ///
    /// **Declared change.** The argument is a plain distance and is compared
    /// against its square. The original compares a squared distance against the
    /// unsquared argument, so a caller asking for sixteen blocks reaches players
    /// within four.
    WithinDistance {
        /// The world the position is in.
        world: String,
        /// The position measured from.
        position: Position,
        /// How far, as a plain distance.
        distance: f64,
    },
}

impl Target {
    /// Returns whether a target addresses a peer.
    #[must_use]
    pub fn reaches(&self, peer: &Peer) -> bool {
        match self {
            Self::One(name) => peer.name == *name,
            Self::Many(names) => names.contains(&peer.name),
            Self::Everyone => true,
            Self::InWorld(world) => peer.world == *world,
            Self::TrackingChunk { world, position } => {
                peer.world == *world && peer.position.chunk() == position.chunk()
            }
            Self::WithinDistance {
                world,
                position,
                distance,
            } => {
                peer.world == *world
                    && *distance >= 0.0
                    && peer.position.squared_distance(*position) <= distance * distance
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::network::Direction;
    use ravel_sdk::{ModId, json};

    use super::{
        Body, Channel, ChannelError, Kind, MAX_BODY_BYTES, Peer, Position, Shape, Suffixes,
        SuffixesUnavailable, Target,
    };

    fn owner() -> ModId {
        "example".parse().expect("a legal mod id")
    }

    fn channel(optional: bool) -> Channel {
        let mut channel = Channel::new(owner(), "shared", 3, optional, Suffixes::placeholder());
        channel
            .declare(Kind::new("report", Direction::ToClient, Shape::Dataless))
            .expect("a fresh name");
        channel
            .declare(Kind::new("state", Direction::ToClient, Shape::Document))
            .expect("a fresh name");
        channel
            .declare(Kind::new("edit", Direction::ToServer, Shape::Stream))
            .expect("a fresh name");
        channel
    }

    fn peer(name: &str, world: &str, position: Position, carries: bool) -> Peer {
        Peer {
            name: name.to_owned(),
            world: world.to_owned(),
            position,
            carries_channel: carries,
        }
    }

    #[test]
    fn the_effective_identifier_is_the_base_with_the_version_suffix_appended() {
        let channel = channel(false);
        assert_eq!(channel.effective_id(), "shared-v3");
        assert_eq!(channel.version(), 3);
    }

    #[test]
    fn a_payload_identifier_is_the_effective_channel_identifier_with_a_further_suffix() {
        let channel = channel(false);
        assert_eq!(channel.payload_id("report"), "shared-v3/report");
        assert!(
            channel
                .payload_id("report")
                .starts_with(&channel.effective_id()),
            "the payload derivation builds on the channel one"
        );
    }

    #[test]
    fn two_versions_derive_two_different_channel_identifiers() {
        let first = Channel::new(owner(), "shared", 1, false, Suffixes::placeholder());
        let second = Channel::new(owner(), "shared", 2, false, Suffixes::placeholder());
        assert_ne!(first.effective_id(), second.effective_id());
    }

    #[test]
    fn the_suffix_patterns_are_absent_rather_than_defaulted() {
        assert_eq!(
            Suffixes::declared(),
            Err(SuffixesUnavailable::NotSupplied),
            "no build input is supplied here, and a placeholder is not an answer"
        );
    }

    #[test]
    fn a_suffix_pattern_that_ignores_its_input_is_refused() {
        assert_eq!(
            Suffixes::parse("-fixed", "/{packet}"),
            Err(SuffixesUnavailable::NoPlaceholder)
        );
        assert_eq!(
            Suffixes::parse("-v{version}", "/fixed"),
            Err(SuffixesUnavailable::NoPlaceholder)
        );
        assert_eq!(
            Suffixes::parse("", "/{packet}"),
            Err(SuffixesUnavailable::Empty)
        );
        assert!(Suffixes::parse(".{version}", ":{packet}").is_ok());
    }

    #[test]
    fn a_supplied_pattern_changes_both_derivations() {
        let suffixes =
            Suffixes::parse("@{version}", "#{packet}").expect("both carry a placeholder");
        let channel = Channel::new(owner(), "shared", 7, false, suffixes);
        assert_eq!(channel.effective_id(), "shared@7");
        assert_eq!(channel.payload_id("report"), "shared@7#report");
    }

    #[test]
    fn chunk_coordinates_keep_saturating_float_conversion() {
        assert_eq!(Position::new(f64::NAN, 0.0, -0.0).chunk(), (0, 0));
        assert_eq!(
            Position::new(f64::INFINITY, 0.0, f64::NEG_INFINITY).chunk(),
            (i64::MAX.div_euclid(16), i64::MIN.div_euclid(16))
        );
        assert_eq!(Position::new(-0.5, 0.0, 15.9).chunk(), (-1, 0));
    }

    #[test]
    fn a_kind_travels_one_way_and_only_a_serverbound_handler_takes_the_sender() {
        let channel = channel(false);
        let clientbound = channel.kind("report").expect("a declared kind");
        let serverbound = channel.kind("edit").expect("a declared kind");
        assert_eq!(clientbound.direction(), Direction::ToClient);
        assert_eq!(serverbound.direction(), Direction::ToServer);
        assert!(!clientbound.handler_takes_sender());
        assert!(serverbound.handler_takes_sender());
    }

    #[test]
    fn the_dataless_shape_writes_nothing_and_reads_nothing() {
        assert_eq!(Body::Dataless.shape(), Shape::Dataless);
        assert!(Body::Dataless.is_bounded());
    }

    #[test]
    fn the_three_body_shapes_are_told_apart() {
        assert_eq!(Body::Document(json!({ "a": 1 })).shape(), Shape::Document);
        assert_eq!(Body::Stream(vec![1, 2]).shape(), Shape::Stream);
    }

    #[test]
    fn a_kind_is_declared_once_and_a_second_declaration_is_refused() {
        let mut channel = channel(false);
        assert_eq!(
            channel.declare(Kind::new("report", Direction::ToServer, Shape::Dataless)),
            Err(ChannelError::Duplicate("report".to_owned()))
        );
        assert_eq!(channel.kinds().len(), 3);
    }

    #[test]
    fn the_ravel_contract_carries_the_derived_payload_identifier_and_the_kinds_direction() {
        let channel = channel(false);
        let contract = channel.contract("report").expect("a declared kind");
        assert_eq!(contract.direction(), Direction::ToClient);
        assert_eq!(contract.version(), 3);
        assert_eq!(contract.id().to_string(), "example:shared-v3/report");
        assert_eq!(
            channel.contract("absent"),
            Err(ChannelError::UnknownKind("absent".to_owned()))
        );
    }

    #[test]
    fn a_suffix_ravel_cannot_express_is_refused_rather_than_renamed() {
        let suffixes =
            Suffixes::parse("@{version}", "#{packet}").expect("both carry a placeholder");
        let mut channel = Channel::new(owner(), "shared", 7, false, suffixes);
        channel
            .declare(Kind::new("report", Direction::ToClient, Shape::Dataless))
            .expect("the first kind");
        assert_eq!(channel.payload_id("report"), "shared@7#report");
        let refusal = channel
            .contract("report")
            .expect_err("`@` and `#` are not legal in a content path");
        assert!(
            matches!(refusal, ChannelError::Identity(_)),
            "a repaired spelling would route two different derivations to one name"
        );

        let other = {
            let suffixes =
                Suffixes::parse(":{version}", ":{packet}").expect("both carry a placeholder");
            let mut other = Channel::new(owner(), "shared", 7, false, suffixes);
            other
                .declare(Kind::new("report", Direction::ToClient, Shape::Dataless))
                .expect("the first kind");
            other
        };
        assert_ne!(
            other.payload_id("report"),
            channel.payload_id("report"),
            "two suffix patterns derive two identifiers"
        );
        assert!(other.contract("report").is_err());
    }

    #[test]
    fn an_optional_channel_refuses_a_peer_that_does_not_carry_it_and_says_nothing() {
        let channel = channel(true);
        let peers = [
            peer("with", "world", Position::new(0.0, 0.0, 0.0), true),
            peer("without", "world", Position::new(0.0, 0.0, 0.0), false),
        ];
        let reached = channel
            .recipients("report", &Body::Dataless, &Target::Everyone, &peers)
            .expect("a clientbound dataless kind");
        assert_eq!(reached.len(), 1);
        assert_eq!(reached[0].name, "with");
    }

    #[test]
    fn a_non_optional_channel_does_not_check_and_the_check_is_channel_level() {
        let channel = channel(false);
        let peers = [peer(
            "without",
            "world",
            Position::new(0.0, 0.0, 0.0),
            false,
        )];
        for name in ["report", "state"] {
            let body = if name == "report" {
                Body::Dataless
            } else {
                Body::Document(json!({}))
            };
            let reached = channel
                .recipients(name, &body, &Target::Everyone, &peers)
                .expect("a clientbound kind");
            assert_eq!(
                reached.len(),
                1,
                "the rule is the channel's, not the packet's"
            );
        }
    }

    #[test]
    fn a_distance_send_reaches_what_the_caller_asked_for_rather_than_its_root() {
        let channel = channel(false);
        let peers = [
            peer("near", "world", Position::new(4.0, 0.0, 0.0), true),
            peer("edge", "world", Position::new(16.0, 0.0, 0.0), true),
            peer("far", "world", Position::new(16.1, 0.0, 0.0), true),
        ];
        let reached = channel
            .recipients(
                "report",
                &Body::Dataless,
                &Target::WithinDistance {
                    world: "world".to_owned(),
                    position: Position::new(0.0, 0.0, 0.0),
                    distance: 16.0,
                },
                &peers,
            )
            .expect("a clientbound kind");
        let names: Vec<&str> = reached.iter().map(|peer| peer.name.as_str()).collect();
        assert_eq!(
            names,
            ["near", "edge"],
            "sixteen blocks means sixteen; the original reaches only four"
        );
    }

    #[test]
    fn a_negative_distance_reaches_nobody() {
        let channel = channel(false);
        let peers = [peer("here", "world", Position::new(0.0, 0.0, 0.0), true)];
        let reached = channel
            .recipients(
                "report",
                &Body::Dataless,
                &Target::WithinDistance {
                    world: "world".to_owned(),
                    position: Position::new(0.0, 0.0, 0.0),
                    distance: -1.0,
                },
                &peers,
            )
            .expect("a clientbound kind");
        assert_eq!(reached.len(), 0);
    }

    #[test]
    fn the_six_send_targets_each_address_what_they_name() {
        let channel = channel(false);
        let peers = [
            peer("a", "overworld", Position::new(0.0, 0.0, 0.0), true),
            peer("b", "overworld", Position::new(100.0, 0.0, 0.0), true),
            peer("c", "nether", Position::new(0.0, 0.0, 0.0), true),
        ];
        let reach = |target: Target| {
            channel
                .recipients("report", &Body::Dataless, &target, &peers)
                .expect("a clientbound kind")
                .iter()
                .map(|peer| peer.name.clone())
                .collect::<Vec<String>>()
        };
        assert_eq!(reach(Target::One("b".to_owned())), ["b"]);
        assert_eq!(
            reach(Target::Many(vec!["a".to_owned(), "c".to_owned()])),
            ["a", "c"]
        );
        assert_eq!(reach(Target::Everyone), ["a", "b", "c"]);
        assert_eq!(reach(Target::InWorld("overworld".to_owned())), ["a", "b"]);
        assert_eq!(
            reach(Target::TrackingChunk {
                world: "overworld".to_owned(),
                position: Position::new(3.0, 0.0, 3.0),
            }),
            ["a"]
        );
        assert_eq!(
            reach(Target::WithinDistance {
                world: "overworld".to_owned(),
                position: Position::new(0.0, 0.0, 0.0),
                distance: 50.0,
            }),
            ["a"]
        );
    }

    #[test]
    fn a_chunk_target_uses_the_chunk_rather_than_the_exact_position() {
        let inside = Position::new(0.0, 0.0, 0.0).chunk();
        assert_eq!(Position::new(15.9, 0.0, 15.9).chunk(), inside);
        assert_ne!(Position::new(16.0, 0.0, 0.0).chunk(), inside);
        assert_ne!(Position::new(-0.1, 0.0, 0.0).chunk(), inside);
    }

    #[test]
    fn a_send_is_refused_four_ways() {
        let channel = channel(false);
        let peers = [peer("a", "world", Position::new(0.0, 0.0, 0.0), true)];
        assert_eq!(
            channel.recipients("absent", &Body::Dataless, &Target::Everyone, &peers),
            Err(ChannelError::UnknownKind("absent".to_owned()))
        );
        assert_eq!(
            channel.recipients("report", &Body::Stream(vec![]), &Target::Everyone, &peers),
            Err(ChannelError::WrongShape {
                declared: Shape::Dataless,
                found: Shape::Stream,
            })
        );
        assert_eq!(
            channel.recipients("edit", &Body::Stream(vec![]), &Target::Everyone, &peers),
            Err(ChannelError::WrongDirection {
                declared: Direction::ToServer,
            }),
            "a serverbound kind has no clientbound recipients"
        );
        assert_eq!(
            channel.recipients(
                "state",
                &Body::Stream(vec![0; MAX_BODY_BYTES + 1]),
                &Target::Everyone,
                &peers,
            ),
            Err(ChannelError::WrongShape {
                declared: Shape::Document,
                found: Shape::Stream,
            })
        );
    }

    #[test]
    fn a_body_past_the_ceiling_is_refused_before_it_reaches_a_peer() {
        let mut channel = channel(false);
        channel
            .declare(Kind::new("bulk", Direction::ToClient, Shape::Stream))
            .expect("a fresh name");
        let peers = [peer("a", "world", Position::new(0.0, 0.0, 0.0), true)];
        assert_eq!(
            channel.recipients(
                "bulk",
                &Body::Stream(vec![0; MAX_BODY_BYTES + 1]),
                &Target::Everyone,
                &peers,
            ),
            Err(ChannelError::BodyTooLarge)
        );
        assert!(
            channel
                .recipients("bulk", &Body::Stream(vec![0; 8]), &Target::Everyone, &peers,)
                .is_ok()
        );
    }

    #[test]
    fn a_channel_declaring_more_kinds_than_the_ceiling_is_refused() {
        let mut channel = Channel::new(owner(), "shared", 1, false, Suffixes::placeholder());
        for index in 0..super::MAX_KINDS {
            channel
                .declare(Kind::new(
                    &format!("k{index}"),
                    Direction::ToClient,
                    Shape::Dataless,
                ))
                .expect("a fresh name");
        }
        assert_eq!(
            channel.declare(Kind::new("one_more", Direction::ToClient, Shape::Dataless)),
            Err(ChannelError::Full(super::MAX_KINDS))
        );
    }
}
