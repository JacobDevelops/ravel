#[test]
fn a_world_path_that_names_no_absolute_directory_is_refused() {
    for named in ["null", "", "world", "./world", "../world"] {
        let refusal =
            super::admissible_world(std::path::Path::new(named), std::path::Path::new("/game"))
                .expect_err("a relative path is not a world directory");
        assert!(
            refusal.contains("not an absolute path"),
            "{named} was refused for the wrong reason: {refusal}"
        );
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    crate::test_support::fresh_directory("world-guard", name)
}

#[test]
fn a_world_path_that_is_absent_or_is_a_file_is_refused() {
    let directory = scratch("absent");
    let absent = directory.join("no-such-world");
    let refusal = super::admissible_world(&absent, &directory)
        .expect_err("a path that is not there is not a world directory");
    assert!(refusal.contains("not an existing directory"), "{refusal}");

    let file = directory.join("world");
    std::fs::write(&file, b"not a directory").expect("the file should be written");
    let refusal =
        super::admissible_world(&file, &directory).expect_err("a file is not a world directory");
    assert!(refusal.contains("not an existing directory"), "{refusal}");
}

#[test]
fn an_existing_absolute_world_directory_is_admitted() {
    let world = scratch("admitted").join("Driven");
    std::fs::create_dir_all(&world).expect("the world should be created");

    super::admissible_world(&world, world.parent().expect("world parent"))
        .expect("an ordinary world directory is writable without an options.txt marker");
}

#[test]
fn the_launch_directory_and_its_ancestors_are_not_world_directories() {
    let root = scratch("contains-game");
    let game = root.join("game");
    std::fs::create_dir_all(&game).expect("launch directory");
    for world in [&root, &game] {
        let refused = super::admissible_world(world, &game).expect_err("contains launch directory");
        assert!(refused.contains("holds the game directory"), "{refused}");
    }
}

#[test]
fn an_unresolvable_launch_directory_never_disables_containment() {
    let root = scratch("missing-game");
    let refused = super::admissible_world(&root, &root.join("missing"))
        .expect_err("missing launch directory fails closed");
    assert!(
        refused.contains("launch directory cannot be resolved"),
        "{refused}"
    );
}

#[cfg(unix)]
#[test]
fn a_world_symlink_cannot_hide_the_launch_directory() {
    let root = scratch("symlink-game");
    let game = root.join("game");
    let world = root.join("world");
    std::fs::create_dir_all(&game).expect("launch directory");
    std::os::unix::fs::symlink(&root, &world).expect("world symlink");
    let refused = super::admissible_world(&world, &game).expect_err("hidden launch directory");
    assert!(refused.contains("holds the game directory"), "{refused}");
}

#[cfg(unix)]
#[test]
fn a_symlinked_pack_directory_cannot_redirect_the_write() {
    let root = scratch("symlink-datapacks");
    let game = root.join("game");
    let world = root.join("world");
    let outside = root.join("outside");
    for directory in [&game, &world, &outside] {
        std::fs::create_dir_all(directory).expect("scratch directory");
    }
    std::os::unix::fs::symlink(&outside, world.join(crate::packs::WORLD_DATA_PACKS))
        .expect("datapacks symlink");
    let refused = super::admissible_world(&world, &game).expect_err("escaping pack target");
    assert!(refused.contains("is a symbolic link"), "{refused}");
    assert!(
        refused.contains(crate::packs::WORLD_DATA_PACKS),
        "{refused}"
    );
}

#[cfg(unix)]
#[test]
fn every_pack_root_and_pending_tree_rejects_symlink_redirection() {
    let root = scratch("symlink-pack-roots");
    let game = root.join("game");
    let world = root.join("world");
    let outside = root.join("outside");
    for directory in [&game, &world, &outside] {
        std::fs::create_dir_all(directory).expect("scratch directory");
    }
    for pack in [
        crate::packs::installed_pack_root(&world),
        crate::packs::staged_pack_root(&world),
    ] {
        for target in crate::packs::replaced_paths(&pack) {
            std::fs::create_dir_all(target.parent().expect("pack parent")).expect("pack holder");
            std::os::unix::fs::symlink(&outside, &target).expect("redirected target");
            let refusal = super::admissible_world(&world, &game).expect_err("redirected pack");
            assert!(refusal.contains("is a symbolic link"), "{refusal}");
            std::fs::remove_file(target).expect("remove symlink");
        }
    }
}

use super::PackOutcome;

#[cfg(unix)]
#[test]
fn world_pack_refusals_leave_files_untouched_and_publish_notices() {
    let root = scratch("pack-refusal-notices");
    let game = root.join("game");
    let outside = root.join("outside");
    let redirected_world = game.join("redirected-world");
    let blocked_world = game.join("blocked-world");
    for directory in [&game, &outside, &redirected_world, &blocked_world] {
        std::fs::create_dir_all(directory).expect("scratch directory");
    }
    std::os::unix::fs::symlink(
        &outside,
        redirected_world.join(crate::packs::WORLD_DATA_PACKS),
    )
    .expect("redirected datapacks");
    let blocked_pack = blocked_world.join(crate::packs::WORLD_DATA_PACKS);
    std::fs::create_dir_all(&blocked_pack).expect("datapacks directory");
    std::fs::write(
        blocked_pack.join(crate::packs::GAME_PACK_DIRECTORY),
        b"do not replace",
    )
    .expect("blocking file");
    let runtime = pack_runtime(game);
    for world in [&redirected_world, &blocked_world] {
        assert_eq!(
            runtime.put_pack(world, super::installed_pack_root),
            PackOutcome::Refused
        );
    }
    assert_eq!(
        std::fs::read_dir(&outside)
            .expect("outside directory")
            .count(),
        0
    );
    assert_eq!(
        std::fs::read(blocked_pack.join(crate::packs::GAME_PACK_DIRECTORY)).expect("blocking file"),
        b"do not replace",
    );
    let notices = notices(&runtime);
    assert!(
        notices.iter().any(|notice| {
            notice.starts_with("no data pack was written into")
                && notice.contains("redirected-world")
        }),
        "{notices:?}"
    );
    assert!(
        notices.iter().any(|notice| {
            notice.contains("could not be given the mods' data pack")
                && notice.contains("blocked-world")
        }),
        "{notices:?}"
    );
}

#[cfg(unix)]
fn pack_runtime(game: std::path::PathBuf) -> crate::runtime::LoaderRuntime {
    let machine = ravel_loader::LifecycleMachine::new(ravel_core::Side::Client, Vec::new())
        .expect("empty host set");
    let mut launch = crate::test_support::test_launch();
    launch.game_directory = game;
    crate::runtime::LoaderRuntime::with_observers(
        machine,
        Vec::new(),
        ravel_observability::Profiler::off(),
        ravel_observability::PreWorldReport::new(ravel_minecraft_adapter::environment()),
        launch,
        std::collections::BTreeMap::new(),
    )
}

#[cfg(unix)]
fn notices(runtime: &crate::runtime::LoaderRuntime) -> Vec<String> {
    runtime
        .inner
        .lock()
        .expect("test lock")
        .report
        .notices()
        .iter()
        .map(|notice| notice.text().to_owned())
        .collect()
}

fn java_int(source: &str, name: &str) -> i32 {
    let (_, rest) = source
        .split_once(&format!("static final int {name} = "))
        .unwrap_or_else(|| panic!("the Java side names `{name}`"));
    rest.split(';')
        .next()
        .expect("a declaration ends in a semicolon")
        .trim()
        .parse()
        .expect("a Java int constant is an integer")
}

fn documented_staged_answers(generator: &str) -> Vec<i32> {
    let (rendered, _) = generator
        .split_once("public static native int ravelWriteStagedDatapack")
        .expect("the generator renders the staging crossing");
    let (_, answers) = rendered
        .rsplit_once("@return")
        .expect("the staging crossing documents what it answers");
    answers
        .split("{@code ")
        .skip(1)
        .filter_map(|tail| tail.split('}').next()?.trim().parse().ok())
        .collect()
}

#[test]
fn every_java_spelling_of_a_staged_answer_is_the_number_rust_gives_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |relative: &str| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|absent| panic!("{relative} is a tracked file: {absent}"))
    };
    let adapter = read("jvm/bootstrap/dev/ravel/jvm/Datapack.java");
    let standin = read("jvm/harness/initialization/dev/ravel/jvm/Hook.java");
    let generator = read("crates/ravel-minecraft-generate/src/render/hooks.rs");

    let nothing = i32::from(PackOutcome::Refused);
    assert_eq!(i32::from(PackOutcome::RuntimeUnavailable), nothing);
    let unchanged = i32::from(PackOutcome::Unchanged);
    let written = i32::from(PackOutcome::Written);

    assert_eq!(java_int(&adapter, "UNSTAGED"), nothing);
    assert_eq!(java_int(&adapter, "STAGED_ANEW"), written);
    assert_eq!(java_int(&standin, "UNSTAGED"), nothing);
    assert_eq!(java_int(&standin, "STAGED_UNCHANGED"), unchanged);
    assert_eq!(java_int(&standin, "STAGED_ANEW"), written);
    assert_eq!(
        documented_staged_answers(&generator),
        vec![nothing, unchanged, written]
    );
}
