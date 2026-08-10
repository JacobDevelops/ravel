//! Which side this process is, asked of the JVM rather than assumed.
//!
//! Until this module existed the composed host wrote `Side::Client` into every
//! launch, so the `sides` list a mod ships could not discriminate in production:
//! a dedicated server would have believed it was a client and started every
//! client-only mod on it. The rule was real and its input was a constant.
//!
//! # What premain actually offers, measured rather than assumed
//!
//! [`game_directory`](crate::game_directory) records what the pinned Prism launch
//! carries, and it rules out the two obvious answers for this question as well:
//! the command line reaches the process with no program arguments at all, and the
//! only property the game sets at startup is `java.awt.headless`. Neither names a
//! side. Ravel is nevertheless *inside* the JVM by the time `ravelInitialize`
//! runs, so there is a third thing to ask that costs no hook, no generated
//! signature and no pinned file: **what can this JVM load?**
//!
//! Mojang ships two distributions from one source tree. The client distribution
//! carries `net/minecraft/client/**`; the dedicated server distribution does not.
//! So the presence of the client's own entry point is a fact about which
//! distribution this process was started from, and it is the fact the side is.
//!
//! # Every answer rests on something being present
//!
//! Absence is a weaker observation than presence — a class can be missing because
//! it is not in this distribution, or because the class loader Ravel is asking
//! cannot see the game at all. So a missing client entry point on its own decides
//! nothing here. It is followed by a second question, [`ANY_SIDE`], which every
//! Minecraft distribution carries, and only its presence turns "no client" into
//! "a server":
//!
//! | [`CLIENT_ONLY`] | [`ANY_SIDE`] | answer |
//! | --- | --- | --- |
//! | present | not asked | [`Side::Client`] |
//! | absent | present | [`Side::DedicatedServer`] |
//! | absent | absent | [`SideError::Undeterminable`] |
//!
//! The third row is the whole point of the second question. A launcher that hides
//! the game's classes from the loader Ravel is called through would otherwise
//! have produced a confident `DedicatedServer` on a client, which is worse than
//! the constant this replaced: nothing would have said so, and a player's
//! client-only mods would simply be missing. Instead the launch fails and names
//! both classes it could not find.
//!
//! # What this cannot answer, and where it can still be wrong
//!
//! - **The integrated server is not a distribution.** [`Side::IntegratedServer`]
//!   is a server running inside a client process, so no question asked of the
//!   class path can produce it and this never answers it. A client launch is
//!   admitted as [`Side::Client`], which is the side the process *is*; a mod that
//!   declares only `integrated-server` is parked there. `docs/guide/limits.md`
//!   states that.
//! - **Only one direction is measured in this repository.** That
//!   `net/minecraft/client/main/Main` and `net/minecraft/SharedConstants` are both
//!   in the pinned client jar is checked against the jar itself by
//!   `ravel-minecraft-adapter`'s generator inputs; that the dedicated server
//!   distribution carries the second and not the first is not, because no server
//!   jar is a pinned input and none may be tracked. A dedicated-server launch has
//!   never been run.
//! - **A bundled server may present neither class at premain.** The published
//!   server jar extracts its real jars from inside itself before the game starts,
//!   so a process that has not reached that point yet can see neither of these
//!   classes. That is the third row: it refuses rather than guessing.
//!
//! The first two rows cost two `FindClass` calls once per launch. Nothing is kept
//! from either — the class reference is dropped before the answer is returned —
//! so ADG-4 has nothing to say here.

use std::fmt;

use jni::Env;
use jni::strings::JNIString;
use ravel_core::Side;

/// The client distribution's own entry point, in internal form.
///
/// This class is what `java` is pointed at on a client launch, so a client
/// process is one that can load it by construction, and the dedicated server
/// distribution ships no `net/minecraft/client/` package to hold it.
const CLIENT_ONLY: &str = "net/minecraft/client/main/Main";

/// A class every Minecraft distribution carries, in internal form.
///
/// It is asked only to tell "this is not the client distribution" apart from
/// "this process cannot see Minecraft at all". It holds the game's own version
/// constants, which is why it is on every side: both distributions are built from
/// one source tree and both need to know what version they are.
const ANY_SIDE: &str = "net/minecraft/SharedConstants";

/// Why this process cannot say which side it is.
#[derive(Debug)]
pub enum SideError {
    /// Neither the client's entry point nor the game itself could be loaded.
    Undeterminable,
    /// The JVM refused a class lookup for a reason other than the class missing.
    Jvm(jni::errors::Error),
}

impl fmt::Display for SideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undeterminable => write!(
                formatter,
                "this process can load neither `{CLIENT_ONLY}` nor `{ANY_SIDE}`, \
                 so it cannot say whether it is a client or a dedicated server"
            ),
            Self::Jvm(error) => {
                write!(formatter, "the JVM refused a class lookup: {error}")
            }
        }
    }
}

impl std::error::Error for SideError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Undeterminable => None,
            Self::Jvm(error) => Some(error),
        }
    }
}

/// The side this process is running as.
///
/// # Errors
///
/// Returns [`SideError::Undeterminable`] when neither class is loadable, and
/// [`SideError::Jvm`] when a lookup fails for any reason other than the class not
/// being there. A launch that cannot name its side installs nothing, because
/// every rule below this point — which mods load, which packets may be sent —
/// is a rule about a side.
pub fn side(env: &mut Env<'_>) -> Result<Side, SideError> {
    decide(|name| loadable(env, name))
}

/// Whether the JVM can load `name`.
///
/// A class that is not there is an answer rather than a failure, which is what
/// separates the two error paths above. `jni` maps that case to
/// [`jni::errors::Error::NoClassDefFound`] and clears the exception it caught, so
/// nothing is left pending on the JVM's thread for the next caller to trip over.
fn loadable(env: &mut Env<'_>, name: &str) -> Result<bool, SideError> {
    match env.find_class(JNIString::from(name)) {
        Ok(_) => Ok(true),
        Err(jni::errors::Error::NoClassDefFound { .. }) => Ok(false),
        Err(refused) => Err(SideError::Jvm(refused)),
    }
}

/// The decision itself, over whatever answers what is loadable.
///
/// Split from the lookup so the rule is testable without a JVM: every row of the
/// table in the module documentation is a test below, including the one that
/// refuses.
fn decide<Loadable>(mut loadable: Loadable) -> Result<Side, SideError>
where
    Loadable: FnMut(&'static str) -> Result<bool, SideError>,
{
    if loadable(CLIENT_ONLY)? {
        return Ok(Side::Client);
    }
    if loadable(ANY_SIDE)? {
        return Ok(Side::DedicatedServer);
    }
    Err(SideError::Undeterminable)
}

#[cfg(test)]
mod tests {
    use ravel_core::Side;

    use super::{ANY_SIDE, CLIENT_ONLY, SideError, decide};

    /// Answers from a list of the classes a JVM would hold, recording what it was
    /// asked so a test can say what the probe actually looked for.
    fn jvm(holding: &[&str]) -> (impl FnMut(&'static str) -> Result<bool, SideError>, Asked) {
        let asked = Asked::default();
        let held: Vec<String> = holding.iter().map(|class| (*class).to_owned()).collect();
        let recorder = std::rc::Rc::clone(&asked.0);
        (
            move |name: &'static str| {
                recorder.borrow_mut().push(name);
                Ok(held.iter().any(|class| class == name))
            },
            asked,
        )
    }

    #[derive(Default)]
    struct Asked(std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>);

    impl Asked {
        fn names(&self) -> Vec<&'static str> {
            self.0.borrow().clone()
        }
    }

    #[test]
    fn a_process_holding_the_clients_entry_point_is_the_client() {
        let (probe, asked) = jvm(&[CLIENT_ONLY, ANY_SIDE]);

        assert_eq!(decide(probe).expect("a determinable side"), Side::Client);
        assert_eq!(
            asked.names(),
            [CLIENT_ONLY],
            "the client answer cost one lookup and did not need the second"
        );
    }

    #[test]
    fn a_process_holding_the_game_without_the_client_is_a_dedicated_server() {
        let (probe, asked) = jvm(&[ANY_SIDE]);

        assert_eq!(
            decide(probe).expect("a determinable side"),
            Side::DedicatedServer
        );
        assert_eq!(asked.names(), [CLIENT_ONLY, ANY_SIDE]);
    }

    /// The missing-input case: absence alone decides nothing.
    #[test]
    fn a_process_that_can_see_no_minecraft_at_all_refuses_to_name_a_side() {
        let (probe, _) = jvm(&[]);

        let refused = decide(probe).expect_err("no side is determinable");
        assert!(matches!(refused, SideError::Undeterminable));
        let said = refused.to_string();
        assert!(said.contains(CLIENT_ONLY), "{said}");
        assert!(said.contains(ANY_SIDE), "{said}");
    }

    /// A lookup that fails for another reason is not read as an absent class.
    #[test]
    fn a_refused_lookup_fails_the_launch_rather_than_deciding_a_side() {
        let refused = decide(|_| Err(SideError::Jvm(jni::errors::Error::NullPtr("under test"))))
            .expect_err("a refused lookup is not an answer");

        assert!(matches!(refused, SideError::Jvm(_)));
    }

    /// Both names are what `FindClass` takes, and each is what it is for.
    ///
    /// A dotted name would make every lookup fail, which the third row above would
    /// then report as a JVM with no Minecraft in it — a launch refused for a
    /// reason that is not true.
    #[test]
    fn the_probed_names_are_internal_form_and_only_one_of_them_is_the_clients() {
        for name in [CLIENT_ONLY, ANY_SIDE] {
            assert!(!name.contains('.'), "{name}");
            assert!(name.starts_with("net/minecraft/"), "{name}");
        }
        assert!(CLIENT_ONLY.starts_with("net/minecraft/client/"));
        assert!(!ANY_SIDE.starts_with("net/minecraft/client/"));
    }
}
