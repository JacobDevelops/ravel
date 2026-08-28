#![forbid(unsafe_code)]

//! M4-W5's gate: dependency, resource, registry and progression closure.
//!
//! Every check here goes through `ravel_pack::validate`, which is the only thing
//! that produces a writable pack, over the content a mod written against
//! `ravel-sdk` actually declares — `ravel-example-content-mod`'s own
//! [`ContentMod::declaration`] rather than a shape invented in this file.
//!
//! # What the fifth section is for
//!
//! M4-W2's review recorded that determinism had been proved same-process only:
//! nothing wrote a pack to disk, so nothing compared bytes across two runs. A
//! same-process comparison cannot see the failures that matter — a hash seed, a
//! locale, an environment variable, a working directory baked into a byte — because
//! both sides of the comparison share all of them. So the last section writes the
//! pack twice from two *processes* and compares every byte, and then scans the
//! written bytes for the machine they were written on.
//!
//! # What is still not checked, and cannot be here
//!
//! Whether the layout under `data/` and `assets/` is the layout Minecraft 1.21.1
//! reads. Nothing in this repository holds the pinned jar, so the tag directory, the
//! `replace` field and the language-file path are this repository's belief about the
//! pack format and are asserted against themselves. `ravel_pack::RESERVED_PREFIX`
//! separates the files that are Ravel's own — where the layout is a decision — from
//! the files that are that belief, and this suite checks the separation rather than
//! pretending to check the belief.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ravel_core::{ContentId, ContentKind, ModId, RegistryKey, RelativePath, Side, Version};
use ravel_data::{ContentDeclaration, TagId};
use ravel_example_content_mod::ContentMod;
use ravel_pack::{
    ClosureError, PackDeclaration, RESERVED_PREFIX, UnprovidedEntry, VANILLA_NAMESPACE,
    ValidatedPack, validate,
};
use ravel_package::PackageManifest;
use ravel_sdk::Declared;

/// A digest shaped like one. Nothing here verifies a package's integrity; that is
/// `ravel-package`'s and it has its own suite.
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The pack's own namespace.
const PACK: &str = "example_pack";

/// The environment variable the cross-process half of the determinism check uses.
const WRITE_TO: &str = "RAVEL_PACK_CONFORMANCE_WRITE_TO";

/// The test the parent process re-runs in a child, named once.
const WRITER: &str = "the_same_pack_written_by_two_processes_is_the_same_bytes";

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn named(namespace: &str, path: &str) -> ContentId {
    ContentId::new(id(namespace), path).expect("a legal content path")
}

fn path(text: &str) -> RelativePath {
    RelativePath::try_from(text.to_owned()).expect("a legal relative path")
}

/// One member's manifest, at `version`, requiring `requires` when it does.
fn manifest(owner: &ModId, version: &str, requires: Option<(&str, &str)>) -> PackageManifest {
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
          "compatibility": {{ "ravel": ">=0.1, <0.2", "minecraft": ["1.21.1"] }},
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

/// The version the pack pins the example mod at.
const MEMBER_VERSION: &str = "1.4.0";

/// A pack holding `ravel-example-content-mod` and nothing else.
///
/// The member is a real mod: its content is whatever `ContentMod::declaration`
/// declares, so a change to the example's content changes what this suite validates
/// rather than leaving a copy of it here to go stale.
fn one_member() -> (
    PackDeclaration,
    Vec<PackageManifest>,
    Vec<ContentDeclaration>,
) {
    let owner = ContentMod::declared_id();
    let mut pack = PackDeclaration::new(id(PACK), Version::new(0, 2, 1));
    pack.pin(owner.clone(), Version::new(1, 4, 0))
        .expect("a legal pin");
    (
        pack,
        vec![manifest(owner, MEMBER_VERSION, None)],
        vec![ContentMod::new().declaration()],
    )
}

#[test]
fn the_pack_this_suite_is_written_against_is_one_whose_four_closures_hold() {
    // The floor. Every refusal below is a departure from this pack, so if this one
    // did not validate, each of them could be failing for the wrong reason.
    let (pack, manifests, content) = one_member();
    let validated = validate(&pack, &manifests, &content).expect("every closure holds");

    // The example mod declares content, so the pack writes files, and the tag file
    // and the language file are the ones the mod itself derives.
    let owner = ContentMod::declared_id();
    assert!(
        validated
            .file(&ContentMod::lanterns(ContentKind::Item).generated_path())
            .is_some(),
        "the member's tag reached the pack"
    );
    assert!(
        validated
            .file(&path(&format!("assets/{owner}/lang/en_us.json")))
            .is_some()
    );
    assert!(validated.overrides().is_empty());
}

// -------------------------------------------------------- dependency closure ---

#[test]
fn a_declared_dependency_the_pack_does_not_hold_is_refused_with_the_offender_named() {
    let owner = ContentMod::declared_id();
    let mut pack = PackDeclaration::new(id(PACK), Version::new(0, 2, 1));
    pack.pin(owner.clone(), Version::new(1, 4, 0))
        .expect("a legal pin");
    let manifests = vec![manifest(
        owner,
        MEMBER_VERSION,
        Some(("library_mod", "^1.0.0")),
    )];

    let refused = validate(&pack, &manifests, &[ContentMod::new().declaration()])
        .expect_err("the library is not in the pack");

    assert!(matches!(refused, ClosureError::Dependency { .. }));
    let text = refused.to_string();
    assert!(text.contains("library_mod"), "{text}");
    assert!(
        Side::ALL.iter().any(|side| text.contains(side.name())),
        "the refusal names the side it does not resolve on: {text}"
    );
}

#[test]
fn a_dependency_present_at_a_version_the_dependant_refuses_is_refused_with_both() {
    let owner = ContentMod::declared_id();
    let mut pack = PackDeclaration::new(id(PACK), Version::new(0, 2, 1));
    pack.pin(owner.clone(), Version::new(1, 4, 0))
        .expect("a legal pin");
    pack.pin(id("library_mod"), Version::new(2, 0, 0))
        .expect("a legal pin");
    let manifests = vec![
        manifest(owner, MEMBER_VERSION, Some(("library_mod", "^1.0.0"))),
        manifest(&id("library_mod"), "2.0.0", None),
    ];
    let content = vec![
        ContentMod::new().declaration(),
        ContentDeclaration::new(id("library_mod")),
    ];

    let refused = validate(&pack, &manifests, &content).expect_err("2.0.0 is outside `^1.0.0`");

    assert!(matches!(refused, ClosureError::Dependency { .. }));
    let text = refused.to_string();
    assert!(text.contains("library_mod"), "{text}");
    assert!(
        text.contains("2.0.0"),
        "the installed version is named: {text}"
    );
    assert!(
        text.contains("^1.0.0"),
        "the accepted range is named: {text}"
    );
}

#[test]
fn a_member_pinned_at_one_version_and_supplied_at_another_is_refused() {
    // The pack-level half of dependency closure, which the resolver cannot see: the
    // set resolves perfectly and is not the set the pack decided on.
    let (pack, _, content) = one_member();
    let refused = validate(
        &pack,
        &[manifest(ContentMod::declared_id(), "1.5.0", None)],
        &content,
    )
    .expect_err("a pin is exact");

    assert_eq!(
        refused,
        ClosureError::PinMismatch {
            member: ContentMod::declared_id().clone(),
            pinned: Version::new(1, 4, 0),
            supplied: Version::new(1, 5, 0),
        }
    );
}

// ---------------------------------------------------------- resource closure ---

#[test]
fn a_pack_referring_to_a_resource_nothing_supplies_is_refused_naming_the_referrer() {
    let (mut pack, manifests, content) = one_member();
    pack.refer_to(
        path("assets/example_pack/textures/gui/progress.png"),
        named(PACK, "smelting"),
    );

    let refused = validate(&pack, &manifests, &content).expect_err("nothing supplies it");

    assert_eq!(
        refused,
        ClosureError::ResourceMissing {
            path: path("assets/example_pack/textures/gui/progress.png"),
            referrer: named(PACK, "smelting"),
        }
    );

    // And the negative control: the same reference holds once the pack supplies it,
    // so the refusal is about the missing file and not about referring at all.
    let mut supplied = pack.clone();
    supplied.supply(
        path("assets/example_pack/textures/gui/progress.png"),
        b"\x89PNG".to_vec(),
    );
    validate(&supplied, &manifests, &content).expect("the pack supplies what it refers to");
}

#[test]
fn a_pack_supplying_one_path_twice_is_refused() {
    let (mut pack, manifests, content) = one_member();
    let recipe = path("data/example_pack/recipe/copper_lantern.json");
    pack.supply(recipe.clone(), b"{}\n".to_vec());
    pack.supply(recipe.clone(), b"{\"changed\": true}\n".to_vec());

    assert_eq!(
        validate(&pack, &manifests, &content),
        Err(ClosureError::SuppliedTwice { path: recipe })
    );
}

#[test]
fn a_resource_the_member_itself_declares_answers_a_reference_to_it() {
    // `ContentMod` supplies a block model. The pack referring to that path is closed
    // by the member rather than by the pack, which is the case a pack author hits
    // most: the file exists because a mod ships it.
    let (mut pack, manifests, content) = one_member();
    pack.refer_to(ContentMod::model_path(), named(PACK, "smelting"));
    validate(&pack, &manifests, &content).expect("a member's own resource answers it");
}

// ---------------------------------------------------------- registry closure ---

#[test]
fn the_namespace_the_pack_allows_unverified_is_spelled_the_way_the_game_spells_it() {
    // Every other test here names the constant symbolically, so an assertion about
    // vanilla moves with the thing it asserts and the spelling is pinned by nothing.
    // The consequence of a wrong spelling is asymmetric rather than merely wrong: a
    // real `minecraft:` member would fall into the refusing branch, which is loud,
    // while whatever the constant did spell would fall into the permissive branch and
    // be allowed unverified, which is silent. So the literal is written out once, here,
    // against the namespace the game's own registry uses. That namespace lives inside
    // the pinned jar, which is not tracked, so this line is the pin and not a
    // derivation — and it is the one place in this suite where that is the right trade.
    assert_eq!(
        VANILLA_NAMESPACE, "minecraft",
        "the namespace a pack may refer to unverified is the game's own"
    );
}

#[test]
fn a_tag_member_in_the_games_namespace_is_allowed_and_deliberately_unverified() {
    // This is the case M4-W2's review left to W5. Adding a vanilla item to a tag is
    // the ordinary reason a pack exists, so it is allowed — and nothing in this
    // workspace holds the game's registry, so it is *not* proof the item exists.
    // `ravel_pack::VANILLA_NAMESPACE` says so where the code is.
    let (pack, manifests, _) = one_member();
    let mut declaration = ContentMod::new().declaration();
    declaration
        .tag(
            &ContentMod::lanterns(ContentKind::Item),
            &RegistryKey::new(ContentKind::Item, named(VANILLA_NAMESPACE, "lantern")),
        )
        .expect("a tag is not a claim on its members");

    let validated =
        validate(&pack, &manifests, &[declaration]).expect("a vanilla member is allowed");
    let tag = ContentMod::lanterns(ContentKind::Item);
    let written = validated.file(&tag.generated_path()).expect("the tag file");
    let text = String::from_utf8(written.bytes().to_vec()).expect("generated JSON is UTF-8");
    assert!(
        text.contains(&format!("{VANILLA_NAMESPACE}:lantern")),
        "the member is written rather than dropped: {text}"
    );
}

#[test]
fn a_tag_member_in_a_namespace_the_pack_pins_nothing_for_is_refused() {
    let (pack, manifests, _) = one_member();
    let absent = RegistryKey::new(ContentKind::Item, named("absent_mod", "lantern"));
    let mut declaration = ContentMod::new().declaration();
    declaration
        .tag(&ContentMod::lanterns(ContentKind::Item), &absent)
        .expect("a tag is not a claim on its members");

    let refused =
        validate(&pack, &manifests, &[declaration]).expect_err("nothing in the pack provides it");

    assert_eq!(
        refused,
        ClosureError::TagMember {
            tag: ContentMod::lanterns(ContentKind::Item).to_string(),
            member: absent,
            kind: UnprovidedEntry::NoSuchMember,
        }
    );
    assert!(
        refused.to_string().contains("absent_mod:lantern"),
        "{refused}"
    );
}

#[test]
fn a_tag_member_the_owning_mod_never_declared_is_refused_even_though_the_mod_is_pinned() {
    // The half that needs the members' own declarations rather than only the pins.
    // The namespace is a mod the pack holds, and the entry does not exist — a typo
    // in a tag, which is the shape this defect takes in practice.
    let owner = ContentMod::declared_id();
    let (pack, manifests, _) = one_member();
    let phantom = RegistryKey::new(ContentKind::Item, named(owner.as_str(), "phantom_lantern"));
    let mut declaration = ContentMod::new().declaration();
    declaration
        .tag(&ContentMod::lanterns(ContentKind::Item), &phantom)
        .expect("a tag is not a claim on its members");

    let refused = validate(&pack, &manifests, &[declaration])
        .expect_err("the owning mod declares no such entry");

    assert_eq!(
        refused,
        ClosureError::TagMember {
            tag: ContentMod::lanterns(ContentKind::Item).to_string(),
            member: phantom,
            kind: UnprovidedEntry::MemberDoesNotDeclareIt,
        }
    );

    // The negative control: the entry the mod really does declare passes the same
    // check, so the refusal is about the entry and not about the namespace.
    let real = ContentMod::new()
        .declaration()
        .entries()
        .iter()
        .find(|key| key.kind() == ContentKind::Item)
        .cloned()
        .expect("the example mod declares an item");
    let mut legal = ContentMod::new().declaration();
    legal
        .tag(&ContentMod::lanterns(ContentKind::Item), &real)
        .expect_err("the mod already put its own item in that tag");
    validate(&pack, &manifests, &[ContentMod::new().declaration()])
        .expect("the shipped declaration holds");
}

#[test]
fn two_members_contributing_to_one_tag_are_written_as_one_file_holding_both() {
    let owner = ContentMod::declared_id();
    let tag = ContentMod::lanterns(ContentKind::Item);
    let mut pack = PackDeclaration::new(id(PACK), Version::new(0, 2, 1));
    pack.pin(owner.clone(), Version::new(1, 4, 0))
        .expect("a legal pin");
    pack.pin(id("second_mod"), Version::new(1, 0, 0))
        .expect("a legal pin");
    let manifests = vec![
        manifest(owner, MEMBER_VERSION, None),
        manifest(&id("second_mod"), "1.0.0", None),
    ];
    let mut second = ContentDeclaration::new(id("second_mod"));
    let other = second
        .entry(ContentKind::Item, "tin_lantern")
        .expect("a legal entry");
    second.tag(&tag, &other).expect("an item joins an item tag");

    let validated = validate(
        &pack,
        &manifests,
        &[ContentMod::new().declaration(), second],
    )
    .expect("contributions merge rather than contest");

    let written = validated.file(&tag.generated_path()).expect("one tag file");
    let text = String::from_utf8(written.bytes().to_vec()).expect("generated JSON is UTF-8");
    assert!(text.contains("second_mod:tin_lantern"), "{text}");
    assert!(text.contains(&format!("{owner}:")), "{text}");
}

#[test]
fn two_members_claiming_one_registry_entry_are_both_named() {
    // Impossible inside one declaration, because an entry is named in the declaring
    // mod's namespace. Possible across two only if a mod declares into a namespace it
    // does not own, which `ContentDeclaration` forbids — so the reachable case is a
    // pack that pins one mod's content twice under two identities, and the layer that
    // catches it is the pack's completeness check rather than the entry table.
    let (pack, manifests, mut content) = one_member();
    content.push(ContentMod::new().declaration());

    assert_eq!(
        validate(&pack, &manifests, &content),
        Err(ClosureError::ContentTwice {
            owner: ContentMod::declared_id().clone(),
        })
    );
}

#[test]
fn a_pinned_member_whose_content_the_pack_never_collected_is_refused() {
    let (pack, manifests, _) = one_member();
    let refused = validate(&pack, &manifests, &[]).expect_err("the union has to be complete");
    assert_eq!(
        refused,
        ClosureError::ContentMissing {
            member: ContentMod::declared_id().clone(),
        }
    );
}

// ------------------------------------------------------- progression closure ---

#[test]
fn a_progression_step_nothing_could_ever_reach_is_refused_by_name() {
    let (mut pack, manifests, content) = one_member();
    pack.progression_mut()
        .declare(named(PACK, "left"), [named(PACK, "right")], None)
        .expect("a legal step");
    pack.progression_mut()
        .declare(named(PACK, "right"), [named(PACK, "left")], None)
        .expect("a legal step");

    let refused = validate(&pack, &manifests, &content).expect_err("neither step has a root");

    assert_eq!(
        refused,
        ClosureError::StateUnreachable {
            state: named(PACK, "left"),
        }
    );
    assert!(
        refused.to_string().contains("can never be reached"),
        "{refused}"
    );
}

#[test]
fn a_progression_step_requiring_a_step_the_pack_does_not_declare_is_refused() {
    let (mut pack, manifests, content) = one_member();
    pack.progression_mut()
        .declare(named(PACK, "smelting"), [], None)
        .expect("a root step");
    pack.progression_mut()
        .declare(named(PACK, "illumination"), [named(PACK, "absent")], None)
        .expect("a step may name a prerequisite not yet declared");

    assert_eq!(
        validate(&pack, &manifests, &content),
        Err(ClosureError::PrerequisiteMissing {
            state: named(PACK, "illumination"),
            prerequisite: named(PACK, "absent"),
        })
    );
}

#[test]
fn a_progression_gate_is_held_to_the_same_rule_a_tag_member_is() {
    let owner = ContentMod::declared_id();
    let (mut pack, manifests, content) = one_member();
    let phantom = RegistryKey::new(ContentKind::Item, named(owner.as_str(), "phantom_lantern"));
    pack.progression_mut()
        .declare(named(PACK, "smelting"), [], Some(phantom.clone()))
        .expect("a legal step");

    assert_eq!(
        validate(&pack, &manifests, &content),
        Err(ClosureError::Gate {
            state: named(PACK, "smelting"),
            entry: phantom,
            kind: UnprovidedEntry::MemberDoesNotDeclareIt,
        })
    );

    // The negative control: the entry the member really declares passes.
    let real = ContentMod::new()
        .declaration()
        .entries()
        .iter()
        .find(|key| key.kind() == ContentKind::Item)
        .cloned()
        .expect("the example mod declares an item");
    let (mut legal, manifests, content) = one_member();
    legal
        .progression_mut()
        .declare(named(PACK, "smelting"), [], Some(real))
        .expect("a legal step");
    validate(&legal, &manifests, &content).expect("the gate names an entry the pack holds");
}

// --------------------------------------------------- determinism, on disk ---

/// Builds the pack the determinism checks write, with every kind of file in it.
fn every_kind_of_file() -> (
    PackDeclaration,
    Vec<PackageManifest>,
    Vec<ContentDeclaration>,
) {
    let (mut pack, manifests, content) = one_member();
    pack.configure(
        ContentMod::declared_id().clone(),
        ravel_sdk::config::ValueDocument::parse(&format!(
            r#"{{ "schemaVersion": 1, "owner": "{}", "values": {{}} }}"#,
            ContentMod::declared_id()
        ))
        .expect("a legal document"),
    )
    .expect("the document names its own mod");
    pack.supply(
        path("data/example_pack/recipe/copper_lantern.json"),
        b"{\n  \"result\": \"content:copper_lantern\"\n}\n".to_vec(),
    );
    (pack, manifests, content)
}

/// Reads every file under `directory`, keyed by its path relative to it.
fn tree(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(&current)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", current.display()));
        for entry in entries {
            let entry = entry.expect("a readable directory entry");
            let target = entry.path();
            if target.is_dir() {
                pending.push(target);
                continue;
            }
            let relative = target
                .strip_prefix(directory)
                .expect("every path is under the directory")
                .to_string_lossy()
                .into_owned();
            found.insert(
                relative,
                fs::read(&target)
                    .unwrap_or_else(|error| panic!("cannot read {}: {error}", target.display())),
            );
        }
    }
    found
}

/// A scratch directory for one test, removed and recreated so it starts empty.
fn scratch(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ravel-pack-conformance-{}-{label}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a scratch directory");
    directory
}

/// Validates and writes the determinism pack into `directory`.
fn write_into(directory: &Path) -> ValidatedPack {
    let (pack, manifests, content) = every_kind_of_file();
    let validated = validate(&pack, &manifests, &content).expect("every closure holds");
    validated.write(directory).expect("the pack is writable");
    validated
}

#[test]
fn the_same_pack_written_by_two_processes_is_the_same_bytes() {
    // One test, two roles. With the variable set this is the child: it writes where
    // it was told and returns. Without it, it is the parent: it writes its own copy,
    // re-runs itself as a child, and compares the two trees byte for byte.
    //
    // A second process is what makes this more than a re-run. It has its own hash
    // seeds, its own address space and its own environment, so an output that
    // depended on any of them differs here and cannot differ in a same-process
    // comparison — which is exactly the gap M4-W2's review recorded.
    if let Ok(target) = std::env::var(WRITE_TO) {
        write_into(Path::new(&target));
        return;
    }

    let mine = scratch("parent");
    write_into(&mine);
    let theirs = scratch("child");

    let executable = std::env::current_exe().expect("a test binary has a path");
    let output = Command::new(&executable)
        .args(["--exact", WRITER, "--nocapture"])
        .env(WRITE_TO, &theirs)
        .output()
        .expect("the test binary runs");
    assert!(
        output.status.success(),
        "the child process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parent = tree(&mine);
    assert!(
        !parent.is_empty(),
        "the parent wrote nothing, so the comparison below would be vacuous"
    );
    assert_eq!(
        parent,
        tree(&theirs),
        "two processes wrote different bytes for the same pack"
    );

    let _ = fs::remove_dir_all(&mine);
    let _ = fs::remove_dir_all(&theirs);
}

#[test]
fn no_written_byte_names_the_machine_the_pack_was_written_on() {
    // The check that catches a machine constant baked into output. A pack is
    // compared across installations by its manifest, so a working directory, a user
    // name, a hostname, a temporary path or a year in any written byte makes two
    // builds of one pack differ for a reason that has nothing to do with the pack.
    let directory = scratch("machine");
    write_into(&directory);
    let written = tree(&directory);
    assert!(!written.is_empty(), "nothing was written to scan");

    let mut forbidden: Vec<String> = vec![
        directory.to_string_lossy().into_owned(),
        std::env::temp_dir().to_string_lossy().into_owned(),
        std::env::current_dir()
            .expect("a working directory")
            .to_string_lossy()
            .into_owned(),
        std::env::current_exe()
            .expect("a test binary has a path")
            .to_string_lossy()
            .into_owned(),
        std::process::id().to_string(),
    ];
    for variable in ["HOME", "USER", "LOGNAME", "HOSTNAME", "PWD", "TMPDIR"] {
        if let Ok(value) = std::env::var(variable)
            && !value.is_empty()
        {
            forbidden.push(value);
        }
    }

    for (relative, bytes) in &written {
        let text = String::from_utf8_lossy(bytes);
        for needle in &forbidden {
            assert!(
                !text.contains(needle.as_str()),
                "`{relative}` holds something about this machine rather than about the pack"
            );
        }
    }

    // And the scan itself is not vacuous: a needle this test would catch is a needle
    // it can find when it is really there.
    let planted = format!("{}\n", forbidden[0]);
    assert!(
        forbidden
            .iter()
            .any(|needle| planted.contains(needle.as_str())),
        "the scan cannot find a value it is looking for, so every assertion above passed vacuously"
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn every_written_path_is_either_the_games_layout_or_ravels_own() {
    // The separation `ravel_pack::RESERVED_PREFIX` exists for. What lands under
    // `data/` and `assets/` is this repository's belief about the 1.21.1 pack format
    // and is not checked against the pinned jar anywhere; what lands under the
    // reserved prefix is Ravel's own format. A written pack holding a third kind of
    // path would be a layout nobody decided.
    let (pack, manifests, content) = every_kind_of_file();
    let validated = validate(&pack, &manifests, &content).expect("every closure holds");

    for file in validated.files() {
        let written = file.path().as_str();
        assert!(
            written.starts_with("data/")
                || written.starts_with("assets/")
                || written.starts_with(RESERVED_PREFIX),
            "`{written}` is in neither the game's layout nor Ravel's own"
        );
    }
    assert!(
        validated
            .file(&path(&format!("{RESERVED_PREFIX}pack.json")))
            .is_some(),
        "the reproducible manifest `SPEC.md` §4.3 requires is one of the written files"
    );
}

#[test]
fn the_reproducible_manifest_holds_every_pin_and_nothing_else_about_the_build() {
    let (pack, manifests, content) = every_kind_of_file();
    let validated = validate(&pack, &manifests, &content).expect("every closure holds");
    let written = validated
        .file(&path(&format!("{RESERVED_PREFIX}pack.json")))
        .expect("the pack writes its manifest");
    let document: serde_json::Value =
        serde_json::from_slice(written.bytes()).expect("the manifest is JSON");

    let members = document
        .get("members")
        .and_then(serde_json::Value::as_object)
        .expect("the manifest lists its members");
    assert_eq!(
        members.keys().collect::<Vec<_>>(),
        pack.members()
            .map(|(member, _)| member.as_str())
            .collect::<Vec<_>>(),
        "the manifest's members are the pack's pins, derived rather than restated"
    );
    for (member, version) in pack.members() {
        assert_eq!(
            members
                .get(member.as_str())
                .and_then(serde_json::Value::as_str),
            Some(version.to_string().as_str())
        );
    }
    assert_eq!(
        document.get("id").and_then(serde_json::Value::as_str),
        Some(pack.id().as_str())
    );
}

#[test]
fn a_tag_the_pack_writes_is_the_layout_and_the_field_this_repository_believes_in() {
    // Asserted against this repository's belief and nothing more. The tag directory,
    // the `replace` field and its value are what `ravel-data` derives; whether
    // Minecraft 1.21.1 reads that file is not checked here or anywhere, because the
    // pinned jar is not a tracked artifact. This test exists so a change to the
    // belief is a decision rather than a silent drift, and its name says which it is.
    let (pack, manifests, content) = one_member();
    let validated = validate(&pack, &manifests, &content).expect("every closure holds");
    let tag: TagId = ContentMod::lanterns(ContentKind::Item);
    let written = validated
        .file(&tag.generated_path())
        .expect("the member's tag reached the pack");

    assert_eq!(
        tag.generated_path().as_str(),
        "data/c/tags/item/lanterns.json"
    );
    let document: serde_json::Value =
        serde_json::from_slice(written.bytes()).expect("a tag file is JSON");
    assert_eq!(
        document.get("replace").and_then(serde_json::Value::as_bool),
        Some(false),
        "Ravel never generates a tag that discards other mods' contributions"
    );
    assert!(
        document
            .get("values")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| !values.is_empty())
    );
}
