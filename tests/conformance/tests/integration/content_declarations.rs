#![forbid(unsafe_code)]

//! Content declarations, registration conflicts, and deterministic generated packs.

use std::collections::BTreeSet;

use ravel_core::{ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet};
use ravel_data::{ContentDeclaration, DeclarationError, TagId, TagSet};
use ravel_example_content_mod::{ContentMod, MODEL, RECIPE};
use ravel_host_api::{HostError, HostMod, Registrar};
use ravel_host_runtime::{Member, compose, write_resource_pack, write_world_pack};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_sdk::Declared;
use ravel_sdk::generate::{GeneratedData, GeneratedFile};
use ravel_sdk::resources::RelativePath;

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn entry(name: &str) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new([Side::Client, Side::DedicatedServer]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

struct Rival {
    id: ModId,
    claims: Vec<RegistryKey>,
}

impl HostMod for Rival {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ravel_host_api::ModError> {
        for key in &self.claims {
            registrar
                .claim(key.clone())
                .map_err(|error| ravel_host_api::ModError::refused(error.to_string()))?;
        }
        Ok(())
    }
}

type Outcome = (ModId, Result<(), HostError>);

fn register_all(
    entries: Vec<LifecycleEntry>,
    mods: &mut [&mut dyn HostMod],
) -> (LifecycleMachine, Vec<Outcome>) {
    let mut machine = LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("a legal step");
    }

    let order: Vec<ModId> = machine.order().cloned().collect();
    let mut outcomes = Vec::new();
    for mod_id in order {
        let mut window = machine
            .registration_window(&mod_id)
            .expect("the plan names it");
        if let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) {
            outcomes.push((
                mod_id.clone(),
                host.register(&mut window).map_err(|error| {
                    error.with_context(mod_id.clone(), LifecycleState::Constructed)
                }),
            ));
        }
        assert!(window.unexpected().is_none(), "the window was open");
    }
    (machine, outcomes)
}

// Literal floors catch declarations that shrink while derived expectations still pass.
const DECLARED_ENTRIES: &[&str] = &[
    "block:content:copper_lantern",
    "item:content:copper_lantern",
    "recipe:content:crafting/copper_lantern",
];

const GENERATED_PATHS: &[&str] = &[
    "assets/content/blockstates/copper_lantern.json",
    "assets/content/items/copper_lantern.json",
    "assets/content/lang/en_us.json",
    "assets/content/lang/fr_fr.json",
    "assets/content/models/block/copper_lantern.json",
    "assets/content/models/item/copper_lantern.json",
    "data/c/tags/block/lanterns.json",
    "data/c/tags/item/lanterns.json",
    "data/content/loot_table/blocks/copper_lantern.json",
    "data/content/recipe/crafting/copper_lantern.json",
];

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

#[test]
fn the_example_still_declares_the_content_this_suite_is_written_against() {
    let content = ContentMod::new();

    assert_eq!(
        content
            .declaration()
            .expect("valid declaration")
            .entries()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        DECLARED_ENTRIES,
        "the example is this suite's subject and every other test here derives its expectation \
         from what the example declares, so a declaration that shrank would satisfy all of them. \
         If the example legitimately declares something else, decide it here."
    );
    assert_eq!(
        content
            .generated()
            .expect("valid declaration")
            .paths()
            .collect::<Vec<_>>(),
        GENERATED_PATHS,
        "the same floor for the generated side: an entry, a tag, a locale or a supplied resource \
         dropped from the example changes this list and nothing else in this file."
    );
}

#[test]
fn every_entry_the_example_declares_is_owned_by_it_after_the_real_barrier_closes() {
    let mut content = ContentMod::new();
    let declared = content
        .declaration()
        .expect("valid declaration")
        .entries()
        .to_vec();
    assert!(
        !declared.is_empty(),
        "an example that declares nothing would make every check below vacuous"
    );

    let (mut machine, outcomes) = register_all(vec![entry("content")], &mut [&mut content]);
    for (mod_id, outcome) in outcomes {
        outcome.unwrap_or_else(|error| panic!("`{mod_id}` was refused: {error}"));
    }

    for key in &declared {
        assert_eq!(
            machine.owner_of(key),
            Some(ContentMod::declared_id()),
            "`{key}` was declared and claimed, so the barrier owns it for this mod"
        );
    }
    assert_eq!(
        machine.registry_claim_count(ContentMod::declared_id()),
        declared.len()
    );

    machine
        .advance_all(LifecycleState::Registered)
        .expect("a legal step");
    assert!(machine.registration_barrier_closed());
}

#[test]
fn a_declared_entry_the_barrier_never_saw_has_no_owner() {
    let never = RegistryKey::new(
        ContentKind::Block,
        ContentId::new(id("content"), "unclaimed").expect("a legal content path"),
    );
    let mut content = ContentMod::new();
    let (machine, _) = register_all(vec![entry("content")], &mut [&mut content]);

    assert_eq!(machine.owner_of(&never), None);
}

#[test]
fn a_declaration_names_its_content_once_and_refuses_the_second_naming() {
    let mut declaration = ContentDeclaration::new(id("example_mod"));
    let block = declaration
        .entry(ContentKind::Block, "copper")
        .expect("a legal, undeclared entry");
    declaration
        .entry(ContentKind::Item, "copper")
        .expect("an item is a different registry");

    assert_eq!(
        declaration.entry(ContentKind::Block, "copper"),
        Err(DeclarationError::DuplicateEntry { key: block })
    );
    assert_eq!(declaration.entries().len(), 2);
}

#[test]
fn a_declaration_that_names_illegal_content_is_refused_before_any_barrier() {
    let mut declaration = ContentDeclaration::new(id("example_mod"));
    for path in ["Copper", "", "copper/", "coppér"] {
        assert!(
            matches!(
                declaration.entry(ContentKind::Block, path),
                Err(DeclarationError::Path { .. })
            ),
            "{path:?} is not a legal content path"
        );
    }
    assert_eq!(declaration.entries(), []);
}

#[test]
fn a_second_mod_claiming_the_examples_block_is_refused_and_told_who_owns_it() {
    let mut content = ContentMod::new();
    let contested = content
        .declaration()
        .expect("valid declaration")
        .entries()
        .iter()
        .find(|key| key.kind() == ContentKind::Block)
        .expect("the example declares a block")
        .clone();
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![contested.clone()],
    };

    let (machine, outcomes) = register_all(
        vec![entry("content"), entry("rival")],
        &mut [&mut content, &mut rival],
    );

    let refusal = outcomes
        .iter()
        .find(|(mod_id, _)| mod_id == &id("rival"))
        .and_then(|(_, outcome)| outcome.as_ref().err())
        .expect("the rival's claim was refused");
    let detail = refusal.to_string();
    assert!(detail.contains("content"), "{detail}");
    assert!(detail.contains(&contested.to_string()), "{detail}");
    assert_eq!(refusal.state(), LifecycleState::Constructed);

    assert_eq!(
        machine.owner_of(&contested),
        Some(ContentMod::declared_id())
    );
    assert_eq!(machine.registry_claim_count(&id("rival")), 0);
}

#[test]
fn a_foreign_claim_cannot_reserve_the_examples_namespace_before_its_owner() {
    let mut content = ContentMod::new();
    let contested = content
        .declaration()
        .expect("valid declaration")
        .entries()
        .first()
        .expect("the example declares content")
        .clone();
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![contested.clone()],
    };

    let (machine, outcomes) = register_all(
        vec![entry("rival"), entry("content")],
        &mut [&mut rival, &mut content],
    );

    let refusal = outcomes
        .iter()
        .find(|(mod_id, _)| mod_id == &id("rival"))
        .and_then(|(_, outcome)| outcome.as_ref().err())
        .expect("the foreign claim was refused before acquiring ownership");
    assert_eq!(refusal.mod_id(), &id("rival"));
    assert_eq!(refusal.state(), LifecycleState::Constructed);
    let detail = refusal.to_string();
    assert!(detail.contains("rival"), "{detail}");
    assert!(detail.contains(&contested.to_string()), "{detail}");

    assert_eq!(
        machine.owner_of(&contested),
        Some(ContentMod::declared_id())
    );
    assert_eq!(machine.registry_claim_count(&id("rival")), 0);
    assert_eq!(
        machine.registry_claim_count(ContentMod::declared_id()),
        content
            .declaration()
            .expect("valid declaration")
            .entries()
            .len()
    );
}

#[test]
fn a_second_mod_claiming_a_name_the_example_did_not_declare_is_accepted() {
    let mut content = ContentMod::new();
    let mine = RegistryKey::new(
        ContentKind::Block,
        ContentId::new(id("rival"), "tin_lantern").expect("a legal content path"),
    );
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![mine.clone()],
    };

    let (machine, outcomes) = register_all(
        vec![entry("content"), entry("rival")],
        &mut [&mut content, &mut rival],
    );
    for (mod_id, outcome) in outcomes {
        outcome.unwrap_or_else(|error| panic!("`{mod_id}` was refused: {error}"));
    }
    assert_eq!(machine.owner_of(&mine), Some(&id("rival")));
}

#[test]
fn two_mods_contributing_to_one_tag_is_not_a_conflict_and_both_members_survive() {
    let content = ContentMod::new().declaration().expect("valid declaration");
    let lanterns = ContentMod::lanterns(ContentKind::Item);

    let mut rival = ContentDeclaration::new(id("rival"));
    let theirs = rival
        .entry(ContentKind::Item, "tin_lantern")
        .expect("a legal, undeclared entry");
    rival
        .tag(&lanterns, &theirs)
        .expect("an item joins an item tag");

    let set = TagSet::merged([&content, &rival]);
    assert_eq!(set.members(&lanterns).len(), 2);
    assert_eq!(
        set.contributors(&lanterns),
        [ContentMod::declared_id().clone(), id("rival")]
    );

    let reversed = TagSet::merged([&rival, &content]);
    assert_eq!(set.members(&lanterns), reversed.members(&lanterns));
    assert_ne!(
        set.contributors(&lanterns),
        reversed.contributors(&lanterns)
    );
}

#[test]
fn a_tag_cannot_hold_a_member_from_a_registry_it_does_not_group() {
    let mut declaration = ContentDeclaration::new(id("example_mod"));
    let block = declaration
        .entry(ContentKind::Block, "copper")
        .expect("a legal, undeclared entry");
    let items = TagId::new(
        ContentKind::Item,
        ContentId::new(id("c"), "ingots").expect("a legal content path"),
    )
    .expect("items are taggable");

    assert_eq!(
        declaration.tag(&items, &block),
        Err(DeclarationError::TagKind {
            tag: items.clone(),
            member: block,
        })
    );
    assert_eq!(declaration.tag_members(&items), []);
}

#[test]
fn two_of_one_mods_declarations_cannot_produce_the_same_file() {
    let mut declaration = ContentDeclaration::new(id("example_mod"));
    declaration
        .translation("en_us", "block.example_mod.copper", "Copper")
        .expect("the first text");

    let generated = RelativePath::try_from("assets/example_mod/lang/en_us.json".to_owned())
        .expect("a legal path");
    assert!(
        matches!(
            declaration.resource(generated.clone(), b"{}".to_vec()),
            Err(DeclarationError::ProducedPath { ref path, .. }) if path == &generated
        ),
        "a locale already writes that file"
    );
}

#[test]
fn one_mod_supplying_one_path_twice_is_refused_rather_than_overwritten() {
    let mut declaration = ContentDeclaration::new(id("example_mod"));
    let model = RelativePath::try_from("assets/example_mod/models/block/lantern.json".to_owned())
        .expect("a legal path");

    declaration
        .resource(model.clone(), b"{}".to_vec())
        .expect("the first supply");
    assert_eq!(
        declaration.resource(model.clone(), b"{\"changed\": true}".to_vec()),
        Err(DeclarationError::DuplicateResource {
            path: model.clone()
        })
    );

    assert_eq!(
        declaration
            .generate()
            .file(&model)
            .map(GeneratedFile::bytes),
        Some(b"{}".as_slice())
    );
}

#[test]
fn two_mods_supplying_one_path_is_an_override_that_names_the_mod_it_shadowed() {
    let english =
        RelativePath::try_from("assets/content/lang/en_us.json".to_owned()).expect("a legal path");
    let later = id("translation_pack");

    let set = ravel_data::ResourceSet::layered([
        (ContentMod::declared_id(), [&english]),
        (&later, [&english]),
    ]);

    assert_eq!(set.supplier(&english), Some(&later));
    assert!(set.has_overrides());
    assert_eq!(
        set.overrides()[0].shadowed,
        [ContentMod::declared_id().clone()]
    );
}

fn declared_in_another_order() -> ContentDeclaration {
    let mut declaration = ContentDeclaration::new(ContentMod::declared_id().clone());

    declaration
        .resource(
            ContentMod::model_path(),
            b"{\n  \"parent\": \"minecraft:block/lantern\"\n}\n".to_vec(),
        )
        .expect("the model is supplied once");
    declaration
        .resource(ContentMod::recipe_path(), RECIPE.to_vec())
        .expect("the recipe is supplied once");

    for (locale, name) in [("fr_fr", "Lanterne de cuivre"), ("en_us", "Copper Lantern")] {
        for prefix in ["item", "block"] {
            declaration
                .translation(
                    locale,
                    &format!("{prefix}.{}.copper_lantern", ContentMod::declared_id()),
                    name,
                )
                .expect("each key is named once per locale");
        }
    }

    let item = declaration
        .entry(ContentKind::Item, "copper_lantern")
        .expect("a legal, undeclared entry");
    declaration
        .tag(&ContentMod::lanterns(ContentKind::Item), &item)
        .expect("an item joins an item tag");

    let block = declaration
        .entry(ContentKind::Block, "copper_lantern")
        .expect("a legal, undeclared entry");
    declaration
        .tag(&ContentMod::lanterns(ContentKind::Block), &block)
        .expect("a block joins a block tag");

    declaration
        .entry(ContentKind::Recipe, "crafting/copper_lantern")
        .expect("a legal, undeclared entry");

    declaration
}

// Compare serialized paths and bytes independently of GeneratedData's equality.
fn as_written(data: &GeneratedData) -> Vec<u8> {
    let mut written = Vec::new();
    for file in data.files() {
        written.extend_from_slice(file.path().as_str().as_bytes());
        written.push(0);
        written.extend_from_slice(file.bytes());
        written.push(0);
    }
    written
}

#[test]
fn generating_one_declaration_twice_produces_identical_bytes() {
    let content = ContentMod::new();
    let first = content.generated().expect("valid declaration");
    let second = content.generated().expect("valid declaration");

    assert!(
        !first.is_empty(),
        "an empty pack would compare equal to any"
    );
    assert_eq!(as_written(&first), as_written(&second));
}

#[test]
fn the_same_content_declared_in_a_different_order_produces_identical_bytes() {
    let expected = ContentMod::new().generated().expect("valid declaration");
    let reordered = declared_in_another_order().generate();

    assert_eq!(
        as_written(&reordered),
        as_written(&expected),
        "generation reads what was declared, never the order it was declared in"
    );

    assert_ne!(
        declared_in_another_order().entries(),
        ContentMod::new()
            .declaration()
            .expect("valid declaration")
            .entries()
    );
}

#[test]
fn a_tags_members_are_generated_in_the_same_order_whichever_order_they_joined_in() {
    let lanterns = ContentMod::lanterns(ContentKind::Item);

    let build = |reversed: bool| {
        let mut declaration = ContentDeclaration::new(id("example_mod"));
        let mut paths = ["tin_lantern", "copper_lantern"];
        if reversed {
            paths.reverse();
        }
        for path in paths {
            let member = declaration
                .entry(ContentKind::Item, path)
                .expect("a legal, undeclared entry");
            declaration
                .tag(&lanterns, &member)
                .expect("an item joins an item tag");
        }
        declaration
    };

    let forwards = build(false);
    let backwards = build(true);
    assert_ne!(
        forwards.tag_members(&lanterns),
        backwards.tag_members(&lanterns),
        "the two declarations differ in the order the members joined"
    );

    let generated = forwards.generate();
    assert_eq!(as_written(&generated), as_written(&backwards.generate()));
    assert_eq!(
        std::str::from_utf8(
            generated
                .file(&lanterns.generated_path())
                .expect("the tag was declared")
                .bytes()
        )
        .expect("generated JSON is UTF-8"),
        "{\n  \"replace\": false,\n  \"values\": [\n    \"example_mod:copper_lantern\",\n    \
         \"example_mod:tin_lantern\"\n  ]\n}\n"
    );
}

#[test]
fn every_generated_path_is_derived_from_a_declaration_rather_than_spelled() {
    let content = ContentMod::new();
    let declaration = content.declaration().expect("valid declaration");
    let generated = content.generated().expect("valid declaration");

    let mut derived: Vec<String> = declaration
        .tags()
        .map(|tag| tag.generated_path().as_str().to_owned())
        .collect();
    for locale in declaration.locales() {
        derived.push(format!(
            "assets/{}/lang/{locale}.json",
            ContentMod::declared_id()
        ));
    }
    for supplied in declaration.resources() {
        derived.push(supplied.as_str().to_owned());
    }
    for entry in declaration.entries() {
        let (owner, path) = (entry.id().namespace(), entry.id().path());
        match entry.kind() {
            ContentKind::Block => {
                derived.push(format!("assets/{owner}/blockstates/{path}.json"));
                derived.push(format!("data/{owner}/loot_table/blocks/{path}.json"));
            }
            ContentKind::Item => {
                derived.push(format!("assets/{owner}/models/item/{path}.json"));
                derived.push(format!("assets/{owner}/items/{path}.json"));
            }
            ContentKind::Recipe | ContentKind::Command => {}
        }
    }
    derived.sort();
    derived.dedup();

    assert_eq!(generated.paths().collect::<Vec<_>>(), derived);
    assert_eq!(generated.len(), derived.len());
}

#[test]
fn a_generated_file_is_the_same_bytes_whichever_way_it_is_reached() {
    let generated = ContentMod::new().generated().expect("valid declaration");
    for file in generated.files() {
        assert_eq!(
            generated.file(file.path()).map(GeneratedFile::bytes),
            Some(file.bytes())
        );
    }
}

#[test]
fn every_generated_document_is_legal_json_and_ends_in_one_newline() {
    for file in ContentMod::new()
        .generated()
        .expect("valid declaration")
        .files()
    {
        let text = std::str::from_utf8(file.bytes()).expect("generated output is UTF-8");
        serde_json::from_str::<serde_json::Value>(text)
            .unwrap_or_else(|error| panic!("{} is not legal JSON: {error}", file.path()));
        assert!(text.ends_with('\n'), "{}", file.path());
        assert!(!text.ends_with("\n\n"), "{}", file.path());
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

    let ravel_host_runtime::Composition {
        machine,
        resources: supplied,
        ..
    } = compose(
        &game,
        Side::DedicatedServer,
        vec![Member {
            package: include_str!("../../../../examples/content-mod/package.json").to_owned(),
            host: Box::new(ContentMod::new()),
        }],
    )
    .expect("the shipped example should compose");
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

    let ravel_host_runtime::Composition { resources, .. } = compose(
        &game,
        Side::Client,
        vec![Member {
            package: include_str!("../../../../examples/content-mod/package.json").to_owned(),
            host: Box::new(ContentMod::new()),
        }],
    )
    .expect("the shipped example should compose");

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

    let ravel_host_runtime::Composition {
        machine,
        mods,
        configurations,
        resources,
    } = compose(
        &game,
        Side::IntegratedServer,
        vec![Member {
            package: include_str!("../../../../examples/content-mod/package.json").to_owned(),
            host: Box::new(ContentMod::new()),
        }],
    )
    .expect("the shipped example should compose");

    let world = game.join("saves").join("New World");
    std::fs::create_dir_all(&world).expect("a writable world directory");

    let written = write_world_pack(&resources, &world).expect("the pack should write");
    ravel_bootstrap_runtime::install(
        machine,
        mods,
        ravel_bootstrap_runtime::pre_world_report(),
        game.clone(),
        resources,
        configurations,
        false,
    )
    .expect("the composed runtime should install");

    assert_eq!(written.root(), world.join("datapacks").join("ravel"));
    assert_eq!(
        written.paths(),
        [
            "data/c/tags/block/lanterns.json",
            "data/c/tags/item/lanterns.json",
            "data/content/loot_table/blocks/copper_lantern.json",
            "data/content/recipe/crafting/copper_lantern.json",
            "pack.mcmeta",
        ]
    );
    assert_eq!(
        std::fs::read_to_string(
            written
                .root()
                .join("data/content/loot_table/blocks/copper_lantern.json")
        )
        .expect("the loot table is readable"),
        LOOT_TABLE
    );
    assert_eq!(
        std::fs::read(
            written
                .root()
                .join("data/content/recipe/crafting/copper_lantern.json")
        )
        .expect("the recipe is readable"),
        RECIPE
    );
    assert!(
        !written.root().join("assets").exists(),
        "a world reads data, and the assets are the instance's"
    );
    for path in written.paths() {
        assert!(written.root().join(path).is_file(), "{path}");
    }

    std::fs::remove_dir_all(&game).expect("the game directory is removable");
}
