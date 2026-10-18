use std::collections::BTreeMap;
use std::fmt;

use ravel_core::{ContentId, ContentIdError, ModId, Side};

/// A mod-owned sound entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEntry {
    id: ContentId,
}

impl AudioEntry {
    /// Returns the mod that owns the entry.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        self.id.namespace()
    }

    /// Returns the name the mod declared it under.
    #[must_use]
    pub fn name(&self) -> &str {
        self.id.path()
    }

    /// Returns the validated identity of this entry.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.id
    }
}

/// An event referencing a declared sound entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioEvent {
    id: ContentId,
}

impl AudioEvent {
    /// Returns the mod that owns the event.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        self.id.namespace()
    }

    /// Returns the entry this event plays.
    #[must_use]
    pub fn entry(&self) -> &str {
        self.id.path()
    }

    /// Returns the validated identity of the entry this event plays.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.id
    }
}

/// A mod's sound entries and events, with client-only playback requests.
///
/// Playback returns data only; no host currently sends it to audio hardware.
///
/// # Examples
///
/// ```
/// use ravel_core::{ModId, Side};
/// use ravel_data::{AudioDeclaration, AudioError};
///
/// let owner: ModId = "example_mod".parse().expect("a legal mod id");
/// let mut declared = AudioDeclaration::new(owner);
///
/// let chime = declared.entry("door_opens", b"the supplied bytes")?;
/// let event = declared.event("door_opens")?;
/// assert_eq!(event.entry(), chime.name());
///
/// // A client plays it; a server is told it cannot, rather than being ignored.
/// declared.play(&event, Side::Client)?;
/// assert!(declared.play(&event, Side::DedicatedServer).is_err());
/// # Ok::<(), AudioError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioDeclaration {
    owner: ModId,
    entries: BTreeMap<String, (ContentId, Vec<u8>)>,
    events: BTreeMap<ContentId, AudioEvent>,
}

impl AudioDeclaration {
    /// Starts an empty declaration owned by `owner`.
    #[must_use]
    pub fn new(owner: ModId) -> Self {
        Self {
            owner,
            entries: BTreeMap::new(),
            events: BTreeMap::new(),
        }
    }

    /// Returns the mod this declaration belongs to.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Declares an entry named `name` whose content is `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::Name`] when `name` is not a legal content path, and
    /// [`AudioError::DuplicateEntry`] when this mod already declared it.
    pub fn entry(&mut self, name: &str, bytes: &[u8]) -> Result<AudioEntry, AudioError> {
        let id = ContentId::new(self.owner.clone(), name)
            .map_err(|source| AudioError::Name { source })?;
        if self.entries.contains_key(name) {
            return Err(AudioError::DuplicateEntry {
                name: name.to_owned(),
            });
        }
        self.entries
            .insert(name.to_owned(), (id.clone(), bytes.to_vec()));
        Ok(AudioEntry { id })
    }

    /// Returns the bytes declared for `name`, if this mod declared it.
    #[must_use]
    pub fn bytes(&self, name: &str) -> Option<&[u8]> {
        self.entries.get(name).map(|(_, bytes)| bytes.as_slice())
    }

    /// Returns the entries this mod owns, in name order.
    pub fn entries(&self) -> impl Iterator<Item = &str> + '_ {
        self.entries.keys().map(String::as_str)
    }

    /// Declares an event that plays the entry named `name`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::UnknownEntry`] when this mod declared no such entry.
    pub fn event(&mut self, name: &str) -> Result<AudioEvent, AudioError> {
        let id = self
            .entries
            .get(name)
            .map(|(id, _)| id.clone())
            .ok_or_else(|| AudioError::UnknownEntry {
                name: name.to_owned(),
            })?;
        let event = AudioEvent { id: id.clone() };
        self.events.insert(id, event.clone());
        Ok(event)
    }

    /// Returns the events this mod declared, in name order.
    pub fn events(&self) -> impl Iterator<Item = &AudioEvent> + '_ {
        self.events.values()
    }

    /// Asks for a declared `event` to be played on `side`.
    ///
    /// Playback requires a declared event rather than an unchecked name.
    ///
    /// ```compile_fail,E0308
    /// use ravel_core::Side;
    /// use ravel_data::AudioDeclaration;
    /// let declared = AudioDeclaration::new("example".parse().unwrap());
    /// declared.play("undeclared", Side::Client);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an unknown-event error or [`AudioError::WrongSide`] on server sides.
    pub fn play(&self, event: &AudioEvent, side: Side) -> Result<PlayRequest, AudioError> {
        if !self.events.contains_key(event.id()) {
            return Err(AudioError::UnknownEvent {
                name: event.entry().to_owned(),
            });
        }
        if side.is_server() {
            return Err(AudioError::WrongSide {
                name: event.entry().to_owned(),
                side,
            });
        }
        Ok(PlayRequest {
            id: event.id.clone(),
            side,
        })
    }
}

/// A playback request value for a host to dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayRequest {
    id: ContentId,
    side: Side,
}

impl PlayRequest {
    /// Returns the mod that asked.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        self.id.namespace()
    }

    /// Returns the entry that would be played.
    #[must_use]
    pub fn entry(&self) -> &str {
        self.id.path()
    }

    /// Returns the validated identity to play.
    #[must_use]
    pub const fn id(&self) -> &ContentId {
        &self.id
    }

    /// Returns the side it would be played on.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }
}

/// Why a mod's own audio declaration, or a play call, was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioError {
    /// The declared name is not a legal content path.
    Name {
        /// Why the name was refused.
        source: ContentIdError,
    },
    /// This mod already declared an entry under that name.
    DuplicateEntry {
        /// The name declared twice.
        name: String,
    },
    /// An event was declared for an entry this mod does not own.
    UnknownEntry {
        /// The entry the event named.
        name: String,
    },
    /// A play call named an entry no event plays.
    UnknownEvent {
        /// The entry the call named.
        name: String,
    },
    /// Playing was asked for on a side that cannot play.
    WrongSide {
        /// The entry the call named.
        name: String,
        /// The side it was asked on.
        side: Side,
    },
}

impl AudioError {
    /// Returns the entry an event named and did not find, if that caused the error.
    #[must_use]
    pub fn unknown_entry(&self) -> Option<&str> {
        match self {
            Self::UnknownEntry { name } => Some(name),
            Self::Name { .. }
            | Self::DuplicateEntry { .. }
            | Self::UnknownEvent { .. }
            | Self::WrongSide { .. } => None,
        }
    }

    /// Returns the side a play call was refused on, if it was refused for that.
    #[must_use]
    pub const fn wrong_side(&self) -> Option<Side> {
        match self {
            Self::WrongSide { side, .. } => Some(*side),
            Self::Name { .. }
            | Self::DuplicateEntry { .. }
            | Self::UnknownEntry { .. }
            | Self::UnknownEvent { .. } => None,
        }
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { source } => write!(formatter, "{source}"),
            Self::DuplicateEntry { name } => write!(
                formatter,
                "this mod already declares the audio entry `{name}`, and one name holds one sound"
            ),
            Self::UnknownEntry { name } => write!(
                formatter,
                "no audio entry `{name}` is declared, so an event for it would play nothing"
            ),
            Self::UnknownEvent { name } => write!(
                formatter,
                "no event plays the audio entry `{name}`, so there is nothing to ask for"
            ),
            Self::WrongSide { name, side } => write!(
                formatter,
                "`{}` has no sound to play with, so `{name}` is refused there rather than skipped",
                side.as_str()
            ),
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Name { source } => Some(source),
            Self::DuplicateEntry { .. }
            | Self::UnknownEntry { .. }
            | Self::UnknownEvent { .. }
            | Self::WrongSide { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ModId, Side};

    use super::{AudioDeclaration, AudioError};

    const BYTES: &[u8] = b"the supplied bytes";

    fn owner() -> ModId {
        "sample".parse().expect("a well formed owner id")
    }

    fn declared() -> AudioDeclaration {
        let mut declared = AudioDeclaration::new(owner());
        declared
            .entry("door_opens", BYTES)
            .expect("a first claim on an unclaimed name is taken");
        declared
            .event("door_opens")
            .expect("an event is declared for an entry that exists");
        declared
    }

    #[test]
    fn a_declared_audio_entry_yields_an_event_that_names_it() {
        let mut declared = AudioDeclaration::new(owner());

        let entry = declared
            .entry("door_opens", BYTES)
            .expect("a first claim on an unclaimed name is taken");
        let event = declared
            .event("door_opens")
            .expect("an event is declared for an entry that exists");

        assert_eq!(event.entry(), entry.name());
        assert_eq!(entry.owner(), &owner());
        assert_eq!(event.owner(), &owner());
        assert_eq!(entry.id(), event.id());
        assert_eq!(entry.id().to_string(), "sample:door_opens");
        assert_eq!(declared.bytes("door_opens"), Some(BYTES));
        assert_eq!(declared.entries().collect::<Vec<_>>(), ["door_opens"]);
    }

    #[test]
    fn an_event_for_no_declared_entry_is_refused() {
        let mut declared = AudioDeclaration::new(owner());

        let refusal = declared
            .event("door_opens")
            .expect_err("an event with nothing to play is refused at declaration");

        assert_eq!(refusal.unknown_entry(), Some("door_opens"));
        assert_eq!(declared.events().count(), 0);
    }

    #[test]
    fn one_name_holds_one_sound() {
        let mut declared = AudioDeclaration::new(owner());
        declared.entry("door_opens", BYTES).expect("a first claim");

        let refusal = declared
            .entry("door_opens", b"other bytes")
            .expect_err("a second claim on one name is refused");

        assert!(matches!(refusal, AudioError::DuplicateEntry { .. }));
        assert_eq!(refusal.unknown_entry(), None);
        // The first bytes are still the ones declared.
        assert_eq!(declared.bytes("door_opens"), Some(BYTES));
    }

    #[test]
    fn a_name_that_is_not_a_content_path_is_refused() {
        let mut declared = AudioDeclaration::new(owner());
        let refusal = declared
            .entry("Door Opens", BYTES)
            .expect_err("a capital and a space are not a content path");
        assert!(matches!(refusal, AudioError::Name { .. }));
    }

    #[test]
    fn playing_on_a_server_is_refused() {
        for side in [Side::DedicatedServer, Side::IntegratedServer] {
            let declared = declared();
            let event = declared.events().next().expect("declared event");
            let refusal = declared
                .play(event, side)
                .expect_err("a server has nothing to play with and says so");

            assert_eq!(refusal.wrong_side(), Some(side));
            assert_eq!(refusal.unknown_entry(), None);
            assert!(refusal.to_string().contains("door_opens"));
        }
    }

    #[test]
    fn playing_on_the_client_is_what_the_server_was_refused() {
        let declared = declared();
        let event = declared.events().next().expect("declared event");
        let request = declared
            .play(event, Side::Client)
            .expect("a client plays what the mod declared");

        assert_eq!(request.entry(), "door_opens");
        assert_eq!(request.side(), Side::Client);
        assert_eq!(request.owner(), &owner());
        assert_eq!(request.id(), event.id());
    }

    #[test]
    fn declaring_the_entry_on_a_server_is_not_refused() {
        let mut declared = AudioDeclaration::new(owner());

        declared
            .entry("door_opens", BYTES)
            .expect("a server may own the bytes it will never play");
    }

    #[test]
    fn an_event_from_another_owner_is_refused_before_the_side_is_read() {
        let mut declared = AudioDeclaration::new(owner());
        declared.entry("door_opens", BYTES).expect("a first claim");
        let mut other = AudioDeclaration::new("other".parse().expect("valid owner"));
        other.entry("door_opens", BYTES).expect("other's entry");
        let event = other.event("door_opens").expect("other's event");

        let refusal = declared
            .play(&event, Side::DedicatedServer)
            .expect_err("an entry no event plays is nothing to ask for");

        assert!(matches!(refusal, AudioError::UnknownEvent { .. }));
        assert_eq!(refusal.wrong_side(), None);
        assert_eq!(
            refusal.to_string(),
            "no event plays the audio entry `door_opens`, so there is nothing to ask for"
        );
    }

    #[test]
    fn an_event_not_declared_in_this_set_is_refused() {
        let source = declared();
        let event = source.events().next().expect("source event");
        let mut target = AudioDeclaration::new(owner());
        target.entry("door_opens", BYTES).expect("target entry");
        assert!(matches!(
            target.play(event, Side::Client),
            Err(AudioError::UnknownEvent { .. })
        ));
    }
}
