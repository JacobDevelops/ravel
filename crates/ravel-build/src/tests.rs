use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use ravel_core::{LifecycleState, MINECRAFT_VERSION_SELECTION, StructuredFailure};
use ravel_loader::{MANIFEST_FILE, PACKAGES_DIRECTORY, discover};
use ravel_package::Sha256Digest;
use sha2::{Digest as _, Sha256};

use super::generate::crate_ident;
use super::{BuildRefused, HOST_LIBRARY, WORKSPACE_DIRECTORY, build, plan};

const ENTRY_SYMBOL: &str = "Java_dev_ravel_jvm_Hook_ravelInitialize";

const EXPORTING_SOURCE: &str = r#"#![forbid(unsafe_code)]

//! A mod a player installed, built on the player's machine.

use ravel_sdk::prelude::*;
use ravel_sdk::{LifecycleState, Mod, ModError, declare_mod, export_mod};

/// The mod this package builds.
#[derive(Default)]
pub struct Installed;

/// Records, from inside this mod's own body, that it was entered.
fn record(mark: &str) {
    use std::io::Write as _;

    let Ok(path) = std::env::var("RAVEL_PROOF") else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{mark}");
}

#[declare_mod(id = "{id}")]
impl Mod for Installed {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        if state == LifecycleState::Running {
            record("running");
        }
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        let _ = registrar;
        record("register");
        Ok(())
    }
}

export_mod!(Installed::default());
"#;

const SILENT_SOURCE: &str = r#"#![forbid(unsafe_code)]

//! A mod that never says what a host should construct.

use ravel_sdk::{LifecycleState, Mod, ModError, declare_mod};

/// The mod this package builds.
#[derive(Default)]
pub struct Silent;

#[declare_mod(id = "{id}")]
impl Mod for Silent {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }
}
"#;

const RECORD: &[u8] = br#"{"origin":"example"}"#;

fn game() -> String {
    ravel_host_runtime::environment().minecraft
}

static NEXT: AtomicU32 = AtomicU32::new(0);

fn source(template: &str, mod_id: &str) -> String {
    template.replace("{id}", mod_id)
}

fn digest_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&hasher.finalize());
    Sha256Digest::from_bytes(digest).to_string()
}

fn instance(label: &str) -> PathBuf {
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ravel-build-{label}-{}-{unique}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale instance is removable");
    }
    fs::create_dir_all(root.join(PACKAGES_DIRECTORY)).expect("an instance is creatable");
    root
}

fn ravel_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels under the repository")
        .to_owned()
}

fn install(instance: &Path, directory: &str, id: &str, crate_name: &str, template: &str) {
    install_with(
        instance,
        directory,
        id,
        crate_name,
        template,
        "ravel-sdk.workspace = true\n",
    );
}

fn install_example(instance: &Path, name: &str) {
    let from = ravel_root().join("examples").join(name);
    let into = instance.join(PACKAGES_DIRECTORY).join(name);
    copy_tree(&from, &into);
}

fn install_with(
    instance: &Path,
    directory: &str,
    id: &str,
    crate_name: &str,
    template: &str,
    dependencies: &str,
) {
    let root = instance.join(PACKAGES_DIRECTORY).join(directory);
    fs::create_dir_all(root.join("src")).expect("a package directory is creatable");
    let source = source(template, id);
    let digest = digest_of(source.as_bytes());
    let record = digest_of(RECORD);
    fs::write(
        root.join(MANIFEST_FILE),
        format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{id}",
              "version": "1.0.0",
              "compatibility": {{ "ravel": ">=0.0.0", "minecraft": ["{game}"] }},
              "sides": ["client"],
              "authors": [{{ "name": "Ravel" }}],
              "license": "MIT",
              "source": {{
                "repository": "https://example.invalid/{id}",
                "files": [{{ "path": "src/lib.rs", "sha256": "{digest}" }}]
              }},
              "provenance": {{ "record": "provenance.json", "sha256": "{record}" }}
            }}"#,
            game = game()
        ),
    )
    .expect("manifest written");
    fs::write(root.join("src/lib.rs"), &source).expect("source written");
    fs::write(root.join("provenance.json"), RECORD).expect("record written");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\n\
             name = \"{crate_name}\"\n\
             version = \"1.0.0\"\n\
             edition.workspace = true\n\
             license.workspace = true\n\
             publish.workspace = true\n\
             rust-version.workspace = true\n\
             \n\
             [dependencies]\n\
             {dependencies}\n\
             [lints]\n\
             workspace = true\n"
        ),
    )
    .expect("cargo manifest written");
}

fn planned(instance: &Path) -> super::HostPlan {
    let discovered = discover(instance).expect("intact packages");
    plan(&discovered, &ravel_root()).expect("a plannable set")
}

#[test]
fn a_plan_over_one_instance_is_the_same_bytes_every_time() {
    let instance = instance("deterministic");
    install(
        &instance,
        "installed-1.0.0",
        "installed_mod",
        "installed-mod",
        EXPORTING_SOURCE,
    );
    install(
        &instance,
        "another-1.0.0",
        "another_mod",
        "another-mod",
        EXPORTING_SOURCE,
    );

    let first = planned(&instance);
    let second = planned(&instance);

    assert_eq!(first, second);
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.fingerprint().len(), 64);
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn the_generated_host_names_every_installed_package_in_mod_id_order() {
    let instance = instance("members");
    install(&instance, "zzz", "alpha_mod", "alpha-mod", EXPORTING_SOURCE);
    install(&instance, "aaa", "beta_mod", "beta-mod", EXPORTING_SOURCE);

    let plan = planned(&instance);
    let ids: Vec<&str> = plan
        .members()
        .iter()
        .map(|member| member.mod_id.as_str())
        .collect();
    assert_eq!(ids, ["alpha_mod", "beta_mod"]);

    let source = &plan.files()["host/src/lib.rs"];
    assert!(
        source.contains("alpha_mod::__ravel_entry(side)"),
        "{source}"
    );
    assert!(source.contains("beta_mod::__ravel_entry(side)"), "{source}");
    assert!(
        source.find("alpha_mod::").expect("named") < source.find("beta_mod::").expect("named"),
        "the member list is not in the order discovery answered in"
    );
    assert!(
        source.contains("include_str!(\"../../mods/zzz/package.json\")"),
        "{source}"
    );
    assert!(
        source.contains("profiling_requested: jni::sys::jboolean"),
        "{source}"
    );

    let agent = include_str!("../../../jvm/bootstrap/dev/ravel/jvm/Agent.java");
    assert!(agent.contains(
        "Hook.ravelInitialize(named, \"true\".equals(System.getProperty(\"ravel.profiling\")))"
    ));

    let workspace = &plan.files()["Cargo.toml"];
    assert!(workspace.contains("\"mods/zzz\""), "{workspace}");
    assert!(workspace.contains("\"mods/aaa\""), "{workspace}");
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn a_package_carrying_no_cargo_manifest_refuses_the_build() {
    let instance = instance("no-cargo");
    install(
        &instance,
        "installed-1.0.0",
        "installed_mod",
        "installed-mod",
        EXPORTING_SOURCE,
    );
    fs::remove_file(
        instance
            .join(PACKAGES_DIRECTORY)
            .join("installed-1.0.0/Cargo.toml"),
    )
    .expect("the manifest is removable");

    let discovered = discover(&instance).expect("an intact package");
    let refused = plan(&discovered, &ravel_root()).expect_err("nothing says what to build");
    assert_eq!(
        refused,
        BuildRefused::MissingCargoManifest {
            directory: "installed-1.0.0".to_owned()
        }
    );
    assert!(refused.to_string().contains("Cargo.toml"));
    assert_eq!(refused.lifecycle_state(), LifecycleState::Validated);
    assert_eq!(refused.failure_kind(), None);
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn a_cargo_manifest_that_names_no_crate_refuses_the_build() {
    let instance = instance("nameless");
    install(
        &instance,
        "installed-1.0.0",
        "installed_mod",
        "installed-mod",
        EXPORTING_SOURCE,
    );
    fs::write(
        instance
            .join(PACKAGES_DIRECTORY)
            .join("installed-1.0.0/Cargo.toml"),
        "[dependencies]\nravel-sdk.workspace = true\n",
    )
    .expect("the manifest is writable");

    let discovered = discover(&instance).expect("an intact package");
    let refused = plan(&discovered, &ravel_root()).expect_err("no crate is named");
    assert!(
        matches!(refused, BuildRefused::CargoManifest { .. }),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("package.name"), "{refused}");
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn two_packages_building_one_crate_refuse_the_build() {
    let instance = instance("clash");
    install(
        &instance,
        "first",
        "first_mod",
        "shared-name",
        EXPORTING_SOURCE,
    );
    install(
        &instance,
        "second",
        "second_mod",
        "shared-name",
        EXPORTING_SOURCE,
    );

    let discovered = discover(&instance).expect("two intact packages");
    let refused = plan(&discovered, &ravel_root()).expect_err("one crate name, two packages");
    let BuildRefused::DuplicateCrate {
        crate_name,
        directory,
        first,
    } = &refused
    else {
        panic!("the refusal should name the clash: {refused:?}");
    };
    assert_eq!(crate_name, "shared-name");
    assert_eq!(directory, "second");
    assert_eq!(first, "first");
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn the_installed_set_is_compiled_into_a_host_that_runs_the_packages_it_names() {
    let cache = std::env::temp_dir().join(format!("ravel-build-cache-{}", std::process::id()));
    let outcome = std::panic::catch_unwind(|| compiling_proof(&cache));
    let cleanup = fs::remove_dir_all(&cache);
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    cleanup.expect("remove the build cache");
}

fn compiling_proof(cache: &Path) {
    let instance = instance("built");
    install(
        &instance,
        "installed-1.0.0",
        "installed_mod",
        "installed-mod",
        EXPORTING_SOURCE,
    );

    install_example(&instance, "content-mod");

    let plan = planned(&instance);
    let library = build(&plan, &instance, cache, &game()).expect("the installed package compiles");

    assert!(library.is_file(), "{}", library.display());
    assert_eq!(
        library.file_name().and_then(std::ffi::OsStr::to_str),
        Some(format!("lib{HOST_LIBRARY}.so").as_str())
    );

    let bytes = fs::read(&library).expect("the built library is readable");
    assert!(
        holds(&bytes, ENTRY_SYMBOL),
        "the built host exports no `{ENTRY_SYMBOL}`"
    );

    drives_the_package_it_was_built_from(&instance, cache);

    let written = generated_times(&plan, &instance);
    let again = build(&plan, &instance, cache, &game()).expect("an unchanged instance rebuilds");
    assert_eq!(again, library);
    assert_eq!(
        written,
        generated_times(&plan, &instance),
        "an unchanged instance rewrote its generated source, which makes every launch cold"
    );

    a_second_instance_keeps_its_own_host(&library, cache);
    the_examples_this_repository_ships_install_and_compile(cache);
    a_package_naming_internal_workspace_dependencies_refuses(cache);
    a_registry_mod_with_explicit_third_party_dependencies_compiles(cache);
    a_package_that_names_no_value_refuses(cache);
    a_version_naming_no_tracked_directory_refuses(&plan, &instance, cache);
    fs::remove_dir_all(&instance).expect("cleanup");
}

fn the_examples_this_repository_ships_install_and_compile(cache: &Path) {
    let instance = instance("examples");
    let examples = ravel_root().join("examples");
    let mut installed = Vec::new();
    let mut entries = fs::read_dir(&examples)
        .expect("the repository has an examples directory")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.join(MANIFEST_FILE).is_file())
        .collect::<Vec<_>>();
    entries.sort();
    for source in &entries {
        let name = source
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .expect("an example with a portable name")
            .to_owned();
        copy_tree(source, &instance.join(PACKAGES_DIRECTORY).join(&name));
        installed.push(name);
    }
    assert!(
        installed.len() >= 3,
        "the examples that ship a manifest are {installed:?}, which is fewer than the three \
         the guide names"
    );

    let plan = planned(&instance);
    let library = build(&plan, &instance, cache, &game())
        .expect("every example this repository ships compiles as an installed package");

    let bytes = fs::read(&library).expect("the built library is readable");
    for member in plan.members() {
        assert!(
            holds(&bytes, &crate_ident(&member.crate_name)),
            "the built host does not name `{}`",
            member.crate_name
        );
    }
    fs::remove_dir_all(&instance).expect("cleanup");
}

const DRIVER_SOURCE: &str = "fn main() {\n\
         \x20   let game = std::env::args().nth(1).expect(\"a game directory\");\n\
         \x20   std::env::set_current_dir(&game).expect(\"the game directory exists\");\n\
         \x20   ravel_host_runtime::initialize(\n\
         \x20       ravel_core::Side::Client,\n\
         \x20       || ravel_bootstrap::members(ravel_core::Side::Client),\n\
         \x20       false,\n\
         \x20   ).expect(\"the generated host initializes\");\n\
         \x20   let world = std::path::Path::new(&game).join(\"saves\").join(\"Driven\");\n\
         \x20   std::fs::create_dir_all(&world).expect(\"a world directory\");\n\
         \x20   assert_eq!(\n\
         \x20       ravel_bootstrap_runtime::stage(world.to_str().expect(\"world path\"), None, true),\n\
         \x20       ravel_bootstrap_runtime::OpenOutcome::Staged,\n\
         \x20   );\n\
         }\n";

fn the_written_packs_hold_the_examples_content(instance: &Path) {
    let assets = instance.join("resourcepacks/ravel/assets/content");
    assert_eq!(
        fs::read_to_string(assets.join("models/block/copper_lantern.json"))
            .unwrap_or_else(|error| panic!("the model reached no pack: {error}")),
        "{\n  \"parent\": \"minecraft:block/lantern\"\n}\n"
    );
    for (path, expected) in [
        (
            "blockstates/copper_lantern.json",
            "\"model\": \"content:block/copper_lantern\"",
        ),
        (
            "models/item/copper_lantern.json",
            "\"parent\": \"content:block/copper_lantern\"",
        ),
        ("items/copper_lantern.json", "\"type\": \"minecraft:model\""),
    ] {
        let written = fs::read_to_string(assets.join(path))
            .unwrap_or_else(|error| panic!("`{path}` reached no pack: {error}"));
        assert!(written.contains(expected), "`{path}`: {written}");
    }

    let english = fs::read_to_string(assets.join("lang/en_us.json"))
        .unwrap_or_else(|error| panic!("the translations reached no pack: {error}"));
    assert!(english.contains("Copper Lantern"), "{english}");
    assert!(
        assets.join("lang/fr_fr.json").is_file(),
        "one locale reached the pack and the other did not"
    );

    let data = instance.join("saves/Driven/datapacks/ravel/data");
    for tag in ["block/lanterns.json", "item/lanterns.json"] {
        let written = fs::read_to_string(data.join("c/tags").join(tag))
            .unwrap_or_else(|error| panic!("`{tag}` reached no world pack: {error}"));
        assert!(written.contains("content:copper_lantern"), "{written}");
    }
    let loot = fs::read_to_string(data.join("content/loot_table/blocks/copper_lantern.json"))
        .unwrap_or_else(|error| panic!("the loot table reached no world pack: {error}"));
    assert!(loot.contains("minecraft:survives_explosion"), "{loot}");
    assert!(loot.contains("content:copper_lantern"), "{loot}");
    let recipe = fs::read_to_string(data.join("content/recipe/crafting/copper_lantern.json"))
        .unwrap_or_else(|error| panic!("the recipe reached no world pack: {error}"));
    assert!(recipe.contains("minecraft:crafting_shaped"), "{recipe}");
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("a package directory is creatable");
    for entry in fs::read_dir(source).expect("a readable package directory") {
        let entry = entry.expect("a readable directory entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("a readable entry kind").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a readable package file");
        }
    }
}

fn a_version_naming_no_tracked_directory_refuses(
    plan: &super::HostPlan,
    instance: &Path,
    cache: &Path,
) {
    let refused = build(plan, instance, cache, "no-tracked-directory-carries-this")
        .expect_err("the selected version names no tracked directory");
    let BuildRefused::Compile { detail } = &refused else {
        panic!("the refusal should be the toolchain's: {refused:?}");
    };
    assert!(
        detail.contains(MINECRAFT_VERSION_SELECTION),
        "the refusal does not name the selection: {detail}"
    );
    assert!(
        detail.contains("no-tracked-directory-carries-this"),
        "the refusal does not name the version that was asked for: {detail}"
    );
    assert!(
        detail.contains(&game()),
        "the refusal does not name a version that is tracked: {detail}"
    );
}

fn write_driver(instance: &Path) -> PathBuf {
    let driver = instance.join("driver");
    fs::create_dir_all(driver.join("src")).expect("a driver directory is creatable");
    let root = ravel_root();
    fs::write(
        driver.join("Cargo.toml"),
        format!(
            "[workspace]\n\
             \n\
             [package]\n\
             name = \"ravel-build-driver\"\n\
             version = \"0.0.0\"\n\
             edition = \"2024\"\n\
             \n\
             [dependencies]\n\
             ravel-core = {{ path = \"{root}/crates/ravel-core\" }}\n\
             ravel-generated-host = {{ path = \"../ravel/host\" }}\n\
             ravel-host-runtime = {{ path = \"{root}/crates/ravel-host-runtime\" }}\n\
             ravel-bootstrap-runtime = {{ path = \"{root}/crates/ravel-bootstrap-runtime\" }}\n",
            root = root.display()
        ),
    )
    .expect("the driver manifest is writable");
    fs::write(driver.join("src/main.rs"), DRIVER_SOURCE).expect("the driver source is writable");
    driver
}

fn drives_the_package_it_was_built_from(instance: &Path, cache: &Path) {
    let driver = write_driver(instance);

    let built = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(driver.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(cache)
        .output()
        .expect("cargo runs");
    assert!(
        built.status.success(),
        "the driver did not build: {}",
        String::from_utf8_lossy(&built.stderr)
    );

    fs::write(
        instance.join("options.txt"),
        "resourcePacks:[\"vanilla\"]\n",
    )
    .expect("initial player settings");
    let marks = instance.join("marks");
    let run = Command::new(cache.join("release/ravel-build-driver"))
        .arg(instance)
        .env("RAVEL_PROOF", &marks)
        .output()
        .expect("the driver runs");
    assert!(
        run.status.success(),
        "the generated host refused to compose: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    the_written_packs_hold_the_examples_content(instance);
    let settings = fs::read_to_string(instance.join("options.txt")).expect("seeded settings");
    assert_eq!(
        settings,
        format!(
            "resourcePacks:[\"vanilla\",\"{}\"]\n",
            ravel_loader::pack_id()
        )
    );
    assert!(instance.join("ravel/seeded-packs").is_file());

    let recorded = fs::read_to_string(&marks).expect("the installed mod recorded nothing");
    assert!(
        recorded.contains("register"),
        "a package no Ravel developer compiled never reached `register`: {recorded}"
    );
    assert!(
        recorded.contains("running"),
        "a package no Ravel developer compiled never reached `running`: {recorded}"
    );
}

fn a_second_instance_keeps_its_own_host(first: &Path, cache: &Path) {
    let second = instance("second-instance");
    install(
        &second,
        "beta-1.0.0",
        "beta_mod",
        "beta-mod",
        EXPORTING_SOURCE,
    );

    let beta =
        build(&planned(&second), &second, cache, &game()).expect("the second instance builds");
    assert_ne!(first, beta);

    let bytes = fs::read(first).expect("the first instance's host is readable");
    assert!(
        holds(&bytes, "installed_mod"),
        "the first host lost its own mod"
    );
    assert!(
        !holds(&bytes, "beta_mod"),
        "the first instance was handed the second instance's host"
    );
    fs::remove_dir_all(&second).expect("cleanup");
}

fn a_package_that_names_no_value_refuses(cache: &Path) {
    let instance = instance("silent");
    install(
        &instance,
        "installed-1.0.0",
        "installed_mod",
        "installed-mod",
        EXPORTING_SOURCE,
    );
    install(
        &instance,
        "silent-1.0.0",
        "silent_mod",
        "silent-mod",
        SILENT_SOURCE,
    );

    let plan = planned(&instance);
    let refused = build(&plan, &instance, cache, &game()).expect_err("one package names no value");
    let BuildRefused::Compile { detail } = &refused else {
        panic!("the refusal should be the compiler's: {refused:?}");
    };
    assert!(detail.contains("__ravel_entry"), "{detail}");
    assert!(detail.contains("silent_mod"), "{detail}");
    assert!(
        !detail.contains(&instance.display().to_string()),
        "the refusal names the instance path"
    );
    assert!(detail.contains("<instance>"), "{detail}");
    assert!(refused.quotes_untrusted_text());
    fs::remove_dir_all(&instance).expect("cleanup");
}

fn holds(bytes: &[u8], needle: &str) -> bool {
    bytes
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn generated_times(plan: &super::HostPlan, instance: &Path) -> Vec<std::time::SystemTime> {
    plan.files()
        .keys()
        .map(|path| {
            fs::metadata(instance.join(WORKSPACE_DIRECTORY).join(path))
                .and_then(|meta| meta.modified())
                .expect("a generated file was written")
        })
        .collect()
}

#[test]
fn generated_dependency_tables_match_the_installation_and_expose_only_the_sdk() {
    let instance = instance("dependency-boundary");
    let plan = planned(&instance);
    let root: toml::Table = fs::read_to_string(ravel_root().join("Cargo.toml"))
        .expect("installation manifest")
        .parse()
        .expect("valid installation manifest");
    let workspace: toml::Table = plan.files()["Cargo.toml"].parse().expect("valid workspace");
    let host: toml::Table = plan.files()["host/Cargo.toml"].parse().expect("valid host");
    let dependencies = workspace["workspace"]["dependencies"]
        .as_table()
        .expect("dependencies");
    assert_eq!(
        dependencies.keys().map(String::as_str).collect::<Vec<_>>(),
        ["ravel-sdk", "serde", "serde_json"]
    );
    for name in ["serde", "serde_json"] {
        assert_eq!(dependencies[name], root["workspace"]["dependencies"][name]);
    }
    assert_eq!(
        host["dependencies"]["jni"],
        root["workspace"]["dependencies"]["jni"]
    );
    for name in ["ravel-core", "ravel-host-runtime"] {
        let path = ravel_root().join("crates").join(name);
        assert_eq!(host["dependencies"][name]["path"].as_str(), path.to_str());
        assert!(host["dependencies"][name].get("workspace").is_none());
        assert!(!dependencies.contains_key(name));
    }
    assert!(
        host["dependencies"]
            .get("ravel-bootstrap-runtime")
            .is_none()
    );
    assert_eq!(
        plan.installation_lock(),
        fs::read_to_string(ravel_root().join("Cargo.lock")).expect("installation lock")
    );
    assert!(!plan.files().contains_key("Cargo.lock"));
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn changing_the_installation_lock_changes_the_plan_fingerprint() {
    let instance = instance("lock-fingerprint");
    let root = instance.join("installation");
    fs::create_dir(&root).expect("installation directory");
    fs::copy(ravel_root().join("Cargo.toml"), root.join("Cargo.toml")).expect("manifest copy");
    let lock = fs::read_to_string(ravel_root().join("Cargo.lock")).expect("installation lock");
    fs::write(root.join("Cargo.lock"), &lock).expect("lock copy");
    let discovered = discover(&instance).expect("empty instance");
    let first = plan(&discovered, &root).expect("first plan");
    fs::write(
        root.join("Cargo.lock"),
        format!("{lock}\n# changed installation\n"),
    )
    .expect("changed lock");
    let second = plan(&discovered, &root).expect("second plan");
    assert_ne!(first.fingerprint(), second.fingerprint());
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn a_malformed_installation_lock_is_refused_as_an_installation_input() {
    let instance = instance("malformed-lock");
    let root = instance.join("installation");
    fs::create_dir(&root).expect("installation directory");
    fs::copy(ravel_root().join("Cargo.toml"), root.join("Cargo.toml")).expect("manifest copy");
    let discovered = discover(&instance).expect("empty instance");
    for lock in ["[[package]", "version = 4\n"] {
        fs::write(root.join("Cargo.lock"), lock).expect("malformed lock");
        let refused = plan(&discovered, &root).expect_err("the seed lock is unusable");
        assert!(
            matches!(&refused, BuildRefused::Installation { file, .. } if file == "Cargo.lock"),
            "{refused:?}"
        );
    }
    fs::remove_dir_all(&instance).expect("cleanup");
}

#[test]
fn generated_files_keep_odd_directory_and_crate_names_as_data() {
    let directory = "odd\"]\n[patch.crates-io]\\name";
    let member = super::PlannedMember {
        mod_id: "odd_mod".to_owned(),
        directory: directory.to_owned(),
        crate_name: "odd \"crate\"\\name".to_owned(),
    };
    let members = [member.clone()];
    let workspace: toml::Table =
        super::generate::workspace_manifest(&members, &ravel_root(), &toml::Table::new())
            .parse()
            .expect("valid workspace");
    assert!(!workspace.contains_key("patch"));
    assert_eq!(
        workspace["workspace"]["members"][1].as_str(),
        Some(format!("mods/{directory}").as_str())
    );
    let host: toml::Table =
        super::generate::host_manifest(&members, &ravel_root(), &toml::Table::new())
            .parse()
            .expect("valid host");
    assert!(!host.contains_key("patch"));
    assert_eq!(
        host["dependencies"][&member.crate_name]["path"].as_str(),
        Some(format!("../mods/{directory}").as_str())
    );
    let source = super::generate::host_source(&members);
    let literal = format!("{:?}", format!("../../mods/{directory}/package.json"));
    assert!(
        source.contains(&format!("include_str!({literal})")),
        "{source}"
    );
}

#[test]
fn lock_adjustment_allows_local_members_but_refuses_unvetted_external_entries() {
    let seed = "version = 4\n\
        [[package]]\nname = \"dependency\"\nversion = \"1.0.0\"\n\
        source = \"registry+https://example.invalid/index\"\nchecksum = \"vetted\"\n";
    let adjusted = format!(
        "{seed}\n[[package]]\nname = \"ravel-generated-host\"\nversion = \"0.0.0\"\n\
         dependencies = [\"installed-mod\"]\n\
         [[package]]\nname = \"installed-mod\"\nversion = \"1.0.0\"\n\
         dependencies = [\"dependency\"]\n"
    );
    let mods = std::collections::BTreeMap::from([("installed-mod", "installed_mod")]);
    super::compile::vetted_dependencies(seed, &adjusted, &mods).expect("only local graph changes");
    for changed in [
        adjusted.replacen("version = \"1.0.0\"", "version = \"2.0.0\"", 1),
        adjusted.replace("checksum = \"vetted\"", "checksum = \"other\""),
        adjusted.replace("example.invalid/index", "other.invalid/index"),
        adjusted.replace("\"dependency\"", "\"unvetted\""),
    ] {
        let refused = super::compile::vetted_dependencies(seed, &changed, &mods)
            .expect_err("an external dependency differs from the installation lock");
        let BuildRefused::UnpinnedDependency {
            mods: requiring, ..
        } = &refused
        else {
            panic!("the refusal should name an unpinned dependency: {refused:?}");
        };
        assert_eq!(requiring, &["installed_mod"]);
        assert!(refused.to_string().contains("installed_mod"), "{refused}");
    }
    let refused = super::compile::vetted_dependencies(seed, "[[package]", &mods)
        .expect_err("the resolved lock is malformed");
    assert!(
        matches!(refused, BuildRefused::ResolvedLock { .. }),
        "{refused:?}"
    );
}

#[test]
fn path_dependencies_keep_the_sdk_boundary_even_when_seeded() {
    let mods = std::collections::BTreeMap::from([("installed-mod", "installed_mod")]);
    for name in ["ravel-sdk", "ravel-host-api", "serde"] {
        let path_package = format!("[[package]]\nname = \"{name}\"\nversion = \"1.0.0\"\n");
        let resolved = format!(
            "{path_package}\n[[package]]\nname = \"installed-mod\"\nversion = \"1.0.0\"\n\
             dependencies = [\"{name} 1.0.0\"]\n"
        );
        let result = super::compile::vetted_dependencies(&path_package, &resolved, &mods);
        if name == "ravel-sdk" {
            result.expect("the seed SDK is the public path boundary");
        } else {
            let refused = result.expect_err("even seeded paths cannot expose host internals");
            assert!(
                matches!(&refused, BuildRefused::UnpinnedDependency { package, mods, .. }
                    if package == name && mods == &["installed_mod"]),
                "{refused:?}"
            );
        }
        let refused = super::compile::vetted_dependencies(
            "[[package]]\nname = \"seed\"\nversion = \"1.0.0\"\n",
            &resolved,
            &mods,
        )
        .expect_err("a path package absent from the seed is unvetted");
        assert!(
            matches!(&refused, BuildRefused::UnpinnedDependency { package, .. } if package == name),
            "{refused:?}"
        );
    }
    let resolved = "[[package]]\nname = \"installed-mod\"\nversion = \"1.0.0\"\n\
        dependencies = [\"other-mod\"]\n\
        [[package]]\nname = \"other-mod\"\nversion = \"1.0.0\"\n";
    let mods = std::collections::BTreeMap::from([
        ("installed-mod", "installed_mod"),
        ("other-mod", "other_mod"),
    ]);
    super::compile::vetted_dependencies(
        "[[package]]\nname = \"seed\"\nversion = \"1.0.0\"\n",
        resolved,
        &mods,
    )
    .expect("planned members may depend on one another");
}

fn a_package_naming_internal_workspace_dependencies_refuses(cache: &Path) {
    let instance = instance("internal-dependency");
    install_with(
        &instance,
        "internal",
        "internal_mod",
        "internal-mod",
        EXPORTING_SOURCE,
        "ravel-sdk.workspace = true\nravel-host-api.workspace = true\n",
    );
    let refused = build(&planned(&instance), &instance, cache, &game())
        .expect_err("the workspace does not lend host internals to mods");
    assert!(refused.to_string().contains("ravel-host-api"), "{refused}");
    assert!(
        refused.to_string().contains("workspace.dependencies"),
        "{refused}"
    );
    fs::remove_dir_all(&instance).expect("cleanup");
}

fn a_registry_mod_with_explicit_third_party_dependencies_compiles(cache: &Path) {
    let instance = instance("registry-numeric-dependency");
    // A prior build leaves a lock from which Cargo pruned every unused pin.
    install(
        &instance,
        "first",
        "first_mod",
        "first-mod",
        EXPORTING_SOURCE,
    );
    build(&planned(&instance), &instance, cache, &game()).expect("the first set builds");
    let source = ravel_root().join("registry/mods/rvl-120-counterpart");
    let manifest: toml::Table = fs::read_to_string(source.join("Cargo.toml"))
        .expect("registry manifest")
        .parse()
        .expect("valid registry manifest");
    assert!(
        manifest["dependencies"]["num-traits"]
            .get("workspace")
            .is_none()
    );
    copy_tree(
        &source,
        &instance
            .join(PACKAGES_DIRECTORY)
            .join("rvl-120-counterpart"),
    );
    let plan = planned(&instance);
    let library = build(&plan, &instance, cache, &game())
        .expect("the standalone numeric dependency survives installation");
    assert!(library.is_file());
    let lock = fs::read_to_string(instance.join(WORKSPACE_DIRECTORY).join("Cargo.lock"))
        .expect("resolved lock");
    assert!(lock.contains("name = \"ravel-generated-host\""));
    assert!(lock.contains("name = \"rvl-120-counterpart\""));
    let mods = plan
        .members()
        .iter()
        .map(|member| (member.crate_name.as_str(), member.mod_id.as_str()))
        .collect();
    super::compile::vetted_dependencies(plan.installation_lock(), &lock, &mods)
        .expect("the installed host uses only vetted external dependencies");
    let pinned = |document: &str| {
        super::compile::lock_packages(document)
            .expect("a lock")
            .into_iter()
            .find(|package| package.get("name").and_then(toml::Value::as_str) == Some("num-traits"))
            .and_then(|package| package.get("version").cloned())
    };
    assert_eq!(pinned(&lock), pinned(plan.installation_lock()));
    assert!(
        pinned(&lock).is_some(),
        "the later dependency was resolved from the seed pin"
    );
    fs::remove_dir_all(&instance).expect("cleanup");
}
