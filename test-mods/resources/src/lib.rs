#![forbid(unsafe_code)]

//! Resource and generated-data test mod.

use ravel_core::{LifecycleState, ModId};
use ravel_data::ResourceSet;
use ravel_host_api::{GameEvent, HostError, HostMod};
use ravel_package::RelativePath;

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "resources";

/// The paths this mod's manifest declares.
///
/// One of them is a language file, which is the path modded Minecraft contests
/// most often: several mods shipping `lang/en.json` is ordinary rather than a
/// conflict, and it is the case `ravel-data`'s layering exists to resolve. The
/// other is this mod's own model, which nobody else declares, so a test can tell
/// a contested path from an uncontested one in the same set.
const DECLARED: [&str; 2] = ["resources/lang/en.json", "resources/models/warp.json"];

/// A mod that declares resources and reloads them.
///
/// `ROADMAP.md`'s M3-W2 row asks for a resource test mod. A resource is neither
/// a command nor an event and this mod is where the difference is shown: a
/// command is routed to one owner, an event is broadcast to everybody, and a
/// resource path is declared by everybody who wants it and read from exactly one
/// of them. Nothing is claimed through the registration barrier, because two
/// mods supplying `lang/en.json` is how resource packs work and refusing the
/// second would turn an ordinary override into a load failure.
///
/// What this mod does at runtime is count reloads. `GameEvent::ResourcesReloaded`
/// is the only event it acts on, and acting on it means recording that the set's
/// resources were rebuilt — not reading a file, because reading one would need a
/// package root and `docs/architecture/dependency-rules.md` keeps a mod away from
/// the loader that knows where packages live.
///
/// # Examples
///
/// ```
/// use ravel_test_mod_resources::ResourcesMod;
///
/// let resources = ResourcesMod::new();
/// assert_eq!(resources.declared().len(), 2);
/// assert_eq!(resources.reloads(), 0);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcesMod {
    id: ModId,
    reloads: usize,
}

impl ResourcesMod {
    /// Builds the mod instance the loader drives.
    ///
    /// # Panics
    ///
    /// Panics if [`MOD_ID`] stops being a legal mod id, which would mean this
    /// mod can no longer be packaged under the identity its manifest declares.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: MOD_ID.parse().expect("`MOD_ID` is a legal mod id"),
            reloads: 0,
        }
    }

    /// Returns the paths this mod declares, in manifest order.
    ///
    /// # Panics
    ///
    /// Panics if a constant in [`DECLARED`] stops being a legal relative path,
    /// which would mean the manifest it mirrors can no longer be validated.
    #[must_use]
    pub fn declared(&self) -> Vec<RelativePath> {
        DECLARED
            .into_iter()
            .map(|path| {
                RelativePath::try_from(path.to_owned()).expect("a legal package-relative path")
            })
            .collect()
    }

    /// Returns how many times the set's resources were rebuilt.
    #[must_use]
    pub const fn reloads(&self) -> usize {
        self.reloads
    }

    /// Layers this mod's declarations under `later`, which takes what it shares.
    ///
    /// A mod cannot see the whole set — the resolved order belongs to the loader
    /// — so this is the two-mod case a unit test can build, and the whole-set
    /// case belongs to the process tests. It is here rather than in a test so
    /// the layering a mod author reasons about is the same call the harness
    /// makes.
    #[must_use]
    pub fn layered_under(&self, later: &ModId, paths: &[RelativePath]) -> ResourceSet {
        let own = self.declared();
        ResourceSet::layered([(&self.id, own.iter()), (later, paths.iter())])
    }
}

impl Default for ResourcesMod {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMod for ResourcesMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Accepts every step and does nothing with it.
    ///
    /// # Errors
    ///
    /// Never returns an error. Declaring resources happens in the manifest,
    /// before any of this mod's code runs.
    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    /// Counts a resource reload and ignores every other event.
    ///
    /// # Errors
    ///
    /// Never returns an error. A mod that failed on a reload would fail the set
    /// for a thing the player did, and `SPEC.md` §9.2's fail-closed path is
    /// about a mod graph rather than a reload.
    fn observe(&mut self, event: GameEvent) -> Result<(), HostError> {
        if event == GameEvent::ResourcesReloaded {
            self.reloads += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId};
    use ravel_host_api::{GameEvent, HostMod};
    use ravel_package::RelativePath;

    use super::{DECLARED, MOD_ID, ResourcesMod};

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let resources = ResourcesMod::new();
        assert_eq!(resources.id(), &MOD_ID.parse::<ModId>().expect("legal id"));
        assert_eq!(ResourcesMod::default(), resources);
        assert_eq!(resources.reloads(), 0);
    }

    #[test]
    fn every_declared_path_is_a_legal_package_relative_path() {
        let declared = ResourcesMod::new().declared();
        assert_eq!(declared.len(), DECLARED.len());
        for (path, text) in declared.iter().zip(DECLARED) {
            assert_eq!(path.as_str(), text);
            // A declared path is joined to a package root before anything opens
            // it, so it must not be absolute and must not climb out.
            assert!(!path.as_str().starts_with('/'), "{text}");
            assert!(!path.as_str().contains(".."), "{text}");
        }
    }

    #[test]
    fn a_later_mod_takes_the_shared_path_and_leaves_the_unshared_one() {
        let resources = ResourcesMod::new();
        let later: ModId = "skin_mod".parse().expect("a legal mod id");
        let shared = path("resources/lang/en.json");
        let own = path("resources/models/warp.json");

        let set = resources.layered_under(&later, std::slice::from_ref(&shared));

        assert_eq!(set.supplier(&shared), Some(&later));
        assert_eq!(
            set.supplier(&own),
            Some(&MOD_ID.parse::<ModId>().expect("legal id"))
        );
        assert_eq!(set.overrides().len(), 1);
        assert_eq!(
            set.overrides()[0].shadowed,
            [MOD_ID.parse::<ModId>().expect("legal id")]
        );
    }

    #[test]
    fn a_later_mod_that_shares_nothing_contests_nothing() {
        let resources = ResourcesMod::new();
        let later: ModId = "other_mod".parse().expect("a legal mod id");

        let set = resources.layered_under(&later, &[path("resources/lang/fr.json")]);

        assert!(!set.has_overrides());
        assert_eq!(set.declared().len(), 3);
    }

    #[test]
    fn only_a_resource_reload_is_counted() {
        let mut resources = ResourcesMod::new();
        for event in GameEvent::ALL {
            resources.observe(event).expect("nothing is refused");
        }
        assert_eq!(resources.reloads(), 1);

        for _ in 0..2 {
            resources
                .observe(GameEvent::ResourcesReloaded)
                .expect("nothing is refused");
        }
        assert_eq!(resources.reloads(), 3);
    }

    #[test]
    fn a_reload_never_fails_the_set() {
        // A reload is something a player did. Failing the set for it would put a
        // mod graph's fail-closed path in the way of a keypress.
        let mut resources = ResourcesMod::new();
        for state in [LifecycleState::Loaded, LifecycleState::Running] {
            resources.enter(state).expect("no step is refused");
        }
        for _ in 0..50 {
            resources
                .observe(GameEvent::ResourcesReloaded)
                .expect("no reload is refused");
        }
        assert_eq!(resources.reloads(), 50);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(ResourcesMod::new());
