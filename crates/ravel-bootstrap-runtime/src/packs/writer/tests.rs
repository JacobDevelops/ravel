#[test]
fn replacement_paths_include_the_root_and_sibling_staging_directory() {
    let root = PathBuf::from("resourcepacks").join("custom");
    assert_eq!(
        super::replaced_paths(&root),
        [root, PathBuf::from("resourcepacks").join("custom.pending")]
    );
}

#[test]
fn diagnostic_paths_use_the_callers_pack_name() {
    let root = PathBuf::from("packs/custom-pack");
    assert_eq!(
        super::relative(&root.join("assets/mod/file"), &root),
        PathBuf::from("custom-pack/assets/mod/file")
    );
}

use std::collections::BTreeSet;
use std::path::PathBuf;

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_data::ContentDeclaration;
use ravel_host_api::{Mod, ModError};
use ravel_loader::{
    LifecycleEntry, LifecycleMachine, LoadedMods, Requirement, Supplied, open_resources,
};

use super::{Half, write_game_pack};

const GAME_PACK_DIRECTORY: &str = "custom-pack";

const METADATA: &str = "{\n  \"pack\": {}\n}\n";

fn id(name: &str) -> ModId {
    name.parse().expect("a legal mod id")
}

fn directory(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("ravel-pack-{}-{name}", std::process::id()));
    if at.exists() {
        std::fs::remove_dir_all(&at).expect("a stale temporary directory is removable");
    }
    std::fs::create_dir_all(&at).expect("a writable temporary directory");
    at
}

struct Supplying {
    id: ModId,
    supplies: Vec<(String, String)>,
}

impl Mod for Supplying {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ContentDeclaration>>, ModError> {
        let mut declaration = ContentDeclaration::new(self.id.clone());
        for (path, bytes) in &self.supplies {
            declaration
                .resource(
                    ravel_core::RelativePath::try_from(path.clone()).expect("a legal path"),
                    bytes.clone().into_bytes(),
                )
                .expect("each path is supplied once");
        }
        Ok(Some(std::borrow::Cow::Owned(declaration)))
    }
}

fn supplied(mods: &[(&str, &[(&str, &str)])]) -> Supplied {
    let mut hosts: Vec<Box<dyn Mod>> = mods
        .iter()
        .map(|(name, paths)| -> Box<dyn Mod> {
            Box::new(Supplying {
                id: id(name),
                supplies: paths
                    .iter()
                    .map(|(path, bytes)| ((*path).to_owned(), (*bytes).to_owned()))
                    .collect(),
            })
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
    let mut machine = LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan");
    open_resources(
        &mut LoadedMods::new(&mut machine, &mut hosts).expect("complete host set"),
        &crate::runtime::environment(),
    )
    .expect("every declaration is its own")
}

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

fn tree(root: &std::path::Path) -> Vec<String> {
    fn walk(at: &std::path::Path, root: &std::path::Path, into: &mut Vec<String>) {
        let entries = std::fs::read_dir(at).expect("a readable directory");
        for entry in entries {
            let path = entry.expect("a readable directory entry").path();
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

#[test]
fn the_written_pack_is_the_asset_half_of_the_set_and_nothing_else() {
    let root = directory("assets").join(GAME_PACK_DIRECTORY);
    let set = supplied(&[(
        "first_mod",
        &[
            ("assets/first_mod/lang/en_us.json", "{}"),
            ("assets/first_mod/models/block/a.json", "{\"a\":1}"),
            ("data/first_mod/tags/item/x.json", "{\"values\":[]}"),
        ],
    )]);

    let written = write_game_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

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
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

#[test]
fn a_contested_path_is_written_from_the_mod_that_supplies_it() {
    let root = directory("contested").join(GAME_PACK_DIRECTORY);
    let shared = "assets/shared/lang/en_us.json";
    let set = supplied(&[
        ("base_mod", &[(shared, "base")]),
        ("skin_mod", &[(shared, "skin")]),
    ]);

    let written = write_game_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

    assert_eq!(tree(&root), [shared, "pack.mcmeta"]);
    assert_eq!(written.paths(), tree(&root));
    assert_eq!(
        std::fs::read_to_string(root.join(shared)).expect("the file is readable"),
        "skin",
        "the later mod supplies it"
    );
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

#[test]
fn a_second_write_leaves_nothing_the_set_no_longer_supplies() {
    let root = directory("replace").join(GAME_PACK_DIRECTORY);
    let first = supplied(&[("first_mod", &[("assets/first_mod/lang/en_us.json", "{}")])]);
    write_game_pack(&root, &first, Half::Assets, METADATA).expect("the first pack writes");

    let second = supplied(&[("other_mod", &[("assets/other_mod/lang/en_us.json", "{}")])]);
    write_game_pack(&root, &second, Half::Assets, METADATA).expect("the second pack writes");

    assert_eq!(
        tree(&root),
        ["assets/other_mod/lang/en_us.json", "pack.mcmeta"]
    );
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

#[test]
fn the_data_half_holds_the_data_paths_and_none_of_the_assets() {
    let root = directory("data").join(GAME_PACK_DIRECTORY);
    let set = supplied(&[(
        "first_mod",
        &[
            ("assets/first_mod/lang/en_us.json", "{}"),
            ("data/c/tags/item/lanterns.json", "{\"values\":[]}"),
            ("data/first_mod/recipe/a.json", "{\"type\":\"x\"}"),
        ],
    )]);

    let written = write_game_pack(&root, &set, Half::Data, METADATA).expect("the pack writes");

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
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

#[test]
fn a_set_supplying_no_assets_writes_a_pack_holding_only_its_metadata() {
    let root = directory("empty").join(GAME_PACK_DIRECTORY);
    let set = supplied(&[(
        "first_mod",
        &[("data/first_mod/tags/item/x.json", "{\"values\":[]}")],
    )]);

    write_game_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

    assert_eq!(tree(&root), ["pack.mcmeta"]);
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

// Contents alone would not detect rewriting identical bytes; compare timestamps too.
#[test]
fn a_second_write_of_an_unchanged_set_touches_nothing() {
    let root = directory("unchanged").join(GAME_PACK_DIRECTORY);
    let set = supplied(&[(
        "first_mod",
        &[
            ("assets/first_mod/lang/en_us.json", "{}"),
            ("assets/first_mod/models/block/a.json", "{\"a\":1}"),
        ],
    )]);
    let first = write_game_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");
    let before = timestamps(&root);
    assert_eq!(before.len(), 3);

    let again = write_game_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

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
        !super::staging(&root).exists(),
        "no staging directory is left behind"
    );

    let changed = supplied(&[(
        "first_mod",
        &[
            ("assets/first_mod/lang/en_us.json", "{}"),
            ("assets/first_mod/models/block/a.json", "{\"a\":2}"),
        ],
    )]);
    let replaced =
        write_game_pack(&root, &changed, Half::Assets, METADATA).expect("the pack writes");
    assert!(replaced.changed(), "a set that differs replaces the tree");
    assert_ne!(timestamps(&root), before);
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

#[test]
fn a_leftover_staging_directory_is_removed_before_the_pack_is_built() {
    let root = directory("leftover").join(GAME_PACK_DIRECTORY);
    let staged = super::staging(&root);
    std::fs::create_dir_all(staged.join("assets/ghost")).expect("a writable directory");
    std::fs::write(staged.join("assets/ghost/old.json"), b"{}").expect("a writable file");
    let set = supplied(&[("first_mod", &[("assets/first_mod/lang/en_us.json", "{}")])]);

    write_game_pack(&root, &set, Half::Assets, METADATA).expect("the pack writes");

    assert_eq!(
        tree(&root),
        ["assets/first_mod/lang/en_us.json", "pack.mcmeta"]
    );
    assert!(!staged.exists(), "the staging directory is renamed away");
    std::fs::remove_dir_all(root.parent().expect("the pack sits in a directory"))
        .expect("remove the temporary pack");
}

#[test]
fn a_pack_that_cannot_be_written_is_refused_and_names_the_path() {
    let at = directory("refused");
    let blocked = at.join("blocked");
    std::fs::write(&blocked, b"not a directory").expect("a writable file");
    let set = supplied(&[("first_mod", &[("assets/first_mod/lang/en_us.json", "{}")])]);

    let refused = write_game_pack(
        &blocked.join(GAME_PACK_DIRECTORY),
        &set,
        Half::Assets,
        METADATA,
    )
    .expect_err("a file in the way refuses the pack");

    assert!(
        refused.to_string().contains("could not be written"),
        "{refused}"
    );
    std::fs::remove_dir_all(at).expect("remove the temporary pack");
}
