//! Declaration validation, tag merging, and resource conflicts.

use ravel_core::{ContentId, ContentKind};
use ravel_data::{ContentDeclaration, ContentDeclarationError, TagId, TagResources};
use ravel_example_content_mod::ContentMod;
use ravel_sdk::Declared;
use ravel_sdk::generate::GeneratedFile;
use ravel_sdk::resources::RelativePath;

use super::id;

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
        Err(ContentDeclarationError::DuplicateEntry { key: block })
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
                Err(ContentDeclarationError::Path { .. })
            ),
            "{path:?} is not a legal content path"
        );
    }
    assert_eq!(declaration.entries(), []);
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

    let set = TagResources::merged([&content, &rival]).expect("valid tags");
    assert_eq!(set.values(&lanterns.generated_path()).len(), 2);
    assert_eq!(
        set.contributors(&lanterns.generated_path()),
        [ContentMod::declared_id().clone(), id("rival")]
    );

    let reversed = TagResources::merged([&rival, &content]).expect("valid tags");
    assert_ne!(
        set.values(&lanterns.generated_path()),
        reversed.values(&lanterns.generated_path())
    );
    assert_ne!(
        set.contributors(&lanterns.generated_path()),
        reversed.contributors(&lanterns.generated_path())
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
        Err(ContentDeclarationError::TagKind {
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
            Err(ContentDeclarationError::ProducedPath { ref path, .. }) if path == &generated
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
        Err(ContentDeclarationError::DuplicateResource {
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
