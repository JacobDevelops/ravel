#![forbid(unsafe_code)]

//! Audio declaration test mod.
//!
//! The mod for [`ravel_sdk::audio`]. It owns one sound, declares the event that
//! plays it, and asks for it on every side — so the side rule is exercised by a
//! mod rather than only by the crate that implements it. The refusal on a server
//! is the property `ROADMAP.md`'s M6-W2 row wants shown: skipping is what would
//! make a mod work in single player and be silent everywhere else.
//!
//! It also asks on the client tick and hands what it asked for to the host
//! through [`ravel_sdk::Mod::take_play_requests`], which is the near end of
//! the outward instruction crossing in `docs/architecture/overview.md`. That makes this
//! mod the producer a host drains rather than a mod that only declares.

use ravel_sdk::audio::{AudioDeclaration, AudioError, PlayRequest};
use ravel_sdk::events::GameEvent;
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, Side, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "audio";

/// The entry this mod owns.
pub const ENTRY: &str = "door_opens";

/// The bytes it supplies for that entry.
///
/// Not a sound. What a sound file holds is a format's decision and nothing in
/// this workspace reads one, so the mod supplies bytes it can name and the
/// declaration carries them unread.
const BYTES: &[u8] = b"the supplied bytes";

/// A mod that owns a sound and plays it on the one side that can.
///
/// # Examples
///
/// ```
/// use ravel_sdk::Side;
/// use ravel_test_mod_audio::AudioMod;
///
/// // The client plays it.
/// let request = AudioMod::play(Side::Client).expect("a client plays it");
/// assert_eq!(request.entry(), "door_opens");
///
/// // Every server side is refused rather than quietly skipped.
/// for side in [Side::IntegratedServer, Side::DedicatedServer] {
///     let refusal = AudioMod::play(side).expect_err("a server has nothing to play with");
///     assert_eq!(refusal.wrong_side(), Some(side));
/// }
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioMod {
    entered: Vec<LifecycleState>,
    asked: Vec<PlayRequest>,
}

impl AudioMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the states this mod was taken through, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }

    /// Returns what this mod has asked for and not yet handed over.
    #[must_use]
    pub fn asked(&self) -> &[PlayRequest] {
        &self.asked
    }

    /// Builds this mod's audio declaration.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError`] if the entry or its event is refused, which would
    /// mean [`ENTRY`] is not a legal name.
    pub fn declaration() -> Result<AudioDeclaration, AudioError> {
        let mut declared = AudioDeclaration::new(Self::declared_id().clone());
        declared.entry(ENTRY, BYTES)?;
        declared.event(ENTRY)?;
        Ok(declared)
    }

    /// Asks for this mod's sound on `side`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::WrongSide`] on every server side.
    pub fn play(side: Side) -> Result<PlayRequest, AudioError> {
        Self::declaration()?.play(ENTRY, side)
    }
}

#[declare_mod(id = "audio")]
impl Mod for AudioMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        if event == GameEvent::ClientTick {
            self.asked.push(
                Self::play(Side::Client).map_err(|refusal| ModError::Refused {
                    mod_id: Self::declared_id().clone(),
                    state: LifecycleState::Running,
                    detail: refusal.to_string(),
                })?,
            );
        }
        Ok(())
    }

    fn take_play_requests(&mut self) -> Vec<PlayRequest> {
        std::mem::take(&mut self.asked)
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::audio::{AudioError, AudioEvent};
    use ravel_sdk::events::GameEvent;
    use ravel_sdk::{Declared, Mod, Side};

    use super::{AudioMod, ENTRY, MOD_ID};

    #[test]
    fn the_declared_identity_is_the_one_this_mod_is_packaged_under() {
        assert_eq!(AudioMod::declared_id().as_str(), MOD_ID);
    }

    #[test]
    fn the_event_this_mod_declares_names_the_entry_it_owns() {
        let declared = AudioMod::declaration().expect("the declaration is well formed");
        let events: Vec<&str> = declared.events().map(AudioEvent::entry).collect();

        assert_eq!(events, [ENTRY]);
        assert_eq!(declared.entries().collect::<Vec<_>>(), [ENTRY]);
    }

    #[test]
    fn an_event_for_a_sound_this_mod_does_not_own_is_refused() {
        let mut declared = AudioMod::declaration().expect("the declaration is well formed");
        let refusal = declared
            .event("door_closes")
            .expect_err("an event with nothing to play is refused");

        assert_eq!(refusal.unknown_entry(), "door_closes");
    }

    /// What the host pulls is what the mod asked for, and only once.
    ///
    /// The second pull is the half worth asserting: a mod that answered its
    /// stored requests without emptying them would replay every sound it has
    /// ever asked for on every tick, and the host cannot tell the difference.
    #[test]
    fn a_tick_asks_once_and_the_host_takes_it_once() {
        let mut audio = AudioMod::new();
        audio
            .observe(GameEvent::ClientTick)
            .expect("a client asks for what it declared");
        assert_eq!(audio.asked().len(), 1);

        let taken = audio.take_play_requests();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].entry(), ENTRY);
        assert_eq!(taken[0].owner(), AudioMod::declared_id());
        assert_eq!(taken[0].side(), Side::Client);
        assert!(audio.take_play_requests().is_empty());
    }

    /// Every event but the client tick asks for nothing.
    #[test]
    fn a_mod_asks_on_the_tick_it_chose_and_no_other_event() {
        let mut audio = AudioMod::new();
        for event in [GameEvent::ServerTick, GameEvent::ResourcesReloaded] {
            audio.observe(event).expect("the mod accepts every event");
        }

        assert!(audio.take_play_requests().is_empty());
    }

    /// Every side, checked from one list rather than one side at a time, so a
    /// side added later is covered by this test without it being edited.
    #[test]
    fn exactly_the_client_plays_and_every_server_side_is_refused() {
        for side in Side::ALL {
            let asked = AudioMod::play(side);
            if side.is_server() {
                let refusal = asked.expect_err("a server has nothing to play with");
                assert_eq!(refusal.wrong_side(), Some(side), "{side:?}");
                assert!(matches!(refusal, AudioError::WrongSide { .. }));
            } else {
                let request = asked.expect("a client plays what the mod declared");
                assert_eq!(request.side(), side);
                assert_eq!(request.entry(), ENTRY);
            }
        }
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(AudioMod::new());
