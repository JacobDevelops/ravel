use ravel_config::{ConfigurationDeclaration, LayerError, ValueDocument};
use ravel_core::paths::{INSTANCE_DIRECTORY, PACK_CONFIG_DIRECTORY};
use ravel_core::{ContentId, ContentKind, ModId, RegistryKey, RelativePath, Side, Version};
use ravel_data::{ContentDeclaration, TagId};
use ravel_package::PackageManifest;

use super::{ClosureError, UnprovidedEntry, VANILLA_NAMESPACE, validate};
use crate::declare::PackDeclaration;

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn named(namespace: &str, path: &str) -> ContentId {
    ContentId::new(id(namespace), path).expect("a legal content path")
}

fn path(text: &str) -> RelativePath {
    RelativePath::try_from(text.to_owned()).expect("a legal relative path")
}

fn manifest(owner: &str, version: &str, requires: Option<(&str, &str)>) -> PackageManifest {
    let dependencies = match requires {
        Some((dependency, range)) => format!(
            r#"[{{ "id": "{dependency}", "relationship": "required", "version": "{range}" }}]"#
        ),
        None => "[]".to_owned(),
    };
    PackageManifest::parse(&format!(
        r#"{{
          "schemaVersion": 1,
          "id": "{owner}",
          "version": "{version}",
          "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] }},
          "sides": ["client", "integrated-server", "dedicated-server"],
          "dependencies": {dependencies},
          "source": {{
            "repository": "https://example.invalid/{owner}",
            "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}]
          }},
          "authors": [{{ "name": "Example" }}],
          "license": "MIT OR Apache-2.0",
          "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
        }}"#
    ))
    .expect("a legal manifest")
}

fn one_member() -> (
    PackDeclaration,
    Vec<PackageManifest>,
    Vec<ContentDeclaration>,
) {
    let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    pack.pin(id("base_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    (
        pack,
        vec![manifest("base_mod", "1.0.0", None)],
        vec![ContentDeclaration::new(id("base_mod"))],
    )
}

#[test]
fn a_pack_whose_four_closures_hold_writes_its_manifest_and_nothing_else() {
    let (pack, manifests, content) = one_member();
    let validated = validate(&pack, &manifests, &content, &[]).expect("every closure holds");

    assert_eq!(
        validated
            .files()
            .iter()
            .map(|file| file.path().as_str())
            .collect::<Vec<_>>(),
        [format!("{INSTANCE_DIRECTORY}/pack.json")]
    );
    assert_eq!(validated.overrides(), []);

    let manifest = validated
        .file(&path(&format!("{INSTANCE_DIRECTORY}/pack.json")))
        .expect("the pack writes its own manifest");
    let text = String::from_utf8(manifest.bytes().to_vec()).expect("the manifest is UTF-8");
    assert_eq!(
        text,
        "{\n  \"id\": \"example_pack\",\n  \"members\": {\n    \"base_mod\": \"1.0.0\"\n  },\n  \
         \"version\": \"0.1.0\"\n}\n"
    );
}

#[test]
fn a_pinned_member_with_no_manifest_is_refused_by_name() {
    let (mut pack, manifests, mut content) = one_member();
    pack.pin(id("absent_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    content.push(ContentDeclaration::new(id("absent_mod")));

    let refused = validate(&pack, &manifests, &content, &[]).expect_err("a member is missing");
    assert_eq!(
        refused,
        ClosureError::MemberMissing {
            member: id("absent_mod")
        }
    );
    assert!(refused.to_string().contains("absent_mod"), "{refused}");
}

#[test]
fn a_manifest_at_a_version_the_pack_did_not_pin_is_refused_with_both_versions() {
    let (pack, _, content) = one_member();
    let refused = validate(&pack, &[manifest("base_mod", "1.1.0", None)], &content, &[])
        .expect_err("the pin is exact");
    assert_eq!(
        refused,
        ClosureError::PinMismatch {
            member: id("base_mod"),
            pinned: Version::new(1, 0, 0),
            supplied: Version::new(1, 1, 0),
        }
    );
    let text = refused.to_string();
    assert!(text.contains("1.0.0") && text.contains("1.1.0"), "{text}");
}

#[test]
fn a_package_the_pack_pins_nothing_for_is_refused_rather_than_installed_quietly() {
    let (pack, mut manifests, content) = one_member();
    manifests.push(manifest("stowaway_mod", "1.0.0", None));

    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::UnpinnedPackage {
            package: id("stowaway_mod")
        })
    );
}

#[test]
fn a_dependency_the_pack_does_not_hold_is_refused_with_the_resolvers_own_reason() {
    let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    pack.pin(id("consumer_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    let manifests = vec![manifest(
        "consumer_mod",
        "1.0.0",
        Some(("library_mod", "^1.0.0")),
    )];
    let content = vec![ContentDeclaration::new(id("consumer_mod"))];

    let refused = validate(&pack, &manifests, &content, &[]).expect_err("the library is absent");
    assert!(matches!(refused, ClosureError::Dependency { .. }));
    let text = refused.to_string();
    assert!(text.contains("library_mod"), "{text}");
    assert!(
        Side::ALL.iter().any(|side| text.contains(side.as_str())),
        "the refusal names the side it does not resolve on: {text}"
    );
    assert!(std::error::Error::source(&refused).is_some());
}

#[test]
fn a_dependency_pinned_outside_the_range_its_dependant_accepts_is_refused() {
    let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    pack.pin(id("consumer_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    pack.pin(id("library_mod"), Version::new(2, 0, 0))
        .expect("a legal pin");
    let manifests = vec![
        manifest("consumer_mod", "1.0.0", Some(("library_mod", "^1.0.0"))),
        manifest("library_mod", "2.0.0", None),
    ];
    let content = vec![
        ContentDeclaration::new(id("consumer_mod")),
        ContentDeclaration::new(id("library_mod")),
    ];

    let refused =
        validate(&pack, &manifests, &content, &[]).expect_err("2.0.0 is outside `^1.0.0`");
    assert!(matches!(refused, ClosureError::Dependency { .. }));
    let text = refused.to_string();
    assert!(
        text.contains("library_mod") && text.contains("2.0.0"),
        "{text}"
    );
}

#[test]
fn a_path_the_pack_refers_to_and_nothing_supplies_is_refused_naming_the_referrer() {
    let (mut pack, manifests, content) = one_member();
    pack.refer_to(
        path("assets/example_pack/textures/lantern.png"),
        content_referrer(),
    );

    let refused = validate(&pack, &manifests, &content, &[]).expect_err("nothing supplies it");
    assert_eq!(
        refused,
        ClosureError::ResourceMissing {
            path: path("assets/example_pack/textures/lantern.png"),
            referrer: content_referrer(),
        }
    );
    let text = refused.to_string();
    assert!(
        text.contains("lantern.png") && text.contains("example_pack:icon"),
        "{text}"
    );
}

fn content_referrer() -> ContentId {
    named("example_pack", "icon")
}

#[test]
fn a_reference_answered_by_the_pack_itself_or_by_a_members_manifest_holds() {
    let (mut pack, manifests, content) = one_member();
    pack.supply(
        path("assets/example_pack/textures/lantern.png"),
        b"\x89PNG".to_vec(),
    )
    .expect("one supplied path");
    pack.refer_to(
        path("assets/example_pack/textures/lantern.png"),
        content_referrer(),
    );
    validate(&pack, &manifests, &content, &[]).expect("the pack supplies what it refers to");

    let mut second = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    second
        .pin(id("base_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    second.refer_to(path("resources/lang/en.json"), content_referrer());
    let with_resource = PackageManifest::parse(&manifest_document_with_resource(
        "base_mod",
        "1.0.0",
        "resources/lang/en.json",
    ))
    .expect("a legal manifest");
    validate(&second, &[with_resource], &content, &[])
        .expect("a member's declared resource answers a reference");
}

fn manifest_document_with_resource(owner: &str, version: &str, resource: &str) -> String {
    format!(
        r#"{{
          "schemaVersion": 1,
          "id": "{owner}",
          "version": "{version}",
          "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["26.0.0"] }},
          "sides": ["client", "integrated-server", "dedicated-server"],
          "resources": ["{resource}"],
          "source": {{
            "repository": "https://example.invalid/{owner}",
            "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}]
          }},
          "authors": [{{ "name": "Example" }}],
          "license": "MIT OR Apache-2.0",
          "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
        }}"#
    )
}

fn two_members() -> (PackDeclaration, Vec<PackageManifest>) {
    let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    pack.pin(id("first_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    pack.pin(id("second_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    (
        pack,
        vec![
            manifest("first_mod", "1.0.0", None),
            manifest("second_mod", "1.0.0", None),
        ],
    )
}

#[test]
fn two_members_contributing_to_one_tag_are_merged_into_one_file_holding_both() {
    let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
    let (pack, manifests) = two_members();

    let mut first = ContentDeclaration::new(id("first_mod"));
    let held = first
        .entry(ContentKind::Item, "copper_lantern")
        .expect("a legal entry");
    first.tag(&tag, &held).expect("an item joins an item tag");
    let mut second = ContentDeclaration::new(id("second_mod"));
    let other = second
        .entry(ContentKind::Item, "tin_lantern")
        .expect("a legal entry");
    second.tag(&tag, &other).expect("an item joins an item tag");

    let validated =
        validate(&pack, &manifests, &[first, second], &[]).expect("contributions merge");
    let written = validated
        .file(&tag.generated_path())
        .expect("one file for the tag");
    let text = String::from_utf8(written.bytes().to_vec()).expect("generated JSON is UTF-8");
    assert!(text.contains("first_mod:copper_lantern"), "{text}");
    assert!(text.contains("second_mod:tin_lantern"), "{text}");
    assert!(
        validated.overrides().is_empty(),
        "merging is not one mod taking a path from another"
    );
}

#[test]
fn raw_and_generated_member_tags_share_launch_append_and_replace_rules() {
    let shared = path("data/c/tags/item/lanterns.json");
    for first_raw in [false, true] {
        for replace in [false, true] {
            let (pack, manifests) = two_members();
            let mut first = ContentDeclaration::new(id("first_mod"));
            if first_raw {
                first
                    .resource(
                        shared.clone(),
                        br#"{"values":["minecraft:stone"],"old":true}"#.to_vec(),
                    )
                    .expect("a legal raw tag");
            } else {
                let held = first.entry(ContentKind::Item, "lantern").expect("an item");
                let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("a tag");
                first.tag(&tag, &held).expect("a matching member");
            }
            let mut second = ContentDeclaration::new(id("second_mod"));
            second.resource(
                shared.clone(),
                format!(r##"{{"replace":{replace},"values":[{{"id":"#c:optional","required":false}}],"custom":1}}"##).into_bytes(),
            ).expect("a legal raw tag");
            // Input declarations are reversed; unrelated mods resolve in mod-id order.
            let validated = validate(&pack, &manifests, &[second, first], &[]).expect("tags merge");
            let actual: serde_json::Value =
                serde_json::from_slice(validated.file(&shared).expect("merged tag").bytes())
                    .expect("tag JSON");
            let earlier = if first_raw {
                "minecraft:stone"
            } else {
                "first_mod:lantern"
            };
            let values = if replace {
                serde_json::json!([{"id":"#c:optional","required":false}])
            } else {
                serde_json::json!([earlier, {"id":"#c:optional","required":false}])
            };
            assert_eq!(
                actual,
                serde_json::json!({"replace":replace,"values":values,"custom":1})
            );
            if replace {
                assert_eq!(
                    validated.overrides(),
                    &[ravel_data::Override {
                        path: shared.clone(),
                        winner: id("second_mod"),
                        shadowed: vec![id("first_mod")],
                    }]
                );
            } else {
                assert_eq!(validated.overrides(), []);
            }
        }
    }
}

#[test]
fn member_tag_replacement_follows_dependency_order_not_owner_spelling() {
    let shared = path("data/c/tags/item/shared.json");
    let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    for owner in ["alpha", "zeta"] {
        pack.pin(id(owner), Version::new(1, 0, 0)).expect("a pin");
    }
    let manifests = [
        manifest("alpha", "1.0.0", Some(("zeta", ">=1.0, <2.0"))),
        manifest("zeta", "1.0.0", None),
    ];
    let mut alpha = ContentDeclaration::new(id("alpha"));
    alpha
        .resource(
            shared.clone(),
            br#"{"replace":true,"values":["minecraft:stone"]}"#.to_vec(),
        )
        .expect("a replacement tag");
    let mut zeta = ContentDeclaration::new(id("zeta"));
    let member = zeta.entry(ContentKind::Item, "thing").expect("an item");
    let tag = TagId::new(ContentKind::Item, named("c", "shared")).expect("a tag");
    zeta.tag(&tag, &member).expect("an item joins an item tag");
    let validated =
        validate(&pack, &manifests, &[alpha, zeta], &[]).expect("dependency order resolves");
    let actual: serde_json::Value =
        serde_json::from_slice(validated.file(&shared).expect("merged tag").bytes())
            .expect("tag JSON");
    assert_eq!(
        actual,
        serde_json::json!({"replace":true,"values":["minecraft:stone"]})
    );
    assert_eq!(
        validated.overrides(),
        &[ravel_data::Override {
            path: shared,
            winner: id("alpha"),
            shadowed: vec![id("zeta")],
        }]
    );
}

#[test]
fn malformed_member_tags_name_the_owner_and_path() {
    let (pack, manifests, mut content) = one_member();
    let shared = path("data/c/tags/item/lanterns.json");
    content[0]
        .resource(shared.clone(), br#"{"values":false}"#.to_vec())
        .expect("a raw resource");
    let ClosureError::TagDocument(error) =
        validate(&pack, &manifests, &content, &[]).expect_err("invalid tag schema is refused")
    else {
        panic!("the refusal must identify the malformed tag");
    };
    assert_eq!(error.mod_id, id("base_mod"));
    assert_eq!(error.path, shared);
}

#[test]
fn two_members_writing_one_path_that_is_not_a_tag_is_refused_and_both_are_named() {
    let shared = path("assets/shared/models/lantern.json");
    let (pack, manifests) = two_members();
    let mut first = ContentDeclaration::new(id("first_mod"));
    first
        .resource(shared.clone(), b"{}".to_vec())
        .expect("a legal supply");
    let mut second = ContentDeclaration::new(id("second_mod"));
    second
        .resource(shared.clone(), b"{\"changed\": true}".to_vec())
        .expect("a legal supply");

    let refused =
        validate(&pack, &manifests, &[first, second], &[]).expect_err("nothing could merge it");
    assert_eq!(
        refused,
        ClosureError::PathContested {
            path: shared,
            first: id("first_mod"),
            second: id("second_mod"),
        }
    );
    assert!(
        refused.to_string().contains("supplying the file itself"),
        "the refusal says how a pack settles it: {refused}"
    );
}

#[test]
fn a_pack_replacing_a_merged_tag_file_names_every_contributor_it_took_it_from() {
    let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
    let (mut pack, manifests) = two_members();
    let mut first = ContentDeclaration::new(id("first_mod"));
    let held = first
        .entry(ContentKind::Item, "copper_lantern")
        .expect("a legal entry");
    first.tag(&tag, &held).expect("an item joins an item tag");
    let mut second = ContentDeclaration::new(id("second_mod"));
    let other = second
        .entry(ContentKind::Item, "tin_lantern")
        .expect("a legal entry");
    second.tag(&tag, &other).expect("an item joins an item tag");
    pack.supply(tag.generated_path(), b"{}\n".to_vec())
        .expect("one supplied path");

    let validated = validate(&pack, &manifests, &[first, second], &[])
        .expect("a pack may write the tag file itself");
    assert_eq!(validated.overrides().len(), 1);
    assert_eq!(validated.overrides()[0].winner, id("example_pack"));
    assert_eq!(
        validated.overrides()[0].shadowed,
        [id("first_mod"), id("second_mod")],
        "a merged file was taken from every mod that contributed to it"
    );
    assert_eq!(
        validated
            .file(&tag.generated_path())
            .map(|file| file.bytes().to_vec()),
        Some(b"{}\n".to_vec())
    );
}

#[test]
fn the_pack_replacing_one_members_file_is_recorded_rather_than_refused() {
    let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
    let (mut pack, manifests, _) = one_member();
    let mut declaration = ContentDeclaration::new(id("base_mod"));
    let held = declaration
        .entry(ContentKind::Item, "copper_lantern")
        .expect("a legal entry");
    declaration
        .tag(&tag, &held)
        .expect("an item joins an item tag");
    pack.supply(tag.generated_path(), b"{}\n".to_vec())
        .expect("one supplied path");

    let validated =
        validate(&pack, &manifests, &[declaration], &[]).expect("a pack may replace a file");
    assert_eq!(validated.overrides().len(), 1);
    assert_eq!(validated.overrides()[0].winner, id("example_pack"));
    assert_eq!(validated.overrides()[0].shadowed, [id("base_mod")]);
    assert_eq!(
        validated
            .file(&tag.generated_path())
            .map(|file| file.bytes().to_vec()),
        Some(b"{}\n".to_vec()),
        "the pack's bytes are the ones written"
    );
}

#[test]
fn ordinary_member_resource_override_is_recorded() -> Result<(), Box<dyn std::error::Error>> {
    let shared = path("assets/base_mod/models/lantern.json");
    let (mut pack, manifests, _) = one_member();
    let mut declaration = ContentDeclaration::new(id("base_mod"));
    declaration.resource(shared.clone(), b"{}".to_vec())?;
    pack.supply(shared.clone(), b"{\"changed\":true}\n".to_vec())?;

    let validated = validate(&pack, &manifests, &[declaration], &[])?;
    assert_eq!(validated.overrides().len(), 1);
    assert_eq!(validated.overrides()[0].path, shared);
    assert_eq!(validated.overrides()[0].winner, id("example_pack"));
    assert_eq!(validated.overrides()[0].shadowed, [id("base_mod")]);
    assert_eq!(
        validated.file(&shared).map(super::PackFile::bytes),
        Some(b"{\"changed\":true}\n".as_slice())
    );
    Ok(())
}

#[test]
fn member_resources_cannot_use_ravels_directory() -> Result<(), Box<dyn std::error::Error>> {
    let reserved = path(&format!("{INSTANCE_DIRECTORY}/member.json"));
    let (pack, manifests, _) = one_member();
    let mut declaration = ContentDeclaration::new(id("base_mod"));
    declaration.resource(reserved.clone(), b"{}".to_vec())?;

    assert_eq!(
        validate(&pack, &manifests, &[declaration], &[]),
        Err(ClosureError::ReservedPath { path: reserved })
    );
    Ok(())
}

#[test]
fn nothing_but_ravel_may_write_into_ravels_own_directory() {
    let (mut pack, manifests, content) = one_member();
    pack.supply(
        path(&format!("{INSTANCE_DIRECTORY}/pack.json")),
        b"{}".to_vec(),
    )
    .expect("one supplied path");

    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::ReservedPath {
            path: path(&format!("{INSTANCE_DIRECTORY}/pack.json")),
        })
    );
}

#[test]
fn a_pinned_member_that_declared_no_content_at_all_is_refused() {
    let (pack, manifests, _) = one_member();
    let refused = validate(&pack, &manifests, &[], &[]).expect_err("the union must be complete");
    assert_eq!(
        refused,
        ClosureError::ContentMissing {
            member: id("base_mod")
        }
    );
    assert!(
        refused.to_string().contains("empty declaration"),
        "the refusal says what to do: {refused}"
    );
}

#[test]
fn a_content_declaration_from_a_mod_the_pack_does_not_pin_is_refused() {
    let (pack, manifests, mut content) = one_member();
    content.push(ContentDeclaration::new(id("stranger_mod")));
    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::ContentUnpinned {
            owner: id("stranger_mod")
        })
    );
}

#[test]
fn one_owner_declaring_its_content_twice_is_refused() {
    let (pack, manifests, mut content) = one_member();
    content.push(ContentDeclaration::new(id("base_mod")));
    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::ContentTwice {
            owner: id("base_mod")
        })
    );
}

#[test]
fn a_tag_member_the_game_owns_is_allowed_and_one_nothing_pins_is_refused() {
    let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
    let vanilla = RegistryKey::new(ContentKind::Item, named(VANILLA_NAMESPACE, "torch"));
    let (pack, manifests, _) = one_member();

    let mut allowed = ContentDeclaration::new(id("base_mod"));
    allowed
        .tag(&tag, &vanilla)
        .expect("a tag is not a claim on its members");
    validate(&pack, &manifests, &[allowed], &[])
        .expect("adding a vanilla item to a tag is ordinary");

    let mut stranger = ContentDeclaration::new(id("base_mod"));
    let absent = RegistryKey::new(ContentKind::Item, named("absent_mod", "lantern"));
    stranger
        .tag(&tag, &absent)
        .expect("a tag is not a claim on its members");
    let refused =
        validate(&pack, &manifests, &[stranger], &[]).expect_err("nothing in the pack provides it");
    assert_eq!(
        refused,
        ClosureError::TagMember {
            tag,
            member: absent,
            kind: UnprovidedEntry::NoSuchMember,
        }
    );
    let text = refused.to_string();
    assert!(text.contains("absent_mod:lantern"), "{text}");
    assert!(text.contains(VANILLA_NAMESPACE), "{text}");
}

#[test]
fn a_tag_member_in_a_pinned_mods_namespace_that_the_mod_never_declared_is_refused() {
    let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
    let (pack, manifests, _) = one_member();
    let mut declaration = ContentDeclaration::new(id("base_mod"));
    let never = RegistryKey::new(ContentKind::Item, named("base_mod", "phantom_lantern"));
    declaration
        .tag(&tag, &never)
        .expect("a tag is not a claim on its members");

    let refused = validate(&pack, &manifests, &[declaration], &[])
        .expect_err("the owner declares no such entry");
    assert_eq!(
        refused,
        ClosureError::TagMember {
            tag,
            member: never,
            kind: UnprovidedEntry::MemberDoesNotDeclareIt,
        }
    );
    assert!(
        refused.to_string().contains("declares no such entry"),
        "{refused}"
    );
}

#[test]
fn equal_entry_paths_in_different_member_namespaces_do_not_conflict() {
    let mut pack = PackDeclaration::new(id("example_pack"), Version::new(0, 1, 0));
    pack.pin(id("first_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    pack.pin(id("second_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    let manifests = vec![
        manifest("first_mod", "1.0.0", None),
        manifest("second_mod", "1.0.0", None),
    ];
    let mut first = ContentDeclaration::new(id("first_mod"));
    first
        .entry(ContentKind::Item, "lantern")
        .expect("a legal entry");
    let mut second = ContentDeclaration::new(id("second_mod"));
    second
        .entry(ContentKind::Item, "lantern")
        .expect("a legal entry");

    validate(&pack, &manifests, &[first, second], &[]).expect("two namespaces, two entries");
}

#[test]
fn a_progression_step_requiring_a_step_nothing_declares_is_refused() {
    let (mut pack, manifests, content) = one_member();
    pack.progression_mut()
        .declare(named("example_pack", "start"), [], None)
        .expect("a legal step");
    pack.progression_mut()
        .declare(
            named("example_pack", "locked"),
            [named("example_pack", "absent")],
            None,
        )
        .expect("a step may name a prerequisite not yet declared");

    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::PrerequisiteMissing {
            state: named("example_pack", "locked"),
            prerequisite: named("example_pack", "absent"),
        })
    );
}

#[test]
fn a_progression_step_nothing_could_ever_reach_is_refused() {
    let (mut pack, manifests, content) = one_member();
    pack.progression_mut()
        .declare(
            named("example_pack", "left"),
            [named("example_pack", "right")],
            None,
        )
        .expect("a legal step");
    pack.progression_mut()
        .declare(
            named("example_pack", "right"),
            [named("example_pack", "left")],
            None,
        )
        .expect("a legal step");

    let refused = validate(&pack, &manifests, &content, &[]).expect_err("neither step has a root");
    assert_eq!(
        refused,
        ClosureError::StateUnreachable {
            state: named("example_pack", "left"),
        }
    );
    assert!(
        refused.to_string().contains("can never be reached"),
        "{refused}"
    );
}

#[test]
fn a_progression_gated_on_an_entry_nothing_provides_is_refused() {
    let (mut pack, manifests, content) = one_member();
    let gate = RegistryKey::new(ContentKind::Item, named("base_mod", "phantom"));
    pack.progression_mut()
        .declare(named("example_pack", "start"), [], Some(gate.clone()))
        .expect("a legal step");

    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::Gate {
            state: named("example_pack", "start"),
            entry: gate,
            kind: UnprovidedEntry::MemberDoesNotDeclareIt,
        })
    );
}

#[test]
fn a_progression_gated_on_an_entry_a_member_declares_holds() {
    let (mut pack, manifests, _) = one_member();
    let mut declaration = ContentDeclaration::new(id("base_mod"));
    let lantern = declaration
        .entry(ContentKind::Item, "lantern")
        .expect("a legal entry");
    pack.progression_mut()
        .declare(named("example_pack", "start"), [], Some(lantern))
        .expect("a legal step");
    pack.progression_mut()
        .declare(
            named("example_pack", "after"),
            [named("example_pack", "start")],
            None,
        )
        .expect("a legal step");

    validate(&pack, &manifests, &[declaration], &[]).expect("the gate is an entry the pack holds");
}

fn configuration(owner: &str) -> ConfigurationDeclaration {
    ConfigurationDeclaration::parse(&format!(
        r#"{{
          "schemaVersion": 1,
          "owner": "{owner}",
          "version": 1,
          "settings": [
            {{
              "key": "limit",
              "valueSchema": {{ "type": "integer" }},
              "default": 4,
              "side": "common",
              "restartRequired": false,
              "packOverride": "allowed"
            }}
          ]
        }}"#
    ))
    .expect("a legal declaration")
}

fn configured(
    values: &str,
) -> (
    PackDeclaration,
    Vec<PackageManifest>,
    Vec<ContentDeclaration>,
) {
    let (mut pack, manifests, content) = one_member();
    let document = ValueDocument::parse(&format!(
        r#"{{ "schemaVersion": 1, "owner": "base_mod", "values": {values} }}"#
    ))
    .expect("a legal document");
    pack.configure(id("base_mod"), document)
        .expect("a document that names its own mod");
    (pack, manifests, content)
}

#[test]
fn a_settings_key_the_owner_does_not_declare_is_refused_to_the_pack_author() {
    let (pack, manifests, content) = configured(r#"{ "limt": 3 }"#);

    let refused = validate(&pack, &manifests, &content, &[configuration("base_mod")])
        .expect_err("`limt` is not a setting `base_mod` declares");

    assert_eq!(
        refused,
        ClosureError::Settings {
            owner: id("base_mod"),
            source: LayerError::PackUnknown {
                key: "limt".parse().expect("a legal setting key"),
            },
        }
    );
    let text = refused.to_string();
    assert!(text.contains("base_mod") && text.contains("limt"), "{text}");
    assert!(std::error::Error::source(&refused).is_some());
}

#[test]
fn a_settings_value_the_owner_would_refuse_is_refused_to_the_pack_author() {
    let (pack, manifests, content) = configured(r#"{ "limit": "three" }"#);

    let refused = validate(&pack, &manifests, &content, &[configuration("base_mod")])
        .expect_err("`limit` is an integer");

    assert!(matches!(
        refused,
        ClosureError::Settings {
            source: LayerError::PackValue { .. },
            ..
        }
    ));
}

#[test]
fn a_pack_stating_settings_for_a_member_that_declares_none_is_refused() {
    let (pack, manifests, content) = configured(r#"{ "limit": 3 }"#);

    assert_eq!(
        validate(&pack, &manifests, &content, &[]),
        Err(ClosureError::SettingsUndeclared {
            owner: id("base_mod"),
        })
    );
    assert!(
        ClosureError::SettingsUndeclared {
            owner: id("base_mod"),
        }
        .to_string()
        .contains("declares none")
    );
}

#[test]
fn a_packs_settings_document_is_written_where_a_launch_reads_the_pack_layer() {
    let (pack, manifests, content) = configured(r#"{ "limit": 3 }"#);

    let validated = validate(&pack, &manifests, &content, &[configuration("base_mod")])
        .expect("every closure holds");
    let written = validated
        .file(&path(&format!(
            "{INSTANCE_DIRECTORY}/{PACK_CONFIG_DIRECTORY}/base_mod.json"
        )))
        .expect("the pack writes its own layer");
    let text = String::from_utf8(written.bytes().to_vec()).expect("the document is UTF-8");
    assert!(text.contains("\"limit\""), "{text}");
    assert!(
        text.ends_with('\n'),
        "every written document ends in a newline"
    );
    assert_eq!(written.path().first_segment(), INSTANCE_DIRECTORY);
}

#[test]
fn validating_the_same_declaration_twice_produces_the_same_files() {
    let (mut pack, manifests, _) = one_member();
    let tag = TagId::new(ContentKind::Item, named("c", "lanterns")).expect("items are tagged");
    let mut declaration = ContentDeclaration::new(id("base_mod"));
    let lantern = declaration
        .entry(ContentKind::Item, "lantern")
        .expect("a legal entry");
    declaration
        .tag(&tag, &lantern)
        .expect("an item joins an item tag");
    declaration
        .translation("en_us", "item.base_mod.lantern", "Lantern")
        .expect("a legal translation");
    pack.supply(
        path("data/example_pack/recipe/lantern.json"),
        b"{}\n".to_vec(),
    )
    .expect("one supplied path");
    pack.progression_mut()
        .declare(named("example_pack", "start"), [], Some(lantern))
        .expect("a legal step");

    let first = validate(&pack, &manifests, std::slice::from_ref(&declaration), &[])
        .expect("every closure holds");
    let second = validate(&pack, &manifests, std::slice::from_ref(&declaration), &[])
        .expect("every closure holds");
    assert_eq!(first, second);
    assert!(
        first
            .files()
            .windows(2)
            .all(|pair| pair[0].path().as_str() < pair[1].path().as_str()),
        "the files are in path order and each path appears once"
    );
}
