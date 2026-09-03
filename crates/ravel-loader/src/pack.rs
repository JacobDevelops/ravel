use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::resources::Supplied;

/// Where a client scans for packs a player can enable.
///
/// The game's own folder source reads this directory, so a pack written here is
/// one the player can see and turn on. Nothing else about it is Ravel's to
/// decide: the name of the directory is the game's.
pub const RESOURCE_PACKS: &str = "resourcepacks";

/// Where a world's automatically enabled packs live.
///
/// A pack here is enabled by the world rather than by the player, which is why
/// the data half goes to a world and the asset half goes to the instance.
pub const WORLD_DATA_PACKS: &str = "datapacks";

/// The one directory Ravel writes in either place.
pub const PACK_DIRECTORY: &str = "ravel";

/// The id the game gives the one directory Ravel writes.
///
/// The prefix is the game's own rule for a pack a folder source listed, and
/// everything after it is [`PACK_DIRECTORY`]. Every side that names the pack — a
/// player's enabled list, a world's selection, the create-world screen's — names
/// it by this.
///
/// Borrowed rather than composed, because the value never changes and every
/// caller only reads it: composing it handed a fresh `String` to each settings
/// and record scan that asked. A test holds the spelling to [`PACK_DIRECTORY`],
/// so the directory's name still has one owner.
#[must_use]
pub fn pack_id() -> &'static str {
    concat!("file/", "ravel")
}

/// The file every pack declares its format in.
const PACK_METADATA: &str = "pack.mcmeta";

/// The half of a supplied set a pack carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Half {
    /// Everything under `assets/`, which a client reads as a resource pack.
    Assets,
    /// Everything under `data/`, which a world reads as a data pack.
    Data,
}

impl Half {
    /// The path prefix this half owns.
    const fn prefix(self) -> &'static str {
        match self {
            Self::Assets => "assets/",
            Self::Data => "data/",
        }
    }

    /// The sentence the written pack describes itself with.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Assets => "Assets supplied by the Ravel mods this instance runs.",
            Self::Data => "Data supplied by the Ravel mods this world was opened with.",
        }
    }
}

/// Writes one half of `supplied` as a pack under `root`, replacing what is there.
///
/// `root` is the directory the pack itself occupies, so a caller decides whether
/// it lands beside a player's other packs or inside a world. `metadata` is the
/// `pack.mcmeta` this build's version reads, which the caller takes from the
/// generated bindings; nothing here names a format or a version.
///
/// # A pack that has not changed is not written
///
/// The tree on disk is compared with the one this would write, and an identical
/// one is left completely alone — not rewritten with the same bytes. A launch
/// writes the asset half every time and a world writes its data half on every
/// open, so most of those writes have nothing to say, and rewriting a file the
/// game may be reading in order to put back what was already there is a risk
/// taken for no result. It also means an unchanged pack keeps its timestamps,
/// which is the only evidence a player has of when Ravel last changed it.
///
/// # The replacement is a rename
///
/// When the tree does differ, the new one is built beside the old under a
/// `.pending` name and renamed over it. Deleting a pack and refilling it in place
/// leaves the game reading a half-written tree if anything goes wrong in the
/// middle, and the world half is written while the world's own pack source is
/// already built. Staging is what `options.rs` does to a player's settings, for
/// the same reason: neither file is Ravel's to leave truncated.
///
/// A leftover staging directory from an interrupted run is removed before every
/// write rather than left to be found. The window it can exist in is between the
/// staged tree being complete and the rename, which is two syscalls wide and is
/// the price of not editing a live tree in place.
///
/// # Errors
///
/// Returns a [`PackError`] for any filesystem refusal. Every one of them fails the
/// launch: a pack half written is a pack the game reads, and a game that serves
/// one mod's model and not another's is worse than a game that does not start.
pub fn write_pack(
    root: &Path,
    supplied: &Supplied,
    half: Half,
    metadata: &str,
) -> Result<Written, PackError> {
    // A set holds one entry per mod that offered a path, so a path several mods
    // supply appears several times and is written once. The seen set is what
    // keeps that a lookup: scanning the files already chosen, once per offer, is
    // quadratic in the size of the pack, and a large modpack is where that stops
    // being free.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut wanted: Vec<(String, Vec<u8>)> =
        vec![(PACK_METADATA.to_owned(), metadata.as_bytes().to_vec())];
    seen.insert(PACK_METADATA);
    for resource in supplied.set().declared() {
        let path = &resource.path;
        if !path.as_str().starts_with(half.prefix()) || !seen.insert(path.as_str()) {
            continue;
        }
        // The supplier's bytes rather than this resource's own. `declared` lists
        // every mod that offered a path, and a contested path is written once
        // from the mod that won it; asking the set means the file on disk is the
        // one the override record names.
        let Some(bytes) = supplied.bytes(path) else {
            continue;
        };
        wanted.push((path.as_str().to_owned(), bytes.to_vec()));
    }
    wanted.sort_by(|(left, _), (right, _)| left.cmp(right));

    let written = Written {
        root: root.to_path_buf(),
        paths: wanted.iter().map(|(path, _)| path.clone()).collect(),
        changed: !holds(root, &wanted),
    };
    if !written.changed {
        return Ok(written);
    }

    let staged = staging(root);
    if staged.exists() {
        remove(&staged)?;
    }
    create(&staged, &staged)?;
    for (path, bytes) in &wanted {
        let file = staged.join(path);
        if let Some(parent) = file.parent() {
            create(parent, &staged)?;
        }
        write(&file, &staged, bytes)?;
    }

    if root.exists() {
        remove(root)?;
    }
    std::fs::rename(&staged, root).map_err(|source| PackError {
        path: root.to_path_buf(),
        action: Action::Replace,
        source,
    })?;
    Ok(written)
}

/// Where a pack is built before it takes the place of the one on disk.
///
/// A sibling rather than a child, because a directory inside the pack would be
/// part of the pack it is replacing.
fn staging(root: &Path) -> PathBuf {
    let mut name = root.file_name().unwrap_or_default().to_os_string();
    name.push(".pending");
    root.with_file_name(name)
}

/// Whether the tree at `root` is already exactly `wanted`.
///
/// Every path and every byte, in both directions: a pack holding one extra file
/// is not the pack this would write, and neither is one whose model has changed.
/// An unreadable tree answers false, which writes rather than guesses.
fn holds(root: &Path, wanted: &[(String, Vec<u8>)]) -> bool {
    let mut found = Vec::new();
    if !collect(root, root, &mut found) {
        return false;
    }
    found.sort();
    if found.len() != wanted.len() {
        return false;
    }
    for ((path, bytes), on_disk) in wanted.iter().zip(&found) {
        if path != on_disk {
            return false;
        }
        match std::fs::read(root.join(on_disk)) {
            Ok(read) if &read == bytes => {}
            _ => return false,
        }
    }
    true
}

/// Every file below `at`, relative to `root`, or false if it cannot be walked.
fn collect(at: &Path, root: &Path, into: &mut Vec<String>) -> bool {
    let Ok(entries) = std::fs::read_dir(at) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let path = entry.path();
        if path.is_dir() {
            if !collect(&path, root, into) {
                return false;
            }
        } else {
            let Ok(relative) = path.strip_prefix(root) else {
                return false;
            };
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    true
}

fn remove(path: &Path) -> Result<(), PackError> {
    std::fs::remove_dir_all(path).map_err(|source| PackError {
        path: path.to_path_buf(),
        action: Action::Replace,
        source,
    })
}

fn create(path: &Path, root: &Path) -> Result<(), PackError> {
    std::fs::create_dir_all(path).map_err(|source| PackError {
        path: relative(path, root),
        action: Action::Create,
        source,
    })
}

fn write(path: &Path, root: &Path, bytes: &[u8]) -> Result<(), PackError> {
    std::fs::write(path, bytes).map_err(|source| PackError {
        path: relative(path, root),
        action: Action::Write,
        source,
    })
}

/// The path a message names, shortened to the pack when it sits inside one.
fn relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).map_or_else(
        |_| path.to_path_buf(),
        |inside| Path::new(PACK_DIRECTORY).join(inside),
    )
}

/// What one written pack holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Written {
    root: PathBuf,
    paths: Vec<String>,
    changed: bool,
}

impl Written {
    /// Returns the directory the pack occupies.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns every file in the pack, in path order, `pack.mcmeta` included.
    #[must_use]
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Whether the tree on disk is not the tree that was there before.
    ///
    /// A pack written where none was and a pack written over a different one
    /// answer alike, because what reads this asks whether anything holding the
    /// old contents has to read them again, and both say yes.
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

/// What the filesystem refused while a pack was being written.
#[derive(Debug)]
pub struct PackError {
    path: PathBuf,
    action: Action,
    source: io::Error,
}

/// Which step of writing a pack was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    /// The previous pack could not be removed.
    Replace,
    /// A directory inside the pack could not be made.
    Create,
    /// A file could not be written.
    Write,
}

impl fmt::Display for PackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = match self.action {
            Action::Replace => "the previous pack at",
            Action::Create => "a directory at",
            Action::Write => "a file at",
        };
        write!(
            formatter,
            "{verb} {} could not be written: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for PackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {

    /// The id names the directory Ravel writes, under the game's own prefix.
    #[test]
    fn the_pack_id_is_the_directory_the_folder_source_would_have_listed() {
        assert_eq!(super::pack_id(), format!("file/{}", super::PACK_DIRECTORY));
    }

    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};
    use crate::resources::{Supplied, open_resources};
    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_data::ContentDeclaration;
    use ravel_host_api::{HostError, HostMod};

    use super::{Half, PACK_DIRECTORY, write_pack};

    /// The metadata a test writes, which is not this crate's to render.
    const METADATA: &str = "{\n  \"pack\": {}\n}\n";

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    fn directory(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("ravel-pack-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("a writable temporary directory");
        at
    }

    /// A mod supplying exactly the paths it is built with.
    struct Supplying {
        id: ModId,
        supplies: Vec<(String, String)>,
    }

    impl HostMod for Supplying {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn content_declaration(&self) -> Option<ContentDeclaration> {
            // Supplied paths only. What a declared entry adds to a pack is
            // `ravel-data`'s to decide and its own tests' to hold; these are
            // about which supplied file lands where.
            let mut declaration = ContentDeclaration::new(self.id.clone());
            for (path, bytes) in &self.supplies {
                declaration
                    .resource(
                        ravel_core::RelativePath::try_from(path.clone()).expect("a legal path"),
                        bytes.clone().into_bytes(),
                    )
                    .expect("each path is supplied once");
            }
            Some(declaration)
        }
    }

    fn supplied(mods: &[(&str, &[(&str, &str)])]) -> Supplied {
        let hosts: Vec<Box<dyn HostMod>> = mods
            .iter()
            .map(|(name, paths)| {
                Box::new(Supplying {
                    id: id(name),
                    supplies: paths
                        .iter()
                        .map(|(path, bytes)| ((*path).to_owned(), (*bytes).to_owned()))
                        .collect(),
                }) as Box<dyn HostMod>
            })
            .collect();
        let entries = mods
            .iter()
            .map(|(name, _)| LifecycleEntry {
                mod_id: id(name),
                sides: SideSet::new([Side::DedicatedServer]).expect("a non-empty side set"),
                requirement: Requirement::Required,
                saved_data_versions: BTreeSet::new(),
            })
            .collect();
        let machine = LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan");
        open_resources(&machine, &hosts).expect("every declaration is its own")
    }

    /// Every file in the pack beside the moment it was last written.
    fn timestamps(root: &std::path::Path) -> Vec<(String, std::time::SystemTime)> {
        tree(root)
            .into_iter()
            .map(|path| {
                let modified = std::fs::metadata(root.join(&path))
                    .expect("a written file has metadata")
                    .modified()
                    .expect("this platform records a modification time");
                (path, modified)
            })
            .collect()
    }

    /// Every file in a directory, relative to it, sorted.
    fn tree(root: &std::path::Path) -> Vec<String> {
        fn walk(at: &std::path::Path, root: &std::path::Path, into: &mut Vec<String>) {
            let entries = std::fs::read_dir(at).expect("a readable directory");
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, into);
                } else {
                    into.push(
                        path.strip_prefix(root)
                            .expect("the walk stays inside the root")
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        let mut found = Vec::new();
        walk(root, root, &mut found);
        found.sort();
        found
    }

    /// The gate property: what lands on disk is the list this test states itself.
    ///
    /// The expected tree is written out here rather than derived from the set, so
    /// a writer that dropped a file or invented one fails rather than agreeing
    /// with itself. The data half is in the set and is deliberately absent from
    /// the assets pack.
    #[test]
    fn the_written_pack_is_the_asset_half_of_the_set_and_nothing_else() {
        let root = directory("assets").join(PACK_DIRECTORY);
        let set = supplied(&[(
            "first_mod",
            &[
                ("assets/first_mod/lang/en_us.json", "{}"),
                ("assets/first_mod/models/block/a.json", "{\"a\":1}"),
                ("data/first_mod/tags/item/x.json", "{\"values\":[]}"),
            ],
        )]);

        let written = write_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

        assert_eq!(
            tree(&root),
            [
                "assets/first_mod/lang/en_us.json",
                "assets/first_mod/models/block/a.json",
                "pack.mcmeta",
            ]
        );
        assert_eq!(written.paths(), tree(&root));
        assert_eq!(
            std::fs::read_to_string(root.join("pack.mcmeta")).expect("the metadata is readable"),
            METADATA
        );
        assert_eq!(
            std::fs::read_to_string(root.join("assets/first_mod/models/block/a.json"))
                .expect("a supplied file is readable"),
            "{\"a\":1}"
        );
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// A contested path is written once, from the mod the set says supplies it.
    #[test]
    fn a_contested_path_is_written_from_the_mod_that_supplies_it() {
        let root = directory("contested").join(PACK_DIRECTORY);
        let shared = "assets/shared/lang/en_us.json";
        let set = supplied(&[
            ("base_mod", &[(shared, "base")]),
            ("skin_mod", &[(shared, "skin")]),
        ]);

        let written = write_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

        assert_eq!(tree(&root), [shared, "pack.mcmeta"]);
        // A path two mods offered is one file and is reported once. The set holds
        // an entry per offer, so a writer that did not collapse them would report
        // the path twice and write it twice.
        assert_eq!(written.paths(), tree(&root));
        assert_eq!(
            std::fs::read_to_string(root.join(shared)).expect("the file is readable"),
            "skin",
            "the later mod supplies it"
        );
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// What the previous launch wrote is gone, not merged with what this one writes.
    #[test]
    fn a_second_write_leaves_nothing_the_set_no_longer_supplies() {
        let root = directory("replace").join(PACK_DIRECTORY);
        let first = supplied(&[("first_mod", &[("assets/first_mod/lang/en_us.json", "{}")])]);
        write_pack(&root, &first, Half::Assets, METADATA).expect("the first pack writes");

        let second = supplied(&[("other_mod", &[("assets/other_mod/lang/en_us.json", "{}")])]);
        write_pack(&root, &second, Half::Assets, METADATA).expect("the second pack writes");

        assert_eq!(
            tree(&root),
            ["assets/other_mod/lang/en_us.json", "pack.mcmeta"]
        );
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// The other half, and the halves do not mix.
    ///
    /// The list is written out here for the reason the asset one is: a writer that
    /// took the whole set would put a mod's models in a world's data pack, and
    /// every model in that set would still be a file the set supplies.
    #[test]
    fn the_data_half_holds_the_data_paths_and_none_of_the_assets() {
        let root = directory("data").join(PACK_DIRECTORY);
        let set = supplied(&[(
            "first_mod",
            &[
                ("assets/first_mod/lang/en_us.json", "{}"),
                ("data/c/tags/item/lanterns.json", "{\"values\":[]}"),
                ("data/first_mod/recipe/a.json", "{\"type\":\"x\"}"),
            ],
        )]);

        let written = write_pack(&root, &set, Half::Data, METADATA).expect("the pack writes");

        assert_eq!(
            tree(&root),
            [
                "data/c/tags/item/lanterns.json",
                "data/first_mod/recipe/a.json",
                "pack.mcmeta",
            ]
        );
        assert_eq!(written.paths(), tree(&root));
        assert!(!root.join("assets").exists());
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// A set that supplies no assets still leaves a pack, so its id stays valid.
    #[test]
    fn a_set_supplying_no_assets_writes_a_pack_holding_only_its_metadata() {
        let root = directory("empty").join(PACK_DIRECTORY);
        let set = supplied(&[("first_mod", &[("data/first_mod/tags/item/x.json", "{}")])]);

        write_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

        assert_eq!(tree(&root), ["pack.mcmeta"]);
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// An unchanged set is not rewritten, which is what the timestamps show.
    ///
    /// The mtimes are the assertion rather than the bytes, because a rewrite that
    /// put the same bytes back would satisfy any comparison of contents. What is
    /// under test is that nothing was touched.
    #[test]
    fn a_second_write_of_an_unchanged_set_touches_nothing() {
        let root = directory("unchanged").join(PACK_DIRECTORY);
        let set = supplied(&[(
            "first_mod",
            &[
                ("assets/first_mod/lang/en_us.json", "{}"),
                ("assets/first_mod/models/block/a.json", "{\"a\":1}"),
            ],
        )]);
        let first = write_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");
        let before = timestamps(&root);
        assert_eq!(before.len(), 3);

        let again = write_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

        assert_eq!(
            again.paths(),
            first.paths(),
            "the same pack is reported either way"
        );
        assert!(
            first.changed(),
            "the first write is what put the pack there"
        );
        assert!(
            !again.changed(),
            "the second write found the pack it would have written"
        );
        assert_eq!(timestamps(&root), before, "nothing on disk was touched");
        assert!(
            !root.with_file_name("ravel.pending").exists(),
            "no staging directory is left behind"
        );

        // And a set that did change is written, so the skip is a comparison
        // rather than a second write never happening at all.
        let changed = supplied(&[(
            "first_mod",
            &[
                ("assets/first_mod/lang/en_us.json", "{}"),
                ("assets/first_mod/models/block/a.json", "{\"a\":2}"),
            ],
        )]);
        let replaced =
            write_pack(&root, &changed, Half::Assets, METADATA).expect("the pack writes");
        assert!(replaced.changed(), "a set that differs replaces the tree");
        assert_ne!(timestamps(&root), before);
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// A staging directory left by an interrupted run is replaced, not merged.
    #[test]
    fn a_leftover_staging_directory_is_removed_before_the_pack_is_built() {
        let root = directory("leftover").join(PACK_DIRECTORY);
        let staged = root.with_file_name("ravel.pending");
        std::fs::create_dir_all(staged.join("assets/ghost")).expect("a writable directory");
        std::fs::write(staged.join("assets/ghost/old.json"), b"{}").expect("a writable file");
        let set = supplied(&[("first_mod", &[("assets/first_mod/lang/en_us.json", "{}")])]);

        write_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

        assert_eq!(
            tree(&root),
            ["assets/first_mod/lang/en_us.json", "pack.mcmeta"]
        );
        assert!(!staged.exists(), "the staging directory is renamed away");
        let _ = std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"));
    }

    /// A filesystem refusal is an error rather than a partial pack.
    #[test]
    fn a_pack_that_cannot_be_written_is_refused_and_names_the_path() {
        let at = directory("refused");
        // A file where the pack's parent directory belongs. The pack cannot be
        // created under it, and nothing here may pretend it was.
        let blocked = at.join("blocked");
        std::fs::write(&blocked, b"not a directory").expect("a writable file");
        let set = supplied(&[("first_mod", &[("assets/first_mod/lang/en_us.json", "{}")])]);

        let refused = write_pack(&blocked.join(PACK_DIRECTORY), &set, Half::Assets, METADATA)
            .expect_err("a file in the way refuses the pack");

        assert!(
            refused.to_string().contains("could not be written"),
            "{refused}"
        );
        let _ = std::fs::remove_dir_all(at);
    }
}
