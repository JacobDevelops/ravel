//! Generated content floors and order-independent serialization.

use ravel_core::ContentKind;
use ravel_data::ContentDeclaration;
use ravel_example_content_mod::{ContentMod, RECIPE};
use ravel_sdk::Declared;
use ravel_sdk::generate::{GeneratedData, GeneratedFile};
use ravel_sdk::server_data::ServerDataKind;

use super::id;

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

fn declared_in_another_order() -> ContentDeclaration {
    let mut declaration = ContentDeclaration::new(ContentMod::declared_id().clone());

    declaration
        .resource(
            ContentMod::model_path(),
            b"{\n  \"parent\": \"minecraft:block/lantern\"\n}\n".to_vec(),
        )
        .expect("the model is supplied once");
    declaration
        .server_entry(
            ServerDataKind::Crafting,
            "crafting/copper_lantern",
            RECIPE.to_vec(),
        )
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
