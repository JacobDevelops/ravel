#![forbid(unsafe_code)]

//! Generates bindings and Java sources from pinned version data.
//!
//! Available jars must reproduce the tracked locks; jar-less builds verify their hashes.
//! [`adapter_facts`] exposes the same typed admission policy to repository tooling,
//! without loading generation documents or jars. Explicit generation renders once;
//! repeatability of full composed outputs is covered by public-fixture tests.

mod classfile;
mod error;
mod inputs;
mod jar;
mod java_type;
mod mapping;
mod render;
mod resolve;
mod roles;
mod stubs;
mod templates;

#[cfg(test)]
mod test_fixtures;

use ravel_core::Sha256Digest;
use ravel_core::paths::HOST_LIBRARY;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub use classfile::{descriptor_classes, descriptor_handle_count, descriptor_parameters};
pub use error::GenerateError;
pub use inputs::{
    AdapterFacts, Eligibility, HookFact, HookInventory, Kind, adapter_facts, hook_inventory,
    hook_inventory_at, lock_paths_at,
};

/// The reading of the jar this generator performs, as recorded in the lock.
const EXTRACTOR: &str = "classfile";

/// Environment variable selecting the version at build time.
pub use ravel_core::MINECRAFT_VERSION_SELECTION as SELECTION;

/// The agent's Class-File API requires Java 24, independently of the game's runtime.
const AGENT_RELEASE_FLOOR: u16 = 24;

/// Reads a UTF-8 version selection without treating an invalid value as absence.
///
/// # Errors
///
/// Returns an error when the value is present but is not valid UTF-8.
pub fn selection(raw: Option<OsString>) -> Result<Option<String>, GenerateError> {
    raw.map(OsString::into_string).transpose().map_err(|_| {
        GenerateError::Input(format!(
            "{SELECTION} is not valid UTF-8, so it names no version directory"
        ))
    })
}

/// Java class holding the generated native declarations.
const HOOK_CLASS: &str = "dev.ravel.jvm.Hook";

/// Absolute library path property, avoiding launcher-managed `java.library.path`.
const LIBRARY_PROPERTY: &str = "ravel.host";

/// The root of the generated Java tree, which this generator owns outright.
const JAVA_ROOT: &str = "target/ravel-jvm/generated";

/// The file one generation holds while it empties and refills the output trees.
const OUTPUT_LOCK: &str = "target/ravel-jvm/.generating";

/// How long a generation waits for another to finish before it gives up.
const LOCK_WAIT_MAX: std::time::Duration = std::time::Duration::from_mins(1);

/// How old a lock has to be before it is read as left behind rather than held.
const LOCK_STALE: std::time::Duration = std::time::Duration::from_mins(5);

const LOCK_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

/// Serializes output sweeps and inspections; downstream compilers rely on task ordering.
///
/// Refreshing after writes gives inspections a fresh stale-lock interval.
struct OutputLock {
    path: PathBuf,
}

impl OutputLock {
    /// Waits for the trees, or reports who is holding them.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock cannot be created, or when another
    /// generation held it for longer than a build should take.
    fn take(root: &Path) -> Result<Self, GenerateError> {
        let path = root.join(OUTPUT_LOCK);
        if let Some(parent) = path.parent() {
            create_directory(parent)?;
        }
        let waited = std::time::Instant::now();
        loop {
            match fs::File::create_new(&path) {
                Ok(_) => return Ok(Self { path }),
                Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => {
                    return Err(GenerateError::io(
                        format!(
                            "output lock {OUTPUT_LOCK} (creation failed; check the directory permissions)"
                        ),
                        error,
                    ));
                }
                Err(_) => {}
            }
            if waited.elapsed() > LOCK_WAIT_MAX {
                return Err(GenerateError::Input(format!(
                    "another build has held {OUTPUT_LOCK} for longer than a build should take; if none is \
                     running, delete it"
                )));
            }
            let stale = fs::metadata(&path)
                .and_then(|held| held.modified())
                .is_ok_and(|held| held.elapsed().is_ok_and(|age| age > LOCK_STALE));
            if stale {
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    // Another waiter may already have removed the same stale lock.
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(GenerateError::io(
                            format!(
                                "stale output lock {OUTPUT_LOCK} (removal failed; if no build is running, delete it)"
                            ),
                            error,
                        ));
                    }
                }
                continue;
            }
            std::thread::sleep(LOCK_POLL_INTERVAL);
        }
    }

    /// Re-dates the existing lock, refusing to recreate a removed one.
    fn refresh(&self) -> Result<(), GenerateError> {
        fs::File::options()
            .write(true)
            .open(&self.path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()))
            .map_err(|error| {
                GenerateError::io(
                    format!("output lock {OUTPUT_LOCK} (could not be re-dated; keep the held lock in place)"),
                    error,
                )
            })
    }

    /// Refuses a lock covering another root.
    fn verify_guards(&self, root: &Path) -> Result<(), GenerateError> {
        if self.path == root.join(OUTPUT_LOCK) {
            return Ok(());
        }
        Err(GenerateError::Input(
            "the output lock does not guard the requested output trees".to_owned(),
        ))
    }
}

impl Drop for OutputLock {
    fn drop(&mut self) {
        // Drop cannot return an error; leave a failed cleanup for stale-lock recovery.
        if let Err(error) = fs::remove_file(&self.path) {
            eprintln!(
                "cannot remove output lock {}: {error}",
                inputs::file_name(&self.path.to_string_lossy())
            );
        }
    }
}

/// Where the generated Java lands, relative to the repository root.
const JAVA_OUTPUT: &str = "target/ravel-jvm/generated/dev/ravel/jvm";

/// Separate stub output prevents packaging classes that would shadow the game.
const STUB_OUTPUT: &str = "target/ravel-jvm/generated-stubs";

/// Where the tracked screen templates live, relative to the repository root.
const SCREEN_TEMPLATES: &str = "jvm/screen/template";

/// Where the generated screen sources land, relative to the repository root.
const SCREEN_OUTPUT: &str = "target/ravel-jvm/generated-screen";

/// Where the harness's stand-in game classes are templated, relative to the root.
const HARNESS_STAND_IN_TEMPLATES: &str = "jvm/harness/template/game";

/// Where the harness's own driver classes are templated, relative to the root.
const HARNESS_DRIVER_TEMPLATES: &str = "jvm/harness/template/driver";

/// Where the harness's stand-in halves of the sibling artifact are templated.
const HARNESS_SIBLING_TEMPLATES: &str = "jvm/harness/template/sibling";

/// Where a version keeps the harness templates it cannot share, inside its own
/// version directory: a source only one version renders is that version's data.
const HARNESS_OVERRIDES: &str = "harness";

/// Where the generated harness sources land, relative to the repository root.
const HARNESS_OUTPUT: &str = "target/ravel-jvm/generated-harness";

/// The package Ravel's own screens are drawn in, wherever the game leaves room
/// for one.
const SCREEN_PACKAGE: &str = "dev.ravel.jvm.screen";

/// Everything one generation renders.
#[derive(Debug, Eq, PartialEq)]
struct Rendered {
    bindings: String,
    exports: String,
    hook_java: String,
    game_names_java: String,
    game_members_java: String,
    legacy_names_java: String,
    menu_layout_java: String,
    /// The Java release the screen jar for this version is compiled for.
    java_release: String,
    /// The Java release the agent jar for this version is compiled for.
    agent_release: String,
    screen_names_java: String,
    lock: String,
    stub_lock: String,
    /// Each stub's path below the stub tree, with its Java source.
    stub_java: Vec<(String, String)>,
    /// Each screen's path below the generated screen tree, with its Java source.
    screen_java: Vec<(String, String)>,
    /// Each harness source's path below the generated harness tree, with its Java.
    harness_java: Vec<(String, String)>,
    /// Which tree rendered each harness source, for the harness to read back.
    harness_origins: String,
    /// Each name a stand-in was written under and the name it stands in for.
    harness_aliases: String,
    /// The templates those screens were rendered from.
    templates: Vec<PathBuf>,
}

/// Rewrites `mappings.lock` and `stubs.lock` from the pinned jar.
///
/// # Errors
///
/// Returns an error for missing or drifted inputs, an absent jar, or unresolved members.
pub fn regenerate_locks(root: &Path, selected: Option<&str>) -> Result<(), GenerateError> {
    let pinned = inputs::pinned(root, selected)?;
    for input in pinned.adapter.inputs.jars() {
        let path = root.join(&input.path);
        if !path.is_file() {
            return Err(GenerateError::Input(format!(
                "the pinned jar is absent at {}; a lock cannot be rewritten from another lock",
                input.path
            )));
        }
    }
    let mapping = mapping::Mapping::load(root, &pinned.adapter)?;
    mapping::verify_reflection(&pinned.surface, &mapping)?;
    let (resolved, stubbed) = resolve_jars(root, &pinned, &mapping)?;

    write_atomically(
        &root.join(&pinned.adapter.inputs.mappings_lock.path),
        &render::locks::lock(&pinned.adapter, &resolved),
    )?;
    write_atomically(
        &root.join(&pinned.adapter.inputs.stubs_lock.path),
        &render::locks::stub_lock(&pinned.adapter, &stubbed),
    )?;
    // Update pins with their locks so the next build accepts the new contents.
    inputs::repin_locks(root, &pinned.directory, &pinned.adapter.inputs)
}

/// Rust outputs owned by Cargo's build directory.
pub struct RustGenerated {
    /// Tracked inputs a build script should watch with `rerun-if-changed`.
    pub watched: Vec<PathBuf>,
    pub bindings: PathBuf,
    pub exports: PathBuf,
}

/// Writes only Rust bindings and exports into `out`.
///
/// # Errors
///
/// Returns an error for invalid or drifted pinned inputs, unresolved members, or output I/O.
pub fn generate_rust(
    root: &Path,
    out: &Path,
    selected: Option<&str>,
) -> Result<RustGenerated, GenerateError> {
    let pinned = inputs::pinned(root, selected)?;
    let rendered = compose_checked(root, &pinned)?;
    create_directory(out)?;
    let bindings = out.join("bindings.rs");
    let exports = out.join("exports.rs");
    write_atomically(&bindings, &rendered.bindings)?;
    write_atomically(&exports, &rendered.exports)?;
    Ok(RustGenerated {
        watched: watched_inputs(root, &pinned, &rendered),
        bindings,
        exports,
    })
}

/// Writes the selected version's Java trees for packaging and harness execution.
///
/// # Errors
///
/// Returns an error for invalid or drifted pinned inputs, unresolved members, or output I/O.
pub fn generate_java(root: &Path, selected: Option<&str>) -> Result<JavaGenerated, GenerateError> {
    generate_java_and_inspect(root, selected, std::convert::identity)
}

/// Keeps the output lock held while a consumer inspects generated Java trees.
fn generate_java_and_inspect<T>(
    root: &Path,
    selected: Option<&str>,
    inspect: impl FnOnce(JavaGenerated) -> T,
) -> Result<T, GenerateError> {
    let pinned = inputs::pinned(root, selected)?;
    let rendered = compose_checked(root, &pinned)?;
    let held = OutputLock::take(root)?;
    let generated = write_java(root, &rendered, &held)?;
    held.refresh()?;
    let inspected = inspect(generated);
    drop(held);
    Ok(inspected)
}

fn compose_checked(root: &Path, pinned: &inputs::PinnedVersion) -> Result<Rendered, GenerateError> {
    // Lock-only builds must still validate their runtime names.
    let mapping = mapping::Mapping::load(root, &pinned.adapter)?;
    mapping::verify_reflection(&pinned.surface, &mapping)?;
    compose(root, pinned, &mapping)
}

/// Resolves the pinned version and renders every artefact from it.
fn compose(
    root: &Path,
    pinned: &inputs::PinnedVersion,
    mapping: &mapping::Mapping,
) -> Result<Rendered, GenerateError> {
    let lock_path = root.join(&pinned.adapter.inputs.mappings_lock.path);
    let tracked = fs::read_to_string(&lock_path).map_err(|error| {
        GenerateError::io(
            format!("version {}, mappings.lock", pinned.adapter.minecraft),
            error,
        )
    })?;

    let stub_path = root.join(&pinned.adapter.inputs.stubs_lock.path);
    let tracked_stubs = fs::read_to_string(&stub_path).map_err(|error| {
        GenerateError::io(
            format!("version {}, stubs.lock", pinned.adapter.minecraft),
            error,
        )
    })?;

    // A machine without the jars has nothing to resolve against, so generating
    // from a lock and comparing the result back to that same lock only says the
    // file is well formed. These pins are what the jars produced, which is what
    // makes a stale lock fail here rather than pass a round trip against itself.
    inputs::verify_input(root, &pinned.adapter.inputs.mappings_lock)?;
    inputs::verify_input(root, &pinned.adapter.inputs.stubs_lock)?;

    let jars = pinned.adapter.inputs.jars();
    let available = jars.clone().any(|input| root.join(&input.path).is_file());
    if available && let Some(missing) = jars.clone().find(|input| !root.join(&input.path).is_file())
    {
        return Err(GenerateError::Input(format!(
            "{} is missing from an incomplete pinned jar set; lock-only generation requires all jars absent",
            missing.path
        )));
    }

    let (resolved, stubbed) = if available {
        resolve_jars(root, pinned, mapping)?
    } else {
        (
            resolve::from_lock(&tracked, &pinned.adapter, &pinned.inventory)?,
            stubs::from_lock(&tracked_stubs, &pinned.adapter, &pinned.seeds)?,
        )
    };

    let lock = render::locks::lock(&pinned.adapter, &resolved);
    if lock != tracked {
        return Err(GenerateError::Drift(
            "what resolved is not what mappings.lock pins; regenerate it deliberately".to_owned(),
        ));
    }
    let stub_lock = render::locks::stub_lock(&pinned.adapter, &stubbed);
    if stub_lock != tracked_stubs {
        return Err(GenerateError::Drift(
            "what the seeds resolve to is not what stubs.lock pins; regenerate it deliberately"
                .to_owned(),
        ));
    }
    compose_render(root, pinned, mapping, &resolved, &stubbed, lock, stub_lock)
}

fn resolve_jars(
    root: &Path,
    pinned: &inputs::PinnedVersion,
    mapping: &mapping::Mapping,
) -> Result<(Vec<resolve::Resolved>, Vec<stubs::Stub>), GenerateError> {
    inputs::verify_input(root, &pinned.adapter.inputs.client_jar)?;
    if let Some(input) = &pinned.adapter.inputs.data_fixer_jar {
        inputs::verify_input(root, input)?;
    }
    inputs::verify_input(root, &pinned.adapter.inputs.version_manifest)?;
    let archive = jar::Archive::open(&root.join(&pinned.adapter.inputs.client_jar.path))?;
    if pinned.adapter.is_legacy() {
        jar::verify_legacy_class_release(&archive)?;
    }
    let resolved = resolve::from_jar(&archive, &pinned.inventory, mapping)?;
    if let Some(input) = &pinned.adapter.inputs.server_jar {
        inputs::verify_input(root, input)?;
        let server = jar::Archive::open(&root.join(&input.path))?;
        resolve::verify_server_hooks(&server, &pinned.inventory, mapping, &resolved)?;
    }
    let library = pinned
        .adapter
        .inputs
        .data_fixer_jar
        .as_ref()
        .map(|input| jar::Archive::open(&root.join(&input.path)))
        .transpose()?;
    mapping::verify_reflected_members(&pinned.surface, mapping, &archive)?;
    jar::verify_pack_format(&pinned.adapter, &archive)?;
    // Hook targets are game members; only stubs may resolve against the library.
    let stubbed = if let Some(library) = &library {
        stubs::from_jar(&[&archive, library], &pinned.seeds, mapping)?
    } else {
        stubs::from_jar(&[&archive], &pinned.seeds, mapping)?
    };
    Ok((resolved, stubbed))
}

fn compose_render(
    root: &Path,
    pinned: &inputs::PinnedVersion,
    mapping: &mapping::Mapping,
    resolved: &[resolve::Resolved],
    stubbed: &[stubs::Stub],
    lock: String,
    stub_lock: String,
) -> Result<Rendered, GenerateError> {
    let shapes = render::shape::shapes(&pinned.inventory, resolved)?;
    let stub_java = stubbed
        .iter()
        .map(|stub| render::stubs::stub_java(&pinned.adapter, stub, mapping))
        .collect::<Result<Vec<_>, _>>()?;
    let roles = roles::Roles::new(&pinned.seeds, &pinned.surface, &pinned.inventory);
    templates::verify_seeded_members_resolve(&roles, stubbed, mapping)?;
    let TemplateSources { screens, harness } = sources(root, pinned, &roles, stubbed, mapping)?;
    let harness_origins = harness_origins(&harness);
    let harness_aliases = harness_aliases(&harness);
    let mut templates = screens.templates;
    let mut harness_java = Vec::new();
    for (tree, sources) in harness {
        templates.extend(sources.templates);
        harness_java.extend(
            sources
                .java
                .into_iter()
                .map(|(path, java)| (format!("{tree}/{path}"), java)),
        );
    }
    Ok(Rendered {
        bindings: render::bindings::bindings(&pinned.adapter, &pinned.inventory, resolved),
        exports: render::exports::exports(&shapes),
        hook_java: render::hooks::hook_java(&pinned.adapter, &shapes),
        game_names_java: render::names::game_names(&pinned.adapter, &pinned.surface),
        game_members_java: render::names::game_members(&pinned.adapter, &pinned.surface, mapping)?,
        legacy_names_java: render::names::legacy_names(&pinned.surface, mapping)?,
        menu_layout_java: render::names::menu_layout(&pinned.adapter, &pinned.inventory),
        java_release: format!("{}\n", pinned.adapter.java_release),
        agent_release: format!("{}\n", pinned.adapter.java_release.max(AGENT_RELEASE_FLOOR)),
        screen_names_java: render::names::screen_names(&pinned.adapter, &screens.names),
        lock,
        stub_lock,
        stub_java,
        screen_java: screens.java,
        harness_java,
        harness_origins,
        harness_aliases,
        templates,
    })
}

/// Records each harness source's origin so consumers need not infer it from generated text.
fn harness_origins(trees: &[(&str, templates::Sources)]) -> String {
    let mut lines = BTreeSet::new();
    for (tree, sources) in trees {
        for (name, overridden) in &sources.origins {
            let origin = if *overridden { "override" } else { "shared" };
            lines.insert(format!("{tree}/{name}={origin}\n"));
        }
    }
    lines.into_iter().collect()
}

/// Records aliases, including an empty file when no names need restoring.
fn harness_aliases(trees: &[(&str, templates::Sources)]) -> String {
    let mut lines = BTreeSet::new();
    for (_, sources) in trees {
        for (written, linked) in &sources.aliases {
            lines.insert(format!("{written}={linked}\n"));
        }
    }
    lines.into_iter().collect()
}

/// Rendered trees retain the labels used to select their overrides.
struct TemplateSources {
    screens: templates::Sources,
    harness: Vec<(&'static str, templates::Sources)>,
}

/// Renders screens, game stand-ins, drivers, and sibling stand-ins against one role table.
fn sources(
    root: &Path,
    pinned: &inputs::PinnedVersion,
    roles: &roles::Roles,
    stubs: &[stubs::Stub],
    mapping: &mapping::Mapping,
) -> Result<TemplateSources, GenerateError> {
    let render = |tree: &templates::Tree<'_>, name_space| {
        templates::render(tree, &pinned.adapter, roles, stubs, mapping, name_space)
    };
    let legacy = pinned.adapter.is_legacy();
    let screens = if legacy {
        pinned.directory.join("templates/screen")
    } else {
        root.join(SCREEN_TEMPLATES)
    };
    let screen_tree = templates::Tree {
        root,
        shared: &screens,
        overrides: screens.join(&pinned.adapter.minecraft),
        purpose: templates::Purpose::Screen,
        stand_ins: false,
    };
    if legacy {
        let name_space = templates::name_space(&screen_tree, roles, mapping)?;
        let screens = render(&screen_tree, name_space)?;
        return Ok(TemplateSources {
            screens,
            harness: Vec::new(),
        });
    }
    let harness = pinned.directory.join(HARNESS_OVERRIDES);
    let game = root.join(HARNESS_STAND_IN_TEMPLATES);
    let driver = root.join(HARNESS_DRIVER_TEMPLATES);
    let sibling = root.join(HARNESS_SIBLING_TEMPLATES);
    let stand_ins = templates::Tree {
        root,
        shared: &game,
        overrides: harness.join("game"),
        purpose: templates::Purpose::StandIn,
        stand_ins: true,
    };
    let name_space = templates::name_space(&stand_ins, roles, mapping)?;
    let screens = render(&screen_tree, name_space)?;
    let mut rendered = Vec::with_capacity(3);
    rendered.push(("game", render(&stand_ins, name_space)?));
    for (name, shared, purpose) in [
        ("driver", &driver, templates::Purpose::Driver),
        ("sibling", &sibling, templates::Purpose::Screen),
    ] {
        let tree = templates::Tree {
            root,
            shared,
            overrides: harness.join(name),
            purpose,
            stand_ins: true,
        };
        rendered.push((name, render(&tree, name_space)?));
    }
    let reached = std::iter::once(&screens)
        .chain(rendered.iter().map(|(_, sources)| sources))
        .collect::<Vec<_>>();
    templates::verify_added_templates_reached(root, &reached)?;
    Ok(TemplateSources {
        screens,
        harness: rendered,
    })
}

/// Java outputs owned by the explicit generation step.
pub struct JavaGenerated {
    /// Generated native hook declarations.
    pub java: PathBuf,
    /// Generated compile-time game stubs.
    pub stubs: PathBuf,
    /// Generated configuration screen sources.
    pub screens: PathBuf,
    /// Generated harness sources.
    pub harness: PathBuf,
    /// Digest of the complete generated Java output tree.
    pub sha256: Sha256Digest,
}

fn write_java(
    root: &Path,
    rendered: &Rendered,
    held: &OutputLock,
) -> Result<JavaGenerated, GenerateError> {
    let mut digest = String::new();
    held.verify_guards(root)?;
    let java_directory = root.join(JAVA_OUTPUT);
    let stub_directory = root.join(STUB_OUTPUT);
    // Sweep every version's old paths before writing the selected version.
    let screen_directory = root.join(SCREEN_OUTPUT);
    let harness_directory = root.join(HARNESS_OUTPUT);
    let java_root = root.join(JAVA_ROOT);
    remove_output_trees(&[
        java_root.as_path(),
        stub_directory.as_path(),
        screen_directory.as_path(),
        harness_directory.as_path(),
    ])?;
    for directory in [
        java_directory.as_path(),
        stub_directory.as_path(),
        screen_directory.as_path(),
        harness_directory.as_path(),
    ] {
        create_directory(directory)?;
    }

    let java = java_directory.join("Hook.java");
    let game_names = java_directory.join("GameNames.java");
    let game_members = java_directory.join("GameMembers.java");
    let legacy_names = java_directory.join("LegacyNames.java");
    let menu_layout = java_directory.join("MenuLayout.java");
    // Consumers compile for the release of these generated sources.
    let java_release = java_directory.join("java-release.txt");
    // The transformer requires the Class-File API even on older game releases.
    let agent_release = java_directory.join("agent-release.txt");
    let screen_names = java_directory.join("ScreenNames.java");
    let harness_origins = harness_directory.join("overrides.txt");
    let harness_aliases = harness_directory.join("aliases.txt");
    for (path, content) in [
        (&java, &rendered.hook_java),
        (&game_names, &rendered.game_names_java),
        (&game_members, &rendered.game_members_java),
        (&legacy_names, &rendered.legacy_names_java),
        (&menu_layout, &rendered.menu_layout_java),
        (&java_release, &rendered.java_release),
        (&agent_release, &rendered.agent_release),
        (&screen_names, &rendered.screen_names_java),
        (&harness_origins, &rendered.harness_origins),
        (&harness_aliases, &rendered.harness_aliases),
    ] {
        place(path, content, &mut digest)?;
    }
    for (directory, sources) in [
        (&stub_directory, &rendered.stub_java),
        (&screen_directory, &rendered.screen_java),
        (&harness_directory, &rendered.harness_java),
    ] {
        write_sources(directory, sources, &mut digest)?;
    }

    Ok(JavaGenerated {
        java,
        stubs: stub_directory,
        screens: screen_directory,
        harness: harness_directory,
        sha256: inputs::sha256(digest.as_bytes()),
    })
}

fn write_sources(
    directory: &Path,
    sources: &[(String, String)],
    digest: &mut String,
) -> Result<(), GenerateError> {
    for (relative, content) in sources {
        let path = directory.join(relative);
        if let Some(parent) = path.parent() {
            create_directory(parent)?;
        }
        place(&path, content, digest)?;
    }
    Ok(())
}

fn watched_inputs(
    root: &Path,
    pinned: &inputs::PinnedVersion,
    rendered: &Rendered,
) -> Vec<PathBuf> {
    let pins = &pinned.adapter.inputs;
    let mut watched = vec![
        pinned.directory.join("adapter.toml"),
        root.join(&pins.mappings_lock.path),
        root.join(&pins.stubs_lock.path),
        root.join(&pins.hook_inventory.path),
        root.join(&pins.stub_seeds.path),
        root.join(&pins.reflective_surface.path),
    ];
    watched.extend(pins.jars().map(|input| root.join(&input.path)));
    watched.push(root.join(&pins.version_manifest.path));
    watched.extend(
        pins.client_mappings
            .iter()
            .chain(pins.mapping_io_jar.iter())
            .map(|input| root.join(&input.path)),
    );
    if pinned.adapter.mapping_source == inputs::MappingSource::FeatherTinyMappings {
        watched.push(root.join(mapping::tiny::NORMALIZER_SOURCE));
    }
    watched.extend(rendered.templates.iter().cloned());
    watched
}

/// Removes every output tree named, so this build writes only its own bytes.
fn remove_output_trees(directories: &[&Path]) -> Result<(), GenerateError> {
    for directory in directories {
        if directory.is_dir() {
            fs::remove_dir_all(directory).map_err(|error| {
                GenerateError::io(inputs::file_name(&directory.to_string_lossy()), error)
            })?;
        }
    }
    Ok(())
}

fn create_directory(directory: &Path) -> Result<(), GenerateError> {
    fs::create_dir_all(directory)
        .map_err(|error| GenerateError::io(inputs::file_name(&directory.to_string_lossy()), error))
}

/// Writes and digests together so the recorded digest covers every generated byte.
fn place(path: &Path, content: &str, digest: &mut String) -> Result<(), GenerateError> {
    write_atomically(path, content)?;
    digest.push_str(&inputs::sha256(content.as_bytes()).to_string());
    digest.push('\0');
    Ok(())
}

/// Writes through a temporary name, so a generator that dies half way leaves
/// nothing behind that a build could pick up.
fn write_atomically(path: &Path, content: &str) -> Result<(), GenerateError> {
    let pending = path.with_extension(format!("pending{}", std::process::id()));
    fs::write(&pending, content)
        .map_err(|error| GenerateError::io(inputs::file_name(&path.to_string_lossy()), error))?;
    fs::rename(&pending, path)
        .map_err(|error| GenerateError::io(inputs::file_name(&path.to_string_lossy()), error))
}

#[cfg(test)]
mod path_redaction_tests {
    use super::{create_directory, write_atomically};

    #[test]
    fn output_failures_name_files_without_local_directories() {
        let root =
            std::env::temp_dir().join(format!("ravel-private-output-path-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("private directory");
        let blocker = root.join("blocker");
        std::fs::write(&blocker, "not a directory").expect("blocking file");
        let target = blocker.join("output.rs");
        let refusals = [
            create_directory(&target).expect_err("file blocks directory"),
            write_atomically(&target, "output").expect_err("file blocks output"),
        ];
        for error in refusals {
            let detail = error.to_string();
            assert!(detail.contains("output.rs"), "{detail}");
            assert!(!detail.contains("ravel-private-output-path"), "{detail}");
            assert!(!detail.contains("blocker"), "{detail}");
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};

    use super::{
        GenerateError, LIBRARY_PROPERTY, LOCK_POLL_INTERVAL, LOCK_STALE, OUTPUT_LOCK, OutputLock,
        SELECTION, generate_java_and_inspect, generate_rust, inputs, selection,
    };

    /// Test fixture combining the independently owned Rust and Java outputs.
    #[derive(Clone, Debug)]
    struct Generated {
        watched: Vec<PathBuf>,
        bindings: PathBuf,
        exports: PathBuf,
        java: PathBuf,
        stubs: PathBuf,
        screens: PathBuf,
        harness: PathBuf,
        sha256: ravel_core::Sha256Digest,
    }

    fn run(root: &Path, out: &Path, selected: Option<&str>) -> Result<Generated, GenerateError> {
        run_and_inspect(root, out, selected, Generated::clone)
    }

    fn run_and_inspect<T>(
        root: &Path,
        out: &Path,
        selected: Option<&str>,
        inspect: impl FnOnce(&Generated) -> T,
    ) -> Result<T, GenerateError> {
        let rust = generate_rust(root, out, selected)?;
        generate_java_and_inspect(root, selected, |java| {
            inspect(&Generated {
                watched: rust.watched,
                bindings: rust.bindings,
                exports: rust.exports,
                java: java.java,
                stubs: java.stubs,
                screens: java.screens,
                harness: java.harness,
                sha256: java.sha256,
            })
        })
    }

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("the crate lives under crates/<name>")
    }

    /// An output directory under the workspace's own build tree.
    ///
    /// It is under `target/` rather than the system temporary directory, so a
    /// run that fails part way cannot accumulate anywhere that matters, and it
    /// is scoped to the running process, because the tests run concurrently and
    /// a shared path would let one of them clear another's output mid-read.
    fn output(name: &str) -> PathBuf {
        let directory = repository_root()
            .join("target/ravel-generate-tests")
            .join(format!("{name}-{}", std::process::id()));
        cleanup(&directory);
        directory
    }

    fn cleanup(directory: &Path) {
        match std::fs::remove_dir_all(directory) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            result => result.expect("remove the test fixture"),
        }
    }

    /// Copies the pinned version's tracked data into a root of its own.
    ///
    /// The copy holds no jars, so generating in it takes the lock path.
    /// Returns the new root and its version directory.
    fn staged(name: &str) -> (PathBuf, PathBuf) {
        let root = output(name);
        // The version is read from the tracked data rather than written here, so
        // nothing outside the version data names a Minecraft version.
        // The version whose class files are the mapping, because these fixtures
        // declare a published mapping of their own to exercise the other path
        // and a version that already declared one would carry two.
        let source = inputs::tracked_version_where(repository_root(), |adapter| {
            !adapter.mapping_source.needs_mappings()
        })
        .directory;
        let relative = source
            .strip_prefix(repository_root())
            .expect("the version data lives under the root");
        let directory = root.join(relative);
        std::fs::create_dir_all(&directory).expect("the staged version directory should be made");
        copy_files(&source, &directory);
        stage_templates(&root);
        (root, directory)
    }

    /// Writes the screen template a staged root generates from.
    ///
    /// The tracked templates name the classes the tracked version's seeds carry,
    /// and a staged root is asked about versions whose seeds carry other things —
    /// including two at once. So a staged root gets a template that names no game
    /// class at all, which every version can answer. What the tracked templates
    /// render against the tracked seeds is what the repository's own build
    /// checks, on every build.
    fn stage_templates(root: &Path) {
        let owned = |declared: &str| {
            format!(
                "{{{{package}}}}\n\n\
                 /** A source that names nothing the game owns. */\n\
                 public final class {declared} {{\n\
                 }}\n"
            )
        };
        std::fs::create_dir_all(root.join(super::HARNESS_STAND_IN_TEMPLATES))
            .expect("the staged template directory should be made");
        for (tree, name) in [
            (super::SCREEN_TEMPLATES, "Drawn"),
            (super::HARNESS_DRIVER_TEMPLATES, "Driven"),
            (super::HARNESS_SIBLING_TEMPLATES, "Sided"),
        ] {
            let directory = root.join(tree);
            std::fs::create_dir_all(&directory)
                .expect("the staged template directory should be made");
            std::fs::write(directory.join(format!("{name}.java.in")), owned(name))
                .expect("the staged template should be writable");
        }

        // A stand-in is named by the role it declares, so its template is named
        // from the staged seed list rather than from a word written here: a word
        // written here would be a role one version happens to bind. Two staged
        // versions bind two sets of roles, which is why each gets its own
        // stand-in under its own version data rather than sharing one — and its
        // own driver, because a stand-in nothing reaches is refused.
        for version in inputs::tracked_versions(root) {
            let seeds = std::fs::read_to_string(
                root.join(format!("minecraft/versions/{version}/stubs.toml")),
            )
            .expect("a staged version carries a seed list");
            let role = seeds
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("role = \"")
                        .and_then(|rest| rest.strip_suffix('"'))
                })
                .find(|role| !role.contains('$'))
                .expect("a staged seed list binds a role");
            let overrides = root.join(format!(
                "minecraft/versions/{version}/{}",
                super::HARNESS_OVERRIDES
            ));
            std::fs::create_dir_all(overrides.join("game"))
                .expect("the staged template directory should be made");
            std::fs::create_dir_all(overrides.join("driver"))
                .expect("the staged template directory should be made");
            std::fs::write(
                overrides.join("game").join(format!("{role}.java.in")),
                owned(&format!("{{{{{role}}}}}")),
            )
            .expect("the staged template should be writable");
            std::fs::write(
                overrides.join("driver").join("Driven.java.in"),
                format!(
                    "{{{{package}}}}\n{{{{import {role}}}}}\n\n\
                     /** A source that reaches the stand-in beside it. */\n\
                     public final class Driven {{\n\
                     \x20   {{{{{role}}}}} held;\n\
                     }}\n"
                ),
            )
            .expect("the staged template should be writable");
        }
    }

    #[test]
    fn public_fixture_composes_identical_full_outputs_twice() {
        let (root, version) = staged_second_version("deterministic-composition");
        let pinned = inputs::pinned(&root, Some(&version)).expect("public fixture version");
        let first = super::compose_checked(&root, &pinned).expect("first composition");
        let second = super::compose_checked(&root, &pinned).expect("second composition");
        assert_eq!(first, second, "every composed output must be deterministic");
        cleanup(&root);
    }

    #[test]
    fn legacy_sources_render_the_local_screen_without_inheriting_a_modern_harness() {
        let (root, directory) = staged("legacy-template-selection");
        let mut pinned = inputs::pinned(&root, None).expect("the staged inputs are pinned");
        pinned.adapter.capabilities = vec![inputs::Capability::Legacy112];
        std::fs::remove_dir_all(directory.join(super::HARNESS_OVERRIDES))
            .expect("remove the staged modern harness overrides");
        let screen = directory.join("templates/screen");
        std::fs::create_dir_all(&screen).expect("create the legacy screen tree");
        let template = screen.join("LegacySupport.java.in");
        std::fs::write(
            &template,
            "{{package}}\npublic final class LegacySupport {}\n",
        )
        .expect("write the version-local support source");
        let roles = super::roles::Roles::new(&pinned.seeds, &pinned.surface, &pinned.inventory);
        let mapping = super::mapping::Mapping::Identity;
        let super::TemplateSources { screens, harness } =
            super::sources(&root, &pinned, &roles, &[], &mapping)
                .expect("local support renders without a harness");
        assert_eq!(screens.templates, [template]);
        assert_eq!(screens.java.len(), 1);
        assert_eq!(screens.java[0].0, "dev/ravel/jvm/screen/LegacySupport.java");
        assert!(harness.is_empty(), "legacy versions have no harness trees");
        std::fs::remove_dir_all(&screen).expect("remove the required screen tree");
        assert!(super::sources(&root, &pinned, &roles, &[], &mapping).is_err());
        cleanup(&root);
    }

    /// The harness's three trees land under three directories of their own.
    ///
    /// The run script sweeps each of them whole rather than naming the files it
    /// wants, so a stand-in that landed beside a driver would be compiled by the
    /// section that runs the driver and a driver that landed beside the stand-ins
    /// would be compiled into the game. That is the property the layout buys, and
    /// it is invisible in any one rendered source.
    #[test]
    fn each_harness_tree_lands_under_its_own_name() {
        let (root, _) = staged("harness-trees");
        let out = output("harness-trees-out");

        let generated = run(&root, &out, None).expect("the staged root generates");
        let trees: BTreeSet<OsString> = std::fs::read_dir(&generated.harness)
            .expect("the harness root should be listable")
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name())
            .collect();

        assert_eq!(
            trees,
            ["driver", "game", "sibling"]
                .into_iter()
                .map(OsString::from)
                .collect::<BTreeSet<_>>()
        );

        cleanup(&root);
        cleanup(&out);
    }

    /// The harness is told which names it was written under, whether or not any
    /// of them moved.
    ///
    /// The empty case is the one worth a test: a reader that took an absent file
    /// for "nothing moved" would read a generator that never ran as a clean
    /// stand-in game, and the run script decides whether to run a section on it.
    #[test]
    fn the_generated_harness_records_the_names_it_was_written_under() {
        let (root, _) = staged("harness-aliases");
        let out = output("harness-aliases-out");

        let generated = run(&root, &out, None).expect("the staged root generates");
        let aliases = std::fs::read_to_string(generated.harness.join("aliases.txt"))
            .expect("the harness root should carry an alias record");

        assert!(
            aliases.is_empty(),
            "a version that leaves its whole game in packages moves nothing: {aliases}"
        );

        cleanup(&root);
        cleanup(&out);
    }

    /// A version whose stand-in game spans both name spaces writes the whole of
    /// it in the unnamed package and records the classes that moved.
    ///
    /// The tracked versions this fixture is copied from leave their game in
    /// packages, so the split is made here: one more role, bound to a class
    /// outside every package, which is what an obfuscated version does to all
    /// but a handful of its classes. It is bound through `reflection.toml`
    /// rather than the stub seeds because the seeds are resolved against a
    /// pinned lock derived from a jar, and no jar carries this class.
    #[test]
    fn a_split_stand_in_game_is_written_bare_and_says_which_names_moved() {
        let (root, directory) = staged("harness-split");
        let out = output("harness-split-out");
        let bare = "BARE";
        let reflection = directory.join("reflection.toml");
        let mut surface =
            std::fs::read_to_string(&reflection).expect("the staged surface should be readable");
        write!(
            surface,
            "\n[[reflected]]\nname = \"{bare}\"\nclass = \"{bare}\"\nruntime = \"{bare}\"\n"
        )
        .expect("write the staged surface string");
        std::fs::write(&reflection, &surface).expect("the staged surface should be writable");
        repin(&directory, "[inputs.reflectiveSurface]", surface.as_bytes());
        let overrides = directory.join(super::HARNESS_OVERRIDES);
        std::fs::write(
            overrides.join("game").join(format!("{bare}.java.in")),
            format!("{{{{package}}}}\npublic final class {{{{{bare}}}}} {{\n}}\n"),
        )
        .expect("the staged template should be writable");
        let driver = overrides.join("driver").join("Driven.java.in");
        let reaching = std::fs::read_to_string(&driver).expect("the staged driver is readable");
        let body = reaching
            .strip_suffix("}\n")
            .expect("the staged driver ends with its own brace");
        std::fs::write(&driver, format!("{body}\x20   {{{{{bare}}}}} bare;\n}}\n"))
            .expect("the staged driver should be writable");

        let generated = run(&root, &out, None).expect("the staged root generates");
        let aliases = std::fs::read_to_string(generated.harness.join("aliases.txt"))
            .expect("the harness root should carry an alias record");

        let lines: Vec<_> = aliases.lines().collect();
        assert!(!lines.is_empty(), "a split version aliases what it moved");
        for line in &lines {
            let (written, linked) = line.split_once('=').expect("a written and a linked name");
            assert_ne!(written, linked, "{line}");
            assert!(!written.contains('/'), "{line}");
            assert!(linked.contains('/'), "{line}");
            let source = generated
                .harness
                .join("game")
                .join(format!("{written}.java"));
            let java = std::fs::read_to_string(&source).expect("the moved class is written bare");
            assert!(
                !java.lines().any(|line| line.starts_with("package ")),
                "{java}"
            );
            assert!(java.contains(&format!("class {written} ")), "{java}");
        }
        // The class the version already left outside every package is written
        // where it links and aliases nothing, which is the half a record of only
        // the moved names would not show.
        assert!(
            generated
                .harness
                .join("game")
                .join(format!("{bare}.java"))
                .is_file(),
            "the unmoved half is written under its own name"
        );

        cleanup(&root);
        cleanup(&out);
    }

    /// Re-pins one staged adapter input against the bytes just written.
    fn repin(directory: &Path, table: &str, bytes: &[u8]) {
        let path = directory.join("adapter.toml");
        let document = std::fs::read_to_string(&path).expect("readable");
        let length = u64::try_from(bytes.len()).expect("the staged input length fits u64");
        let rewritten = inputs::replace_pin(&document, table, &inputs::sha256(bytes), length)
            .expect("the staged inputs pin that table");
        std::fs::write(&path, rewritten).expect("writable");
    }

    /// The output lock spans the caller's read, not just the write.
    ///
    /// That span is the whole reason `run_and_inspect` exists over [`run`], and
    /// it is the one thing no other test can see: every assertion they make about
    /// a generated file passes whether the lock was dropped before the read or
    /// after it.
    #[test]
    fn the_output_lock_is_held_across_the_inspection_and_released_after() {
        let (root, _) = staged("held-lock");
        let out = output("held-lock-out");
        let lock = root.join(OUTPUT_LOCK);

        run_and_inspect(&root, &out, None, |_| {
            assert!(
                lock.is_file(),
                "the trees may not be sweepable while the closure reads them"
            );
        })
        .expect("the staged root generates");

        assert!(
            !lock.exists(),
            "the lock is the caller's for the call and no longer"
        );

        cleanup(&root);
        cleanup(&out);
    }

    #[test]
    fn refreshing_the_lock_re_dates_the_held_file_and_refuses_a_missing_one() {
        let root = output("lock-refresh");
        let held = OutputLock::take(&root).expect("a fresh root has no holder");
        let path = root.join(OUTPUT_LOCK);
        let long_ago = std::time::SystemTime::now() - LOCK_STALE * 2;
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(long_ago))
            .expect("the lock can be aged");

        held.refresh().expect("a held lock re-dates");
        let age = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .map(|modified| modified.elapsed().unwrap_or_default())
            .expect("the lock has a modification time");
        assert!(
            age < LOCK_STALE,
            "the lock reads as held again, not abandoned: {age:?}"
        );

        std::fs::remove_file(&path).expect("a waiter can remove a stale lock");
        let error = held
            .refresh()
            .expect_err("a lock another holder replaced is not recreated");
        assert!(
            matches!(&error, GenerateError::Io { source, .. }
            if source.kind() == std::io::ErrorKind::NotFound),
            "{error}"
        );
        assert!(error.to_string().contains("could not be re-dated"));
        assert!(!path.exists(), "nothing recreated the file");

        std::mem::forget(held);
        cleanup(&root);
    }

    #[test]
    fn a_stale_regular_file_lock_is_reclaimed() {
        let root = output("stale-file-lock");
        let held = OutputLock::take(&root).expect("initial holder");
        let path = root.join(OUTPUT_LOCK);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now() - LOCK_STALE * 2))
            .expect("age abandoned lock");
        std::mem::forget(held);
        let replacement = OutputLock::take(&root).expect("reclaim stale regular file");
        let age = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("fresh lock timestamp")
            .elapsed()
            .expect("timestamp is not in the future");
        assert!(age < LOCK_STALE, "{age:?}");
        drop(replacement);
        assert!(!path.exists());
        cleanup(&root);
    }

    #[test]
    fn a_second_output_lock_waits_for_the_live_holder() {
        let root = output("contended-lock");
        let held = OutputLock::take(&root).expect("first holder");
        std::thread::scope(|scope| {
            let (started, starting) = std::sync::mpsc::sync_channel(0);
            let (finished, finishing) = std::sync::mpsc::channel();
            let root = &root;
            let waiter = scope.spawn(move || {
                started.send(()).expect("announce waiter");
                let second = OutputLock::take(root);
                finished.send(second).expect("return second holder");
            });
            starting.recv().expect("waiter started");
            let early = finishing.recv_timeout(LOCK_POLL_INTERVAL * 4);
            drop(held);
            assert!(
                matches!(early, Err(std::sync::mpsc::RecvTimeoutError::Timeout)),
                "a live holder cannot be reclaimed"
            );
            let second = finishing
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("waiter completes after release")
                .expect("second holder");
            drop(second);
            waiter.join().expect("waiter thread");
        });
        assert!(!root.join(OUTPUT_LOCK).exists());
        cleanup(&root);
    }

    #[cfg(unix)]
    #[test]
    fn stale_lock_unlink_failure() {
        let root = output("stale-directory-lock");
        let lock = root.join(OUTPUT_LOCK);
        std::fs::create_dir_all(&lock).expect("create the directory lock fixture");
        let long_ago = std::time::SystemTime::now() - LOCK_STALE * 2;
        std::fs::File::open(&lock)
            .and_then(|file| file.set_modified(long_ago))
            .expect("age the directory lock fixture");

        let result = OutputLock::take(&root).map(|_| ());
        cleanup(&root);
        let error = result.expect_err("a directory lock cannot be unlinked as a stale file");
        assert!(matches!(&error, GenerateError::Io { .. }));
        assert!(error.to_string().contains("removal failed"));
        assert!(
            error
                .to_string()
                .contains("if no build is running, delete it")
        );
    }

    #[test]
    fn a_machine_without_the_jars_generates_from_a_lock_that_matches_its_pin() {
        let (root, _) = staged("jarless-honest");
        let out = output("jarless-honest-out");

        let generated =
            run(&root, &out, None).expect("an unedited lock still matches what is pinned");

        assert!(generated.bindings.is_file());

        cleanup(&root);
        cleanup(&out);
    }

    #[test]
    fn declared_lock_paths_drive_loading_and_watched_inputs() {
        let (root, directory) = staged("relocated-locks");
        let out = output("relocated-locks-out");
        let adapter_path = directory.join("adapter.toml");
        let mut document = std::fs::read_to_string(&adapter_path).expect("staged adapter");
        let pinned = inputs::pinned(&root, None).expect("staged input paths");
        for input in [
            &pinned.adapter.inputs.mappings_lock,
            &pinned.adapter.inputs.stubs_lock,
        ] {
            let relocated = format!("relocated/{}", inputs::file_name(&input.path));
            let destination = root.join(&relocated);
            std::fs::create_dir_all(destination.parent().expect("parent")).expect("lock directory");
            std::fs::rename(root.join(&input.path), &destination).expect("move declared lock");
            document = document.replace(&input.path, &relocated);
        }
        std::fs::write(adapter_path, document).expect("update declared paths");
        let generated = run(&root, &out, None).expect("declared lock paths are authoritative");
        assert!(
            generated
                .watched
                .contains(&root.join("relocated/mappings.lock"))
        );
        assert!(
            generated
                .watched
                .contains(&root.join("relocated/stubs.lock"))
        );
        assert!(!generated.watched.contains(&directory.join("mappings.lock")));
        assert!(!generated.watched.contains(&directory.join("stubs.lock")));
        cleanup(&root);
        cleanup(&out);
    }

    #[test]
    fn partial_pinned_jar_sets_do_not_fall_back_to_the_locks() {
        let (root, _) = staged("partial-pinned-jars");
        let mut pinned = inputs::pinned(&root, None).expect("staged inputs");
        pinned.adapter.inputs.data_fixer_jar = None;
        pinned.adapter.inputs.client_jar.path = "client.jar".to_owned();
        let mut server = pinned.adapter.inputs.client_jar.clone();
        server.path = "server.jar".to_owned();
        pinned.adapter.inputs.server_jar = Some(server);
        let mapping = super::mapping::Mapping::Identity;
        for (present, missing) in [("client.jar", "server.jar"), ("server.jar", "client.jar")] {
            let path = root.join(present);
            std::fs::write(&path, []).expect("present jar marker");
            let error = super::compose(&root, &pinned, &mapping)
                .expect_err("a partial fixture is not an offline fixture");
            assert!(error.to_string().contains(missing), "{error}");
            std::fs::remove_file(path).expect("remove present jar marker");
        }
        cleanup(&root);
    }

    #[test]
    fn a_machine_without_the_jars_refuses_a_lock_that_no_longer_matches_its_pin() {
        let (root, directory) = staged("jarless-stale");
        let out = output("jarless-stale-out");
        // The lock's own bytes are left alone, so nothing but the pin can tell
        // this apart from the honest case: parsing the lock and rendering it back
        // still agrees with itself, which is the hole this pin closes.
        let adapter = directory.join("adapter.toml");
        let document = std::fs::read_to_string(&adapter).expect("readable");
        let length = std::fs::metadata(directory.join("mappings.lock"))
            .expect("the staged lock should be there")
            .len();
        let stale = inputs::replace_pin(
            &document,
            "[inputs.mappingsLock]",
            &ravel_core::Sha256Digest::from_bytes([0; 32]),
            length,
        )
        .expect("the staged adapter inputs pin the lock");
        assert_ne!(stale, document, "the pin has to actually be made stale");
        std::fs::write(&adapter, stale).expect("writable");

        let error =
            run(&root, &out, None).expect_err("a lock that is not what is pinned must be refused");

        assert!(matches!(error, GenerateError::Drift(_)), "{error}");
        let message = error.to_string();
        assert!(message.contains("mappings.lock"), "{message}");
        assert!(message.contains("the adapter inputs pin"), "{message}");

        cleanup(&root);
        cleanup(&out);
    }

    /// Every template the three trees render for one version.
    ///
    /// Counted per tree rather than as one number, so a tree that stopped being
    /// rendered shows up here as the tree it is, and the version's own overrides
    /// count with them because a change to one has to re-run the generator.
    ///
    /// Counted as the union of the two directories' file names rather than as
    /// their sum: an override *replaces* the shared template of its own name and
    /// the generator watches the one it rendered, so adding the directories said
    /// one file more than the build ever read for every override that replaced
    /// something. It agreed with the sum only for a version whose overrides all
    /// add.
    fn tracked_templates(root: &Path, version: &str) -> usize {
        let overrides = root.join(format!(
            "minecraft/versions/{version}/{}",
            super::HARNESS_OVERRIDES
        ));
        let mut templates = 0;
        for (tree, held_by_the_version) in [
            (
                super::SCREEN_TEMPLATES,
                root.join(super::SCREEN_TEMPLATES).join(version),
            ),
            (super::HARNESS_STAND_IN_TEMPLATES, overrides.join("game")),
            (super::HARNESS_DRIVER_TEMPLATES, overrides.join("driver")),
            (super::HARNESS_SIBLING_TEMPLATES, overrides.join("sibling")),
        ] {
            let held: BTreeSet<OsString> = [root.join(tree), held_by_the_version]
                .iter()
                .filter_map(|directory| std::fs::read_dir(directory).ok())
                .flat_map(Iterator::flatten)
                .filter(|entry| entry.path().to_string_lossy().ends_with(".java.in"))
                .map(|entry| entry.file_name())
                .collect();
            assert!(
                !held.is_empty(),
                "the repository tracks templates for {tree}"
            );
            templates += held.len();
        }
        templates
    }

    /// A template a version replaces is one template and not two.
    ///
    /// The staged root is where a replacing override exists to be counted: it
    /// writes a driver beside the shared one of the same name, so the sum the
    /// count used to be reads one file more than the generator watched. The
    /// tracked repository happens to have no replacing override in the tree the
    /// artefact test asks about, which is why that test passed while the rule
    /// was wrong.
    #[test]
    fn a_replaced_template_is_counted_once() {
        let (root, _) = staged("replaced-count");
        let out = output("replaced-count-out");

        let generated = run(&root, &out, None).expect("the staged inputs should generate");
        let version = inputs::tracked_versions(&root)
            .into_iter()
            .next()
            .expect("the staged root carries the pinned version");

        assert_eq!(
            generated.watched.len(),
            9 + tracked_templates(&root, &version)
        );

        cleanup(&root);
        cleanup(&out);
    }

    #[test]
    fn rust_generation_writes_only_into_its_output_directory() {
        let (root, _) = staged("rust-only");
        let out = output("rust-only-out");
        let generated = generate_rust(&root, &out, None).expect("generate Rust outputs");
        assert!(generated.bindings.is_file());
        assert!(generated.exports.is_file());
        assert!(!root.join("target").exists());
        assert_eq!(
            std::fs::read_dir(&out).expect("read Rust outputs").count(),
            2
        );
        cleanup(&root);
        cleanup(&out);
    }

    #[test]
    fn generating_from_the_tracked_inputs_writes_every_artefact() {
        let out = output("artefacts");

        // The version whose class files are the mapping: the assertions below are
        // about screens that land in Ravel's screen package and a template set
        // with no per-version override, both of which are that version's shape.
        let version = inputs::tracked_version_where(repository_root(), |adapter| {
            !adapter.mapping_source.needs_mappings()
        })
        .adapter
        .minecraft;
        run_and_inspect(repository_root(), &out, Some(&version), |generated| {
            assert!(generated.bindings.is_file());
            assert!(generated.exports.is_file());
            assert!(generated.java.is_file());
            assert_eq!(generated.sha256.to_string().len(), 64);
            assert_eq!(
                generated.watched.len(),
                9 + tracked_templates(repository_root(), &version)
            );

            generating_from_the_tracked_inputs_check_screens(generated);

            let bindings = std::fs::read_to_string(&generated.bindings).expect("readable");
            assert!(bindings.contains("pub static TARGETS"));
            assert!(bindings.contains("HookKind::InstanceField"));

            let exports = std::fs::read_to_string(&generated.exports).expect("readable");
            assert!(exports.contains("jni_mangle"));
            assert!(
                !exports.contains("lastAmount"),
                "a field is not a call target and gets no export"
            );

            every_declared_crossing(&generated.java);
        })
        .expect("the tracked inputs should generate");

        cleanup(&out);
    }

    fn generating_from_the_tracked_inputs_check_screens(generated: &Generated) {
        let screen = generated
            .screens
            .join("dev/ravel/jvm/screen/RavelSettingsScreen.java");
        let drawn = std::fs::read_to_string(&screen).expect("the screen should be generated");
        assert!(drawn.contains("package dev.ravel.jvm.screen;"), "{drawn}");
        assert!(drawn.contains("extends Screen"), "{drawn}");
        assert!(
            !drawn.contains("{{"),
            "every placeholder is rendered: {drawn}"
        );

        let names = generated.java.with_file_name("ScreenNames.java");
        let declared = std::fs::read_to_string(&names).expect("the screen names should be written");
        assert!(
            declared.contains(
                "static final String RAVEL_SETTINGS_SCREEN = \
                 \"dev.ravel.jvm.screen.RavelSettingsScreen\";"
            ),
            "{declared}"
        );

        let screen = generated
            .stubs
            .join("net/minecraft/client/gui/screens/Screen.java");
        let stub = std::fs::read_to_string(&screen).expect("the seeded class should be stubbed");
        assert!(
            stub.contains("package net.minecraft.client.gui.screens;"),
            "{stub}"
        );
        assert!(stub.contains("class Screen"), "{stub}");
        assert!(
            stub.contains(
                "extends \
                     net.minecraft.client.gui.components.events.AbstractContainerEventHandler"
            ),
            "the focus a scrolled screen restores is declared above Screen, so the stub has \
                 to reach it: {stub}"
        );
    }

    /// Every crossing the generated hook class declares, and nothing but them.
    ///
    /// Its own function because the artefact test is otherwise long enough to
    /// hide it, and this list keeps a single exported symbol surface.
    fn every_declared_crossing(generated_java: &Path) {
        let java = std::fs::read_to_string(generated_java).expect("readable");
        // The library is loaded from a file a `-D` property
        // names, and a launch that names none refuses before anything else runs
        // rather than searching a path the launcher owns.
        assert!(
            java.contains("System.load(file.getAbsolutePath());"),
            "{java}"
        );
        assert!(!java.contains("System.loadLibrary("), "{java}");
        assert!(
            java.contains(&format!("LIBRARY_PROPERTY = \"{LIBRARY_PROPERTY}\"")),
            "{java}"
        );
        assert!(java.contains("named == null || named.isEmpty()"), "{java}");
        assert!(java.contains("!file.isFile()"), "{java}");
        assert!(java.contains("public static native long nativeInvocations();"));
        // The screen's two crossings are declared on this class and nowhere
        // else: Ravel has one exported symbol surface, so a screen that
        // declared its own natives would be a second one.
        assert!(java.contains("public static native String ravelSettingsDocument(String owner);"));
        assert!(java.contains("public static native String ravelSaveSettings(String payload);"));
        assert!(java.contains("public static native String ravelModSurface();"));
        assert!(java.contains(
            "public static native void ravelInitialize(int side, boolean profilingRequested);"
        ));
        assert!(!java.contains("ravelEnableProfiling"));
        assert!(java.contains("public static native String ravelProfilingReport();"));
        // The world's two crossings are declared here for the same reason the
        // screen's are, and they carry bytes rather than a handle: the aggregate
        // is a payload Java never reads.
        assert!(java.contains(
            "public static native String ravelOpenWorldPersistence(\n            \
         byte[] aggregate, String identity, boolean freshWorld);"
        ));
        assert!(
            java.contains(
                "public static native byte[] ravelSaveWorldPersistence(String identity);"
            )
        );
        // The earlier of the two world crossings, and the one that carries a
        // directory: it writes files before the game builds that world's pack
        // repository.
        assert!(java.contains("public static native void ravelWriteWorldDatapack(String world);"));
        // The staging crossing, which carries the screen's directory rather than
        // a world's: the two differ by one level and the names say which is which.
        assert!(
            java.contains("public static native int ravelWriteStagedDatapack(String directory);")
        );
        // The id the staged pack is selected by, which Java asks for rather than
        // spelling: the directory it is composed from is the native side's.
        assert!(java.contains("public static native String ravelPackId();"));
    }

    /// Where the synthetic second version's data lives.
    ///
    /// It is a fixture rather than a second directory under `minecraft/versions`
    /// because it is invented: the version it names does not exist and the
    /// classes its inventory reviews do not exist either, so it is a question to
    /// ask the generator rather than a version anything ships. Staged as the
    /// only version it asks M4-W6's question, whether the generator works on a
    /// version that is not the pinned one; staged beside the pinned one it asks
    /// this record's, whether a build reads the version it was told to.
    const SECOND_VERSION: &str = "tests/fixtures/public/versions";

    /// The synthetic version's own directory and the name it carries.
    ///
    /// The directory name is read off the filesystem rather than written here,
    /// so no source file in any crate spells a version, which is the property
    /// `ravel-verify-scaffold` enforces and these tests would otherwise be the
    /// first thing to break.
    fn synthetic_version() -> (PathBuf, String) {
        let source = repository_root().join(SECOND_VERSION);
        let mut versions = std::fs::read_dir(&source)
            .expect("the synthetic version fixture should be readable")
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        versions.sort();
        let [version] = versions.as_slice() else {
            panic!("the fixture holds one synthetic version, not {versions:?}");
        };
        (source.join(version), version.clone())
    }

    /// Copies one version directory's files into `directory`.
    ///
    /// Files only: a version directory also holds the templates that version
    /// renders instead of a shared one, and a staged root writes its own rather
    /// than inheriting templates written against the tracked seeds.
    fn copy_version(source: &Path, directory: &Path) {
        copy_files(source, directory);
    }

    /// Copies every file directly inside `source` into `directory`.
    fn copy_files(source: &Path, directory: &Path) {
        std::fs::create_dir_all(directory).expect("the staged directory should be made");
        for entry in std::fs::read_dir(source).expect("listable") {
            let file = entry.expect("a readable entry").file_name();
            if source.join(&file).is_dir() {
                continue;
            }
            std::fs::copy(source.join(&file), directory.join(&file)).expect("copyable");
        }
    }

    /// Stages the synthetic second version as if it were the pinned one.
    fn staged_second_version(name: &str) -> (PathBuf, String) {
        let (source, version) = synthetic_version();
        let root = output(name);
        copy_version(&source, &root.join("minecraft/versions").join(&version));
        stage_templates(&root);
        (root, version)
    }

    /// Stages the tracked version data and the synthetic one side by side.
    ///
    /// This is the shape selection exists for and the one nothing in the
    /// repository holds yet: two well-formed version directories under one root,
    /// neither of which a build may pick on its own. Returns the root, the
    /// tracked version's name, and the synthetic one's.
    fn staged_two_versions(name: &str) -> (PathBuf, String, String) {
        let (root, directory) = staged(name);
        let tracked = directory
            .file_name()
            .expect("a version directory is named")
            .to_string_lossy()
            .into_owned();
        let (source, synthetic) = synthetic_version();
        copy_version(&source, &root.join("minecraft/versions").join(&synthetic));
        assert_ne!(
            tracked, synthetic,
            "two directories that are one version prove nothing"
        );
        // Again, now that both versions are staged: each of them renders a
        // stand-in named by a role of its own.
        stage_templates(&root);
        (root, tracked, synthetic)
    }

    /// Each of two tracked versions generates, and only the one that was named.
    ///
    /// This is what makes version selection a build-time choice rather than a
    /// refusal: the same generator, the same root, two well-formed directories,
    /// and the artefacts name whichever one the build asked for. Both directions
    /// are run, because a selector that always returned the first one would pass
    /// half of this.
    #[test]
    fn a_build_generates_the_version_it_names_and_not_the_other() {
        let (root, tracked, synthetic) = staged_two_versions("selected");

        for (wanted, other) in [(&tracked, &synthetic), (&synthetic, &tracked)] {
            let out = output(&format!("selected-out-{wanted}"));
            let generated = run(&root, &out, Some(wanted)).expect("the named version generates");
            for artefact in [&generated.bindings, &generated.java] {
                let text = std::fs::read_to_string(artefact).expect("readable");
                assert!(
                    text.contains(wanted),
                    "the artefact should name the selected version: {}",
                    artefact.display()
                );
                assert!(
                    !text.contains(other.as_str()),
                    "a selected build may not emit the version it did not select: {}",
                    artefact.display()
                );
            }

            cleanup(&out);
        }

        cleanup(&root);
    }

    /// Two versions and nothing naming one is refused, with both names in the
    /// refusal.
    ///
    /// Picking one would be a silent answer to a question the operator should
    /// have answered, and a refusal that named neither would leave them guessing
    /// at what to set.
    #[test]
    fn two_versions_and_no_selection_is_refused_naming_both() {
        let (root, tracked, synthetic) = staged_two_versions("unselected");
        let out = output("unselected-out");

        let error = run(&root, &out, None).expect_err("two versions need one named");

        assert!(matches!(error, GenerateError::Input(_)));
        let message = error.to_string();
        assert!(message.contains(SELECTION), "{message}");
        assert!(message.contains(&tracked), "{message}");
        assert!(message.contains(&synthetic), "{message}");
        assert!(!out.exists(), "a refused build writes nothing");

        cleanup(&root);
    }

    /// A selection no directory carries is refused with what was actually there.
    #[test]
    fn a_selection_no_directory_carries_is_refused_with_what_was_found() {
        let (root, tracked, synthetic) = staged_two_versions("unknown");
        let out = output("unknown-out");

        let error = run(&root, &out, Some("no-such-version"))
            .expect_err("a name nothing carries selects nothing");

        assert!(matches!(error, GenerateError::Input(_)));
        let message = error.to_string();
        assert!(message.contains("no-such-version"), "{message}");
        assert!(message.contains(&tracked), "{message}");
        assert!(message.contains(&synthetic), "{message}");
        assert!(!out.exists(), "a refused build writes nothing");

        cleanup(&root);
    }

    /// An empty selection reads as a refusal rather than a sentence with a hole.
    #[test]
    fn an_empty_selection_is_refused_in_words_a_reader_can_act_on() {
        let (root, tracked, synthetic) = staged_two_versions("empty");
        let out = output("empty-out");

        let error = run(&root, &out, Some("")).expect_err("an empty name selects nothing");

        let message = error.to_string();
        assert!(message.contains("names nothing"), "{message}");
        assert!(message.contains(&tracked), "{message}");
        assert!(message.contains(&synthetic), "{message}");
        assert!(!out.exists(), "a refused build writes nothing");

        cleanup(&root);
    }

    /// A malformed directory this build did not select does not fail the build.
    ///
    /// This is the record's load-bearing claim and the one two well-formed
    /// directories cannot ask: a generator that swept every tracked version
    /// would pass every other test here and then break every build the day a
    /// malformed second directory landed, for a reason no artefact of that build
    /// derives from. Selecting the corrupt one is asserted too, because a
    /// corruption the generator would have tolerated anyway proves nothing.
    #[test]
    fn a_malformed_unselected_directory_does_not_fail_the_build() {
        let (root, tracked, synthetic) = staged_two_versions("corrupt-unselected");
        let corrupt = root
            .join("minecraft/versions")
            .join(&synthetic)
            .join("adapter.toml");
        std::fs::write(&corrupt, b"this is not toml [").expect("the fixture should be writable");

        let out = output("corrupt-unselected-out");
        run(&root, &out, Some(&tracked)).expect("a build that selected the other one generates");

        let other = output("corrupt-selected-out");
        let error = run(&root, &other, Some(&synthetic))
            .expect_err("a corruption the generator tolerates proves nothing");
        assert!(matches!(error, GenerateError::Input(_)), "{error}");

        for directory in [&root, &out, &other] {
            cleanup(directory);
        }
    }

    /// A selection that is not UTF-8 is refused rather than read as absence.
    ///
    /// `build.rs` reads the raw environment value; this is the parse it hands
    /// over. Silently treating it as unset would generate the only tracked
    /// version under an operator who explicitly asked for another.
    #[test]
    fn a_selection_that_is_not_utf8_is_refused() {
        use std::os::unix::ffi::OsStringExt;

        assert!(
            selection(None)
                .expect("nothing selected is not an error")
                .is_none()
        );
        assert_eq!(
            selection(Some(OsString::from("a-readable-name")))
                .expect("a readable value is a selection"),
            Some("a-readable-name".to_owned())
        );

        let error = selection(Some(OsString::from_vec(vec![0xff])))
            .expect_err("a value that is not UTF-8 names no directory");
        assert!(matches!(error, GenerateError::Input(_)));
        assert!(error.to_string().contains(SELECTION), "{error}");
    }

    /// Naming the only tracked version generates what naming nothing generates.
    ///
    /// This is what makes the decision cost nothing to adopt. Under a single
    /// tracked version, selection is the behaviour that already existed, and the
    /// comparison covers every Rust output and the Java trees' digest.
    #[test]
    fn selecting_the_only_version_generates_what_selecting_nothing_does() {
        // Staged rather than the repository, which tracks more than one version
        // now. What this proves is about a root holding exactly one, which is
        // the case the claim is about.
        let (root, directory) = staged("only-selection");
        let tracked = directory
            .file_name()
            .expect("the version directory is named")
            .to_string_lossy()
            .into_owned();
        let default = output("default-selection");
        let named = output("named-selection");

        let unselected = run(&root, &default, None).expect("generates");
        let selected = run(&root, &named, Some(&tracked)).expect("generates");

        assert_eq!(unselected.sha256, selected.sha256);
        for (left, right) in [
            (&unselected.bindings, &selected.bindings),
            (&unselected.exports, &selected.exports),
        ] {
            assert_eq!(
                std::fs::read(left).expect("read default Rust output"),
                std::fs::read(right).expect("read selected Rust output")
            );
        }

        for directory in [&default, &named, &root] {
            cleanup(directory);
        }
    }

    /// M4-W6's gate: the shared adapter is not welded to the pinned version.
    ///
    /// Nothing about this run is the pinned version's. The version data is another's,
    /// the classes and members it names are that version's own, and the generator
    /// reading it is the same code, unedited, that reads the pinned one. What the
    /// run produces names the second version and nothing but it, which is the
    /// statement: version-specific knowledge lives in version data, and the crate
    /// that reads it holds none.
    ///
    /// The hook ids are the exception that proves the rule. `ravel-minecraft` says
    /// an id is the only name that survives a game update, and general crates name
    /// ids: `ravel-bootstrap-runtime` projects events from them. So the fixture reviews the
    /// same id set the pinned version does, and the assertion below is that both
    /// tables answer to every one of them. A general crate needs no edit for a
    /// second version precisely because that set is what it depends on.
    #[test]
    fn the_generator_reads_a_second_version_without_naming_the_pinned_one() {
        let (root, version) = staged_second_version("second-version");
        let out = output("second-version-out");

        let generated = run(&root, &out, None).expect("a second version's data should generate");
        let tracked = inputs::tracked_versions(repository_root());
        assert!(
            !tracked.contains(&version),
            "a fixture that is a tracked version proves nothing"
        );

        // No generated artefact may name the pinned version, and the two that
        // record which version they came from have to record this one. The
        // exported symbols are the third and name no version at all, which is
        // itself the right answer: an export is a symbol the JVM resolves by
        // name, so a version in one would make the ABI version-specific.
        for artefact in [&generated.bindings, &generated.exports, &generated.java] {
            let text = std::fs::read_to_string(artefact).expect("readable");
            for other in &tracked {
                assert!(
                    !text.contains(other.as_str()),
                    "nothing generated from the second version may name a tracked one: {}",
                    artefact.display()
                );
            }
        }
        for artefact in [&generated.bindings, &generated.java] {
            let text = std::fs::read_to_string(artefact).expect("readable");
            assert!(
                text.contains(&version),
                "the artefact should name the version it was generated from: {}",
                artefact.display()
            );
        }
        let exports = std::fs::read_to_string(&generated.exports).expect("readable");
        assert!(
            !exports.contains(&version),
            "an exported symbol is resolved by name, so no version belongs in one"
        );

        the_generator_reads_a_second_version_check_surface(&root, &version, &generated);
        cleanup(&root);
        cleanup(&out);
    }

    fn the_generator_reads_a_second_version_check_surface(
        root: &Path,
        version: &str,
        generated: &Generated,
    ) {
        // Every reviewed id, in both versions, mapped onto different members.
        let second = inputs::pinned(root, None).expect("the staged data should parse");
        let bindings = std::fs::read_to_string(&generated.bindings).expect("readable");
        let pinned_inventory = inputs::tracked_version_where(repository_root(), |adapter| {
            !adapter.mapping_source.needs_mappings()
        })
        .inventory;
        let mut ids = pinned_inventory
            .hooks
            .iter()
            .map(|hook| hook.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let mut second_ids = second
            .inventory
            .hooks
            .iter()
            .map(|hook| hook.id.as_str())
            .collect::<Vec<_>>();
        second_ids.sort_unstable();
        assert_eq!(
            ids, second_ids,
            "a second version of the same capability surface reviews the same ids"
        );
        for hook in &second.inventory.hooks {
            assert!(
                bindings.contains(&format!("HookId::new(\"{}\")", hook.id)),
                "{} reached no generated target",
                hook.id
            );
        }
        assert!(
            !pinned_inventory.hooks.iter().any(|hook| second
                .inventory
                .hooks
                .iter()
                .any(|other| other.id == hook.id && other.class == hook.class)),
            "a second version that maps an id onto the same class is the same version"
        );

        // The stub half is version data too, so it moves with the rest.
        for seed in &second.seeds.stubs {
            let stub = generated.stubs.join(format!(
                "{}.java",
                crate::classfile::internal_name(&seed.class)
            ));
            let text = std::fs::read_to_string(&stub).expect("the seeded class should be stubbed");
            assert!(text.contains(version), "{text}");
        }
    }

    #[test]
    fn generation_is_reproducible_byte_for_byte() {
        let root = repository_root();
        // The version whose class files are the mapping. `run` reads a
        // translating source's published mapping on every build, jars or no
        // jars, and that file is local material — so selecting any
        // tracked version here would make this test pass on the one machine
        // holding it and fail everywhere else. What it asserts holds of every
        // version; what it needs is a version it can always build.
        let version =
            inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings())
                .adapter
                .minecraft;
        let first = output("once");
        let second = output("twice");

        let one = run(root, &first, Some(&version)).expect("generates");
        let two = run(root, &second, Some(&version)).expect("generates");

        assert_eq!(one.sha256, two.sha256);
        for (left, right) in [(&one.bindings, &two.bindings), (&one.exports, &two.exports)] {
            assert_eq!(
                std::fs::read_to_string(left).expect("readable"),
                std::fs::read_to_string(right).expect("readable")
            );
        }

        for directory in [&first, &second] {
            cleanup(directory);
        }
    }

    #[test]
    fn a_root_without_version_data_generates_nothing() {
        let error = run(
            Path::new("/nonexistent-ravel-root"),
            Path::new("/tmp"),
            None,
        )
        .expect_err("there is nothing to generate from");

        assert!(matches!(
            &error,
            GenerateError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(error.to_string().starts_with("cannot access"));
        assert!(!error.to_string().contains("/nonexistent-ravel-root"));
    }

    #[test]
    fn every_refusal_says_what_kind_of_refusal_it_is() {
        for (error, expected) in [
            (GenerateError::Input(String::new()), "an input is unusable"),
            (GenerateError::Drift(String::new()), "an input drifted"),
            (
                GenerateError::Unresolved(String::new()),
                "a hook does not resolve",
            ),
            (
                GenerateError::Extraction(String::new()),
                "extraction failed",
            ),
            (
                GenerateError::Unsupported(String::new()),
                "a hook cannot be generated",
            ),
        ] {
            assert!(error.to_string().starts_with(expected), "{error}");
        }
    }

    /// Rewrites the staged adapter inputs, one whole line at a time.
    fn rewrite(directory: &Path, from: &str, to: &str) {
        let path = directory.join("adapter.toml");
        let document = std::fs::read_to_string(&path).expect("readable");
        assert!(
            document.contains(from),
            "{from} is not in the staged inputs"
        );
        std::fs::write(&path, document.replace(from, to)).expect("writable");
    }

    #[test]
    fn a_source_and_a_mapping_pin_that_disagree_are_refused_before_anything_resolves() {
        let (root, directory) = staged("mapping-pairing");
        let out = output("mapping-pairing-out");
        let source = "mappingSource = \"unobfuscated-class-files\"";
        let mappings = "\n[inputs.clientMappings]\npath = \"nowhere/client.txt\"\nsha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n";

        rewrite(
            &directory,
            source,
            "mappingSource = \"official-proguard-mappings\"",
        );
        let missing = run(&root, &out, None).expect_err("a source that reads a mapping needs one");
        assert!(
            missing
                .to_string()
                .contains("the adapter inputs declare none"),
            "{missing}"
        );

        rewrite(
            &directory,
            "mappingSource = \"official-proguard-mappings\"",
            &format!("{source}{mappings}"),
        );
        let spurious =
            run(&root, &out, None).expect_err("a source that reads none may not pin one");
        assert!(
            spurious
                .to_string()
                .contains("the adapter inputs declare a mapping"),
            "{spurious}"
        );

        cleanup(&root);
        cleanup(&out);
    }

    /// Rewrites the staged reflective surface and re-pins it, as an edit would.
    ///
    /// The hash is updated in the same breath because a hand edit that leaves it
    /// stale is refused by the pin and asks a different question. What these
    /// tests ask is whether a *consistently pinned* name that the mapping does
    /// not agree with is caught, which is the defect a presence check misses.
    fn restate_surface(directory: &Path, from: &str, to: &str) {
        let path = directory.join("reflection.toml");
        let document = std::fs::read_to_string(&path).expect("readable");
        assert!(
            document.contains(from),
            "{from} is not in the staged surface"
        );
        let rewritten = document.replacen(from, to, 1);
        std::fs::write(&path, &rewritten).expect("writable");
        let adapter = directory.join("adapter.toml");
        let inputs = std::fs::read_to_string(&adapter).expect("readable");
        let pinned = inputs
            .lines()
            .find(|line| line.contains(&inputs::sha256(document.as_bytes()).to_string()))
            .expect("the staged inputs pin the surface");
        let repinned = inputs
            .replace(
                pinned,
                &format!("sha256 = \"{}\"", inputs::sha256(rewritten.as_bytes())),
            )
            .replace(
                &format!("bytes = {}", document.len()),
                &format!("bytes = {}", rewritten.len()),
            );
        std::fs::write(&adapter, repinned).expect("writable");
    }

    /// The line of the staged surface that declares a value under one key.
    ///
    /// The value is read out of the tracked data rather than written here, so no
    /// source in this crate spells a game class name — the property
    /// `ravel-verify-scaffold` enforces over `crates/`.
    fn declared_line(directory: &Path, key: &str) -> String {
        let document =
            std::fs::read_to_string(directory.join("reflection.toml")).expect("readable");
        document
            .lines()
            .find(|line| line.starts_with(&format!("{key} = ")))
            .expect("the staged surface declares the key")
            .to_owned()
    }

    /// The defect a presence check over reviewed names cannot see: the reviewed
    /// name is right and the name the JVM will link is stale, so the check passes
    /// and every reflective reach fails at runtime.
    #[test]
    fn a_runtime_name_the_mapping_does_not_resolve_to_is_refused() {
        let (root, directory) = staged("surface-runtime");
        let out = output("surface-runtime-out");
        let declared = declared_line(&directory, "runtime");

        restate_surface(&directory, &declared, "runtime = \"nowhere.Absent\"");

        let error = run(&root, &out, None).expect_err("a stale runtime name must be refused");

        assert!(matches!(error, GenerateError::Drift(_)), "{error}");
        assert!(
            error.to_string().contains("links as nowhere.Absent"),
            "{error}"
        );
        cleanup(&root);
        cleanup(&out);
    }

    /// Declares the actual members the jar-less fixture resolves on each named class.
    fn fixture_proguard(pinned: &inputs::PinnedVersion, stubs: &[super::stubs::Stub]) -> String {
        let mut mapping = String::new();
        for reflected in &pinned.surface.classes {
            writeln!(mapping, "{} -> {}:", reflected.class, reflected.runtime)
                .expect("write mapping class");
            let owner = crate::classfile::internal_name(&reflected.class);
            let mut members = BTreeSet::new();
            for stub in stubs.iter().filter(|stub| stub.class == owner) {
                members.extend(
                    stub.members
                        .iter()
                        .map(|member| (member.name.as_str(), member.descriptor.as_str())),
                );
            }
            for hook in &pinned.inventory.hooks {
                if crate::classfile::internal_name(&hook.class) == owner {
                    members.insert((hook.member.as_str(), hook.descriptor.as_str()));
                }
            }
            for member in &reflected.members {
                if let inputs::Lookup::Descriptor(descriptor) = &member.lookup {
                    members.insert((member.name.as_str(), descriptor.as_str()));
                }
            }
            for (name, descriptor) in members {
                if descriptor.starts_with('(') {
                    let (parameters, returns) = super::classfile::signature(descriptor)
                        .expect("reviewed method descriptor");
                    writeln!(
                        mapping,
                        "    {returns} {name}({}) -> {name}",
                        parameters.join(",")
                    )
                    .expect("write mapping method");
                } else {
                    let (kind, _) =
                        super::classfile::java_type(descriptor).expect("reviewed field descriptor");
                    writeln!(mapping, "    {kind} {name} -> {name}").expect("write mapping field");
                }
            }
        }
        mapping
    }

    /// Restages the copied version as an obfuscated one that publishes a mapping.
    ///
    /// This is the shape no tracked version has yet and the one the surface check
    /// exists for: the class files declare nothing a reviewer can read, so a
    /// published file is the only thing that decides the linked names. The
    /// mapping is built from the surface the staged version already declares,
    /// rather than written here, so no source in this crate spells a game class
    /// name and the staged version stays internally consistent — what changes is
    /// that a file now has to *say* what was previously true by identity.
    fn stage_proguard(root: &Path, directory: &Path) {
        let identity = "mappingSource = \"unobfuscated-class-files\"";
        let obfuscated = "mappingSource = \"official-proguard-mappings\"";

        let pinned = inputs::pinned(root, None).expect("the staged fixture is pinned");
        let stub_lock = std::fs::read_to_string(directory.join("stubs.lock")).expect("stub lock");
        let stubs = super::stubs::from_lock(&stub_lock, &pinned.adapter, &pinned.seeds)
            .expect("the fixture stub lock resolves");
        let mapping = fixture_proguard(&pinned, &stubs);
        assert!(!mapping.is_empty(), "the staged surface declares nothing");
        let mapping_path = directory.join("client.txt");
        std::fs::write(&mapping_path, &mapping).expect("writable");

        // The lock carries the source that decided its names and the digest of
        // the file that did, so restating the source without it is a lock that
        // disagrees with what regenerating it would produce.
        let lock_path = directory.join("mappings.lock");
        let lock = std::fs::read_to_string(&lock_path).expect("readable");
        let jar_pin = lock
            .lines()
            .find(|line| line.starts_with("clientJarSha256 = "))
            .expect("the lock pins the jar it read");
        let relocked = lock.replace(identity, obfuscated).replace(
            jar_pin,
            &format!(
                "{jar_pin}\nclientMappingsSha256 = \"{}\"",
                inputs::sha256(mapping.as_bytes())
            ),
        );
        std::fs::write(&lock_path, &relocked).expect("writable");

        let relative = directory
            .strip_prefix(root)
            .expect("the version is staged under the root")
            .join("client.txt");
        let adapter_path = directory.join("adapter.toml");
        let adapter = std::fs::read_to_string(&adapter_path).expect("readable");
        let restated = adapter
            .replace(identity, obfuscated)
            .replace(
                &inputs::sha256(lock.as_bytes()).to_string(),
                &inputs::sha256(relocked.as_bytes()).to_string(),
            )
            .replace(
                &format!("bytes = {}", lock.len()),
                &format!("bytes = {}", relocked.len()),
            )
            + &format!(
                "\n[inputs.clientMappings]\npath = \"{}\"\nsha256 = \"{}\"\nbytes = {}\n",
                relative.display(),
                inputs::sha256(mapping.as_bytes()),
                mapping.len()
            );
        std::fs::write(&adapter_path, restated).expect("writable");
    }

    /// The case the guard used to skip: no jars, and a source whose names only a
    /// published mapping decides. That is the only case where the game is
    /// actually obfuscated, so a surface trusted here is a surface nothing checks.
    #[test]
    fn a_machine_without_the_jars_re_derives_an_obfuscated_surface_from_the_mapping() {
        let (root, directory) = staged("surface-proguard");
        let out = output("surface-proguard-out");
        stage_proguard(&root, &directory);

        let generated = run(&root, &out, None).expect("the mapping says what the surface declares");

        assert!(generated.java.is_file());

        cleanup(&root);
        cleanup(&out);
    }

    /// The same staging with one runtime name restated: a lock-only build under
    /// an obfuscated source used to render this into the shipped jar untouched.
    #[test]
    fn a_stale_runtime_name_is_refused_without_the_jars_under_an_obfuscated_source() {
        let (root, directory) = staged("surface-proguard-stale");
        let out = output("surface-proguard-stale-out");
        stage_proguard(&root, &directory);
        let declared = declared_line(&directory, "runtime");

        restate_surface(&directory, &declared, "runtime = \"nowhere.Absent\"");

        let error = run(&root, &out, None).expect_err("a stale runtime name must be refused");

        assert!(matches!(error, GenerateError::Drift(_)), "{error}");
        assert!(
            error.to_string().contains("links as nowhere.Absent"),
            "{error}"
        );
        cleanup(&root);
        cleanup(&out);
    }

    /// A surface copied from another version keeps a Mojang name the target
    /// renamed away, and the natural way to restate it keeps both names in step.
    /// The mapping then names neither, so a pass-through would compare the stale
    /// name with itself and agree — which is the rename this whole record is for.
    #[test]
    fn a_reviewed_class_the_mapping_never_names_is_refused_rather_than_read_as_itself() {
        let (root, directory) = staged("surface-proguard-absent");
        let out = output("surface-proguard-absent-out");
        stage_proguard(&root, &directory);
        let restated = format!(
            "{}\n{}",
            declared_line(&directory, "class"),
            declared_line(&directory, "runtime")
        );

        restate_surface(
            &directory,
            &restated,
            "class = \"nowhere.Absent\"\nruntime = \"nowhere.Absent\"",
        );

        let error = run(&root, &out, None).expect_err("a class the mapping omits must be refused");

        assert!(matches!(error, GenerateError::Unresolved(_)), "{error}");
        assert!(
            error.to_string().contains("names no class nowhere.Absent"),
            "{error}"
        );
        cleanup(&root);
        cleanup(&out);
    }

    /// All three declared values are interpolated into `GameNames`: the role as a
    /// field name, the runtime name as its string literal, and the reviewed name
    /// as the comment beside it. A value that is not the thing it is declared as
    /// is refused where the file is read rather than rendered and compiled.
    #[test]
    fn a_declared_value_that_is_not_a_java_name_is_refused_before_it_is_rendered() {
        for (label, key, restated) in [
            (
                "role",
                "name",
                "name = \"MINECRAFT; static native void y()\"",
            ),
            ("runtime", "runtime", "runtime = \"not a class name\""),
            ("reviewed", "class", "class = \"not a class name\""),
        ] {
            let (root, directory) = staged(&format!("surface-illegal-{label}"));
            let out = output(&format!("surface-illegal-{label}-out"));

            restate_surface(&directory, &declared_line(&directory, key), restated);

            let error = run(&root, &out, None).expect_err("a value that is not one is refused");

            assert!(matches!(error, GenerateError::Drift(_)), "{error}");
            assert!(error.to_string().contains("which is not one"), "{error}");
            cleanup(&root);
            cleanup(&out);
        }
    }

    /// One role names one class, so a file declaring a role twice has said two
    /// things about one generated constant and only one of them can be rendered.
    #[test]
    fn a_reflective_surface_declaring_a_role_twice_is_refused() {
        let (root, directory) = staged("surface-duplicate");
        let repeated = format!(
            "[[reflected]]\n{}\n{}\n{}\n\n[[reflected]]",
            declared_line(&directory, "name"),
            declared_line(&directory, "class"),
            declared_line(&directory, "runtime")
        );

        restate_surface(&directory, "[[reflected]]", &repeated);

        let error = inputs::pinned(&root, None).expect_err("one role names one class");

        assert!(matches!(error, GenerateError::Drift(_)), "{error}");
        assert!(error.to_string().contains("twice"), "{error}");
        cleanup(&root);
    }

    /// The generated class carries both names the surface declares, because two
    /// audiences read it: the JVM links the value and a reviewer reads the name
    /// beside it.
    #[test]
    fn the_generated_game_names_carry_the_linked_name_and_the_reviewed_one() {
        let out = output("game-names");
        let root = repository_root();
        // The version whose class files are the mapping. `run` reads a
        // translating source's published mapping on every build, jars or no
        // jars, and that file is local material — so selecting any
        // tracked version here would make this test pass on the one machine
        // holding it and fail everywhere else. What it asserts holds of every
        // version; what it needs is a version it can always build.
        let pinned =
            inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings());

        run_and_inspect(root, &out, Some(&pinned.adapter.minecraft), |generated| {
            let names = generated
                .java
                .parent()
                .expect("the generated Java has a directory")
                .join("GameNames.java");
            let source = std::fs::read_to_string(&names).expect("the class should be generated");
            assert!(source.contains("final class GameNames {"), "{source}");
            assert!(
                !source.contains("native"),
                "GameNames declares no native method, keeping one native symbol surface: {source}"
            );
            // Both names of every declared role, in one place a reviewer reads and
            // the JVM links out of. Read from the surface rather than written here,
            // so no source in this crate spells a game class name.
            for reflected in &pinned.surface.classes {
                assert!(
                    source.contains(&format!(
                        "    /** {}. */\n    static final String {} = \"{}\";",
                        reflected.class, reflected.name, reflected.runtime
                    )),
                    "{} is not carried as a pair: {source}",
                    reflected.name
                );
            }
        })
        .expect("the tracked inputs should generate");

        cleanup(&out);
    }

    /// A driver names Ravel's own screen by the whole name, wherever it landed.
    ///
    /// The screens move with the game: a screen extends a game class, so a
    /// version that draws the game in the unnamed package draws them there too.
    /// A driver is in the unnamed package under every version, so it can import
    /// nothing and has to spell whatever name this version gave the screen. What
    /// is asserted is that the two agree — the name the driver spells is the
    /// binary name the screen tree actually wrote — which is invisible in the
    /// driver's own text, because the template says only which screen it means.
    #[test]
    fn a_driver_spells_ravels_screen_where_this_version_drew_it() {
        let out = output("driver-screen-name");
        let root = repository_root();
        // The version whose class files are the mapping, for the reason the
        // test above states: any other needs local mapping material.
        let pinned =
            inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings());

        run_and_inspect(root, &out, Some(&pinned.adapter.minecraft), |generated| {
            // Where the screen landed, read off the tree rather than written here:
            // its path below the screen root is its package, and the two together
            // are the name a driver has to spell.
            let mut screens = Vec::new();
            collect_java(&generated.screens, &generated.screens, &mut screens);
            let settings = screens
                .iter()
                .find(|name| name.ends_with("RavelSettingsScreen"))
                .expect("the screen tree draws a settings screen");

            let driver = std::fs::read_to_string(
                generated
                    .harness
                    .join("driver")
                    .join("SettingsScreenMain.java"),
            )
            .expect("the settings driver should be generated");

            assert!(
                driver.contains(&format!("{settings}.open(")),
                "the driver has to open the screen where it was drawn, not {settings}: {driver}"
            );
            assert!(
                !driver.contains(&format!("import {settings};")),
                "a driver is in the unnamed package and imports nothing: {driver}"
            );
        })
        .expect("the tracked inputs should generate");

        cleanup(&out);
    }

    /// Every generated Java source below one root, as the binary name it holds.
    fn collect_java(root: &Path, directory: &Path, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_java(root, &path, found);
            } else if path.extension().and_then(|end| end.to_str()) == Some("java") {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                found.push(
                    relative
                        .with_extension("")
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "."),
                );
            }
        }
    }

    #[test]
    fn a_client_mapping_that_no_longer_hashes_to_its_pin_is_refused_before_its_text_is_read() {
        let root = repository_root();
        // The version that declares no mapping, because this test gives it one
        // whose hash is wrong and a version that already had one would carry two.
        let pinned =
            inputs::tracked_version_where(root, |adapter| !adapter.mapping_source.needs_mappings());
        let mut adapter = pinned.adapter;
        // A real file under a hash that is not its own. Nothing may be read out
        // of it, and nothing may be opened before it is refused.
        adapter.mapping_source = inputs::MappingSource::OfficialProguardMappings;
        adapter.inputs.client_mappings = Some(inputs::Input {
            path: adapter.inputs.hook_inventory.path.clone(),
            sha256: Some("0".repeat(64).try_into().expect("complete digest")),
            bytes: None,
            upstream: None,
        });

        let error = super::mapping::Mapping::load(root, &adapter)
            .expect_err("a drifted mapping must be refused");

        assert!(matches!(error, GenerateError::Drift(_)), "{error}");
        assert!(error.to_string().contains("hooks.toml"), "{error}");
        assert!(!error.to_string().contains('/'), "a message names no path");
    }
}
