use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, RelativePath, StructuredFailure};
use ravel_data::{GeneratedData, GeneratedFile, ResourceSet};
use ravel_host_api::HostMod;

use crate::lifecycle::LifecycleMachine;

/// Layers every admitted mod's files into the one set the game reads from.
///
/// The counterpart of [`crate::open_configuration`], and it exists for the reason
/// that one does: a trait method nothing drives over a set is a contract closed in
/// the type and open in every composition. A mod supplies the bytes of anything
/// Ravel does not generate the shape of, and until something asked every mod for
/// them they stayed inside the mod's own crate — a registered item with a model
/// nothing served.
///
/// The walk is the resolved load order, which is what decides a contested path. A
/// registry entry has one owner and the barrier refuses a second claim; a resource
/// path has one *supplier* and the mod later in load order takes it. That is an
/// override rather than a conflict, so nothing here refuses it — but nothing here
/// hides it either, and [`ResourceSet::overrides`] names every mod a path was
/// taken from.
///
/// A mod this side parks is skipped. Its code does not run here, and serving its
/// model anyway would put a client mod's assets in a dedicated server's pack.
///
/// # Errors
///
/// Returns [`ResourceRefused::ForeignDeclaration`] when a mod hands over a
/// declaration owned by another identity. Every path a declaration generates is
/// derived from its owner's namespace, so accepting one would file a mod's files
/// under a name it does not own and attribute them to the mod that handed them
/// over — two answers to who supplied a path, from one call.
pub fn open_resources(
    machine: &LifecycleMachine,
    mods: &[Box<dyn HostMod>],
) -> Result<Supplied, ResourceRefused> {
    let mut generated: Vec<(ModId, GeneratedData)> = Vec::new();

    for mod_id in machine.order().cloned().collect::<Vec<ModId>>() {
        if !machine
            .entry(&mod_id)
            .is_ok_and(|entry| entry.sides.loads_on(machine.side()))
        {
            continue;
        }
        let Some(host) = mods.iter().find(|host| host.id() == &mod_id) else {
            continue;
        };
        let Some(declaration) = host.content_declaration() else {
            continue;
        };
        if declaration.owner() != &mod_id {
            return Err(ResourceRefused::ForeignDeclaration {
                mod_id,
                owner: declaration.owner().clone(),
            });
        }
        generated.push((mod_id, declaration.generate()));
    }

    let set = ResourceSet::layered(
        generated
            .iter()
            .map(|(owner, files)| (owner, files.files().iter().map(GeneratedFile::path))),
    );

    Ok(Supplied { generated, set })
}

/// Every file the admitted set supplies, layered, with the bytes behind it.
///
/// The bytes are kept rather than dropped after layering for the same reason
/// [`crate::Configured`] keeps its resolved settings: whatever writes the pack
/// needs the winner's content, and generating it a second time from the same
/// declarations would be a second answer to what a mod supplies. There is one
/// generation per launch and this holds its result.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Supplied {
    generated: Vec<(ModId, GeneratedData)>,
    set: ResourceSet,
}

impl Supplied {
    /// Returns which mod supplies each path, and every override it recorded.
    #[must_use]
    pub const fn set(&self) -> &ResourceSet {
        &self.set
    }

    /// Returns each mod's generated pack, in the load order it was asked in.
    #[must_use]
    pub fn generated(&self) -> &[(ModId, GeneratedData)] {
        &self.generated
    }

    /// Returns the bytes the game reads at `path`, from the mod that supplies it.
    ///
    /// The supplier is read off the layered set rather than by searching the
    /// packs, so a path several mods generate answers with the winner's bytes and
    /// not with whichever copy is found first.
    #[must_use]
    pub fn bytes(&self, path: &RelativePath) -> Option<&[u8]> {
        let supplier = self.set.supplier(path)?;
        self.generated
            .iter()
            .find(|(owner, _)| owner == supplier)
            .and_then(|(_, files)| files.file(path))
            .map(GeneratedFile::bytes)
    }
}

/// Why an admitted set's files could not be layered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceRefused {
    /// A mod handed over a declaration owned by another identity.
    ForeignDeclaration {
        /// The mod that handed it over.
        mod_id: ModId,
        /// The identity the declaration names as its owner.
        owner: ModId,
    },
}

impl fmt::Display for ResourceRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::ForeignDeclaration { mod_id, owner } = self;
        write!(
            formatter,
            "`{mod_id}` supplies content declared as `{owner}`'s, so its files would be filed \
             under a namespace it does not own"
        )
    }
}

impl std::error::Error for ResourceRefused {}

impl StructuredFailure for ResourceRefused {
    /// Not one of `SPEC.md` §9.2's dependency-graph outcomes: the graph resolved
    /// and a mod's own declaration is what was refused.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// The registration window is where a mod's content is settled, and a
    /// declaration that names another owner is refused on the same statement.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Registered
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{ContentKind, LifecycleState, ModId, RelativePath, Side, SideSet};
    use ravel_data::ContentDeclaration;
    use ravel_host_api::{HostError, HostMod};

    use super::{ResourceRefused, open_resources};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    /// A mod that supplies the paths it is built with, under the owner it names.
    struct Supplying {
        id: ModId,
        declared_owner: ModId,
        supplies: Vec<(RelativePath, Vec<u8>)>,
    }

    impl Supplying {
        fn new(name: &str, supplies: &[(&str, &str)]) -> Self {
            Self {
                id: id(name),
                declared_owner: id(name),
                supplies: supplies
                    .iter()
                    .map(|(at, bytes)| (path(at), (*bytes).as_bytes().to_vec()))
                    .collect(),
            }
        }

        fn declaring_for(mut self, other: &str) -> Self {
            self.declared_owner = id(other);
            self
        }
    }

    impl HostMod for Supplying {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn content_declaration(&self) -> Option<ContentDeclaration> {
            let mut declaration = ContentDeclaration::new(self.declared_owner.clone());
            declaration
                .entry(ContentKind::Item, "thing")
                .expect("a legal, undeclared entry");
            for (at, bytes) in &self.supplies {
                declaration
                    .resource(at.clone(), bytes.clone())
                    .expect("each path is supplied once");
            }
            Some(declaration)
        }
    }

    /// A mod that declares no content at all, which is most mods.
    struct Silent {
        id: ModId,
    }

    impl HostMod for Silent {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }
    }

    fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan")
    }

    /// The whole point of the walk: bytes a mod supplies reach the set.
    #[test]
    fn every_declaring_mod_supplies_its_files_and_a_silent_one_supplies_none() {
        let mods: Vec<Box<dyn HostMod>> = vec![
            Box::new(Supplying::new("first_mod", &[("assets/first.json", "1")])),
            Box::new(Silent { id: id("mute_mod") }),
        ];
        let machine = machine(vec![
            entry("first_mod", &[Side::DedicatedServer]),
            entry("mute_mod", &[Side::DedicatedServer]),
        ]);

        let supplied = open_resources(&machine, &mods).expect("both mods are legal");

        assert_eq!(
            supplied.set().supplier(&path("assets/first.json")),
            Some(&id("first_mod"))
        );
        assert_eq!(
            supplied.bytes(&path("assets/first.json")),
            Some(b"1".as_ref())
        );
        assert_eq!(
            supplied.generated().len(),
            1,
            "a silent mod generates nothing"
        );
        assert!(supplied.set().overrides().is_empty());
    }

    /// Two mods, one path: the later supplies it and the shadowed one is named.
    ///
    /// The second half is the assertion that makes this a test of *order* rather
    /// than of set membership: the same two mods in the opposite order answer with
    /// the opposite supplier, so a walk that ignored load order would fail one of
    /// the two halves whatever it picked.
    #[test]
    fn the_mod_later_in_load_order_supplies_a_contested_path() {
        let shared = "assets/shared/lang/en_us.json";
        let contested = path(shared);

        for (first, second) in [("base_mod", "skin_mod"), ("skin_mod", "base_mod")] {
            let mods: Vec<Box<dyn HostMod>> = vec![
                Box::new(Supplying::new(first, &[(shared, first)])),
                Box::new(Supplying::new(second, &[(shared, second)])),
            ];
            let machine = machine(vec![
                entry(first, &[Side::DedicatedServer]),
                entry(second, &[Side::DedicatedServer]),
            ]);

            let supplied = open_resources(&machine, &mods).expect("both mods are legal");

            assert_eq!(
                supplied.set().supplier(&contested),
                Some(&id(second)),
                "`{second}` loads last"
            );
            assert_eq!(supplied.bytes(&contested), Some(second.as_bytes()));
            let overrides = supplied.set().overrides();
            assert_eq!(overrides.len(), 1);
            assert_eq!(overrides[0].winner, id(second));
            assert_eq!(
                overrides[0].shadowed,
                [id(first)],
                "the shadowed mod is named"
            );
        }
    }

    /// A mod this side parks supplies nothing, because its code does not run here.
    #[test]
    fn a_parked_mod_supplies_nothing_on_the_side_it_does_not_load_on() {
        let mods: Vec<Box<dyn HostMod>> = vec![Box::new(Supplying::new(
            "client_mod",
            &[("assets/client.json", "c")],
        ))];
        let machine = machine(vec![entry("client_mod", &[Side::Client])]);

        let supplied = open_resources(&machine, &mods).expect("a parked mod refuses nothing");

        assert!(supplied.generated().is_empty());
        assert_eq!(supplied.bytes(&path("assets/client.json")), None);
    }

    /// A declaration owned by somebody else is refused rather than filed.
    #[test]
    fn a_mod_supplying_another_mods_content_is_refused() {
        let mods: Vec<Box<dyn HostMod>> = vec![Box::new(
            Supplying::new("thief_mod", &[("assets/taken.json", "t")]).declaring_for("owner_mod"),
        )];
        let machine = machine(vec![entry("thief_mod", &[Side::DedicatedServer])]);

        assert_eq!(
            open_resources(&machine, &mods),
            Err(ResourceRefused::ForeignDeclaration {
                mod_id: id("thief_mod"),
                owner: id("owner_mod"),
            })
        );
    }
}
