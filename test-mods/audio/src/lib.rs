#![forbid(unsafe_code)]

//! Audio declaration test mod.
//!
//! The mod for [`ravel_sdk::audio`]. It owns one sound, declares the event that
//! plays it, and asks for it on every side — so the side rule is exercised by a
//! mod rather than only by the crate that implements it. The refusal on a server
//! is the property `ROADMAP.md`'s M6-W2 row wants shown: skipping is what would
//! make a mod work in single player and be silent everywhere else.

use ravel_sdk::audio::{AudioDeclaration, AudioError, PlayRequest};
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
}

#[cfg(test)]
mod tests {
    use ravel_sdk::audio::{AudioError, AudioEvent};
    use ravel_sdk::{Declared, Side};

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
