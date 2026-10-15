#![forbid(unsafe_code)]

//! What a mod declares, what happens when two mods declare the same thing, and
//! why generated output is the same bytes twice.
//!
//! M4-W2's gate names three properties: declarations, conflicts, and
//! deterministic output. Each is checked here against something that can refuse
//! — the loader's own registration barrier, the declaration model's own
//! refusals, and byte comparison — rather than against a value this file
//! computed the same way twice.
//!
//! The mod under test is `examples/content-mod`, an example written against the
//! public facade alone. That is deliberate: a conformance suite that built its
//! own declaration would be testing the model, and the deliverable is a surface a
//! mod author can reach. Everything the example does, it does through
//! `ravel-sdk`.
//!
//! The barrier is the real one. `LifecycleMachine` owns the claim table, decides
//! every refusal, and is the same code a running game uses, so a conflict here is
//! the conflict a player would hit. A stub registrar in this file would prove the
//! stub.

use std::collections::BTreeSet;

use ravel_core::{ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet};
use ravel_data::{ContentDeclaration, DeclarationError, TagId, TagSet};
use ravel_example_content_mod::{ContentMod, MODEL, RECIPE};
use ravel_host_api::{HostError, HostMod, Registrar};
use ravel_host_runtime::{Member, compose, install_packs, write_resource_pack, write_world_pack};
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

/// A mod that claims keys somebody else's namespace names.
///
/// A [`ContentId`]'s namespace is not required to be the mod that registers the
/// content — `ravel-core` documents that, because a pack legitimately replaces a
/// recipe it did not write — so this is what a *second* claim on the example's
/// block looks like, and the barrier is what refuses it.
struct Rival {
    id: ModId,
    claims: Vec<RegistryKey>,
}

impl HostMod for Rival {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        for key in &self.claims {
            registrar
                .claim(key.clone())
                .map_err(|refused| HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: refused.to_string(),
                })?;
        }
        Ok(())
    }
}

/// What one mod's registration did, kept beside the mod it belongs to.
///
/// The refusals are returned rather than unwrapped, because a conflict is what
/// three of these tests are about.
type Outcome = (ModId, Result<(), HostError>);

/// Drives a plan to the registration window and registers each mod in order.
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
            outcomes.push((mod_id.clone(), host.register(&mut window)));
        }
        assert!(window.unexpected().is_none(), "the window was open");
    }
    (machine, outcomes)
}

// ----------------------------------------------------------- declarations ---

/// The registry entries the example declares, spelled out rather than derived.
///
/// Every other assertion in this file reads the declaration and checks what the
/// barrier or the generator made of it, so all of them hold when the example
/// declares *less*: a review deleted the recipe and this suite stayed at exit 0
/// while the example's own tests caught it. A floor derived from the subject is
/// not a floor, so this one is a literal.
const DECLARED_ENTRIES: &[&str] = &[
    "block:content:copper_lantern",
    "item:content:copper_lantern",
    "recipe:content:crafting/copper_lantern",
];

/// The paths the example's declaration generates, spelled out for the same reason.
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

/// The blockstates file Ravel writes for a block whose author supplied none.
///
/// The bytes, not the shape. A file naming the wrong model is a file that exists
/// and still renders the placeholder, which is what a launch found.
const BLOCKSTATES: &str = r#"{
  "variants": {
    "": {
      "model": "content:block/copper_lantern"
    }
  }
}
"#;

/// The item model an item that places a block inherits.
const ITEM_MODEL: &str = r#"{
  "parent": "content:block/copper_lantern"
}
"#;

/// The item definition the newer of the two tracked versions resolves through.
const ITEM_DEFINITION: &str = r#"{
  "model": {
    "model": "content:block/copper_lantern",
    "type": "minecraft:model"
  }
}
"#;

/// The loot table a block that drops itself carries.
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
        content.generated().paths().collect::<Vec<_>>(),
        GENERATED_PATHS,
        "the same floor for the generated side: an entry, a tag, a locale or a supplied resource \
         dropped from the example changes this list and nothing else in this file."
    );
}

#[test]
fn every_entry_the_example_declares_is_owned_by_it_after_the_real_barrier_closes() {
    let mut content = ContentMod::new();
    let declared = content.declaration().entries().to_vec();
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
        machine.registry_claims(ContentMod::declared_id()),
        declared.len()
    );

    // The barrier really did close, so nothing can be added after the window.
    machine
        .advance_all(LifecycleState::Registered)
        .expect("a legal step");
    assert!(machine.registration_barrier_closed());
}

#[test]
fn a_declared_entry_the_barrier_never_saw_has_no_owner() {
    // The counterpart to the test above: this is what "the barrier owns it"
    // looks like when nothing claimed it, so that assertion is falsifiable.
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

// ---------------------------------------------------------------- conflicts ---

#[test]
fn a_second_mod_claiming_the_examples_block_is_refused_and_told_who_owns_it() {
    let mut content = ContentMod::new();
    let contested = content
        .declaration()
        .entries()
        .iter()
        .find(|key| key.kind() == ContentKind::Block)
        .expect("the example declares a block")
        .clone();
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![contested.clone()],
    };

    // `content` is required-before nothing, so the plan's order is the order the
    // entries are given, and the example registers first.
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

    // The first claimant still owns it: a refused claim changes nothing.
    assert_eq!(
        machine.owner_of(&contested),
        Some(ContentMod::declared_id())
    );
    assert_eq!(machine.registry_claims(&id("rival")), 0);
}

#[test]
fn the_example_is_refused_when_it_is_the_second_mod_to_reach_the_barrier() {
    // The other direction of the same conflict, and the one that exercises the
    // mod-facing path: here the example's own `claim_all` meets the refusal. A
    // suite that only ever refused the rival would pass with `claim_all`
    // discarding refusals, which a mutation demonstrated.
    let mut content = ContentMod::new();
    let contested = content
        .declaration()
        .entries()
        .first()
        .expect("the example declares content")
        .clone();
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![contested.clone()],
    };

    // The rival is first in the plan, so it reaches the barrier first.
    let (machine, outcomes) = register_all(
        vec![entry("rival"), entry("content")],
        &mut [&mut rival, &mut content],
    );

    let refusal = outcomes
        .iter()
        .find(|(mod_id, _)| mod_id == ContentMod::declared_id())
        .and_then(|(_, outcome)| outcome.as_ref().err())
        .expect("the example's claim was refused");
    assert_eq!(refusal.mod_id(), ContentMod::declared_id());
    assert_eq!(refusal.state(), LifecycleState::Constructed);
    let detail = refusal.to_string();
    assert!(detail.contains("rival"), "{detail}");
    assert!(detail.contains(&contested.to_string()), "{detail}");

    assert_eq!(machine.owner_of(&contested), Some(&id("rival")));
    assert_eq!(machine.registry_claims(ContentMod::declared_id()), 0);
}

#[test]
fn a_second_mod_claiming_a_name_the_example_did_not_declare_is_accepted() {
    // The negative control for the conflict above. If any second claim were
    // refused, the test above would pass without the barrier comparing anything.
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
    // A tag has no owner, so this is the case a registry-shaped refusal would get
    // wrong. Nothing about it reaches the barrier at all.
    let content = ContentMod::new().declaration();
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

    // Merging is order-independent in membership and load-ordered in credit.
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
    // Within one mod this is a mistake; across mods the same path is an override
    // and no failure at all, which the next test shows.
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
    // The same-kind counterpart to the test above, which covers a tag or a locale
    // colliding with a supplied path. Two supplies of one path is the collision a
    // mod can make on its own, and it is the mod's mistake rather than an
    // override: an override needs a second mod, which the next test is about.
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

    // The refusal left the first bytes in place rather than half-replacing them.
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

// ------------------------------------------------------ deterministic output ---

/// Declares the example's content in a deliberately different order.
///
/// Same content, different sequence of calls: the tags before the entries, the
/// locales in reverse, the supplied model first. If generation read declaration
/// order anywhere, this is the value that would differ.
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

/// Renders one pack as the bytes it would be written as, path and content.
///
/// Comparing this rather than the values is the point: two [`GeneratedData`]
/// values could compare equal through a derived `PartialEq` that ignored a field,
/// and a byte string cannot.
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
    let first = content.generated();
    let second = content.generated();

    assert!(
        !first.is_empty(),
        "an empty pack would compare equal to any"
    );
    assert_eq!(as_written(&first), as_written(&second));
}

#[test]
fn the_same_content_declared_in_a_different_order_produces_identical_bytes() {
    let expected = ContentMod::new().generated();
    let reordered = declared_in_another_order().generate();

    assert_eq!(
        as_written(&reordered),
        as_written(&expected),
        "generation reads what was declared, never the order it was declared in"
    );

    // And the declarations themselves are not equal as values in every respect —
    // entry order differs — so the byte equality above is not equality of the
    // inputs restated.
    assert_ne!(
        declared_in_another_order().entries(),
        ContentMod::new().declaration().entries()
    );
}

#[test]
fn a_tags_members_are_generated_in_the_same_order_whichever_order_they_joined_in() {
    // The example puts one member in each of its tags, so nothing about it can
    // show that member order does not follow declaration order. This declares two
    // members both ways round and requires the same file — the case that makes
    // the sort inside generation load-bearing rather than incidental.
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
    let declaration = content.declaration();
    let generated = content.generated();

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
    // The conventional files a version needs for what was declared. Derived from
    // the entries the same way the tags above are derived from the tag ids, so a
    // path is still never spelled twice — once here and once in the generator.
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
    let generated = ContentMod::new().generated();
    for file in generated.files() {
        assert_eq!(
            generated.file(file.path()).map(GeneratedFile::bytes),
            Some(file.bytes())
        );
    }
}

#[test]
fn every_generated_document_is_legal_json_and_ends_in_one_newline() {
    for file in ContentMod::new().generated().files() {
        let text = std::str::from_utf8(file.bytes()).expect("generated output is UTF-8");
        serde_json::from_str::<serde_json::Value>(text)
            .unwrap_or_else(|error| panic!("{} is not legal JSON: {error}", file.path()));
        assert!(text.ends_with('\n'), "{}", file.path());
        assert!(!text.ends_with("\n\n"), "{}", file.path());
    }
}

/// The launch path, end to end: a composed host is handed the mod's own files.
///
/// This is the property the rest of the file could not reach. Everything above
/// asks the mod what it generates, and a mod whose bytes never left its crate
/// answers all of it correctly while a player sees the missing-model placeholder.
/// So the mod goes through `ravel_host_runtime::compose` — the same function every
/// host's JNI entry calls — under the package document it actually ships, and what
/// is read back is the composition's own supplied set.
///
/// The expected paths are the mod's `generated()` rather than a list written here.
/// A literal list would pass while the trait method handed over a different
/// declaration from the one the pack is built from, which is the drift this whole
/// surface exists to make impossible.
#[test]
fn a_composed_host_is_supplied_every_file_the_content_mod_generates() {
    let game = std::env::temp_dir().join(format!("ravel-supplied-content-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game);
    std::fs::create_dir_all(&game).expect("a writable temporary directory");

    let (machine, _, _, supplied) = compose(
        &game,
        Side::DedicatedServer,
        vec![Member {
            package: include_str!("../../../examples/content-mod/package.json").to_owned(),
            host: Box::new(ContentMod::new()) as Box<dyn HostMod>,
        }],
    )
    .expect("the shipped example should compose");
    assert_eq!(
        machine.state(ContentMod::declared_id()),
        Ok(LifecycleState::Running)
    );

    // The model by name, before anything derived. It is the file that made this
    // test necessary: a version decides its shape so Ravel generates none, the mod
    // supplies the bytes, and until the host asked for them a player saw the
    // missing-model placeholder for an item the game had registered. A comparison
    // against `generated()` alone would hold for a declaration that had dropped it.
    let model = ContentMod::model_path();
    assert_eq!(
        supplied.set().supplier(&model),
        Some(ContentMod::declared_id())
    );
    assert_eq!(supplied.bytes(&model), Some(MODEL));

    let generated = ContentMod::new().generated();
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

    let _ = std::fs::remove_dir_all(&game);
}

/// The instance a launch leaves behind: the mod's assets are a pack on disk.
///
/// The end of the path the test above starts. `write_resource_pack` is the call
/// `ravel_host_runtime::initialize` makes, so what lands here is what a launch
/// writes rather than a rehearsal of it.
///
/// The expected tree is written out below rather than derived from the mod. A
/// comparison against `generated()` would pass for a writer that put the data half
/// in the resource pack, or dropped the metadata, or wrote nothing at all — the
/// three things this is for. What the metadata's number *is* is not asserted here:
/// it is version data, and the adapter refuses one the pinned jar disagrees with.
#[test]
fn a_launch_writes_the_mods_assets_as_a_pack_the_client_scans() {
    let game = std::env::temp_dir().join(format!("ravel-launch-pack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game);
    std::fs::create_dir_all(&game).expect("a writable temporary directory");

    let (_, _, _, resources) = compose(
        &game,
        Side::Client,
        vec![Member {
            package: include_str!("../../../examples/content-mod/package.json").to_owned(),
            host: Box::new(ContentMod::new()) as Box<dyn HostMod>,
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
    // The three files a block needs beside the model its author wrote, held to
    // the exact bytes rather than to their presence: a blockstates file naming
    // the wrong model, or an item model parented at nothing, is a file that
    // exists and still shows the placeholder.
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

    let metadata = std::fs::read_to_string(written.root().join("pack.mcmeta"))
        .expect("the metadata is readable");
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

    let _ = std::fs::remove_dir_all(&game);
}

/// The world half: opening a world leaves the mods' data where the world reads it.
///
/// `write_world_pack` is the call the world-open crossing makes, reached here
/// through the host runtime so a test and a launch take one route to it. The set
/// it writes is the one `initialize` installs, which is what `install_packs` does.
///
/// **What this cannot show is that the world reads it during the same session.**
/// The game builds a world's pack list before it reaches the crossing that knows
/// which world is opening, so a pack written here is read from the next open of
/// that world. That is recorded on `ravel_bootstrap`'s own module and is the
/// reason this test asserts about the filesystem and not about a session.
#[test]
fn opening_a_world_leaves_the_mods_data_where_that_world_reads_it() {
    let game = std::env::temp_dir().join(format!("ravel-world-data-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game);
    std::fs::create_dir_all(&game).expect("a writable temporary directory");

    let (_, _, _, resources) = compose(
        &game,
        Side::IntegratedServer,
        vec![Member {
            package: include_str!("../../../examples/content-mod/package.json").to_owned(),
            host: Box::new(ContentMod::new()) as Box<dyn HostMod>,
        }],
    )
    .expect("the shipped example should compose");
    install_packs(resources);

    let world = game.join("saves").join("New World");
    std::fs::create_dir_all(&world).expect("a writable world directory");

    let written = write_world_pack(&world)
        .expect("the pack should write")
        .expect("a launch that installed a set writes one");

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
    // The loot table is written at the path the game derives from the block's
    // own identifier, which is that identifier with `blocks/` in front of it. A
    // table one directory up is a table nothing loads and a block that drops
    // nothing, which is what a launch found.
    assert_eq!(
        std::fs::read_to_string(
            written
                .root()
                .join("data/content/loot_table/blocks/copper_lantern.json")
        )
        .expect("the loot table is readable"),
        LOOT_TABLE
    );
    // The recipe is the author's bytes, unchanged. Ravel generates no body for
    // one: the two tracked versions spell an ingredient differently, so there is
    // no shape a generator could pick that is right on both.
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

    let _ = std::fs::remove_dir_all(&game);
}
