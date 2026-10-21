//! Host composition and on-disk resource and world data packs.

use ravel_bootstrap_runtime::write_resource_pack;
use ravel_core::{LifecycleState, Side};
use ravel_example_content_mod::{ContentMod, MODEL, RECIPE};
use ravel_host_runtime::{LaunchError, Member, compose};
use ravel_package::PackageManifest;
use ravel_sdk::Declared;

const BLOCKSTATES: &str = r#"{
  "variants": {
    "": {
      "model": "content:block/copper_lantern"
    }
  }
}
"#;

const ITEM_MODEL: &str = r#"{
  "parent": "content:block/copper_lantern"
}
"#;

const ITEM_DEFINITION: &str = r#"{
  "model": {
    "model": "content:block/copper_lantern",
    "type": "minecraft:model"
  }
}
"#;

const LOOT_TABLE: &str = r#"{
  "pools": [
    {
      "bonus_rolls": 0.0,
      "conditions": [
        {
          "condition": "minecraft:survives_explosion"
        }
      ],
      "entries": [
        {
          "name": "content:copper_lantern",
          "type": "minecraft:item"
        }
      ],
      "rolls": 1.0
    }
  ],
  "type": "minecraft:block"
}
"#;

// Outside the example's declared range, admission refusal is the obligation instead.
fn compose_content(
    game: &std::path::Path,
    side: Side,
) -> Option<ravel_bootstrap_runtime::Composition> {
    let package = include_str!("../../../../../examples/content-mod/package.json");
    let manifest = PackageManifest::parse(package).expect("the shipped manifest is valid");
    let running = ravel_minecraft_adapter::environment().minecraft;
    let compatible = manifest.compatibility.check_minecraft(&running).is_ok();
    let composed = compose(
        game,
        side,
        vec![Member {
            package: package.to_owned(),
            value: Box::new(ContentMod::new()),
        }],
    );
    if compatible {
        Some(composed.expect("the compatible shipped example should compose"))
    } else {
        let expected = ravel_package::CompatibilityError::Minecraft {
            declared: manifest
                .compatibility
                .minecraft
                .iter()
                .map(ToString::to_string)
                .collect(),
            running,
        };
        assert!(
            matches!(composed, Err(LaunchError::Compatibility(actual)) if actual == expected),
            "composition must reject the example with its declared compatibility error"
        );
        assert_eq!(
            std::fs::read_dir(game)
                .expect("the refused game directory is readable")
                .count(),
            0,
            "incompatible content must be refused before composition writes files"
        );
        std::fs::remove_dir_all(game).expect("the refused game directory is removable");
        None
    }
}

// Exercise the same composition path as the host, not only the mod's generator.
#[test]
fn a_composed_host_is_supplied_every_file_the_content_mod_generates() {
    let game = std::env::temp_dir().join(format!("ravel-supplied-content-{}", std::process::id()));
    if game.exists() {
        std::fs::remove_dir_all(&game).expect("a stale game directory is removable");
    }
    std::fs::create_dir_all(&game).expect("a writable temporary directory");

    let Some(ravel_bootstrap_runtime::Composition {
        loaded,
        resources: supplied,
        ..
    }) = compose_content(&game, Side::DedicatedServer)
    else {
        return;
    };
    let machine = loaded.machine();
    assert_eq!(
        machine.state(ContentMod::declared_id()),
        Ok(LifecycleState::Running)
    );

    let model = ContentMod::model_path();
    assert_eq!(
        supplied.set().supplier(&model),
        Some(ContentMod::declared_id())
    );
    assert_eq!(supplied.bytes(&model), Some(MODEL));

    let generated = ContentMod::new().generated().expect("valid declaration");
    assert!(!generated.is_empty(), "the example generates files");
    assert_eq!(
        supplied
            .set()
            .declared()
            .iter()
            .map(|resource| resource.path.as_str())
            .collect::<Vec<_>>(),
        generated.paths().collect::<Vec<_>>()
    );
    for file in generated.files() {
        assert_eq!(
            supplied.set().supplier(file.path()),
            Some(ContentMod::declared_id()),
            "{}",
            file.path()
        );
        assert_eq!(
            supplied.bytes(file.path()),
            Some(file.bytes()),
            "{}",
            file.path()
        );
    }

    std::fs::remove_dir_all(&game).expect("the game directory is removable");
}

#[test]
fn a_launch_writes_the_mods_assets_as_a_pack_the_client_scans() {
    let game = std::env::temp_dir().join(format!("ravel-launch-pack-{}", std::process::id()));
    if game.exists() {
        std::fs::remove_dir_all(&game).expect("a stale game directory is removable");
    }
    std::fs::create_dir_all(&game).expect("a writable temporary directory");

    let Some(ravel_bootstrap_runtime::Composition { resources, .. }) =
        compose_content(&game, Side::Client)
    else {
        return;
    };

    let written = write_resource_pack(&game, &resources).expect("the pack should write");

    assert_eq!(
        written.root(),
        game.join("resourcepacks").join("ravel"),
        "the pack lands where the client's folder source scans"
    );
    assert_eq!(
        written.paths(),
        [
            "assets/content/blockstates/copper_lantern.json",
            "assets/content/items/copper_lantern.json",
            "assets/content/lang/en_us.json",
            "assets/content/lang/fr_fr.json",
            "assets/content/models/block/copper_lantern.json",
            "assets/content/models/item/copper_lantern.json",
            "pack.mcmeta",
        ]
    );
    for (path, expected) in [
        ("blockstates/copper_lantern.json", BLOCKSTATES),
        ("models/item/copper_lantern.json", ITEM_MODEL),
        ("items/copper_lantern.json", ITEM_DEFINITION),
    ] {
        assert_eq!(
            std::fs::read_to_string(written.root().join("assets/content").join(path))
                .unwrap_or_else(|error| panic!("{path} reached no pack: {error}")),
            expected,
            "{path}"
        );
    }

    assert!(
        !written.root().join("data").exists(),
        "the data half belongs to a world, not to the instance"
    );
    assert_eq!(
        std::fs::read(
            written
                .root()
                .join("assets/content/models/block/copper_lantern.json")
        )
        .expect("the model is readable"),
        MODEL,
        "the model the mod supplies is the model on disk"
    );

    assert_resource_pack_metadata(written.root());

    std::fs::remove_dir_all(&game).expect("the game directory is removable");
}

fn assert_resource_pack_metadata(root: &std::path::Path) {
    let metadata =
        std::fs::read_to_string(root.join("pack.mcmeta")).expect("the metadata is readable");
    let parsed: serde_json::Value =
        serde_json::from_str(&metadata).expect("the metadata is legal JSON");
    let declared = parsed.get("pack").expect("the metadata declares a pack");
    assert!(
        declared.get("pack_format").is_some() || declared.get("min_format").is_some(),
        "the pack declares a format this version reads: {metadata}"
    );
    assert!(
        declared
            .get("description")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|text| text.contains("Ravel")),
        "the pack says whose it is: {metadata}"
    );
}

// The game discovers this pack on the next world open, not during this session.
#[test]
fn opening_a_world_leaves_the_mods_data_where_that_world_reads_it() {
    let game = std::env::temp_dir().join(format!("ravel-world-data-{}", std::process::id()));
    if game.exists() {
        std::fs::remove_dir_all(&game).expect("a stale game directory is removable");
    }
    std::fs::create_dir_all(&game).expect("a writable temporary directory");

    let Some(composition) = compose_content(&game, Side::IntegratedServer) else {
        return;
    };

    let world = game.join("saves").join("New World");
    std::fs::create_dir_all(&world).expect("a writable world directory");

    ravel_bootstrap_runtime::install(
        composition,
        ravel_observability::PreWorldReport::new(ravel_minecraft_adapter::environment()),
        game.clone(),
        ravel_observability::Profiler::off(),
    )
    .expect("the composed runtime should install");

    assert_eq!(
        ravel_bootstrap_runtime::write_world_datapack(&world),
        ravel_bootstrap_runtime::PackOutcome::Written,
    );
    assert_eq!(
        ravel_bootstrap_runtime::open_world_data(
            world.to_str().expect("the temporary world path is UTF-8"),
            None,
            true,
        ),
        ravel_bootstrap_runtime::OpenOutcome::Staged,
    );
    let root = world.join("datapacks").join("ravel");
    let paths = [
        "data/c/tags/block/lanterns.json",
        "data/c/tags/item/lanterns.json",
        "data/content/loot_table/blocks/copper_lantern.json",
        "data/content/recipe/crafting/copper_lantern.json",
        "pack.mcmeta",
    ];
    assert_eq!(world_pack_paths(&root), paths);
    assert_eq!(
        std::fs::read_to_string(root.join("data/content/loot_table/blocks/copper_lantern.json"))
            .expect("the loot table is readable"),
        LOOT_TABLE
    );
    assert_eq!(
        std::fs::read(root.join("data/content/recipe/crafting/copper_lantern.json"))
            .expect("the recipe is readable"),
        RECIPE
    );
    assert!(
        !root.join("assets").exists(),
        "a world reads data, and the assets are the instance's"
    );
    for path in paths {
        assert!(root.join(path).is_file(), "{path}");
    }

    std::fs::remove_dir_all(&game).expect("the game directory is removable");
}

fn world_pack_paths(root: &std::path::Path) -> Vec<String> {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::with_capacity(5);
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).expect("the generated directory is readable") {
            let path = entry.expect("the generated entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                paths.push(
                    path.strip_prefix(root)
                        .expect("the generated path is inside the pack")
                        .to_str()
                        .expect("generated paths are UTF-8")
                        .to_owned(),
                );
            }
        }
    }
    paths.sort();
    paths
}
