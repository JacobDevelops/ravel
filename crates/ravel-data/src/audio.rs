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
/// The bootstrap carries playback requests outward to the Java adapter.
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
/// let chime = declared.entry("door_opens")?;
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
    // Every entry shares `owner`, so its path keys it without allocating to look it up.
    entries: BTreeMap<String, ContentId>,
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

    /// Declares an entry named `name`; sound resources are supplied through content declarations.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::Name`] when `name` is not a legal content path, and
    /// [`AudioError::DuplicateEntry`] when this mod already declared it.
    pub fn entry(&mut self, name: &str) -> Result<AudioEntry, AudioError> {
        let id = ContentId::new(self.owner.clone(), name)
            .map_err(|source| AudioError::Name { source })?;
        if self.entries.contains_key(name) {
            return Err(AudioError::DuplicateEntry {
                name: name.to_owned(),
            });
        }
        self.entries.insert(name.to_owned(), id.clone());
        Ok(AudioEntry { id })
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
            .cloned()
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

    /// Returns the declared event that plays the entry named `name`, declaring nothing.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::UnknownEntry`] when this mod declared no such entry, and
    /// [`AudioError::UnknownEvent`] when it declared the entry but no event plays it.
    pub fn declared_event(&self, name: &str) -> Result<&AudioEvent, AudioError> {
        let id = self
            .entries
            .get(name)
            .ok_or_else(|| AudioError::UnknownEntry {
                name: name.to_owned(),
            })?;
        self.events
            .get(id)
            .ok_or_else(|| AudioError::UnknownEvent { id: id.clone() })
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
                id: event.id().clone(),
            });
        }
        if side.is_server() {
            return Err(AudioError::WrongSide {
                id: event.id().clone(),
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
    /// An event lookup named an entry this mod does not own.
    UnknownEntry {
        /// The entry the event named.
        name: String,
    },
    /// No declared event plays this entry, or a play call named an undeclared event.
    UnknownEvent {
        /// The qualified entry the event plays, owner included.
        id: ContentId,
    },
    /// Playing was asked for on a side that cannot play.
    WrongSide {
        /// The qualified entry the event plays, owner included.
        id: ContentId,
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
            Self::UnknownEvent { id } => write!(
                formatter,
                "no event plays the audio entry `{id}`, so there is nothing to ask for"
            ),
            Self::WrongSide { id, side } => write!(
                formatter,
                "`{}` has no sound to play with, so `{id}` is refused there rather than skipped",
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

    fn owner() -> ModId {
        "sample".parse().expect("a well formed owner id")
    }

    fn declared() -> AudioDeclaration {
        let mut declared = AudioDeclaration::new(owner());
        declared
            .entry("door_opens")
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
            .entry("door_opens")
            .expect("a first claim on an unclaimed name is taken");
        let event = declared
            .event("door_opens")
            .expect("an event is declared for an entry that exists");

        assert_eq!(event.entry(), entry.name());
        assert_eq!(entry.owner(), &owner());
        assert_eq!(event.owner(), &owner());
        assert_eq!(entry.id(), event.id());
        assert_eq!(entry.id().to_string(), "sample:door_opens");
        assert_eq!(declared.entries().collect::<Vec<_>>(), ["door_opens"]);
    }

    #[test]
    fn looking_up_an_event_declares_nothing() {
        let mut declared = AudioDeclaration::new(owner());
        assert!(matches!(
            declared.declared_event("door_opens"),
            Err(AudioError::UnknownEntry { .. })
        ));
        declared.entry("door_opens").expect("a first claim");

        let refusal = declared
            .declared_event("door_opens")
            .expect_err("an entry no event plays has no event to find");
        assert_eq!(
            refusal,
            AudioError::UnknownEvent {
                id: "sample:door_opens".parse().expect("a legal content id"),
            }
        );
        assert_eq!(declared.events().count(), 0);

        let event = declared.event("door_opens").expect("the entry exists");
        assert_eq!(declared.declared_event("door_opens"), Ok(&event));
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
        declared.entry("door_opens").expect("a first claim");

        let refusal = declared
            .entry("door_opens")
            .expect_err("a second claim on one name is refused");

        assert!(matches!(refusal, AudioError::DuplicateEntry { .. }));
        assert_eq!(refusal.unknown_entry(), None);
        assert_eq!(declared.entries().collect::<Vec<_>>(), ["door_opens"]);
    }

    #[test]
    fn a_name_that_is_not_a_content_path_is_refused() {
        let mut declared = AudioDeclaration::new(owner());
        let refusal = declared
            .entry("Door Opens")
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
            .entry("door_opens")
            .expect("a server may declare an entry it will never play");
    }

    #[test]
    fn an_event_from_another_owner_is_refused_before_the_side_is_read() {
        let mut declared = AudioDeclaration::new(owner());
        declared.entry("door_opens").expect("a first claim");
        let mut other = AudioDeclaration::new("other".parse().expect("valid owner"));
        other.entry("door_opens").expect("other's entry");
        let event = other.event("door_opens").expect("other's event");

        let refusal = declared
            .play(&event, Side::DedicatedServer)
            .expect_err("an entry no event plays is nothing to ask for");

        assert!(matches!(refusal, AudioError::UnknownEvent { .. }));
        assert_eq!(refusal.wrong_side(), None);
        assert_eq!(
            refusal.to_string(),
            "no event plays the audio entry `other:door_opens`, so there is nothing to ask for"
        );
    }

    #[test]
    fn an_event_not_declared_in_this_set_is_refused() {
        let source = declared();
        let event = source.events().next().expect("source event");
        let mut target = AudioDeclaration::new(owner());
        target.entry("door_opens").expect("target entry");
        assert!(matches!(
            target.play(event, Side::Client),
            Err(AudioError::UnknownEvent { .. })
        ));
    }
}
