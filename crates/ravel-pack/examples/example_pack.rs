#![forbid(unsafe_code)]

//! Declares, validates, and reports an example pack.
//!
//! Pass a directory to write it: `cargo run -p ravel-pack --example example_pack -- /tmp/example-pack`.
//! With no argument, lists the validated files without writing them.

use std::path::Path;
use std::process::ExitCode;

use ravel_config::{ConfigurationDeclaration, ValueDocument};
use ravel_core::{ContentId, ContentKind, ModId, RelativePath, Version};
use ravel_data::{ContentDeclaration, TagId};
use ravel_pack::{PackDeclaration, ValidatedPack, validate};
use ravel_package::PackageManifest;

/// The pack's own namespace, which its progression steps are named in.
const PACK: &str = "example_pack";

/// The two mods this pack selects. A pack pins exact versions, never ranges.
const MEMBERS: [(&str, Version); 2] = [
    ("copper_mod", Version::new(1, 4, 0)),
    ("lantern_mod", Version::new(2, 0, 1)),
];

fn main() -> ExitCode {
    let pack = declare();
    let manifests: Vec<PackageManifest> = MEMBERS
        .iter()
        .map(|(member, version)| manifest(member, version))
        .collect();
    let content = vec![copper(), lanterns()];
    let configurations = vec![brightness()];

    let validated = match validate(&pack, &manifests, &content, &configurations) {
        Ok(validated) => validated,
        Err(refused) => {
            // A pack author's build stops here, and the sentence names what to fix.
            eprintln!("this pack does not hold together: {refused}");
            return ExitCode::FAILURE;
        }
    };

    report(&validated);
    match std::env::args().nth(1) {
        Some(directory) => match validated.write(Path::new(&directory)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(refused) => {
                eprintln!("{refused}");
                ExitCode::FAILURE
            }
        },
        None => ExitCode::SUCCESS,
    }
}

/// Declares the pack: its pins, its overrides, its own files, and its progression.
fn declare() -> PackDeclaration {
    let mut pack = PackDeclaration::new(id(PACK), Version::new(0, 2, 1));
    for (member, version) in MEMBERS {
        pack.pin(id(member), version)
            .expect("each member is pinned once");
    }

    pack.configure(
        id("lantern_mod"),
        ValueDocument::parse(
            r#"{
              "schemaVersion": 1,
              "owner": "lantern_mod",
              "values": { "lantern.brightness": 12 }
            }"#,
        )
        .expect("a legal override document"),
    )
    .expect("the document names the mod it is filed under");

    // References are explicit; resource closure does not inspect recipe bytes.
    pack.supply(
        path("data/example_pack/recipe/copper_lantern.json"),
        b"{\n  \"result\": \"lantern_mod:copper_lantern\"\n}\n".to_vec(),
    );
    pack.supply(
        path("assets/example_pack/textures/gui/progress.png"),
        b"\x89PNG\r\n\x1a\n".to_vec(),
    );
    pack.refer_to(
        path("assets/example_pack/textures/gui/progress.png"),
        content(PACK, "smelting"),
    );

    let smelting = content(PACK, "smelting");
    pack.progression_mut()
        .declare(smelting.clone(), [], None)
        .expect("a root step");
    pack.progression_mut()
        .declare(
            content(PACK, "illumination"),
            [smelting],
            Some(ravel_core::RegistryKey::new(
                ContentKind::Item,
                content("lantern_mod", "copper_lantern"),
            )),
        )
        .expect("a step gated on an entry a member declares");

    pack
}

/// What `lantern_mod` declares a pack may state, which is what checks the layer.
fn brightness() -> ConfigurationDeclaration {
    ConfigurationDeclaration::parse(
        r#"{
          "schemaVersion": 1,
          "owner": "lantern_mod",
          "version": 1,
          "settings": [
            {
              "key": "lantern.brightness",
              "valueSchema": { "type": "integer", "minimum": 0, "maximum": 15 },
              "default": 8,
              "side": "common",
              "restartRequired": false,
              "packOverride": "locked"
            }
          ]
        }"#,
    )
    .expect("a legal configuration declaration")
}

/// What `copper_mod` contributes: one ingot, in the shared ingot tag.
fn copper() -> ContentDeclaration {
    let mut declaration = ContentDeclaration::new(id("copper_mod"));
    let ingot = declaration
        .entry(ContentKind::Item, "copper_ingot")
        .expect("a legal entry");
    declaration
        .tag(&ingots(), &ingot)
        .expect("an item joins an item tag");
    declaration
        .translation("en_us", "item.copper_mod.copper_ingot", "Copper Ingot")
        .expect("a legal translation");
    declaration
}

/// The lantern mod's item, contributing to the same shared tag as the copper mod.
fn lanterns() -> ContentDeclaration {
    let mut declaration = ContentDeclaration::new(id("lantern_mod"));
    let lantern = declaration
        .entry(ContentKind::Item, "copper_lantern")
        .expect("a legal entry");
    declaration
        .tag(&ingots(), &lantern)
        .expect("an item joins an item tag");
    declaration
        .translation("en_us", "item.lantern_mod.copper_lantern", "Copper Lantern")
        .expect("a legal translation");
    declaration
}

/// The shared convention tag both members contribute to.
fn ingots() -> TagId {
    TagId::new(ContentKind::Item, content("c", "ingots")).expect("items are taggable")
}

/// Prints what the pack writes and what it took from a member.
fn report(validated: &ValidatedPack) {
    println!("the pack writes:");
    for file in validated.files() {
        println!("  {}", file.path().as_str());
    }
    for taken in validated.overrides() {
        println!("the pack supplies {taken}");
    }
}

/// One member's manifest, as a pack author would read it out of the package.
fn manifest(owner: &str, version: &Version) -> PackageManifest {
    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    // `lantern_mod` requires `copper_mod`, so the pack's dependency closure has
    // something to be about: dropping the pin below makes `validate` refuse.
    let dependencies = if owner == "lantern_mod" {
        r#"[{ "id": "copper_mod", "relationship": "required", "version": "^1.0.0" }]"#
    } else {
        "[]"
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

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn content(namespace: &str, path: &str) -> ContentId {
    ContentId::new(id(namespace), path).expect("a legal content path")
}

fn path(text: &str) -> RelativePath {
    RelativePath::try_from(text.to_owned()).expect("a legal relative path")
}
