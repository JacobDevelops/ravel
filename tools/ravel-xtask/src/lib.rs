#![forbid(unsafe_code)]

//! Repository automation for Ravel.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

mod instance_scan;
mod package_digests;

pub use instance_scan::{Finding, InstanceScan, scan_instance};
pub use package_digests::{DigestOutcome, StaleDigest, refresh_digests};

const CRATE_NAMES: &[&str] = &[
    "ravel-core",
    "ravel-package",
    "ravel-resolve",
    "ravel-loader",
    "ravel-sdk",
    "ravel-sdk-macros",
    "ravel-host-api",
    "ravel-host-runtime",
    "ravel-bootstrap",
    "ravel-composed-host",
    "ravel-build",
    "ravel-minecraft",
    "ravel-minecraft-adapter",
    "ravel-config",
    "ravel-data",
    "ravel-save",
    "ravel-network",
    "ravel-observability",
    "ravel-pack",
    "ravel-acceptance",
    "ravel-testkit",
];

const TEST_MOD_NAMES: &[&str] = &[
    "audio",
    "client-only",
    "commands",
    "configuration",
    "dependency-base",
    "dependency-consumer",
    "events",
    "failure-cases",
    "inter-mod-service",
    "interception",
    "networking",
    "noop",
    "optional-service",
    "provision",
    "registries",
    "render-stages",
    "resources",
    "saved-data",
    "server-data",
    "server-only",
    "waystone-ledger",
    "world-shaping",
];

/// Example crates a mod author reads, and the only crates whose manifests are
/// deliberately as small as an author's.
const EXAMPLE_NAMES: &[&str] = &[
    "content-mod",
    "guide-snippets",
    "minimal-mod",
    "stateful-mod",
];

const EXPERIMENT_NAMES: &[&str] = &[
    "adg-1-jvm-entry",
    "adg-2-guest",
    "adg-2-mod-execution",
    "adg-3-bindings",
    "adg-4-state",
    "adg-5-prism",
    "m2-loader-overhead",
    "m3-pack-compile",
    "m3-profiling-overhead",
    "m6-outward-crossing",
    "spike-harness",
];

/// Gate directories under `evidence/m1/spikes` and the gate ID they must carry.
const SPIKE_GATES: &[(&str, &str)] = &[
    ("adg-1", "ADG-1"),
    ("adg-2", "ADG-2"),
    ("adg-3", "ADG-3"),
    ("adg-4", "ADG-4"),
    ("adg-5", "ADG-5"),
];

/// Every milestone manifest the evidence gate reads, named rather than discovered.
///
/// Discovery by directory scan gave the gate no floor. Moving all four manifests
/// away left `verify scaffold` reporting zero manifests, zero violations, and
/// "verification passed" — the absence of the evidence was flattened into the
/// absence of a complaint by `.into_iter().flatten().flatten()`, and `ROOT_FILES`
/// asked only for two READMEs. Naming the manifests is what makes a missing one a
/// violation, and the scan is now checked against this list rather than being the
/// whole of it.
const MILESTONE_MANIFESTS: &[(&str, &str)] = &[
    ("evidence/m0/manifest.json", "M0"),
    ("evidence/m1/manifest.json", "M1"),
    ("evidence/m2/manifest.json", "M2"),
    ("evidence/m3/manifest.json", "M3"),
    ("evidence/m4/manifest.json", "M4"),
    ("evidence/m5/manifest.json", "M5"),
];

const ROOT_FILES: &[&str] = &[
    ".cargo/config.toml",
    ".config/nextest.toml",
    ".envrc",
    ".gitignore",
    ".markdownlint.yaml",
    ".tangled/workflows/ci.yml",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
    "ROADMAP.md",
    "SPEC.md",
    "deny.toml",
    "devenv.lock",
    "devenv.nix",
    "devenv.yaml",
    "docs/architecture/m1-measurement-gate.md",
    "docs/decisions/README.md",
    "evidence/m1/README.md",
    "evidence/m2/README.md",
    "docs/runbooks/darwin-verification.md",
    "experiments/README.md",
    "flake.lock",
    "flake.nix",
    "jvm/README.md",
    "jvm/bootstrap/dev/ravel/jvm/Agent.java",
    "jvm/bootstrap/dev/ravel/jvm/HookTarget.java",
    "jvm/bootstrap/dev/ravel/jvm/Screens.java",
    "jvm/bootstrap/dev/ravel/jvm/Transformer.java",
    "jvm/build/build-agent.sh",
    "jvm/build/build-screen.sh",
    "jvm/harness/Main.java",
    "jvm/harness/net/minecraft/util/StringUtil.java",
    "jvm/harness/run-harness.sh",
    "jvm/screen/template/RavelModsScreen.java.in",
    "jvm/screen/template/RavelSettingsScreen.java.in",
    "jvm/screen/template/Wire.java.in",
    "minecraft/README.md",
    "nix/ci.nix",
    "nix/common.nix",
    "nix/darwin.nix",
    "nix/linux.nix",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "schemas/configuration-override.schema.json",
    "schemas/configuration.schema.json",
    "schemas/evidence.schema.json",
    "schemas/fixture.schema.json",
    "schemas/pack.schema.json",
    "schemas/package.schema.json",
    "schemas/performance-collector.schema.json",
    "schemas/performance-result.schema.json",
    "schemas/provenance.schema.json",
    "schemas/runtime-inputs.schema.json",
    "schemas/saved-data.schema.json",
    "schemas/spike-result.schema.json",
    "tests/conformance/Cargo.toml",
    "tests/fixtures/README.md",
    "tests/fixtures/local/README.md",
    "tests/fixtures/public/README.md",
    "tools/ravel-cli/Cargo.toml",
    "tools/ravel-xtask/Cargo.toml",
];

/// The directory holding one tracked directory per supported Minecraft version.
const VERSION_DATA: &str = "minecraft/versions";

/// The files every tracked version directory carries.
///
/// These seven were spelled once, as `minecraft/versions/26.1.2/...`, inside
/// [`ROOT_FILES`]. A path is a fine way to require a file and a poor way to
/// require a *shape*: the moment a second version directory is tracked, a literal
/// list asks nothing of it, so a version arriving without its locks is caught by
/// nothing. `version-selection-is-a-build-time-choice.md` makes that gap explicit —
/// a build reads the directory it selected and no other, so the guarantee that
/// every tracked version is well formed has no owner but this sweep.
const VERSION_FILES: [&str; 7] = [
    "README.md",
    "adapter.toml",
    "hooks.toml",
    "mappings.lock",
    "reflection.toml",
    "stubs.lock",
    "stubs.toml",
];

/// The one file in [`VERSION_FILES`] that carries prose rather than a declaration.
const VERSION_PROSE_FILE: &str = "README.md";

/// The version a tracked version-data file declares about itself.
///
/// Every file in [`VERSION_FILES`] but the README writes a top-level
/// `minecraft = "..."` key, and the first one is the file's own claim: the keys
/// below it belong to `[[hook]]` and `[[reflected]]` tables that declare no
/// version.
fn declared_version(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find_map(|line| quoted_value(line, "minecraft"))
}

/// Every version directory tracked under [`VERSION_DATA`], in name order.
fn tracked_versions(root: &Path) -> Result<Vec<String>, String> {
    let entries = fs::read_dir(root.join(VERSION_DATA))
        .map_err(|error| format!("cannot read {VERSION_DATA}: {error}"))?;
    let mut versions = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    versions.sort();
    Ok(versions)
}

/// Holds every tracked version directory to the same file set, to the version its
/// own name spells, and to a countable inventory.
///
/// The self-declaration half is what a copy-paste cannot satisfy.
/// `cp -r minecraft/versions/26.1.2 minecraft/versions/1.21.1` produces seven
/// present files and a countable inventory while every one of them still declares
/// `26.1.2`, so a directory serving one version's bindings under another's name
/// was well formed in shape and wrong in content. `generate::inputs::pinned`
/// compares the same three keys but only for the directory a build *selected*, and
/// its own comment delegates the all-versions case here, so an unselected
/// directory was checked by nothing at all.
///
/// The countability half is not decoration either. `hook_counts` is what every
/// figure derived from an inventory rests on, and it is the only thing in this file
/// that reads an inventory's *structure* rather than grepping its lines, so an entry
/// missing its kind or its descriptor is a malformed version that no other check
/// notices. Under one tracked directory the document check below happened to run
/// it; under two, the one the document does not name would go unread.
fn verify_version_directories(root: &Path, violations: &mut Vec<String>) {
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    if versions.is_empty() {
        violations.push(format!(
            "{VERSION_DATA} holds no version directory, so every check derived from version data \
             sweeps nothing and passes by having nothing to say"
        ));
        return;
    }

    for version in &versions {
        for file in VERSION_FILES {
            let relative = format!("{VERSION_DATA}/{version}/{file}");
            if !root.join(&relative).is_file() {
                violations.push(format!("missing required file: {relative}"));
                continue;
            }
            if file == VERSION_PROSE_FILE {
                continue;
            }
            let text = match fs::read_to_string(root.join(&relative)) {
                Ok(text) => text,
                Err(error) => {
                    violations.push(format!("cannot read {relative}: {error}"));
                    continue;
                }
            };
            match declared_version(&text) {
                Some(declared) if declared == version => {}
                Some(declared) => violations.push(format!(
                    "{relative} declares Minecraft {declared} while its directory is named \
                     {version}; a directory copied from another version and never re-pinned \
                     serves one version's data under another version's name"
                )),
                None => violations.push(format!(
                    "{relative} declares no `minecraft` version, so nothing holds it to the \
                     version {VERSION_DATA}/{version} is named for"
                )),
            }
            if file == "hooks.toml"
                && let Err(error) = hook_counts(&text)
            {
                violations.push(format!("cannot count {relative}: {error}"));
            }
        }
    }
}

/// One tracked version's reviewed hook inventory, read once for every check that
/// derives something from it.
struct HookInventory {
    /// The version directory's name.
    version: String,
    /// The inventory's repository-relative path, which the messages name.
    path: String,
    /// The file's text.
    text: String,
}

/// Every tracked version's inventory, skipping the ones that cannot be read.
///
/// Silently, because [`verify_version_directories`] is the check that reports a
/// missing or unreadable inventory, and a second report of the same fact from
/// every consumer is noise rather than coverage.
fn hook_inventories(root: &Path) -> Vec<HookInventory> {
    let Ok(versions) = tracked_versions(root) else {
        return Vec::new();
    };
    versions
        .into_iter()
        .filter_map(|version| {
            let path = format!("{VERSION_DATA}/{version}/hooks.toml");
            let text = fs::read_to_string(root.join(&path)).ok()?;
            Some(HookInventory {
                version,
                path,
                text,
            })
        })
        .collect()
}

/// Returns the files that define the minimum public repository scaffold.
#[must_use]
pub fn required_scaffold_paths() -> Vec<String> {
    ROOT_FILES
        .iter()
        .map(|path| (*path).to_owned())
        .chain(
            MILESTONE_MANIFESTS
                .iter()
                .map(|(relative, _)| (*relative).to_owned()),
        )
        .chain(
            CRATE_NAMES
                .iter()
                .map(|name| format!("crates/{name}/Cargo.toml")),
        )
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/Cargo.toml")),
        )
        .chain(
            EXPERIMENT_NAMES
                .iter()
                .map(|name| format!("experiments/{name}/Cargo.toml")),
        )
        .collect()
}

/// Checks the repository's initial public scaffold and tracked-file policy.
#[must_use]
pub fn verify_scaffold(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    for relative in required_scaffold_paths() {
        if !root.join(&relative).is_file() {
            violations.push(format!("missing required file: {relative}"));
        }
    }

    for relative in maintained_rust_roots(root) {
        match fs::read_to_string(root.join(&relative)) {
            Ok(source) if declares_unsafe_prohibition(&source) => {}
            Ok(_) => violations.push(format!("missing unsafe prohibition: {relative}")),
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }

    verify_ignored_boundaries(root, &mut violations);
    verify_version_directories(root, &mut violations);
    let inventories = hook_inventories(root);
    verify_hook_inventory_document(root, &inventories, &mut violations);
    verify_decision_hook_claims(root, &inventories, &mut violations);
    verify_campaign_inventory(root, &mut violations);
    verify_review_round_counts(root, &mut violations);
    verify_review_blocking_counts(root, &mut violations);
    verify_manifest_percentages(root, &mut violations);
    verify_void_limits(root, &mut violations);
    verify_harness_lifecycles(root, &mut violations);
    verify_persistence_reflection(root, &mut violations);
    verify_hook_name_literals(root, &inventories, &mut violations);
    verify_transformer_shape(root, &inventories, &mut violations);
    verify_screen_templates(root, &mut violations);
    verify_screen_overrides(root, &mut violations);
    verify_version_readmes(root, &mut violations);
    verify_guide_enumerations(root, &mut violations);
    verify_schema_documents(root, &mut violations);
    verify_version_boundary(root, &mut violations);
    verify_example_inventory(root, &mut violations);
    verify_test_mod_inventory(root, &mut violations);
    verify_experiment_fence(root, &mut violations);
    violations.extend(verify_evidence_records(root));

    match tracked_paths(root) {
        Ok(paths) => {
            violations.extend(verify_tracked_paths(paths.iter().map(String::as_str)));
            violations.extend(verify_tracked_contents(root, &paths));
            verify_game_type_claims(root, &paths, &mut violations);
        }
        Err(error) => violations.push(error),
    }

    violations
}

/// Whether a crate root carries the unsafe prohibition as its own inner attribute.
///
/// A `contains` test was satisfied by prose. `crates/ravel-bootstrap/src/lib.rs`
/// quotes `#![forbid(unsafe_code)]` in a doc comment explaining why the crate can
/// keep it, so deleting the real attribute and adding a working `unsafe` block
/// left this check green. The boundary itself never moved — `Cargo.toml` forbids
/// `unsafe_code` for the whole workspace — which is exactly what made the
/// redundant control's vacuity invisible. Anchoring on the attribute's own line
/// is what a doc comment cannot satisfy.
fn declares_unsafe_prohibition(source: &str) -> bool {
    source
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("#![forbid(unsafe_code)]"))
}

/// The document that describes one version's inventory, and M3-W0's closing evidence.
const HOOK_DOCUMENT: &str = "docs/architecture/m3-hook-targets.md";

/// The launch record that enumerates the inventory M3's campaigns reviewed.
const CAMPAIGN_MATRIX: &str = "evidence/m3/linux-process-matrix.json";

/// The prose that restates that record's inventory figures.
const CAMPAIGN_DOCUMENT: &str = "evidence/m3/README.md";

/// The milestones whose prose is exempt from the round-count rule.
///
/// Both are closed and their reviews are over, so the counts their records
/// restate cannot go stale: no further round will run against them. Bringing them
/// in would mean rewording a closed milestone's prose to satisfy a rule written
/// after it. Every other milestone is swept, and the set is derived from
/// [`MILESTONE_MANIFESTS`] rather than listed, for the reason
/// [`round_count_subjects`] gives.
const ROUND_COUNT_EXEMPT: [&str; 2] = ["M0", "M1"];

/// The plan, which belongs to no milestone's evidence and cites all of it.
const PLAN_DOCUMENT: &str = "ROADMAP.md";

/// The harness whose runs are the world lifecycles M3's prose counts.
const PERSISTENCE_HARNESS: &str = "jvm/harness/run-harness.sh";

/// What `hooks.toml` holds, counted from `hooks.toml` and from nothing else.
struct HookCounts {
    /// Every `[[hook]]` block.
    entries: usize,
    /// Blocks whose `kind` is a method, so blocks the generator gives a prologue.
    methods: usize,
    /// Blocks whose `kind` is a field, which carry no parameters.
    fields: usize,
    /// Reference-type method parameters the generator renders as a `JavaHandle`.
    handles: usize,
}

/// Holds `docs/architecture/m3-hook-targets.md` to the inventory it describes.
///
/// M3-W0 closes on that document and `evidence/m3/manifest.json` pins its digest
/// as an M3 input, so the gate was satisfied by a document that had gone stale:
/// it still said 35 entries, 31 methods and 23 handle parameters after ADG-7's two
/// world persistence seams entered the inventory, while tabling those two seams
/// further down the same file. A digest binds a document to its bytes and never to
/// the thing it is about, which is why nothing caught it. These four counts are
/// derivable from `hooks.toml` alone, so they are derived here and compared rather
/// than trusted.
///
/// The document is about one version — its own title says so — and the version it
/// is about is read out of the version-directory paths it links into rather than
/// out of a constant here. That is what stops one document's figures from being
/// asserted against a second version's inventory, which is a different file with a
/// different count and no obligation to agree. A document that links into no
/// version directory, or into more than one, names no subject and is refused: a
/// second version needs a second document rather than a shared one that has to be
/// true twice.
fn verify_hook_inventory_document(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    let document = match fs::read_to_string(root.join(HOOK_DOCUMENT)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!("cannot read {HOOK_DOCUMENT}: {error}"));
            return;
        }
    };

    let named = documented_versions(&document);
    let mut naming = named.iter();
    let (Some(version), None) = (naming.next(), naming.next()) else {
        let found = named.iter().cloned().collect::<Vec<_>>().join(", ");
        violations.push(format!(
            "{HOOK_DOCUMENT} links into {} version directories ({found}); it states one \
             inventory's counts, so the inventory it is held to has to be the one it names",
            named.len()
        ));
        return;
    };

    let Some(inventory) = inventories
        .iter()
        .find(|inventory| &inventory.version == version)
    else {
        violations.push(format!(
            "{HOOK_DOCUMENT} states the counts of {VERSION_DATA}/{version}, which is not a \
             tracked version whose inventory can be read"
        ));
        return;
    };

    let counts = match hook_counts(&inventory.text) {
        Ok(counts) => counts,
        Err(error) => {
            violations.push(format!("cannot count {}: {error}", inventory.path));
            return;
        }
    };
    violations.extend(hook_document_violations(
        &document,
        &inventory.path,
        &counts,
    ));
}

/// Every version directory a document links into.
///
/// A link is structure rather than prose, which is why the subject is read from
/// one. The name is taken as the run of non-slash, non-whitespace characters that
/// a `/` closes, so a sentence that writes the directory itself without naming a
/// version inside it contributes nothing.
fn documented_versions(document: &str) -> BTreeSet<String> {
    let prefix = format!("{VERSION_DATA}/");
    let mut named = BTreeSet::new();
    for (start, _) in document.match_indices(prefix.as_str()) {
        let rest = &document[start + prefix.len()..];
        let bytes = rest.as_bytes();
        let mut end = 0;
        while end < bytes.len() && bytes[end] != b'/' && !bytes[end].is_ascii_whitespace() {
            end += 1;
        }
        if end > 0 && bytes.get(end) == Some(&b'/') {
            named.insert(rest[..end].to_owned());
        }
    }
    named
}

/// The directory holding the accepted decision records.
const DECISION_RECORDS: &str = "docs/decisions";

/// The word a decision record denies a hook with.
///
/// Paired with [`DENIAL_VERBS`] rather than used alone, because `not` alone
/// appears in most sentences and `hook` alone appears in every one of these
/// records dozens of times. What the pair reaches is the shape a denial takes:
/// the negation adjacent to a form of the verb.
const DENIAL: &str = "not";

/// The forms of `hook` a denial may be written with.
///
/// The inflected passive is the more natural phrasing of the sentence this check
/// exists over — "the `extractRenderState` seam is not hooked" — and holding only
/// the infinitive left that outside the anchor.
const DENIAL_VERBS: [&str; 3] = ["hook", "hooked", "hooking"];

/// The absolute denial of any per-frame crossing, which one hook falsifies.
const NO_PER_FRAME: &str = "nothing crosses per frame";

/// The inventory entry that makes [`NO_PER_FRAME`] false.
const PER_FRAME_MEMBER: &str = "extractRenderState";

/// Holds a decision record's claims about hooks to the inventory that declares
/// them.
///
/// Two accepted decisions survived eight review rounds describing a hook surface
/// the repository does not have. ADG-6 said the inventory omitted
/// `extractRenderState` and that no crossing happened per frame, while
/// `hooks.toml` had declared `screen.extract-render-state` since M3-W0 and a
/// launch had rewritten it. The only ADR checks that existed asked whether the
/// file was present, so a record could deny any mechanism it liked. This binds the
/// denial to the inventory: a record may not deny a hook the inventory declares,
/// and may not deny per-frame crossing while a per-frame member is declared.
///
/// Known limits, and they are narrow on purpose.
///
/// It reads two anchors and no more. A denial has to be written as [`DENIAL`]
/// adjacent to one of [`DENIAL_VERBS`], with a hook id or a member name the
/// inventory declares within [`COUNT_LOOKBACK`] words on either side of that pair,
/// and the absolute per-frame denial has to be written as [`NO_PER_FRAME`]. Both
/// inflections and both word orders are reached, so "does not hook
/// `extractRenderState`" and "the `extractRenderState` seam is not hooked" both
/// fire. A record that denies the same hook without the negation and the verb —
/// "the extraction seam is left alone", "Ravel leaves the extraction seam alone" —
/// is not reached, and no general prose parser is attempted here for the reason
/// `hook_document_violations` gives about telling a current claim from a
/// historical one.
///
/// It cannot tell a claim from a quotation. A record that quotes a withdrawn
/// denial in order to withdraw it fails this check exactly as one that still
/// asserts it does, so an amendment has to describe the sentence it repeals rather
/// than reprint it. That is the cost of not parsing prose, it is paid in ADG-6's
/// amendment, and it is the right way round: a false sentence in an accepted
/// record is worse than an amendment that has to paraphrase.
///
/// It knows one per-frame member, named above. A future version whose per-frame
/// seam is called something else needs that name added, and the check reports
/// nothing until it is.
///
/// It reads every tracked version's inventory rather than one. A decision record
/// is version-agnostic prose and denies a mechanism outright, so the claim is
/// false as soon as any tracked version declares the hook, and the message names
/// the inventory that falsifies it.
fn verify_decision_hook_claims(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    let mut sources: Vec<(String, &str)> = Vec::new();
    for inventory in inventories {
        for hook in declared_hook_names(&inventory.text) {
            sources.push((hook, inventory.path.as_str()));
        }
    }
    let declared = sources
        .iter()
        .map(|(hook, _)| hook.clone())
        .collect::<Vec<_>>();
    let per_frame = sources
        .iter()
        .find(|(hook, _)| hook == PER_FRAME_MEMBER)
        .map(|(_, path)| *path);

    let mut records: Vec<PathBuf> = match fs::read_dir(root.join(DECISION_RECORDS)) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|kind| kind == "md"))
            .collect(),
        Err(error) => {
            violations.push(format!("cannot read {DECISION_RECORDS}: {error}"));
            return;
        }
    };
    records.sort();

    for path in records {
        let name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let record = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                violations.push(format!("cannot read {DECISION_RECORDS}/{name}: {error}"));
                continue;
            }
        };
        for denied in denied_hook_names(&record, &declared) {
            let inventory = sources
                .iter()
                .find(|(hook, _)| *hook == denied)
                .map_or("", |(_, path)| *path);
            violations.push(format!(
                "{DECISION_RECORDS}/{name} says `{denied}` is not hooked while {inventory} \
                 declares it"
            ));
        }
        if let Some(inventory) = per_frame
            && record
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
                .contains(NO_PER_FRAME)
        {
            violations.push(format!(
                "{DECISION_RECORDS}/{name} says \"{NO_PER_FRAME}\" while {inventory} \
                 declares `{PER_FRAME_MEMBER}`, which the inventory itself calls the per-frame seam"
            ));
        }
    }
}

/// Every hook id and member name the inventory declares.
///
/// Both, because a record names a hook either way: ADG-6's consequence bullet
/// wrote the member and its section prose wrote the id.
fn declared_hook_names(inventory: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in inventory.lines().map(str::trim) {
        for key in ["id", "member"] {
            if let Some(value) = line.strip_prefix(key).map(str::trim_start)
                && let Some(value) = value.strip_prefix('=')
            {
                let value = value.trim().trim_matches('"');
                if !value.is_empty() {
                    names.push(value.to_owned());
                }
            }
        }
    }
    names
}

/// Every declared hook a record denies within reach of a denial.
///
/// Both sides of the denial are read, because English puts the thing denied on
/// either one: "does not hook the extraction seam" and "the extraction seam is not
/// hooked" say the same thing.
fn denied_hook_names(record: &str, declared: &[String]) -> Vec<String> {
    let bare = |word: &str| {
        word.trim_matches(|character: char| !character.is_alphanumeric())
            .to_owned()
    };
    let words: Vec<&str> = record.split_whitespace().collect();
    let mut denied = Vec::new();
    for (at, window) in words.windows(2).enumerate() {
        let anchored = bare(window[0]).eq_ignore_ascii_case(DENIAL)
            && DENIAL_VERBS
                .iter()
                .any(|verb| bare(window[1]).eq_ignore_ascii_case(verb));
        if !anchored {
            continue;
        }
        let before = at.saturating_sub(COUNT_LOOKBACK)..at;
        let after = (at + 2)..(at + 2 + COUNT_LOOKBACK).min(words.len());
        for word in words[before].iter().chain(&words[after]) {
            let word = bare(word);
            if declared.contains(&word) && !denied.contains(&word) {
                denied.push(word);
            }
        }
    }
    denied
}

/// Counts the entries, kinds and handle parameters one `hooks.toml` declares.
///
/// An inventory declaring no entry is refused rather than counted as four zeroes.
/// Zero is a number a truncated file reaches by having nothing in it, and every
/// consumer reads a successful count as evidence the inventory was read: a
/// version truncated to its header would contribute nothing to
/// `declared_hook_names`, so no decision record could be falsified by it, and its
/// document's figures would be satisfied by emptiness.
fn hook_counts(inventory: &str) -> Result<HookCounts, String> {
    let mut counts = HookCounts {
        entries: 0,
        methods: 0,
        fields: 0,
        handles: 0,
    };
    for block in inventory.split("[[hook]]").skip(1) {
        let body = block
            .split("\n[")
            .next()
            .expect("splitting a string yields at least one part");
        let mut kind = None;
        let mut descriptor = None;
        for line in body.lines().map(str::trim) {
            if let Some(value) = quoted_value(line, "kind") {
                kind = Some(value);
            } else if let Some(value) = quoted_value(line, "descriptor") {
                descriptor = Some(value);
            }
        }
        let (Some(kind), Some(descriptor)) = (kind, descriptor) else {
            return Err("a `[[hook]]` entry declares no kind or no descriptor".to_owned());
        };

        counts.entries += 1;
        if kind.ends_with("-field") {
            counts.fields += 1;
        } else {
            counts.methods += 1;
            counts.handles += descriptor_handles(descriptor)
                .ok_or_else(|| format!("`{descriptor}` is not a method descriptor"))?;
        }
    }

    if counts.entries == 0 {
        return Err("the inventory declares no `[[hook]]` entry".to_owned());
    }

    Ok(counts)
}

/// Every count the document states that the inventory does not bear out.
///
/// Each count is anchored on the phrase it is written in rather than searched for
/// loosely, and *every* occurrence of that phrase is checked. A second stale
/// sentence somewhere else in the file is the way this document went wrong the
/// first time, so finding one right occurrence is not enough.
///
/// Known limit: only these four phrasings are reached. A sentence that states a
/// count some other way — "the inventory holds 99 targets" — is not checked, and
/// the next reader should not assume coverage this does not have. Closing it means
/// sweeping every number the document writes against a count noun, and that sweep
/// cannot be written here: the same file says `hooks.toml` "grew from 6 entries",
/// a figure about the past that a current-inventory sweep would report as wrong,
/// so telling a current claim from a historical one in arbitrary prose is the
/// general parser this gate is deliberately not. What holds instead is that each
/// current claim is written in one of these phrasings, and the document's own
/// coverage table is what a new sentence should extend rather than restate.
/// `campaign_document_violations` carries the same limit for the same reason.
fn hook_document_violations(document: &str, inventory: &str, counts: &HookCounts) -> Vec<String> {
    let mut violations = Vec::new();
    for (phrase, side, expected, subject) in [
        (" entries to ", Side::After, counts.entries, "hook entries"),
        (
            " methods and ",
            Side::Before,
            counts.methods,
            "method entries",
        ),
        (" fields", Side::Before, counts.fields, "field entries"),
        (
            " handle parameters",
            Side::Before,
            counts.handles,
            "handle parameters",
        ),
    ] {
        let stated = stated_counts(document, phrase, side);
        if stated.is_empty() {
            violations.push(format!(
                "{HOOK_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it is \
                 held to {inventory}'s {expected} {subject}"
            ));
            continue;
        }
        for count in stated {
            if count != expected {
                violations.push(format!(
                    "{HOOK_DOCUMENT} states {count} {subject} where {inventory} declares \
                     {expected}"
                ));
            }
        }
    }
    violations
}

/// Which side of a phrase the number it states sits on.
#[derive(Clone, Copy)]
enum Side {
    /// `31 methods and` — the number precedes the phrase.
    Before,
    /// `entries to 35` — the number follows it.
    After,
}

/// Every number written against one phrase in a document.
fn stated_counts(document: &str, phrase: &str, side: Side) -> Vec<usize> {
    let mut counts = Vec::new();
    let mut offset = 0;
    while let Some(found) = document[offset..].find(phrase) {
        let at = offset + found;
        offset = at + phrase.len();
        let digits = match side {
            Side::Before => {
                let head = document[..at].trim_end();
                let start =
                    head.len() - head.chars().rev().take_while(char::is_ascii_digit).count();
                &head[start..]
            }
            Side::After => {
                let tail = document[offset..].trim_start();
                let end = tail.chars().take_while(char::is_ascii_digit).count();
                &tail[..end]
            }
        };
        if let Ok(count) = digits.parse() {
            counts.push(count);
        }
    }
    counts
}

/// The inventory figures one launch campaign observed.
struct CampaignInventory {
    /// Distinct classes the campaign's reviewed inventory held.
    classes: usize,
    /// Callable targets across those classes.
    targets: usize,
    /// Callable targets the superseded campaign's inventory held.
    prior: usize,
    /// Launches the surface campaign records.
    launches: usize,
}

/// Holds `evidence/m3/README.md` to the launch record whose figures it restates.
///
/// The same defect the hook document carried, in the place the fifth M3-W8 review
/// round predicted it next: `33` and `15` were written into the campaign prose and
/// into `rewrite.classesInInventory` / `rewrite.callableTargetsInInventory`, and
/// nothing recomputed either.
///
/// The campaign's launch count is bound the same way and was added for the same
/// reason: the prose wrote "three surface launches" beside figures that were
/// checked, with nothing holding that one to `surfaceCampaign.launches`.
///
/// These figures are *historical* and are checked as such. A launch campaign
/// observed one inventory in one build, so a later hook entering `hooks.toml` does
/// not falsify what that campaign rewrote, and deriving them from `hooks.toml` —
/// the way `verify_hook_inventory_document` derives the architecture document's —
/// would make the record lie about its own launches. What can go stale is the
/// prose against the record: `rewrite.targetInventory` enumerates every class and
/// method the campaign reviewed, so the two counts are derivable from the record
/// itself, and the prose is derivable from them.
///
/// Known limits, the same ones `hook_document_violations` carries, and there are
/// two of them rather than one. Each figure is anchored on the phrase it is
/// written in, so a sentence stating a count in *new* phrasing is not reached —
/// and a sentence that *replaces* an anchored occurrence with different wording is
/// not reached either, because the occurrences that survive keep the anchor alive
/// and the check reports no missing anchor. It is checked at every occurrence *of
/// an anchored phrase*, which is what caught the hook document's second stale
/// sentence, and the anchors are the ones the prose is now written to use. So what
/// this proves is that the anchored sentences agree with the record, not that
/// every figure in the document does.
///
/// A multiset of the integers adjacent to the subject nouns was considered and
/// does not close either hole: the noun is what identifies which figure an integer
/// is, and both holes are wordings that change the noun, so a check that ignored
/// the noun would have to flag every integer near it and would be a false red in
/// prose this dense. Where a hole of this shape *is* closable, it is closed by
/// forbidding the figure outright rather than by matching it —
/// `verify_review_round_counts` does that, and it is the round count's fix
/// precisely because no phrasing of a restated round count is wanted at all.
fn verify_campaign_inventory(root: &Path, violations: &mut Vec<String>) {
    let matrix = root.join(CAMPAIGN_MATRIX);
    if !matrix.is_file() {
        violations.push(format!("missing required file: {CAMPAIGN_MATRIX}"));
        return;
    }
    let Some(derived) = campaign_inventory(&matrix, violations) else {
        return;
    };

    for (field, expected, subject) in [
        (
            "classesInInventory",
            derived.classes,
            "classes its own target inventory enumerates",
        ),
        (
            "callableTargetsInInventory",
            derived.targets,
            "callable targets its own target inventory enumerates",
        ),
    ] {
        match matrix_count(&matrix, &format!(".rewrite.{field}")) {
            Ok(recorded) if recorded == expected => {}
            Ok(recorded) => violations.push(format!(
                "{CAMPAIGN_MATRIX} records {field} {recorded} against the {expected} {subject}"
            )),
            Err(error) => violations.push(error),
        }
    }

    match fs::read_to_string(root.join(CAMPAIGN_DOCUMENT)) {
        Ok(document) => violations.extend(campaign_document_violations(&document, &derived)),
        Err(error) => violations.push(format!("cannot read {CAMPAIGN_DOCUMENT}: {error}")),
    }
}

/// Derives the campaign's inventory figures from the record's own enumeration.
fn campaign_inventory(matrix: &Path, violations: &mut Vec<String>) -> Option<CampaignInventory> {
    // A duplicate would let `unique` absorb an inventory that lists a target
    // twice, so the enumeration is required to be an enumeration first.
    for (filter, subject) in [
        (
            "[.rewrite.targetInventory[].class]",
            "lists a class more than once",
        ),
        (
            "[.rewrite.targetInventory[].methods[]]",
            "lists a callable target more than once",
        ),
    ] {
        let listed = matrix_count(matrix, &format!("{filter} | length"));
        let distinct = matrix_count(matrix, &format!("{filter} | unique | length"));
        match (listed, distinct) {
            (Ok(listed), Ok(distinct)) if listed == distinct => {}
            (Ok(_), Ok(_)) => {
                violations.push(format!("{CAMPAIGN_MATRIX}'s target inventory {subject}"));
                return None;
            }
            (Err(error), _) | (_, Err(error)) => {
                violations.push(error);
                return None;
            }
        }
    }

    let classes = matrix_count(
        matrix,
        "[.rewrite.targetInventory[].class] | unique | length",
    );
    let targets = matrix_count(matrix, "[.rewrite.targetInventory[].methods[]] | length");
    let prior = matrix_count(matrix, ".matrixNotCarriedForward.priorCallableTargets");
    let launches = matrix_count(matrix, ".surfaceCampaign.launches");
    match (classes, targets, prior, launches) {
        (Ok(classes), Ok(targets), Ok(prior), Ok(launches)) => Some(CampaignInventory {
            classes,
            targets,
            prior,
            launches,
        }),
        (Err(error), _, _, _)
        | (_, Err(error), _, _)
        | (_, _, Err(error), _)
        | (_, _, _, Err(error)) => {
            violations.push(error);
            None
        }
    }
}

/// Reads one whole number out of the launch record.
fn matrix_count(matrix: &Path, filter: &str) -> Result<usize, String> {
    let value = jq_value(matrix, filter)?;
    value.parse().map_err(|_| {
        format!("{CAMPAIGN_MATRIX} does not record a whole number at `{filter}`, but `{value}`")
    })
}

/// Every inventory figure the campaign prose states that its record does not bear.
fn campaign_document_violations(document: &str, derived: &CampaignInventory) -> Vec<String> {
    let mut violations = Vec::new();
    // Markdown wraps where the line ran out, so an anchor phrase in the file is
    // split by a newline as often as not. Reflowing to single spaces first is what
    // makes the anchors survive an editor rewrapping the paragraph.
    let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
    let document = document.as_str();
    for (phrase, side, expected, subject) in [
        (
            " callable targets",
            Side::Before,
            derived.targets,
            "callable targets",
        ),
        (
            " classes in the reviewed inventory",
            Side::Before,
            derived.classes,
            "classes in the reviewed inventory",
        ),
        (
            "moved from ",
            Side::After,
            derived.prior,
            "callable targets before the count moved",
        ),
    ] {
        let stated = stated_counts(document, phrase, side);
        if stated.is_empty() {
            violations.push(format!(
                "{CAMPAIGN_DOCUMENT} no longer writes a count against `{phrase}`, so nothing in it \
                 is held to {CAMPAIGN_MATRIX}'s {expected} {subject}"
            ));
            continue;
        }
        for count in stated {
            if count != expected {
                violations.push(format!(
                    "{CAMPAIGN_DOCUMENT} states {count} {subject} where {CAMPAIGN_MATRIX} records \
                     {expected}"
                ));
            }
        }
    }

    // The launch count is spelled out in this document's prose rather than written
    // in digits, so it is read word-adjacent like the round counts rather than
    // digit-anchored like the figures above.
    let stated = numbers_before(document, &["surface", "launches"]);
    if stated.is_empty() {
        violations.push(format!(
            "{CAMPAIGN_DOCUMENT} no longer writes a count against `surface launches`, so nothing \
             in it is held to {CAMPAIGN_MATRIX}'s {} launches in the surface campaign",
            derived.launches
        ));
    }
    for count in stated {
        if count != derived.launches {
            violations.push(format!(
                "{CAMPAIGN_DOCUMENT} states {count} surface launches where {CAMPAIGN_MATRIX} \
                 records {}",
                derived.launches
            ));
        }
    }
    violations
}

/// The units a count is spelled from, and every count up to twenty.
///
/// See [`numeral`] for where the reading stops and what escapes above it.
const CARDINALS: [&str; 21] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

/// The tens a compound count is spelled from, `twenty` first.
const TENS: [&str; 8] = [
    "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// One word read as a number, whether it was written in digits or spelled out.
///
/// Spelled reading stops at ninety-nine, and the ceiling is stated rather than
/// implied. It used to stop at twenty on the argument that every stale figure any
/// round had found was below twenty, so extending was speculation. That argument
/// no longer holds: a round measured the hole and found it swallowed the whole
/// tail rather than stopping at twenty — `forty` was read by nothing, and so was
/// `twenty-one`, because the hyphen matched no entry either. M3 took twelve
/// independent review rounds, so a milestone reaching a count above twenty is a
/// thing that happens rather than a thing to speculate about.
///
/// What still escapes: a hundred and above, and a compound written without a
/// hyphen (`twenty one`, `one hundred and five`), which reads as its parts. A
/// count written in digits is read at any size and always was. Of the checks
/// built on this, only [`verify_review_round_counts`] fails open on an unreadable
/// numeral, because it is a prohibition rather than a match; the others require a
/// readable count and report its absence.
fn numeral(word: &str) -> Option<usize> {
    let word = word
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_ascii_lowercase();
    if !word.is_empty() && word.chars().all(|character| character.is_ascii_digit()) {
        return word.parse().ok();
    }
    if let Some(count) = CARDINALS.iter().position(|spelled| *spelled == word) {
        return Some(count);
    }
    if let Some(tens) = TENS.iter().position(|spelled| *spelled == word) {
        return Some(tens_value(tens));
    }
    let (tens, unit) = word.split_once('-')?;
    let tens = TENS.iter().position(|spelled| *spelled == tens)?;
    let unit = CARDINALS.iter().position(|spelled| *spelled == unit)?;
    // `twenty-zero` and `twenty-eleven` are not counts anyone writes, and reading
    // them would invent a figure the prose never stated.
    if !(1..=9).contains(&unit) {
        return None;
    }
    Some(tens_value(tens) + unit)
}

/// What one entry of [`TENS`] is worth, `twenty` being the first.
fn tens_value(at: usize) -> usize {
    (at + 2) * 10
}

/// How far back from a noun a number counting it may sit.
///
/// Four, which is what the stale sites needed: the widest of them wrote `five
/// tracked independent M3-W8 rounds`, four words between the figure and the noun
/// it counted. Wider than that starts collecting numbers from the clause before.
const COUNT_LOOKBACK: usize = 4;

/// Every number written against `noun`, however it was spelled and whatever
/// adjectives sit between.
///
/// Word-adjacent rather than phrase-anchored, so a figure survives the sentence
/// around it being rewritten — which is the hole a phrase-anchored check leaves.
/// What it cannot survive is the number moving further from the noun than
/// [`COUNT_LOOKBACK`] words, and the checks that use it say so.
fn numbers_before(document: &str, noun: &[&str]) -> Vec<usize> {
    let words: Vec<&str> = document.split_whitespace().collect();
    let mut found = Vec::new();
    for (at, window) in words.windows(noun.len()).enumerate() {
        if !window_is_noun(noun, window) {
            continue;
        }
        let from = at.saturating_sub(COUNT_LOOKBACK);
        if let Some(count) = words[from..at].iter().rev().find_map(|word| numeral(word)) {
            found.push(count);
        }
    }
    found
}

/// Every number written after `noun`, within the same window [`numbers_before`]
/// reads backwards.
///
/// A count that follows its noun counts it exactly as one that precedes it does —
/// `the rounds recorded here number five` is the same claim as `five rounds` — so
/// a check that only looked backwards could be walked around by moving the figure
/// past the word. The word immediately after the noun is skipped, which is what
/// keeps a numeral that labels rather than counts (`Round 5`, `rounds to 0.05`)
/// out of the window.
fn numbers_after(document: &str, noun: &[&str]) -> Vec<usize> {
    let words: Vec<&str> = document.split_whitespace().collect();
    let mut found = Vec::new();
    for (at, window) in words.windows(noun.len()).enumerate() {
        if !window_is_noun(noun, window) {
            continue;
        }
        let from = at + noun.len() + 1;
        let to = (at + noun.len() + COUNT_LOOKBACK).min(words.len());
        if from >= to {
            continue;
        }
        if let Some(count) = words[from..to].iter().find_map(|word| numeral(word)) {
            found.push(count);
        }
    }
    found
}

/// Whether this run of words is the noun, whatever punctuation sits around it.
fn window_is_noun(noun: &[&str], window: &[&str]) -> bool {
    noun.iter().zip(window).all(|(expected, word)| {
        word.trim_matches(|character: char| !character.is_alphanumeric())
            .eq_ignore_ascii_case(expected)
    })
}

/// Every noun this repository's prose has used for a set of independent review
/// rounds.
///
/// Plural only, and that is a limit rather than an oversight: a singular round
/// noun sits beside counts of other things all through these records — `that
/// round found four more`, `the audit recomputed 728 file digests` — so holding
/// the singular would report those instead of a tally of rounds.
const ROUND_NOUNS: [&str; 5] = ["rounds", "verdicts", "audits", "reviews", "iterations"];

/// Keeps the count of a milestone's independent review rounds in the one record
/// that counts them.
///
/// Six review rounds have now found a stale count in this evidence set, and the
/// round count itself was the recurring one: five sites restated it, a repair
/// fixed one of them, and the other four went stale together the next time a
/// round ran — two of them inside the `review` block the closure gate reads. The
/// repository already declares the answer in `manifest.json`: the record says how
/// many rounds have run and nothing else restates it. This enforces that answer
/// instead of trusting prose to keep it.
///
/// Known limit, stated rather than implied: this catches a *cardinal* count of
/// rounds, spelled either way and on either side of the noun — `six rounds`, `5
/// rounds`, `the rounds recorded here number five`. An ordinal naming one round
/// that has already run (`the fourth round`) is a reference into the record's own
/// numbering and never drifts, so it is allowed; an ordinal naming a round that
/// has not run yet (`a sixth independent round decides that`) does drift and is
/// not caught here. Nothing but the record counting for itself closes that one.
///
/// Every word the prose has reached for is held, because a rule that missed the
/// noun a sentence chose would be the same gap in a new place: M2's record calls
/// its rounds audits, and a round restated as a review or an iteration is the same
/// restatement. [`ROUND_NOUNS`] states the one limit that costs something — the
/// noun has to be plural — and the numeral immediately after the noun is walked
/// past for the reason [`numbers_after`] gives.
fn verify_review_round_counts(root: &Path, violations: &mut Vec<String>) {
    for (record, documents) in round_count_subjects() {
        if !root.join(&record).is_file() {
            violations.push(format!("missing required file: {record}"));
        }
        for relative in documents {
            // A milestone need not carry a README; M0 does not. What it may not
            // do is carry one nobody sweeps, so an absent document is skipped and
            // a present unreadable one is reported.
            if !root.join(&relative).exists() {
                continue;
            }
            match fs::read_to_string(root.join(&relative)) {
                Ok(document) => {
                    for (count, noun) in round_count_restatements(&document) {
                        violations.push(format!(
                            "{relative} restates that {count} review {noun} exist; \
                             {record} is the one record that counts them"
                        ));
                    }
                }
                Err(error) => violations.push(format!("cannot read {relative}: {error}")),
            }
        }
    }

    // `ROADMAP.md` is swept against the rule rather than against a record, and it
    // had to be added the hard way. The sweep above is written per-milestone, so
    // it reaches the documents that sit beside a record and no others — and the
    // plan document sits beside none of them while restating figures from all of
    // them. It carried a count of M3's rounds that was off by one, and the round
    // that found that was a round after the last one the figure could have been
    // right for, which is the whole argument for the rule. Correcting the number
    // would have left a number to correct again.
    match fs::read_to_string(root.join(PLAN_DOCUMENT)) {
        Ok(document) => {
            for (count, noun) in round_count_restatements(&document) {
                violations.push(format!(
                    "{PLAN_DOCUMENT} restates that {count} review {noun} exist; each milestone's \
                     review.md is the one record that counts its own rounds, and a count written \
                     in the plan is falsified by the next round"
                ));
            }
        }
        Err(error) => violations.push(format!("cannot read {PLAN_DOCUMENT}: {error}")),
    }
}

/// The record that counts each milestone's rounds, and the prose held to it.
///
/// This used to be three hand-written tuples, and every milestone that gained a
/// review record had to be remembered into them. It was not: M4's own evidence
/// cited this check as the enforcement while nothing under `evidence/m4/` was
/// swept, and a round measured that gap by writing a false count into
/// `evidence/m4/README.md` and getting exit 0. The rule was correct and its
/// subject list was stale, which is the same defect the round count itself keeps
/// producing — one statement of a fact, and a second place that has to be kept
/// agreeing with it by hand.
///
/// So the subjects are derived from [`MILESTONE_MANIFESTS`] rather than listed.
/// A milestone that gains a manifest is swept the day it lands, and the only
/// thing left to maintain is [`ROUND_COUNT_EXEMPT`], which is a statement about
/// two closed milestones rather than a list that grows.
fn round_count_subjects() -> Vec<(String, [String; 2])> {
    MILESTONE_MANIFESTS
        .iter()
        .filter(|(_, milestone)| !ROUND_COUNT_EXEMPT.contains(milestone))
        .map(|(manifest, milestone)| {
            let directory = format!("evidence/{}", milestone.to_ascii_lowercase());
            (
                format!("{directory}/review.md"),
                [format!("{directory}/README.md"), (*manifest).to_owned()],
            )
        })
        .collect()
}

/// Every count of review rounds one document states, with the noun it counted.
fn round_count_restatements(document: &str) -> Vec<(usize, &'static str)> {
    // A round and its verdict are one thing counted two ways, and the stale sites
    // used both nouns, so both are held to the record.
    ROUND_NOUNS
        .into_iter()
        .flat_map(|noun| {
            let mut counts = numbers_before(document, &[noun]);
            counts.extend(numbers_after(document, &[noun]));
            counts.into_iter().map(move |count| (count, noun))
        })
        .collect()
}

/// Every per-round blocking count a review record's round headings declare.
///
/// A heading reads `### Round 5 — FAIL, two blocking`, so the figure sits
/// against the word `blocking` and [`numbers_before`] is what reads it. Only
/// the first such figure on a heading is taken; a heading naming two counts is
/// a heading nobody writes, and reading both would invent a tally.
/// Each round heading, paired with the blocking count it declares.
///
/// A heading that declares none reads as `None` rather than being dropped: a
/// round without a count used to vanish from the sum and from the round tally
/// together, so the record agreed with itself by losing the row that disagreed.
fn round_heading_blocking_counts(document: &str) -> Vec<(String, Option<usize>)> {
    document
        .lines()
        .filter(|line| line.starts_with(ROUND_HEADING))
        .map(|line| {
            (
                line.to_owned(),
                numbers_before(line, &["blocking"]).first().copied(),
            )
        })
        .collect()
}

/// How a review record marks the start of one round's section.
const ROUND_HEADING: &str = "### Round ";

/// Holds a review record's headline tally of blocking findings to its own
/// per-round headings.
///
/// `evidence/m5/review.md` opened by saying six rounds raised nine blocking
/// findings. Its own headings summed to eight: the ninth item in its list of
/// instances was found by the round-4 repair, and the record itself treats
/// repair-found instances as a separate category one paragraph later. So the
/// document the manifest names as the one place that counts published a wrong
/// version of the count it exists to hold, in the direction that flattered the
/// campaign, and nothing anywhere summed the headings that would have said so.
///
/// The fix is the one the rest of this file uses: give the figure a single
/// source and derive the rest. The headings are that source, because each is
/// written by the round it describes and none of them can be edited without
/// editing the section under it.
///
/// Known limits, stated rather than implied. This reads the summary a record
/// opens with — everything before its first round heading — and not the round
/// sections, because a count inside a section is a count of that round and
/// disagreeing with the campaign total is what it is for. And it reaches a
/// record whose rounds carry their blocking count in the heading; M3's and M4's
/// records are laid out differently and are not retrofitted, for the reason
/// `docs/decisions/claims-without-a-second-reading.md` gives about rewriting
/// evidence a review already certified.
fn verify_review_blocking_counts(root: &Path, violations: &mut Vec<String>) {
    for (record, _) in round_count_subjects() {
        let path = root.join(&record);
        if !path.is_file() {
            continue;
        }
        // An unreadable record is already reported by the round-count sweep.
        let Ok(document) = fs::read_to_string(&path) else {
            continue;
        };
        let per_round = round_heading_blocking_counts(&document);
        if per_round.is_empty() {
            continue;
        }
        // A record whose headings never carry a count uses another layout and is
        // out of scope; one that carries a count anywhere has to carry it
        // everywhere, or the round without one drops out of the sum unseen.
        if per_round.iter().all(|(_, count)| count.is_none()) {
            continue;
        }
        for (heading, _) in per_round.iter().filter(|(_, count)| count.is_none()) {
            violations.push(format!(
                "{record} has a round heading that declares no blocking count, so the round is \
                 absent from the tally it belongs to: {}",
                heading.trim()
            ));
        }
        let total: usize = per_round.iter().filter_map(|(_, count)| *count).sum();
        let rounds = per_round.len();
        let summary = document
            .split(ROUND_HEADING)
            .next()
            .unwrap_or_default()
            .to_owned();

        let mut stated = numbers_before(&summary, &["blocking", "findings"]);
        stated.extend(numbers_after(&summary, &["blocking", "findings"]));
        if stated.is_empty() {
            violations.push(format!(
                "{record} states no tally of the blocking findings its {rounds} round headings \
                 declare; the summary carries that figure so a reader is not left to sum them"
            ));
        }
        for count in stated {
            if count != total {
                violations.push(format!(
                    "{record} states that {count} blocking findings were raised; its own round \
                     headings declare {total} across {rounds} rounds, and the headings are where \
                     that figure is written"
                ));
            }
        }
    }
}

/// Keeps a percentage out of a manifest's prose, because nothing recomputes one.
///
/// The seventh stale absolute figure in this evidence set was a per-file line
/// coverage percentage written in the present tense into `evidence/m2`'s guard.rs
/// deviation — `54.55%` against a measured `28.72%` — and it was found by the
/// review whose own job was to sweep stale prose, which had enumerated three
/// other items and missed this one. Two more percentages sat in the same
/// sentence.
///
/// Nothing in this repository recomputes a per-file percentage: `ravel-coverage`
/// holds the workspace to a floor and no gate reads a file's own figure, so every
/// such number in a manifest is a present-tense claim the next commit can falsify.
/// The fix is the one [`verify_review_round_counts`] uses for the round count —
/// forbid the figure rather than match it — because a manifest is not where a
/// measurement belongs. The record that measured it says it, and that record is
/// bound by digest.
///
/// Known limit, stated rather than implied: this bans the `%` character, so a
/// percentage spelled out in words (`28.72 percent`) is not reached, and neither
/// is a bare ratio. A manifest field that carries a percentage as a number —
/// `tests.lineCoveragePercent` — is untouched and is held to its own arithmetic
/// elsewhere; what is refused is the sign, which is only ever prose.
fn verify_manifest_percentages(root: &Path, violations: &mut Vec<String>) {
    for (relative, milestone) in MILESTONE_MANIFESTS {
        match fs::read_to_string(root.join(relative)) {
            Ok(manifest) => {
                for line in manifest.lines().filter(|line| line.contains('%')) {
                    let figure = line
                        .split('%')
                        .next()
                        .map(|head| {
                            head.chars()
                                .rev()
                                .take_while(|character| {
                                    character.is_ascii_digit() || *character == '.'
                                })
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect::<String>()
                        })
                        .filter(|figure| !figure.is_empty())
                        .unwrap_or_else(|| "a percentage".to_owned());
                    violations.push(format!(
                        "{relative} writes {figure}% into {milestone}'s prose; no gate recomputes \
                         a percentage a manifest states, so it belongs in the record that \
                         measured it"
                    ));
                }
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }
}

/// Holds the void set to the record that voided it and the roadmap that says so.
///
/// The M2 milestone gate validates all six of `loader-overhead.json`'s limits
/// identically — each one a number above its measurement — and three of them were
/// voided by the record's own expiry clause when ADG-2 chose source compilation
/// over linking. Nothing inside the record marks those three dead, so a reader who
/// runs the gate and not `ROADMAP.md` reads a green on three limits that describe a
/// shape which no longer exists. The record is not edited, because a recorded
/// measurement is not edited after the fact; what is bound instead is the
/// disclosure. [`VOID_LIMITS`] is the one list, the record has to carry every
/// metric on it, `ROADMAP.md` has to name every one of them, and it may not name a
/// limit that still stands. Deleting the roadmap's declaration, or quietly voiding
/// a live loader limit, is then a violation rather than a silent change of what
/// the budget means.
///
/// Known limit: the declaration is the roadmap paragraph that ends in `are void`,
/// so moving the void set into prose that does not say `are void` is not reached.
/// That wording is what the roadmap uses, and a paragraph is the smallest unit
/// this can be read from without a heading convention the file does not have.
fn verify_void_limits(root: &Path, violations: &mut Vec<String>) {
    let record = root.join(BUDGET_RECORDS[0]);
    let recorded = match jq_value(&record, "[.budget.limits[].metric] | join(\" \")") {
        Ok(recorded) => recorded,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let recorded: Vec<&str> = recorded.split_whitespace().collect();
    let roadmap = match fs::read_to_string(root.join("ROADMAP.md")) {
        Ok(roadmap) => roadmap,
        Err(error) => {
            violations.push(format!("cannot read ROADMAP.md: {error}"));
            return;
        }
    };
    let Some(declaration) = roadmap
        .split("\n\n")
        .find(|paragraph| paragraph.contains("are void"))
        .map(str::to_owned)
    else {
        violations.push(
            "ROADMAP.md declares no limit void, and evidence/m2/loader-overhead.json's expiry \
             clause voided three of its own"
                .to_owned(),
        );
        return;
    };
    let roadmap = declaration;

    for (metric, why) in VOID_LIMITS {
        if !recorded.contains(metric) {
            violations.push(format!(
                "{} is treated as void and {} carries no limit for it, so the void set describes a \
                 record that is not there",
                metric, BUDGET_RECORDS[0]
            ));
        }
        if !roadmap.contains(metric) {
            violations.push(format!(
                "ROADMAP.md's declaration of which limits are void does not name {metric}, which \
                 is {why}; a limit the gate stops comparing and the roadmap stops declaring is \
                 indistinguishable from one that passed"
            ));
        }
    }
    for metric in recorded {
        if VOID_LIMITS.iter().any(|(void, _)| *void == metric) {
            continue;
        }
        if roadmap.contains(metric) {
            violations.push(format!(
                "ROADMAP.md declares {metric} void where it says which of {}'s limits are, and \
                 that limit still stands",
                BUDGET_RECORDS[0]
            ));
        }
    }
}

/// Holds M3's prose to the number of world lifecycles the harness actually runs.
///
/// `over nine world lifecycles` was true and ungated: appending a tenth
/// `persistence_run` case left the scaffold gate green. The harness now declares
/// the figure in one place and asserts it against the runs it made, so a case
/// added or removed there fails the harness; this binds the two records that cite
/// the figure to the same declaration, so a stale citation fails the scaffold.
fn verify_harness_lifecycles(root: &Path, violations: &mut Vec<String>) {
    let harness = match fs::read_to_string(root.join(PERSISTENCE_HARNESS)) {
        Ok(harness) => harness,
        Err(error) => {
            violations.push(format!("cannot read {PERSISTENCE_HARNESS}: {error}"));
            return;
        }
    };
    let declared = harness
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("persistence_lifecycles_declared="))
        .and_then(|value| value.parse::<usize>().ok());
    let Some(declared) = declared else {
        violations.push(format!(
            "{PERSISTENCE_HARNESS} no longer declares `persistence_lifecycles_declared`, so no \
             record's world-lifecycle count is held to it"
        ));
        return;
    };

    for relative in [CAMPAIGN_DOCUMENT, CAMPAIGN_MATRIX] {
        match fs::read_to_string(root.join(relative)) {
            Ok(document) => {
                let stated = numbers_before(&document, &["world", "lifecycles"]);
                if stated.is_empty() {
                    violations.push(format!(
                        "{relative} no longer writes a count against `world lifecycles`, so \
                         nothing in it is held to {PERSISTENCE_HARNESS}'s {declared}"
                    ));
                }
                for count in stated {
                    if count != declared {
                        violations.push(format!(
                            "{relative} states {count} world lifecycles where \
                             {PERSISTENCE_HARNESS} declares {declared}"
                        ));
                    }
                }
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }
}

/// The tracked sources compiled into the always-present agent jar.
const AGENT_JAR_SOURCES: &str = "jvm/bootstrap";

/// What makes a string literal in those sources a game class name.
///
/// `com.mojang.` is here because the game's own libraries are as version-coupled as
/// its own classes and are reached the same way: ADG-7 turns on
/// `com.mojang.serialization.Codec` and `adapter.toml` pins `com.mojang:datafixerupper`
/// as a second class source, so a reflective reach into one would be the same defect
/// wearing a different prefix.
const GAME_PACKAGES: [&str; 2] = ["net.minecraft.", "com.mojang."];

/// The per-version file declaring the game classes the agent jar resolves by name.
///
/// This was a seventeen-name constant in this file, and the names in it are
/// `26.1.2`'s: at `1.21.1` `net.minecraft.resources.Identifier` is called
/// `ResourceLocation` and `net.minecraft.world.level.storage.SavedDataStorage` is
/// called `DimensionDataStorage`. A single global list asserted one version's
/// spelling against every other and passed while doing it, which is the defect a
/// second version directory would have shipped with. The set is version data now,
/// beside the `hooks.toml` whose classes it complements, and the check below runs
/// once per tracked version.
///
/// Two sources resolve one set, and for the same reason: ADG-4 admits a hook that
/// observes or vetoes, so anything the host has to *perform* — storing a world's
/// aggregate, registering a mod's block — is performed by a hand-written adapter
/// that reaches the game by name. Adding a name to either source without adding it
/// to a version's file fails the scaffold, which is the whole point of the file.
const REFLECTION_FILE: &str = "reflection.toml";

/// One tracked version's declared reflective surface, read beside its inventory.
struct ReflectiveSurface {
    /// The declaring file's repository-relative path, which the messages name.
    path: String,
    /// Every role the file declares, in the order it declares them.
    roles: Vec<Reflected>,
}

/// One declared role and the Mojang name a reviewer reads it as.
///
/// The runtime name the same table carries is required to be there and is not
/// kept: what it resolves to is decided by the version's mapping, which only the
/// generator holds, so re-deriving it here would be the weaker of two copies of
/// one rule. See [`verify_persistence_reflection`].
struct Reflected {
    /// The constant a shared source under [`AGENT_JAR_SOURCES`] spells.
    name: String,
    /// The Mojang name a reviewer reads.
    class: String,
}

/// Every tracked version's declared reflective surface.
///
/// Keyed on the tracked versions rather than on anything a version might be
/// missing, because a version whose file cannot be read is a version whose
/// reflective surface goes unchecked, and that is a fact about this check rather
/// than about the missing file.
fn reflective_surfaces(root: &Path, violations: &mut Vec<String>) -> Vec<ReflectiveSurface> {
    let versions = match tracked_versions(root) {
        Ok(versions) => versions,
        Err(error) => {
            violations.push(error);
            return Vec::new();
        }
    };

    let mut surfaces = Vec::new();
    for version in versions {
        let path = format!("{VERSION_DATA}/{version}/{REFLECTION_FILE}");
        match fs::read_to_string(root.join(&path)) {
            Ok(text) => match declared_roles(&text, &path) {
                Ok(roles) => surfaces.push(ReflectiveSurface { path, roles }),
                Err(violation) => violations.push(violation),
            },
            Err(error) => violations.push(format!("cannot read {path}: {error}")),
        }
    }
    surfaces
}

/// Reads the `[[reflected]]` triples one declaring file carries.
///
/// A table missing any of the three is refused rather than skipped. Two of the
/// names are what a reviewer and the JVM each read, and the third is what a
/// shared source spells, so a table short of one is a role that resolves to
/// nothing or a name held to nothing, and dropping it silently is how a surface
/// goes unchecked while the count still looks right.
fn declared_roles(text: &str, path: &str) -> Result<Vec<Reflected>, String> {
    let mut roles: Vec<Reflected> = Vec::new();
    let mut name = None;
    let mut class = None;
    let mut runtime = None;
    let finish = |name: &mut Option<String>,
                  class: &mut Option<String>,
                  runtime: &mut Option<String>,
                  roles: &mut Vec<Reflected>|
     -> Result<(), String> {
        match (name.take(), class.take(), runtime.take()) {
            (None, None, None) => Ok(()),
            (Some(name), Some(class), Some(_runtime)) => {
                roles.push(Reflected { name, class });
                Ok(())
            }
            (name, class, _) => Err(format!(
                "{path} declares a reflected table without all three of name, class and runtime \
                 ({}); a role that resolves to nothing is a name held to nothing",
                name.or(class).unwrap_or_else(|| "unnamed".to_owned())
            )),
        }
    };
    for line in text.lines().map(str::trim) {
        if line == "[[reflected]]" {
            finish(&mut name, &mut class, &mut runtime, &mut roles)?;
            continue;
        }
        if let Some(value) = quoted_value(line, "name") {
            name = Some(value.to_owned());
        } else if let Some(value) = quoted_value(line, "class") {
            class = Some(value.to_owned());
        } else if let Some(value) = quoted_value(line, "runtime") {
            runtime = Some(value.to_owned());
        }
    }
    finish(&mut name, &mut class, &mut runtime, &mut roles)?;
    Ok(roles)
}

/// Pins the game class names the always-present agent jar's own sources carry.
///
/// ADG-1 forbids that jar from *declaring* a game type and `build-agent.sh`
/// enforces it by construction, compiling with no `-cp` at all. ADG-4's adapters
/// resolve the game classes they need by name instead, and that name used to be a
/// string literal in the shared source. It cannot be: `jvm/bootstrap` is one
/// source tree serving every tracked version, so a literal there spells one
/// version's names and is wrong about every other, and an obfuscated version
/// makes it wrong at runtime while a Mojang-name presence check passes.
///
/// So the mechanism is generated rather than written, exactly as `HookTarget` is.
/// Each version's `reflection.toml` declares a role, the Mojang name a reviewer
/// reads, and the runtime name the JVM links; the generator renders the roles
/// into `dev.ravel.jvm.GameNames` with the runtime names as values; and the
/// shared sources spell `GameNames.<role>`. This holds that mechanism in three
/// ways, and fails in both directions on the third:
///
/// - A game-class literal anywhere under [`AGENT_JAR_SOURCES`] is a violation
///   outright. There is no longer a declared set that excuses one, because a
///   literal is the defect rather than a thing to be declared.
/// - Every role a source spells is declared by every tracked version. A role one
///   version answers and another does not is a source that compiles against one
///   game and not the other.
/// - Every declared role is spelled by some source. A role nothing reaches says
///   the surface drifted rather than that it is pinned.
///
/// **What it cannot do, and who does it instead.** It does not re-derive a
/// runtime name from a mapping. That is the generator's, in
/// `ravel-minecraft-adapter`'s `mapping::verify_reflection`, which refuses a
/// declared runtime name the version's mapping source does not resolve its
/// Mojang name to and runs on every build that can read that mapping — always,
/// under an unobfuscated source, because the identity needs no file. Writing the
/// same rule here would be one rule in two places that can disagree, and this one
/// would be the weaker copy: a build reads the version it selected, so only the
/// generator holds the mapping at all, and for an unselected version the pinned
/// material may not be on the machine.
///
/// It still cannot detect a game-side rename, for the reason it never could: both
/// sides of what it compares are Ravel's own. Nor does it reach anything below a
/// class — the member names [`method`] resolves by name and arity, the two
/// `LevelResource` field constants, and the three-argument storage constructor are
/// checked only by `jvm/harness/persistence/net/minecraft/**`, which are stand-ins
/// Ravel wrote, so that harness asserts Ravel's stubs against Ravel's reflection.
///
/// Three further limits, stated because a gate that promises more than it
/// enforces is the defect this one was written to close. It sees only the
/// prefixes [`GAME_PACKAGES`] lists, so a game name under any other package
/// escapes the literal half entirely. It reads one whole literal at a time, so
/// both spellings of a written-out name are caught — [`game_class_literals`]
/// normalises the internal `net/minecraft/…` form the transformer writes — while
/// a name assembled by concatenation, formatted into a template, or held in a
/// constant declared elsewhere is not a literal it can see at all. And it reads
/// only the tracked sources under
/// [`AGENT_JAR_SOURCES`], while `build-agent.sh` compiles those *and* the
/// generated tree: generated `Hook.java` carries an internal game name for every
/// class it transforms, and `GameNames` carries the runtime names themselves.
/// Both halves are generated from pinned version data by construction, which is
/// why this function does not reach for a build output.
///
/// `evidence/m3/manifest.json` records the surface as a named deviation and
/// `ROADMAP.md` records bringing it under `stubs.toml` and `mappings.lock` as owed
/// work, which needs the re-pin ceremony against the pinned jar.
fn verify_persistence_reflection(root: &Path, violations: &mut Vec<String>) {
    let surfaces = reflective_surfaces(root, violations);

    let sources = java_sources(&root.join(AGENT_JAR_SOURCES));
    if sources.is_empty() {
        violations.push(format!(
            "{AGENT_JAR_SOURCES} holds no Java source, so no game class the agent jar resolves is \
             held to a declared set"
        ));
        return;
    }

    let mut spelled: Vec<(String, String)> = Vec::new();
    for path in &sources {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        match fs::read_to_string(path) {
            Ok(source) => {
                for name in game_class_literals(&source) {
                    violations.push(format!(
                        "{relative} spells the game class {name} as a literal; \
                         {AGENT_JAR_SOURCES} is one source tree for every tracked version, so a \
                         game name written here is one version's spelling asserted over all of \
                         them and is the obfuscated name of none. Reach it as {GAME_NAMES}.<role> \
                         and declare the role in each version's {REFLECTION_FILE}"
                    ));
                }
                spelled.extend(
                    generated_roles(&source)
                        .into_iter()
                        .map(|role| (relative.clone(), role)),
                );
            }
            Err(error) => violations.push(format!("cannot read {relative}: {error}")),
        }
    }

    for surface in &surfaces {
        let declaring = &surface.path;
        let declared: Vec<&str> = surface
            .roles
            .iter()
            .map(|role| role.name.as_str())
            .collect();

        for (relative, role) in &spelled {
            if !declared.contains(&role.as_str()) {
                violations.push(format!(
                    "{relative} resolves the game class {GAME_NAMES}.{role} and {declaring} \
                     declares no such role; a role one version answers and another does not is a \
                     shared source that compiles against one game and not the other"
                ));
            }
        }

        for role in &surface.roles {
            if !spelled.iter().any(|(_, spelled)| spelled == &role.name) {
                violations.push(format!(
                    "{declaring} declares the role {} for {} and no source under \
                     {AGENT_JAR_SOURCES} resolves it; a declared name nothing uses says the \
                     surface drifted rather than that it is pinned",
                    role.name, role.class
                ));
            }
        }
    }

    verify_resolved_class_figure(root, &surfaces, violations);
}

/// Every Java source under one directory, in path order.
fn java_sources(directory: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_files(directory, &mut sources);
    sources.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "java")
    });
    sources.sort();
    sources
}

/// How a source under [`AGENT_JAR_SOURCES`] names one generated hook.
const HOOK_NAME_MATCH: &str = "hookMethod().equals(\"";

/// Holds the transformer's hook-name literals to the reviewed hook inventories.
///
/// `Transformer.java` decides which of ADG-7's seams a rewritten target is by
/// comparing the generated hook name against a literal. Those literals used to be
/// class and member names, and the class half was reached by
/// [`verify_persistence_reflection`]; the names are per-version data now and the
/// literals bind to nothing, so a version declaring the same seam under another
/// id renders another hook name, every comparison is false, and the seam's
/// transform is silently not applied to a jar that still builds.
///
/// A literal here has to name a hook every tracked version declares, for the
/// same reason a role does: [`AGENT_JAR_SOURCES`] is one source tree for every
/// version, so a seam one version answers and another does not is a transformer
/// that installs against one game and not the other. The other direction is not a
/// rule — most hooks are ordinary observers no seam matches on.
///
/// **What it cannot do.** It compares an id with a literal after dropping case
/// and separators, rather than restating how the generator turns one into the
/// other. That is deliberate: the camel-case rule lives in
/// `ravel-minecraft-adapter`'s `render::java_name` and a copy here would be one
/// rule in two places that can disagree. The cost is that it would not notice a
/// generator that changed its capitalisation, which no source here would compile
/// against either way. And it reads only the `equals` form above, so a comparison
/// written any other way — `contains`, a switch, a name held in a constant — is
/// outside it.
fn verify_hook_name_literals(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    let flattened = |name: &str| -> String {
        name.chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .collect()
    };

    for path in java_sources(&root.join(AGENT_JAR_SOURCES)) {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        // The literal sweep in `verify_persistence_reflection` reports an
        // unreadable source already, and one file cannot be two violations.
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let mut rest = source.as_str();
        while let Some(at) = rest.find(HOOK_NAME_MATCH) {
            rest = &rest[at + HOOK_NAME_MATCH.len()..];
            let Some((literal, tail)) = rest.split_once('"') else {
                break;
            };
            rest = tail;
            for inventory in inventories {
                if !inventory
                    .text
                    .lines()
                    .filter_map(|line| quoted_value(line, "id"))
                    .any(|id| flattened(id) == flattened(literal))
                {
                    violations.push(format!(
                        "{relative} matches the generated hook {literal} and {} declares no hook \
                         it is rendered from; the hook names are per-version data, so a seam one \
                         version declares under another id renders another name here, every \
                         comparison is false, and the transform is silently not applied",
                        inventory.path
                    ));
                }
            }
        }
    }
}

/// Where the shape check records the versions it held the transformer to.
///
/// `devenv tasks run` shows neither stdout nor stderr of a task that succeeds,
/// so a check that speaks only by printing is invisible in the gate that runs
/// it. The file is the fact, and it is written from the versions that held
/// rather than from the versions that were opened.
const TRANSFORMER_SHAPE_RECORD: &str = "target/ravel-jvm/transformer-shape.txt";

/// Where in a target's parameter list one of the transformer's loads sits.
#[derive(Clone, Copy)]
enum Parameter {
    /// A fixed position, counted from the first declared parameter.
    At(usize),
    /// The last declared parameter, wherever the ones before it drifted.
    Last,
}

/// What an injected instruction needs to find where it loads.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    /// `aload`: a class or an array.
    Reference,
    /// `iload` against a declared `boolean`.
    Boolean,
    /// `iload` against a declared `int`.
    Int,
    /// A method that returns nothing.
    Void,
}

/// One seam `Transformer.java` injects into, and what its sequence assumes.
struct Seam {
    /// The hook id the generated hook name is rendered from.
    id: &'static str,
    /// Whether the sequence loads the receiver, which only an instance method has.
    receiver: bool,
    /// Every parameter the sequence loads, and what the load needs to find.
    loads: &'static [(Parameter, Shape)],
    /// The return kinds a sequence is written for, or nothing when the seam's
    /// sequence does not read the return at all.
    returns: &'static [Shape],
}

/// The shapes `Transformer.java` injects against, stated as data.
///
/// The transform derives every local slot from the target's own descriptor, so
/// it survives a parameter that moves. What it cannot derive is what it expects
/// to find once it gets there: an `aload` against an `int` and an `iload`
/// against a reference are both a `VerifyError` at class load, which is a launch
/// that aborts rather than a build that fails. Both defects this table was
/// written for were exactly that — `iload` at a position 1.21.1's
/// `doWorldLoad` does not have, and an int-return sequence at a method that
/// returns nothing — and neither was reachable by any check, because the
/// transformer compiles against no version at all.
///
/// **What it cannot do.** It is the Rust statement of what the Java emits, so
/// the two can disagree: an assumption added there and not here is unchecked.
/// It holds the shapes rather than the seams — the set of seams is
/// [`verify_hook_name_literals`]'s subject, and a hook name no version declares
/// is reported there instead of twice.
const TRANSFORMER_SEAMS: [Seam; 3] = [
    Seam {
        id: "world.open-persistence",
        receiver: true,
        loads: &[
            (Parameter::At(0), Shape::Reference),
            (Parameter::At(2), Shape::Reference),
            (Parameter::Last, Shape::Boolean),
        ],
        returns: &[Shape::Void],
    },
    Seam {
        id: "world.save-persistence",
        receiver: true,
        loads: &[],
        returns: &[],
    },
    Seam {
        id: "screen.title-menu-options",
        receiver: true,
        loads: &[
            (Parameter::At(0), Shape::Int),
            (Parameter::At(1), Shape::Int),
        ],
        returns: &[Shape::Int, Shape::Void],
    },
];

/// Holds the transformer's injected sequences to the shapes each version declares.
///
/// [`verify_hook_name_literals`] asks whether a seam the transformer names is
/// declared at all. This asks the next question: whether what it is declared as
/// is what the injected bytecode assumes. Nothing had, and a transform written
/// against one version's parameter list is not a build failure anywhere — the
/// agent jar compiles against no game, so the first thing to notice is the JVM
/// verifying the rewritten class during a launch.
///
/// A version that declares none of these seams is not this check's business:
/// the sibling above is what refuses that. What is refused here is a run in
/// which no tracked version named a single one of them, because a sweep with no
/// subject passes for the same reason a correct one does.
fn verify_transformer_shape(
    root: &Path,
    inventories: &[HookInventory],
    violations: &mut Vec<String>,
) {
    let mut held = Vec::new();
    let mut named = 0;
    for inventory in inventories {
        let before = violations.len();
        let mut seams = 0;
        for seam in &TRANSFORMER_SEAMS {
            let Some((kind, descriptor)) = declared_hook(&inventory.text, seam.id) else {
                continue;
            };
            seams += 1;
            named += 1;
            verify_seam_shape(inventory, seam, kind, descriptor, violations);
        }
        if violations.len() == before && seams > 0 {
            held.push(format!(
                "held: Minecraft {} against {seams} transformer seams",
                inventory.version
            ));
        }
    }

    if named == 0 {
        violations.push(format!(
            "no tracked version declares a seam {AGENT_JAR_SOURCES}'s transformer injects into, \
             so its parameter positions and return kinds were checked against nothing"
        ));
    }

    let record = root.join(TRANSFORMER_SHAPE_RECORD);
    let written = record
        .parent()
        .map_or_else(|| Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&record, held.join("\n") + "\n"));
    if let Err(error) = written {
        violations.push(format!(
            "cannot record the verified transformer shapes in \
             {TRANSFORMER_SHAPE_RECORD}: {error}"
        ));
    }
}

/// Every assumption one seam makes that one version's declaration does not bear out.
fn verify_seam_shape(
    inventory: &HookInventory,
    seam: &Seam,
    kind: &str,
    descriptor: &str,
    violations: &mut Vec<String>,
) {
    let path = &inventory.path;
    let id = seam.id;
    if seam.receiver && kind != "instance-method" {
        violations.push(format!(
            "{path} declares {id} as a {kind} and the transformer loads its receiver; local zero \
             is the first argument on a static method, so the injected call would hand the \
             adapter an argument in place of the object the seam is about"
        ));
    }

    let Some(parameters) = descriptor_parameters(descriptor) else {
        violations.push(format!(
            "{path} declares {id} at `{descriptor}`, which is not a method descriptor, so the \
             transformer's slots are derived from nothing"
        ));
        return;
    };

    for (position, shape) in seam.loads {
        let (index, declared) = match position {
            Parameter::At(index) => (*index, parameters.get(*index)),
            Parameter::Last => (parameters.len().saturating_sub(1), parameters.last()),
        };
        let Some(declared) = declared else {
            violations.push(format!(
                "{path} declares {id} with {} parameters and the transformer loads parameter \
                 {index}; a load past the declared list is a local the method never received",
                parameters.len()
            ));
            continue;
        };
        if !shape.matches(declared) {
            violations.push(format!(
                "{path} declares parameter {index} of {id} as `{declared}` and the transformer \
                 loads {} there; the load and the local disagree, which the class file verifier \
                 refuses at load time rather than anything here refusing at build time",
                shape.instruction()
            ));
        }
    }

    if !seam.returns.is_empty() {
        let returned = descriptor
            .split_once(')')
            .map_or(descriptor, |(_, returned)| returned);
        if !seam.returns.iter().any(|shape| shape.matches(returned)) {
            violations.push(format!(
                "{path} declares {id} returning `{returned}` and the transformer injects at its \
                 return for {}; a sequence written for another return kind either matches no \
                 return instruction and is silently never applied, or leaves the stack the \
                 verifier refuses",
                seam.returns
                    .iter()
                    .copied()
                    .map(Shape::instruction)
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        }
    }
}

impl Shape {
    /// Whether one declared type is the shape an injected instruction needs.
    fn matches(self, declared: &str) -> bool {
        match self {
            Self::Reference => declared.starts_with('L') || declared.starts_with('['),
            Self::Boolean => declared == "Z",
            Self::Int => declared == "I",
            Self::Void => declared == "V",
        }
    }

    /// How a violation names the instruction this shape belongs to.
    fn instruction(self) -> &'static str {
        match self {
            Self::Reference => "a reference",
            Self::Boolean => "a boolean",
            Self::Int => "an int",
            Self::Void => "nothing",
        }
    }
}

/// One hook's declared kind and descriptor, or nothing when the id is absent.
///
/// An entry declaring the id without one of the two reads as absent, because
/// [`hook_counts`] is what reports an entry missing either and one file is not
/// two violations.
fn declared_hook<'a>(inventory: &'a str, id: &str) -> Option<(&'a str, &'a str)> {
    for block in inventory.split("[[hook]]").skip(1) {
        let body = block
            .split("\n[")
            .next()
            .expect("splitting a string yields at least one part");
        let mut declared = None;
        let mut kind = None;
        let mut descriptor = None;
        for line in body.lines().map(str::trim) {
            if let Some(value) = quoted_value(line, "id") {
                declared = Some(value);
            } else if let Some(value) = quoted_value(line, "kind") {
                kind = Some(value);
            } else if let Some(value) = quoted_value(line, "descriptor") {
                descriptor = Some(value);
            }
        }
        if declared == Some(id) {
            return Some((kind?, descriptor?));
        }
    }
    None
}

/// The templates the Ravel-owned screen jar's sources are generated from.
const SCREEN_JAR_SOURCES: &str = "jvm/screen/template";

/// Refuses a screen template that spells a game name itself.
///
/// A screen extends the game's own `Screen` and overrides its methods, so every
/// game name it uses is a constant-pool entry the JVM links directly: unlike the
/// agent jar's adapters, it cannot reach one through a runtime string. That is
/// why the sources are generated — `jvm/screen/template` is one tree for every
/// tracked version, and the generator renders each `{{…}}` into the name that
/// version links, in the same pass that emits the stubs they compile against.
///
/// A Mojang name written into a template escapes that pass. It is one version's
/// spelling asserted over all of them and it is the obfuscated name of none, so
/// it is refused whatever the tracked versions declare, exactly as a game literal
/// under [`AGENT_JAR_SOURCES`] is.
///
/// **What it cannot do.** It sees only the prefixes [`GAME_PACKAGES`] lists, and
/// a template that reaches a game class through a simple name it never spells in
/// full — one the generator resolves to nothing and the build then fails on — is
/// caught by the generator rather than here.
fn verify_screen_templates(root: &Path, violations: &mut Vec<String>) {
    let directory = root.join(SCREEN_JAR_SOURCES);
    let templates = screen_templates(&directory);
    if templates.is_empty() {
        violations.push(format!(
            "{SCREEN_JAR_SOURCES} holds no screen template, so the screen jar is generated from \
             nothing"
        ));
        return;
    }

    for path in templates {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(source) = fs::read_to_string(&path) else {
            violations.push(format!("cannot read {relative}"));
            continue;
        };
        for (index, line) in source.lines().enumerate() {
            for spelled in game_names_outside_placeholders(line) {
                violations.push(format!(
                    "{relative}:{} spells the game name {spelled} itself. A screen template is one \
                     source for every tracked version and a game name written here is one \
                     version's spelling asserted over all of them — the obfuscated name of none. \
                     Name it inside a placeholder, which the generator renders into the name that \
                     version links",
                    index + 1
                ));
            }
        }
    }
}

/// Holds every version README to the inventory sitting beside it.
///
/// A README that miscounts the file next to it is worse than one that says
/// nothing, because a reader believes it: 1.21.1's said twenty-seven hooks and
/// ten non-goals while `hooks.toml` carried twenty-nine and eight. The sentence
/// is required rather than optional, so a version cannot opt out of being
/// counted by staying silent.
fn verify_version_readmes(root: &Path, violations: &mut Vec<String>) {
    let Ok(versions) = tracked_versions(root) else {
        return;
    };
    for version in versions {
        let directory = root.join(VERSION_DATA).join(&version);
        let Ok(inventory) = fs::read_to_string(directory.join("hooks.toml")) else {
            continue;
        };
        let Ok(counts) = hook_counts(&inventory) else {
            continue;
        };
        let non_goals = inventory.matches("[[nonGoal]]").count();
        let expected = format!(
            "carries {} reviewed hooks and records {non_goals} non-goals",
            counts.entries
        );
        let readme = format!("{VERSION_DATA}/{version}/README.md");
        let Ok(prose) = fs::read_to_string(directory.join(VERSION_PROSE_FILE)) else {
            violations.push(format!("cannot read {readme}"));
            continue;
        };
        if !prose.contains(&expected) {
            violations.push(format!(
                "{readme} does not state \"{expected}\", which is what \
                 {VERSION_DATA}/{version}/hooks.toml declares"
            ));
        }
    }
}

/// Holds the per-version screen template overrides to their contract.
///
/// Three things, and each is a silent failure rather than a loud one if nobody
/// checks it. A directory named for no tracked version contributes nothing and
/// raises nothing, which is the inverse of the generator's refusal of an
/// override that replaces nothing. An override whose name matches no shared
/// template is a screen only one version draws, and the agent jar opens the same
/// constants under every version. And an override that differs from the shared
/// template only inside its placeholders is a rename, which a role binding in
/// that version's `stubs.toml` already handles — reaching for a whole second
/// source there buys a copy that has to be maintained twice, so it is refused
/// here rather than left to a reviewer to notice.
///
/// This is repository-wide and runs whatever any build selected, which is the
/// point: the generator reads one version's overrides and can say nothing about
/// the rest.
fn verify_screen_overrides(root: &Path, violations: &mut Vec<String>) {
    let directory = root.join(SCREEN_JAR_SOURCES);
    let Ok(versions) = tracked_versions(root) else {
        violations.push(format!("cannot read {VERSION_DATA}"));
        return;
    };
    let Ok(entries) = fs::read_dir(&directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !versions.contains(&name) {
            violations.push(format!(
                "{SCREEN_JAR_SOURCES}/{name} is named for no tracked version, so every template \
                 in it is read by no build and refused by nothing"
            ));
            continue;
        }
        let Ok(overrides) = fs::read_dir(&path) else {
            violations.push(format!("cannot read {SCREEN_JAR_SOURCES}/{name}"));
            continue;
        };
        for over in overrides.flatten() {
            let file = over.file_name().to_string_lossy().into_owned();
            if over.path().is_dir() {
                violations.push(format!(
                    "{SCREEN_JAR_SOURCES}/{name}/{file} is a directory, and a version's templates \
                     are read from one level only, so anything inside it is left out silently"
                ));
                continue;
            }
            if !file.ends_with(SCREEN_TEMPLATE_SUFFIX) {
                continue;
            }
            let shared = directory.join(&file);
            if !shared.is_file() {
                violations.push(format!(
                    "{SCREEN_JAR_SOURCES}/{name}/{file} overrides no shared template, and the \
                     agent jar opens the same screens under every version"
                ));
                continue;
            }
            let (Ok(left), Ok(right)) =
                (fs::read_to_string(&shared), fs::read_to_string(over.path()))
            else {
                violations.push(format!("cannot read {SCREEN_JAR_SOURCES}/{name}/{file}"));
                continue;
            };
            if outside_placeholders(&left) == outside_placeholders(&right) {
                violations.push(format!(
                    "{SCREEN_JAR_SOURCES}/{name}/{file} differs from the shared template only \
                     inside its placeholders, which is a rename. Bind the roles in \
                     {VERSION_DATA}/{name}/stubs.toml instead; a per-version source is for an API \
                     that genuinely differs, and this one buys a second copy to maintain"
                ));
            }
        }
    }
}

/// One template's text with every placeholder replaced by one sentinel.
///
/// What is left is the Java a template renders regardless of which names it asks
/// for, so two templates that agree here differ only in the names — which is a
/// rename, and a rename is a binding rather than a source.
fn outside_placeholders(source: &str) -> String {
    let mut text = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(index) = rest.find("{{") {
        text.push_str(&rest[..index]);
        text.push('\u{0}');
        let after = &rest[index + 2..];
        match after.find("}}") {
            Some(end) => rest = &after[end + 2..],
            None => return text,
        }
    }
    text.push_str(rest);
    text
}

/// Every screen template under one directory, in a stable order.
fn screen_templates(directory: &Path) -> Vec<PathBuf> {
    let mut templates = Vec::new();
    collect_files(directory, &mut templates);
    templates.retain(|path| {
        path.to_str()
            .is_some_and(|name| name.ends_with(SCREEN_TEMPLATE_SUFFIX))
    });
    templates.sort();
    templates
}

/// The suffix a screen template carries over the Java source it renders.
const SCREEN_TEMPLATE_SUFFIX: &str = ".java.in";

/// Every game name one template line carries outside a `{{…}}`.
fn game_names_outside_placeholders(line: &str) -> Vec<String> {
    let mut spelled = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let (outside, remainder) = match rest.split_once("{{") {
            Some((outside, inside)) => (
                outside,
                inside.split_once("}}").map_or("", |(_, after)| after),
            ),
            None => (rest, ""),
        };
        for package in GAME_PACKAGES {
            let mut search = outside;
            while let Some(index) = search.find(package) {
                let after = &search[index..];
                let end = after
                    .find(|character: char| {
                        !(character.is_alphanumeric() || character == '.' || character == '_')
                    })
                    .unwrap_or(after.len());
                spelled.push(after[..end].trim_end_matches('.').to_owned());
                search = &after[end..];
            }
        }
        rest = remainder;
    }
    spelled
}

/// The generated Java class the shared adapters read their game names out of.
const GAME_NAMES: &str = "GameNames";

/// Every role a source reaches through the generated class.
fn generated_roles(source: &str) -> Vec<String> {
    let mut roles = Vec::new();
    let mut rest = source;
    let anchor = format!("{GAME_NAMES}.");
    while let Some(index) = rest.find(&anchor) {
        let after = &rest[index + anchor.len()..];
        let end = after
            .find(|character: char| !character.is_ascii_uppercase() && character != '_')
            .unwrap_or(after.len());
        if end > 0 {
            roles.push(after[..end].to_owned());
        }
        rest = &after[end..];
    }
    roles.sort();
    roles.dedup();
    roles
}

/// The record that states in prose how large the reflective surface is.
const JVM_ENTRY_DECISION: &str = "docs/decisions/adg-1-jvm-entry.md";

/// Anchors the one prose figure written about the set above to the set itself.
///
/// The list is designed to grow: bringing a member under the adapter means adding
/// a class name to it, and the check above already fails in both directions when
/// the jar and the list disagree. What nothing read was the sentence in ADG-1 that
/// counts the list for a reader, so the figure could go stale on exactly the edit
/// the rest of this function exists to force. It is one line of prose against one
/// `len()`, which is the cheapest anchor in this repository and the reason there
/// is no excuse for the figure being ungated.
///
/// Known limit: this holds the count and nothing else. A record that states the
/// right number and describes the wrong classes passes, because prose about what
/// the names *are* is held by nothing but reading.
///
/// The agent jar the sentence describes exists once per version, so the record
/// owes a figure per *distinct* surface size: the figures it states and the sizes
/// the tracked versions declare have to be the same set. Holding one figure
/// against every surface's `len()` was unsatisfiable the moment two versions
/// differed in size — no wording makes one number equal two — which hard-blocked
/// the second version in the gate instead of in the work package that owns it.
/// Set equality is the same rule under one version, where the set of sizes is a
/// singleton, and under two it asks for the amendment the record can actually
/// write. Versions that agree on a size share one figure, which is why the
/// comparison is over distinct values rather than over a figure per version.
fn verify_resolved_class_figure(
    root: &Path,
    surfaces: &[ReflectiveSurface],
    violations: &mut Vec<String>,
) {
    let record = match fs::read_to_string(root.join(JVM_ENTRY_DECISION)) {
        Ok(record) => record,
        Err(error) => {
            violations.push(format!("cannot read {JVM_ENTRY_DECISION}: {error}"));
            return;
        }
    };
    let stated = numbers_before(&record, &["classes", "by", "name"]);
    if stated.is_empty() {
        violations.push(format!(
            "{JVM_ENTRY_DECISION} states no count of the classes the agent jar's adapters resolve \
             by name; the figure is the one thing about that set a reader takes on trust, so it is \
             required rather than optional"
        ));
        return;
    }
    let stated: BTreeSet<usize> = stated.into_iter().collect();
    let mut declaring: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for surface in surfaces {
        declaring
            .entry(surface.roles.len())
            .or_default()
            .push(surface.path.as_str());
    }

    for (declared, paths) in &declaring {
        if !stated.contains(declared) {
            violations.push(format!(
                "{} declares {declared} classes and {JVM_ENTRY_DECISION} states no such count; \
                 the record owes one figure per distinct surface size",
                paths.join(", ")
            ));
        }
    }
    for count in &stated {
        if !declaring.contains_key(count) {
            let sizes = declaring
                .keys()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            violations.push(format!(
                "{JVM_ENTRY_DECISION} says the agent jar's adapters resolve {count} classes by \
                 name while no tracked version declares that many ({sizes})"
            ));
        }
    }
}

/// The author-facing documents whose enumerations are held to the code.
const GUIDE_DIRECTORY: &str = "docs/guide";

/// The crate declaring the refusals the guide enumerates.
const NETWORK_SOURCE: &str = "crates/ravel-network/src/lib.rs";

/// The enum the guide states in full, and the one it only draws from.
const ENUMERATED_IN_FULL: &str = "WireError";
const NAMED_IN_PART: &str = "DeliveryRefused";

/// The document holding the passage that claims to state [`ENUMERATED_IN_FULL`]
/// in full.
const ENUMERATING_DOCUMENT: &str = "docs/guide/running.md";

/// The sentence that makes the completeness claim, and the anchor the passage
/// making it is found by.
///
/// The claim is what creates the obligation, so the claim is what locates the
/// list held to it. A passage that stops claiming completeness would need no
/// check — but deleting the sentence while leaving the list is the cheap way out,
/// so its absence is a violation rather than a pass.
const COMPLETENESS_CLAIM: &str = "is the whole of those refusals";

/// Holds the guide's enumeration of a public enum to the enum itself.
///
/// `docs/guide/running.md` told an author that five listed refusals "are the
/// whole of `WireError`" — a count that did not match its own list, over a list
/// that omitted `NotUtf8` and `NotABool` and included a refusal belonging to a
/// different enum. Those two omissions are exactly the variants that ambush
/// someone writing an exhaustive `match`, and the sentence claiming completeness
/// is what stopped them checking.
///
/// `tests/conformance/tests/guide_snippets.rs` compiles every fenced Rust block
/// in the guide and deliberately reaches no prose, which its own module doc says.
/// This is the prose half, and the two directions are held over different spans
/// on purpose. A `WireError::` variant named anywhere in the guide has to be a
/// variant the enum declares, because a name that does not exist misleads an
/// author wherever it is written. Completeness is held over the one passage that
/// claims it — [`COMPLETENESS_CLAIM`] in [`ENUMERATING_DOCUMENT`] — because that
/// passage is what an author reads as the whole set, and a variant named in
/// passing on some other page does not make their list complete. Holding
/// completeness over the concatenated directory instead proves the weaker
/// property: a round dropped a variant from the enumeration, pasted the string
/// into `limits.md`, and the check returned 0. A seventh variant fails as an
/// omission and a renamed one fails twice, by both names.
///
/// [`NAMED_IN_PART`] is held one way only, and the asymmetry is deliberate rather
/// than an oversight: the guide names `DeliveryRefused::WrongSide` because that is
/// the refusal an author meets at the boundary, and it has no reason to enumerate
/// the rest of that enum. So a name the guide states must exist, and a variant the
/// guide does not state is not a violation.
///
/// What this cannot do: it reads variant *names* and nothing else, so a variant
/// the guide names and describes wrongly passes here. Only the rustdoc beside the
/// declaration says what each one carries, which is why the guide points at it
/// rather than restating it. And it holds one passage, found by one sentence, so
/// a second passage elsewhere in the guide claiming the same set in different
/// words is held by nothing until its own claim is named here.
fn verify_guide_enumerations(root: &Path, violations: &mut Vec<String>) {
    let source = match fs::read_to_string(root.join(NETWORK_SOURCE)) {
        Ok(source) => source,
        Err(error) => {
            violations.push(format!("cannot read {NETWORK_SOURCE}: {error}"));
            return;
        }
    };
    let mut guide = String::new();
    let mut documents = Vec::new();
    collect_files(&root.join(GUIDE_DIRECTORY), &mut documents);
    documents.sort();
    for path in &documents {
        match fs::read_to_string(path) {
            Ok(document) => guide.push_str(&document),
            Err(error) => violations.push(format!(
                "cannot read {}: {error}",
                relative_label(root, path)
            )),
        }
    }

    let passage = match fs::read_to_string(root.join(ENUMERATING_DOCUMENT)) {
        Ok(document) => enumerating_passage(&document).map(str::to_owned),
        Err(error) => {
            violations.push(format!("cannot read {ENUMERATING_DOCUMENT}: {error}"));
            return;
        }
    };
    let Some(passage) = passage else {
        violations.push(format!(
            "{ENUMERATING_DOCUMENT} no longer holds the passage saying {ENUMERATED_IN_FULL} \
             \"{COMPLETENESS_CLAIM}\", so no list an author reads as complete is held to \
             {NETWORK_SOURCE}; the claim is what this check exists for and dropping it while \
             keeping the list is not a way out of it"
        ));
        return;
    };

    for name in [ENUMERATED_IN_FULL, NAMED_IN_PART] {
        let declared = declared_variants(&source, name);
        if declared.is_empty() {
            violations.push(format!(
                "{NETWORK_SOURCE} declares no variant for {name}, so the enumeration \
                 {GUIDE_DIRECTORY} states is held to nothing"
            ));
            continue;
        }
        let stated = stated_variants(&guide, name);
        for extra in stated.difference(&declared) {
            violations.push(format!(
                "{GUIDE_DIRECTORY} names {name}::{extra} and {NETWORK_SOURCE} declares no such \
                 variant; an author told to match on a name that does not exist finds out at the \
                 compiler rather than here"
            ));
        }
        if name != ENUMERATED_IN_FULL {
            continue;
        }
        let enumerated = stated_variants(&passage, name);
        for missing in declared.difference(&enumerated) {
            violations.push(format!(
                "{ENUMERATING_DOCUMENT}'s enumeration of {name} omits {name}::{missing}, which \
                 {NETWORK_SOURCE} declares; a variant left out of a list an author reads as \
                 complete is the one their match arm will not have, and naming it on another page \
                 does not complete this one"
            ));
        }
    }
}

/// The paragraph that claims to state an enum in full, found by the claim itself.
///
/// A paragraph rather than the document, because the document is where a variant
/// may legitimately be named in passing and the paragraph is where the set is
/// asserted whole.
fn enumerating_passage(document: &str) -> Option<&str> {
    document
        .split("\n\n")
        .find(|paragraph| paragraph.contains(COMPLETENESS_CLAIM))
}

/// Every variant one public enum declares, read from its own block.
fn declared_variants(source: &str, name: &str) -> BTreeSet<String> {
    let opening = format!("pub enum {name} {{");
    let mut variants = BTreeSet::new();
    let mut inside = false;
    for line in source.lines() {
        if !inside {
            inside = line.trim() == opening;
            continue;
        }
        if line == "}" {
            break;
        }
        // A variant is the only thing declared at the block's own indentation
        // that starts with a capital; a field inside one is indented further and
        // an attribute or a doc comment starts with a sigil.
        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        if rest.starts_with(char::is_uppercase) {
            let variant: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !variant.is_empty() {
                variants.insert(variant);
            }
        }
    }
    variants
}

/// Every variant of one enum a document names in the `Enum::Variant` form.
fn stated_variants(document: &str, name: &str) -> BTreeSet<String> {
    let prefix = format!("{name}::");
    let mut variants = BTreeSet::new();
    let mut rest = document;
    while let Some(at) = rest.find(&prefix) {
        rest = &rest[at + prefix.len()..];
        let variant: String = rest
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !variant.is_empty() {
            variants.insert(variant);
        }
    }
    variants
}

/// Refuses the claim that an artifact *names* no game type, in tracked text.
///
/// Two M3 repair passes each swept for the sentence "names no game type" by hand
/// and each left sites behind, and a third pass found more. A grep finds them all
/// at once, which is why this is a gate rather than a fourth sweep. The claim is
/// false of two shipped artifacts, both of them generated: `Hook.java` carries
/// the internal name of every class it transforms, and `GameNames.java` is
/// nothing but game class names, one for every role the version's reflective
/// surface declares. The tracked sources carry none of them, which is what
/// [`verify_persistence_reflection`] holds them to; the jar they compile into
/// carries both generated classes.
/// What holds is narrower and enforced by construction — the jar *declares* no
/// game type, so `jvm/build/build-agent.sh` compiles it with no `-cp` at all and
/// needs no licensed fixture, and a source declaring one would fail to compile.
///
/// So the naming form is refused and the declaring form is left alone. A
/// quotation of the naming form is left alone too: `docs/decisions/adg-1-jvm-entry.md`
/// and [`verify_persistence_reflection`]'s own documentation each quote
/// "names no game type" in order to say it was false, and a record of a mistake is
/// not the mistake.
///
/// **What it cannot catch, stated because a gate that promises more than it
/// enforces is the defect this repository keeps finding.** It matches four verbs
/// against one noun phrase within one line, so the same false claim survives any
/// rewording — "carries no game class", "mentions no Minecraft type", or a
/// sentence that breaks across two lines. It reads what a file says and never what
/// an artifact holds, so it cannot tell a true claim from a false one, only a
/// refused phrasing from a permitted one: the declaring form would pass here even
/// if `build-agent.sh` grew a `-cp`. And it exempts `evidence/`, whose closed
/// manifests and review records report the false claim as a finding and cannot be
/// edited to satisfy a gate — so a new evidence record could reintroduce it
/// unseen.
fn verify_game_type_claims(root: &Path, paths: &[String], violations: &mut Vec<String>) {
    // Assembled rather than written out, so the refused phrasing is not a literal
    // in this file and this check does not open by reporting itself.
    let claims: Vec<String> = ["name", "names", "named", "naming"]
        .into_iter()
        .map(|verb| format!("{verb} no game type"))
        .collect();

    for relative in paths {
        // A symlink's tracked content is its target path, which carries no prose.
        let path = root.join(relative);
        if relative.starts_with("evidence/") || path.is_symlink() {
            continue;
        }
        let text = match fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => {
                violations.push(format!("cannot scan tracked file {relative}: {error}"));
                continue;
            }
        };

        for (index, line) in text.lines().enumerate() {
            for claim in &claims {
                if !line.contains(claim.as_str()) || line.contains(&format!("\"{claim}\"")) {
                    continue;
                }
                violations.push(format!(
                    "{relative}:{} says an artifact `{claim}`. That is false of the always \
                     present agent jar and of generated Hook.java, both of which carry game \
                     class names as strings. State the claim ADG-1 enforces instead — the jar \
                     *declares* no game type, so it compiles with no `-cp` and no licensed \
                     fixture — or put the false form in quotation marks where the point is that \
                     it was false.",
                    index + 1
                ));
            }
        }
    }
}

/// Every game class name one Java source carries as a string literal.
///
/// Literals only, and one line at a time, so a game class named in a doc comment is
/// read as the prose it is and an unpaired quotation mark in prose cannot shift the
/// parity of the rest of the file. Both spellings count: a class file names a class
/// in internal `net/minecraft/…` form and `Class.forName` takes the binary
/// `net.minecraft.…` form, and a source writing either one has written the same
/// coupling.
fn game_class_literals(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*"))
        })
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .map(|literal| literal.replace('/', "."))
        .filter(|literal| {
            GAME_PACKAGES
                .iter()
                .any(|package| literal.starts_with(package))
        })
        .collect()
}

/// Reads `name = "value"` out of one line, or nothing.
fn quoted_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(name)?.trim_start();
    rest.strip_prefix('=')?
        .trim_start()
        .strip_prefix('"')?
        .strip_suffix('"')
}

/// How many parameters of a method descriptor the generator renders as handles.
///
/// A reference is a handle, an array of anything is a reference, and
/// `java/lang/String` is the one reference the generator renders as a string
/// instead. Returns nothing when the descriptor is not a method descriptor.
fn descriptor_handles(descriptor: &str) -> Option<usize> {
    Some(
        descriptor_parameters(descriptor)?
            .iter()
            .filter(|parameter| {
                parameter.starts_with('[')
                    || (parameter.starts_with('L') && **parameter != "Ljava/lang/String;")
            })
            .count(),
    )
}

/// Each parameter of a method descriptor, in declaration order.
///
/// Split rather than counted, because a slot and a position are two different
/// numbers: a `long` or a `double` occupies two locals, so nothing that walks
/// the list can be replaced by an index into the text. Returns nothing when the
/// descriptor is not a method descriptor.
fn descriptor_parameters(descriptor: &str) -> Option<Vec<&str>> {
    let mut rest = descriptor.strip_prefix('(')?.split_once(')')?.0;
    let mut parameters = Vec::new();
    while !rest.is_empty() {
        let arrays = rest.len() - rest.trim_start_matches('[').len();
        let named = &rest[arrays..];
        let length = if let Some(class) = named.strip_prefix('L') {
            class.split_once(';')?.0.len() + 2
        } else {
            if !matches!(
                named.as_bytes().first()?,
                b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z'
            ) {
                return None;
            }
            1
        };
        let (parameter, tail) = rest.split_at(arrays + length);
        parameters.push(parameter);
        rest = tail;
    }
    Some(parameters)
}

/// Recomputes every digest a milestone manifest records for its own results.
///
/// The schema constrains the shape of `benchmarks.results[].sha256` and nothing
/// recomputed the value, so recorded digests drifted from the files they name
/// the moment a later change edited one of those files. A manifest whose
/// digests do not bind its own records is an index rather than evidence, and
/// the drift is invisible precisely because every field still validates.
///
/// `benchmarks.results[]` and every `platformEvidence[].baselines[]` digest that
/// names a tracked file are recomputed against the working copy, because a
/// milestone owns those files. `inputs[]` names files outside the evidence
/// directory that go on changing after the milestone closes, so it is
/// recomputed against the revision the manifest pins instead; hashing it here
/// would rewrite history rather than verify it.
#[must_use]
pub fn verify_evidence_records(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let mut manifests = fs::read_dir(root.join("evidence"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path().join("manifest.json"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    manifests.sort();

    // A discovery that found nothing is not a tree with nothing to check; it is a
    // check with nothing to say. `required_scaffold_paths` names each milestone
    // manifest, so an absent one is reported there by name; this is the floor for
    // the sweep itself.
    if manifests.is_empty() {
        violations.push(
            "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
             vacuously"
                .to_owned(),
        );
        return violations;
    }

    // A digest can only bind a file the repository holds. Listing the tracked set
    // is therefore part of the binding, and a listing that fails takes the whole
    // sweep with it rather than letting every digest resolve against whatever the
    // working copy happens to contain.
    let tracked = match tracked_paths(root) {
        Ok(paths) => paths.into_iter().collect::<BTreeSet<String>>(),
        Err(error) => {
            violations.push(error);
            return violations;
        }
    };

    for manifest in &manifests {
        verify_recorded_digests(root, manifest, &tracked, &mut violations);
        verify_recorded_baselines(root, manifest, &tracked, &mut violations);
        verify_recorded_review(root, manifest, &tracked, &mut violations);
        verify_recorded_inputs(root, manifest, &tracked, &mut violations);
        verify_recorded_run(root, manifest, &mut violations);
        verify_admitted_revisions(root, manifest, &mut violations);
        verify_admitted_inventory(root, manifest, &tracked, &mut violations);
    }

    for (relative, milestone) in MILESTONE_MANIFESTS {
        let manifest = root.join(relative);
        if manifest.is_file() {
            verify_manifest_invariants(root, &manifest, milestone, &mut violations);
        }
    }

    violations
}

/// The closure rules that do not turn on a review verdict.
///
/// `ravel:verify` is what `AGENTS.md` tells every agent to run, and it ran no
/// milestone closure rule at all: `ravel:verify-milestone:*` was a separate,
/// opt-in task, so `revision.changeId` provenance, schema conformance and the
/// recorded arithmetic were checked only by whoever thought to ask. Moving the
/// whole of closure into `ravel:verify` was not available, because M1, M2 and M3
/// record `review.result: "fail"` by design and the repository's own gate has to
/// keep exiting zero.
///
/// So closure is split by what decides it. Everything here is decided by the
/// document and the repository — the schema, the pinned change, the numbers
/// agreeing with each other — and runs on every gate. The verdict rules, which
/// ask what the independent reviewer concluded and whether any finding is
/// blocking, stay in the per-milestone task, because a failing review is a true
/// statement about an open milestone rather than a broken repository.
fn verify_manifest_invariants(
    root: &Path,
    manifest_path: &Path,
    milestone: &str,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    match evidence_schema_matches(root, manifest_path) {
        Ok(true) => {}
        Ok(false) => violations.push(format!("{label} does not conform to the evidence schema")),
        Err(error) => violations.push(error),
    }
    verify_redaction(manifest_path, &label, violations);

    for (filter, requirement) in [
        (
            ".schemaVersion == 1".to_owned(),
            "must record schemaVersion 1",
        ),
        (
            format!(".milestone == \"{milestone}\""),
            "must record the milestone of the directory it sits in",
        ),
        (
            format!(".workPackages | length > 0 and all(startswith(\"{milestone}-W\"))"),
            "must carry work-package IDs belonging to its own milestone",
        ),
    ] {
        require_manifest_rule(
            manifest_path,
            &filter,
            &format!("{label} {requirement}"),
            violations,
        );
    }
    for (filter, requirement) in MANIFEST_INVARIANTS {
        require_manifest_rule(
            manifest_path,
            filter,
            &format!("{label} {requirement}"),
            violations,
        );
    }
    verify_manifest_change(root, manifest_path, violations);
    // Whether a finding anchors the symbol it names is decided by this repository
    // and by no reviewer, so it runs on every gate rather than only where a
    // milestone is closed. A deviation goes stale between milestones, which is
    // exactly when nobody is running the closure task.
    verify_finding_anchors(root, manifest_path, milestone, violations);
    if milestone == "M4" {
        // These two ask whether two documents in the tree agree with each other
        // and with the repository, which is what this function's doc comment
        // calls a rule the repository decides. A review verdict has no bearing on
        // either, so holding them in the opt-in milestone task alone would leave
        // the gate every agent is told to run unable to see an inventory that had
        // gone stale. The milestone gate runs them too, and the two never run in
        // the same invocation.
        verify_m4_surface(root, violations);
        verify_m4_process_matrix(root, violations);
        verify_m4_gate_figures(root, manifest_path, violations);
    }
    if milestone == "M5" {
        // The same argument as M4's above: whether the recorded arithmetic closes
        // and whether the manifest's figures follow from the record's parts are
        // questions about two documents in this tree, which no review verdict
        // bears on. Holding them in the opt-in milestone task alone would leave
        // the gate every agent is told to run unable to see an inventory that had
        // drifted from its own summary.
        verify_m5_inventory_arithmetic(root, violations);
        verify_m5_carried_figures(root, manifest_path, violations);
    }
}

/// The first milestone whose findings must anchor the symbols they name.
///
/// Not retrofitted, and the boundary is a date rather than a judgement about
/// which older findings would have passed: rewriting a closed milestone's
/// findings to satisfy a rule written after them changes evidence an independent
/// review already certified.
const FIRST_ANCHORED_MILESTONE: u32 = 5;

/// The Rust item keywords a definition can follow.
const RUST_DEFINITION_KEYWORDS: [&str; 8] = [
    "fn", "struct", "enum", "trait", "union", "const", "static", "type",
];

/// Requires a finding that names a Rust symbol to carry a checkable anchor.
///
/// A deviation is prose that no measurement takes, so it does not fail when it
/// stops being true — it ages into a false statement while every gate stays
/// green. M4's own evidence carries the shape: a deviation said a symbol had no
/// caller in a running game, the sides half of it was repaired afterwards, and
/// nothing in the repository could have noticed if the repair had left the
/// sentence behind.
///
/// The anchor does not verify the finding, and this check does not pretend to.
/// It holds the one narrow claim inside a finding that a machine can hold — that
/// the named symbol has no caller outside a stated path set — so that the tree
/// moving out from under a finding is loud. `docs/decisions/` records the
/// decision that makes it mandatory from M5 forward and states what it buys.
///
/// Both halves are checked against the tree rather than against the author. What
/// counts as a Rust symbol is decided by what the tracked sources actually
/// define, not by the writer's intention, so a finding cannot exempt itself by
/// declining to think about the rule; and a recorded anchor is checked by
/// sweeping the tracked sources, so it cannot be satisfied by assertion.
///
/// Known limit, stated rather than implied: a candidate is only taken seriously
/// when it is distinguishable from ordinary prose — four characters or more, and
/// either carrying an underscore or an interior capital. A symbol named `open`
/// or `side` is defined in this tree and is also an English word, and requiring
/// an anchor for every sentence containing one would make the rule unusable. So
/// this reaches findings that name real internal symbols and does not claim to
/// reach every possible phrasing.
fn verify_finding_anchors(
    root: &Path,
    manifest_path: &Path,
    milestone: &str,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let tracked = match tracked_paths(root) {
        Ok(tracked) => tracked,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let sources = tracked
        .iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
        })
        .cloned()
        .collect::<Vec<_>>();
    let defined = defined_rust_symbols(root, &sources);

    let findings = match jq_value(
        manifest_path,
        r#"(.deviations[]?, .risks[]?)
            | (.summary | gsub("[\n\t]"; " "))
              + "\t" + (.anchor.symbol // "")
              + "\t" + ((.anchor.paths // []) | join(","))"#,
    ) {
        Ok(findings) => findings,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let anchored_from = milestone
        .strip_prefix('M')
        .and_then(|number| number.parse::<u32>().ok())
        .is_some_and(|number| number >= FIRST_ANCHORED_MILESTONE);

    for finding in findings.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = finding.split('\t');
        let summary = fields.next().unwrap_or_default();
        let symbol = fields.next().unwrap_or_default();
        let paths = fields.next().unwrap_or_default();
        let named = named_rust_symbols(summary, &defined);

        if symbol.is_empty() {
            if anchored_from && !named.is_empty() {
                violations.push(format!(
                    "{label} carries a finding naming the Rust {} {} with no anchor; a finding \
                     naming a symbol must claim something a grep can falsify",
                    if named.len() == 1 {
                        "symbol"
                    } else {
                        "symbols"
                    },
                    named.join(", ")
                ));
            }
            continue;
        }

        if !named.iter().any(|candidate| candidate == symbol) {
            violations.push(format!(
                "{label} anchors a finding on {symbol}, which the finding's own summary does not \
                 name; an anchor over a symbol nobody wrote about constrains nothing"
            ));
        }
        if !defined.contains(symbol) {
            violations.push(format!(
                "{label} anchors a finding on {symbol}, which no tracked Rust source defines"
            ));
            continue;
        }
        let allowed = paths
            .split(',')
            .filter(|prefix| !prefix.is_empty())
            .collect::<Vec<_>>();
        for source in sources_naming(root, &sources, symbol) {
            if !allowed.iter().any(|prefix| source.starts_with(prefix)) {
                violations.push(format!(
                    "{label} anchors a finding on {symbol} outside {paths}, and \
                     {source} names it; the finding has outlived the tree it describes"
                ));
            }
        }
    }
}

/// Every item name the tracked Rust sources define, filtered to the ones a
/// finding could be naming on purpose.
fn defined_rust_symbols(root: &Path, sources: &[String]) -> BTreeSet<String> {
    let mut defined = BTreeSet::new();
    for relative in sources {
        let Ok(source) = fs::read_to_string(root.join(relative)) else {
            continue;
        };
        for line in source.lines() {
            let mut words = line.split_whitespace();
            while let Some(word) = words.next() {
                let keyword = word.trim_end_matches('!');
                if keyword == "macro_rules" || RUST_DEFINITION_KEYWORDS.contains(&keyword) {
                    let Some(name) = words.clone().next() else {
                        continue;
                    };
                    let name = name
                        .split(|character: char| !is_identifier_character(character))
                        .next()
                        .unwrap_or_default();
                    if distinguishable_symbol(name) {
                        defined.insert(name.to_owned());
                    }
                }
            }
        }
    }
    defined
}

/// The identifiers one finding's summary names that the tree defines.
fn named_rust_symbols(summary: &str, defined: &BTreeSet<String>) -> Vec<String> {
    summary
        .split(|character: char| !is_identifier_character(character))
        .filter(|word| defined.contains(*word))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Every tracked Rust source that names one symbol.
fn sources_naming(root: &Path, sources: &[String], symbol: &str) -> Vec<String> {
    sources
        .iter()
        .filter(|relative| {
            fs::read_to_string(root.join(relative)).is_ok_and(|source| {
                source
                    .split(|character: char| !is_identifier_character(character))
                    .any(|word| word == symbol)
            })
        })
        .cloned()
        .collect()
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

/// Whether a name is one a finding would have written deliberately.
fn distinguishable_symbol(name: &str) -> bool {
    name.len() >= 4
        && name.starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
        && (name.contains('_')
            || name
                .chars()
                .skip(1)
                .any(|character| character.is_ascii_uppercase()))
}

/// Names a path relative to the repository, for a violation a reader has to act on.
fn relative_label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Checks the gate run a milestone manifest records against itself.
///
/// A manifest attests a run it cannot contain. The log is private, and the
/// revision the run was made on is below the tip by construction, because
/// recording a result changes the tree that produced it — so no check can ask a
/// manifest to name the state it sits in. What can be bound is that the numbers
/// agree with each other and with the gate they claim to have passed, and that a
/// manifest recording a run records its result at all. That the pinned revision
/// resolves is already bound by `verify_recorded_inputs`, which hashes every
/// input there and fails when it cannot.
///
/// The milestone gate applies the same arithmetic to the one milestone it is
/// closing. This applies it to every manifest on every scaffold run, because the
/// numbers a closed milestone records are exactly the ones nobody looks at again.
fn verify_recorded_run(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    let records_run = match jq_matches(
        manifest_path,
        r#".commands | type == "array" and length > 0"#,
    ) {
        Ok(records_run) => records_run,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let records_tests = match jq_matches(manifest_path, r#".tests | type == "object""#) {
        Ok(records_tests) => records_tests,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    // Neither field present used to return with nothing said, which made a
    // manifest that recorded no run at all indistinguishable from one whose run
    // succeeded. `del(.tests, .commands)` cleared this whole check, and the
    // scaffold schema-validates only the named milestone manifests, so a document
    // that dropped both fields had nothing left to answer for.
    if !records_run && !records_tests {
        violations.push(format!(
            "{label} records neither a gate run nor a test result, so there is nothing here a \
             gate can hold it to"
        ));
        return;
    }
    if records_run && !records_tests {
        violations.push(format!("{label} records a gate run but no test result"));
    }
    if records_run {
        require_manifest_rule(
            manifest_path,
            "[.commands[].exitStatus] | all(. == 0)",
            &format!("{label} records a gate run that did not succeed"),
            violations,
        );
    }
    if !records_tests {
        return;
    }
    for (filter, violation) in [
        (
            ".tests.total == (.tests.passed + .tests.failed + .tests.skipped)",
            "records test counts that do not sum to its total",
        ),
        (
            ".tests.total > 0 and .tests.failed == 0 and .tests.skipped == 0",
            "records no passing test run",
        ),
        (
            ".tests.lineCoveragePercent >= 80",
            "records line coverage below the floor the gate enforces",
        ),
    ] {
        require_manifest_rule(
            manifest_path,
            filter,
            &format!("{label} {violation}"),
            violations,
        );
    }
}

/// Requires a milestone's manifest to name every record its directory holds.
///
/// A digest binds a record only where the manifest names it, so a record the
/// manifest omits is bound by nothing: with `prism-launch.json` removed from
/// `benchmarks.results`, that record's `nativeInvocations` and `classesRewritten`
/// were rewritten to invented numbers and `verify scaffold` exited zero, because
/// the digest that would have caught it was no longer recorded. Each milestone
/// gate names the records its own closure requires, and neither that nor
/// `verify_recorded_digests` can see a file nobody mentions.
///
/// So the comparison is a set equality in both directions. A tracked record the
/// manifest does not name is unbound evidence; a name with no record behind it is
/// a digest over nothing, which `verify_recorded_digests` reports by name and this
/// leaves to it. Untracked files are not records: they are refused by the tracked
/// boundary rules instead, and admitting them here would let an ignored file
/// demand a digest.
fn verify_admitted_inventory(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let Some(directory) = manifest_path.parent() else {
        return;
    };
    let prefix = format!("{}/", relative_label(root, directory));
    let held = tracked
        .iter()
        .filter_map(|path| path.strip_prefix(&prefix))
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "json")
                && *path != "manifest.json"
        })
        .collect::<BTreeSet<_>>();
    let named = match jq_value(
        manifest_path,
        r".benchmarks.results[]?.name,
          (.platformEvidence[]?.baselines[]?.result? | select(. != null) | .name)",
    ) {
        Ok(named) => named,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let named = named
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();

    for record in held.difference(&named) {
        violations.push(format!(
            "{label} names no record {record}, which its own milestone directory tracks; a record \
             no manifest names is a record no digest covers"
        ));
    }
}

/// Every pin a record carries, wherever inside the record it sits.
///
/// One record holds more than one trial — `evidence/m3/pack-compile.json` pins a
/// revision under `trial` and a second under `variants.trial` — so a filter that
/// read `.trial.revision` alone would leave the others free to pin anything.
const TRIAL_PINS: &str = r#"[paths(objects) as $p
        | select(($p | length) >= 2 and ($p[-1]) == "revision" and ($p[-2]) == "trial")
        | {at: ($p | map(tostring) | join(".")), pin: getpath($p)}]
    | .[]
    | .at + " " + (.pin.id // "-") + " " + (.pin.changeId // "-") + " " + (.pin.dirty | type)"#;

/// Requires every pin inside an admitted record to name a tree a reader can obtain.
///
/// `verify_pinned_commit` applies the visible-commit and changeId-owns-id tests to
/// a manifest's own `.revision`, and `verify_recorded_change` applies change
/// provenance only under `evidence/m1/spikes/`. So the revision a *record* pinned
/// was constrained by nothing anywhere: `evidence/m2/loader-overhead.json`'s
/// `trial.revision.id` was set to forty zeroes and its `changeId` to a change that
/// exists nowhere, the manifest digest was rebound, and `verify scaffold` exited
/// zero. A record whose pin binds nothing is a measurement of an unnamed tree.
///
/// Absence and unobtainability are separated deliberately, because they are not
/// the same claim. A record that pins no commit claims no tree, which some records
/// legitimately do and say so in their own prose; there is nothing there to
/// refuse, and the skip is announced rather than silent. A record that pins one
/// makes a statement a reader can act on, and this requires it to be true: the
/// commit has to be reachable from visible heads — the distinction `visible_commit`
/// draws, and the one that separates a commit a clone carries from one only this
/// working copy's operation log can see — and it has to be the commit its own
/// change is at now.
///
/// The escape is a disclosure rather than a tolerance. A pin that fails is
/// permitted only where the admitting manifest lists it under
/// `unobtainableRevisions` quoting that record, that commit and that change, so an
/// excuse cannot cover a pin it does not name; and a disclosed pin that starts
/// passing is itself a violation, so the excuse cannot outlive the defect.
/// What a disclosure may not say, because the gate is what says it.
///
/// A disclosure's `reason` is free prose and no gate can check a judgement about
/// what a reader loses. What it *can* check is that the prose does not also
/// classify the failure, which is the one part of it that goes stale: a round
/// re-pinned a record to a real commit of a different change, updated only the
/// disclosure's `id`, and the excuse still read "reachable from no visible head"
/// while the gate was computing a different reason entirely. Refusing the
/// restatement leaves exactly one statement of why a pin failed — the computed one,
/// printed beside the prose on every run — so the two cannot disagree.
const CLASSIFICATION_PHRASES: [&str; 2] = [
    "reachable from no visible head",
    "the commit its own recorded change is at",
];

fn verify_admitted_revisions(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = relative_label(root, manifest_path);
    if !root.join(".jj").exists() {
        println!(
            "- {label}: the pins its records carry are not recomputed, because this checkout has \
             no jj repository to resolve them in"
        );
        return;
    }
    let Some(directory) = manifest_path.parent() else {
        return;
    };
    // Both places a manifest admits a record. M0 admits its two launch records as
    // baselines rather than as benchmark results, and a rule that read only
    // `benchmarks.results` would leave those two free to pin anything.
    let names = match jq_value(
        manifest_path,
        r".benchmarks.results[]?.name,
          (.platformEvidence[]?.baselines[]?.result? | select(. != null) | .name)",
    ) {
        Ok(names) => names,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let disclosed = match disclosed_pins(manifest_path) {
        Ok(disclosed) => disclosed,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    for (pin, reason) in &disclosed {
        for restated in CLASSIFICATION_PHRASES {
            if reason.contains(restated) {
                violations.push(format!(
                    "{label} discloses the pin `{pin}` with prose that classifies the failure \
                     itself (`{restated}`); a disclosure states what a reader loses and this gate \
                     states why the pin failed, so prose that says why goes stale the next time \
                     the pin moves and nothing catches it"
                ));
            }
        }
    }
    // One disclosure covers every pin that quotes the same record, commit and
    // change, because one record can carry the same pin at more than one trial —
    // `evidence/m3/pack-compile.json` does. So which disclosures were used is
    // tracked beside them rather than by consuming them.
    let mut used = BTreeSet::new();

    for name in names.lines().filter(|name| !name.trim().is_empty()) {
        let record = directory.join(name);
        // An admitted record that is not there is reported by the digest rule,
        // which is where a reader is told to put it back.
        if !record.is_file() {
            continue;
        }
        match jq_value(&record, TRIAL_PINS) {
            Ok(pins) => {
                for pin in pins.lines().filter(|pin| !pin.trim().is_empty()) {
                    verify_one_pin(root, &label, name, pin, &disclosed, &mut used, violations);
                }
            }
            Err(error) => violations.push(error),
        }
    }

    for pin in disclosed.keys().filter(|pin| !used.contains(*pin)) {
        violations.push(format!(
            "{label} discloses the pin `{pin}` as unobtainable, and no record it admits carries \
             it; a disclosure that names no pin excuses nothing and hides the next one"
        ));
    }
}

/// Holds one pin to the disclosure or to the tests, and reports which.
fn verify_one_pin(
    root: &Path,
    label: &str,
    name: &str,
    pin: &str,
    disclosed: &BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let fields = pin.split_whitespace().collect::<Vec<_>>();
    let [at, id, change, dirty] = fields.as_slice() else {
        violations.push(format!(
            "{label} admits {name}, which records a trial revision this gate cannot read: {pin}"
        ));
        return;
    };
    // A run on a dirty tree measured something no commit holds, so whether it was
    // dirty is part of what the pin means. Left unstated it reads as clean.
    if *dirty != "boolean" {
        violations.push(format!(
            "{label} admits {name}, whose trial at {at} does not declare whether the tree it \
             measured was dirty"
        ));
    }
    if *id == "-" {
        println!(
            "- {label}: {name} pins no commit at {at}, so it claims no tree and none is recomputed"
        );
        return;
    }
    if *change == "-" {
        violations.push(format!(
            "{label} admits {name}, whose trial at {at} pins the commit {id} and no change; every \
             rewrite abandons a commit, so a pin no change names cannot be recovered"
        ));
        return;
    }
    let key = format!("{name} {id} {change}");
    match (pin_is_obtainable(root, id, change), disclosed.get(&key)) {
        (Ok(()), None) => {}
        (Ok(()), Some(_)) => {
            // Counted as used even though it is refused, so the one defect is
            // reported once rather than again as a disclosure nothing carries.
            used.insert(key);
            violations.push(format!(
                "{label} discloses {name}'s pin {id} as unobtainable and it now resolves to the \
                 commit change {change} is at; remove the disclosure rather than leaving an excuse \
                 the defect has outlived"
            ));
        }
        (Err(why), Some(reason)) => {
            used.insert(key);
            println!(
                "- {label}: {name}'s pin {id} at {at} is disclosed rather than recomputed, \
                 because {why}. What the record says a reader loses by it, which is prose this \
                 gate does not check: {reason}"
            );
        }
        (Err(why), None) => violations.push(format!(
            "{label} admits {name}, whose trial at {at} pins {id}, and {why}; re-pin it to the \
             commit change {change} is at and re-measure there, or disclose it under \
             unobtainableRevisions with what a reader loses"
        )),
    }
}

/// Whether a pinned commit is one a reader can obtain under its own change.
fn pin_is_obtainable(root: &Path, id: &str, change: &str) -> Result<(), String> {
    let pinned = visible_commit(root, id).map_err(|error| format!("that commit {error}"))?;
    let current =
        visible_commit(root, change).map_err(|error| format!("its change {change} {error}"))?;
    if pinned == current {
        return Ok(());
    }
    Err(format!(
        "change {change} is at {current} instead, so the pin names a tree that change no longer \
         holds"
    ))
}

/// The pins a manifest discloses as unobtainable, keyed as they are compared.
fn disclosed_pins(manifest_path: &Path) -> Result<BTreeMap<String, String>, String> {
    let listed = jq_value(
        manifest_path,
        r#".unobtainableRevisions[]? | .record + " " + .id + " " + .changeId + " " + .reason"#,
    )?;
    let mut disclosed = BTreeMap::new();
    for line in listed.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.splitn(4, ' ');
        let key = match (fields.next(), fields.next(), fields.next()) {
            (Some(record), Some(id), Some(change)) => format!("{record} {id} {change}"),
            _ => {
                return Err(format!(
                    "{} discloses an unobtainable revision naming no record, commit and change: \
                     {line}",
                    redacted_path(manifest_path)
                ));
            }
        };
        disclosed.insert(key, fields.next().unwrap_or_default().to_owned());
    }
    Ok(disclosed)
}

/// Recomputes every input digest against the revision the manifest pins.
///
/// An input digest is a claim about one revision, so it has to be hashed there.
/// `revision.id` is that revision, and `verify_pinned_commit` requires it to be
/// the visible commit `revision.changeId` is at — so it is the state of the
/// milestone's own change as the repository holds it now, not the tree the
/// recorded commands ran on. Those ran earlier and on a commit the rewrites
/// abandoned; `tests` and `commands` are attestations about that run and nothing
/// recomputes them, which each manifest's deviations state.
///
/// What this binds is narrower and real: the files the milestone names as its
/// inputs hash, at the revision it pins, to the values it records. Hashing them in
/// the working copy instead would rewrite history rather than verify it, because
/// `inputs[]` names files outside the evidence directory that go on changing after
/// a milestone closes.
///
/// Hashing at the pinned revision answers only half of it. A file deleted since
/// the milestone closed still hashes there, so a closed record went on binding two
/// fixture descriptors the ATM10 retarget removed and the gate said nothing. That
/// is the first of the two questions
/// [`machinery-is-not-material.md`](../../../docs/decisions/machinery-is-not-material.md)
/// separates — does everything this record named still exist — and a deletion
/// falsifies what the record claims to have measured. So every input name is held
/// against the tracked set as well, which needs no revision and therefore reports
/// in a plain git tree where the digest recompute cannot.
///
/// The revision is resolved through jj, and a checkout without a jj repository is
/// skipped rather than failed, because there is nothing there to resolve a revision
/// with. That is the one gap left: a clone of this repository does reach the pinned
/// commits, since the colocated repository exports the visible stack to git HEAD, so
/// `jj git init --colocate` in a clone recomputes every digest — but a plain git tree
/// cannot, and reporting every input unreadable there would turn a provenance check
/// into noise. The skip says so on stdout, because a whole provenance check that
/// evaporates in silence reads exactly like one that passed.
fn verify_recorded_inputs(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
    let records = match jq_value(manifest_path, r#".inputs[]? | .name + " " + .sha256"#) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    let mut inputs = Vec::new();
    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        match record.rsplit_once(' ') {
            Some((name, recorded)) => inputs.push((name, recorded)),
            None => violations.push(format!("{label} records an input with no digest: {record}")),
        }
    }

    for (name, _) in &inputs {
        if !tracked.contains(*name) {
            violations.push(format!(
                "{label} records input {name}, which the repository no longer holds, so the \
                 digest binds a path no reader can resolve"
            ));
        }
    }

    if !root.join(".jj").exists() {
        println!(
            "- {label}: input digests are not recomputed, because this checkout has no jj \
             repository to resolve the pinned revision in"
        );
        return;
    }
    let revision = match jq_value(manifest_path, ".revision.id") {
        Ok(value) if value != "null" && !value.is_empty() => value,
        Ok(_) => {
            violations.push(format!(
                "{label} records inputs but no revision to hash them at"
            ));
            return;
        }
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    for (name, recorded) in inputs {
        match revision_digest(root, &revision, name) {
            Ok(actual) if actual == recorded => {}
            Ok(actual) => violations.push(format!(
                "{label} records input {name} as {recorded} but revision {revision} holds {actual}"
            )),
            Err(error) => violations.push(format!("{label} records input {name}: {error}")),
        }
    }
}

/// Hashes a tracked file as it stood in one revision.
fn revision_digest(root: &Path, revision: &str, name: &str) -> Result<String, String> {
    let output = Command::new("jj")
        .args(["file", "show", "-r", revision, "--", name])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot read it at revision {revision}: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot read it at revision {revision}"));
    }
    Ok(hex_digest(&output.stdout))
}

/// Compares each recorded result digest against the file it names.
fn verify_recorded_digests(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let records = match jq_value(
        manifest_path,
        r#".benchmarks.results[]? | .name + " " + .sha256"#,
    ) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(directory) = manifest_path.parent() else {
        return;
    };

    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!("{label} records a result with no digest: {record}"));
            continue;
        };
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "result",
                root,
                tracked,
                manifest_path,
                directory,
                name,
                recorded,
            },
            violations,
        );
    }
}

/// Recomputes every baseline digest a manifest records for a tracked file.
///
/// `benchmarks.results[]` is not the only place a manifest hashes its own
/// records. M0 records a launch summary and a launch validation for each
/// baseline workload, and two of those four digests name tracked evidence files
/// that nothing recomputed, which is the same rot in a second place: correct
/// today, and green on a stale value the moment a later change edits the record.
///
/// A baseline's `result` names a tracked record inside the milestone and is
/// recomputed. Its `validation` names a private launch capture that never enters
/// the repository, which is the one legitimately unbound digest here, and it is
/// announced as unbound rather than silently passed over.
///
/// The two are read through separate filters, because telling them apart by how
/// the name happened to read was itself a hole. Rewording a `result` name from
/// `linux-vanilla-launch.json` to prose failed the whitespace-and-extension
/// heuristic, and failing it produced an announced skip rather than a violation —
/// so a digest could be detached from the file it bound by editing a string, and
/// both the scaffold and the M0 gate stayed at zero. The schema now types the two
/// fields differently, `hashedArtifact` against `privateCapture`, so which digests
/// can be bound is declared in one place and read here rather than guessed.
fn verify_recorded_baselines(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let results = match jq_value(
        manifest_path,
        r#".platformEvidence[]?.baselines[]?.result
           | select(. != null) | .name + " " + .sha256"#,
    ) {
        Ok(results) => results,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let validations = match jq_value(
        manifest_path,
        r".platformEvidence[]?.baselines[]?.validation | select(. != null) | .name",
    ) {
        Ok(validations) => validations,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let Some(directory) = manifest_path.parent() else {
        return;
    };

    for name in validations.lines().filter(|name| !name.trim().is_empty()) {
        println!(
            "- {label}: the baseline validation digest recorded for \"{name}\" is not \
             recomputed, because a launch validation is a private capture that never enters the \
             repository"
        );
    }

    for record in results.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!(
                "{label} records a baseline with no digest: {record}"
            ));
            continue;
        };
        if !names_a_path(name) {
            violations.push(format!(
                "{label} records a baseline result by description rather than by path, so its \
                 digest binds nothing obtainable: {name}"
            ));
            continue;
        }
        verify_tracked_digest(
            &TrackedDigest {
                label: &label,
                kind: "baseline",
                root,
                tracked,
                manifest_path,
                directory,
                name,
                recorded,
            },
            violations,
        );
    }
}

/// Recomputes the digest a manifest records for its own independent review.
///
/// This is the field a milestone is closed on. The gate asked only that
/// `review.record` exist, and the schema typed its `name` as free text, so three
/// of the four manifests named the review by title — "M1 independent
/// architecture review" — and the recorded sha256 bound nothing a reader could
/// obtain. A digest over an unobtainable artifact is weaker than no digest: it
/// reads as proof that a review happened, was independent, and found what the
/// manifest says, while confirming none of it. Two milestones were recorded
/// closed that way.
///
/// So a review record names a repository-relative path inside its own milestone
/// and is hashed here, through the same body as the result and baseline digests.
/// A name that reads as prose is a violation rather than an announced skip: a
/// private capture is legitimately outside the repository, but a review nobody
/// can read is not evidence of a review, and the whole point of the field is
/// that a reader who did not run the review can obtain it.
///
/// Absence stays legal, because a milestone under construction has no review
/// yet. What refuses to close on it is the milestone gate, which requires the
/// record; `verify_general_closure` is where that belongs.
fn verify_recorded_review(
    root: &Path,
    manifest_path: &Path,
    tracked: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let label = relative_label(root, manifest_path);
    let record = match jq_value(
        manifest_path,
        r#".review.record? | select(. != null) | .name + " " + .sha256"#,
    ) {
        Ok(record) => record,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    if record.is_empty() {
        return;
    }
    let Some(directory) = manifest_path.parent() else {
        return;
    };
    let Some((name, recorded)) = record.rsplit_once(' ') else {
        violations.push(format!(
            "{label} records a review record with no digest: {record}"
        ));
        return;
    };
    if !names_a_path(name) {
        violations.push(format!(
            "{label} records a review record by title rather than by path, so its digest \
             binds nothing obtainable: {name}"
        ));
        return;
    }
    verify_tracked_digest(
        &TrackedDigest {
            label: &label,
            kind: "review record",
            root,
            tracked,
            manifest_path,
            directory,
            name,
            recorded,
        },
        violations,
    );
    verify_recorded_verdict(&label, manifest_path, &directory.join(name), violations);
}

/// Requires the bound review record to state the verdict `review.result` claims.
///
/// Binding the record made it obtainable and left the two halves free to
/// disagree: nothing compared the verdict in the document to the field a
/// milestone closes on. That is not hypothetical. `review.result` was set to
/// `pass` while `evidence/m1/review.md` still read `Result: FAIL` on line 3, and
/// `verify milestone M1` exited 0 — the strongest field in the manifest closed a
/// milestone against the document it names, because agreement was a habit rather
/// than a rule.
///
/// So the record's front matter is parsed and compared. The first `Result:` line
/// is the verdict, which is the shape all four records already use, and a record
/// stating none is a violation rather than a pass: a document that does not say
/// what it concluded cannot corroborate a field that says it for it.
fn verify_recorded_verdict(
    label: &str,
    manifest_path: &Path,
    record: &Path,
    violations: &mut Vec<String>,
) {
    let claimed = match jq_value(manifest_path, r#".review.result? // """#) {
        Ok(claimed) => claimed,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    if claimed.is_empty() || claimed == "null" {
        violations.push(format!(
            "{label} binds a review record but records no review.result for it to agree with"
        ));
        return;
    }
    // An unreadable record is already reported by the digest, which cannot hash
    // what it cannot read, so this stays quiet rather than doubling it.
    let Ok(text) = fs::read_to_string(record) else {
        return;
    };
    match text.lines().find_map(stated_verdict) {
        Some(stated) if stated == claimed => {}
        Some(stated) => violations.push(format!(
            "{label} records review.result {claimed} while the review record it binds states \
             {stated}"
        )),
        None => violations.push(format!(
            "{label} binds a review record that states no verdict, so nothing checks that \
             review.result {claimed} is what the review concluded"
        )),
    }
}

/// The verdict one line of a review record states, if it states one.
///
/// Kept to the `Result:` front matter every tracked record uses, tolerating the
/// heading and emphasis markers a Markdown record may carry around it. Anything
/// outside the three verdicts the schema allows is no verdict at all, so a
/// record whose front matter says something else reports as stating none.
fn stated_verdict(line: &str) -> Option<String> {
    let line = line.trim().trim_start_matches(['#', '*', ' ']);
    let value = line
        .strip_prefix("Result:")
        .or_else(|| line.strip_prefix("Result**:"))?;
    let value = value.trim().trim_matches(['*', ' ']).to_ascii_lowercase();
    ["pass", "fail", "not-required"]
        .contains(&value.as_str())
        .then_some(value)
}

/// Whether a recorded name is a path in the repository rather than prose.
///
/// This is the shape the schema's `hashedArtifact.name` pattern requires. It is
/// checked here too, so that a manifest the scaffold reaches before any schema
/// validation still cannot detach a digest from its file, and so that the
/// violation names the field a reader has to fix.
fn names_a_path(name: &str) -> bool {
    !name.contains(char::is_whitespace) && Path::new(name).extension().is_some()
}

/// One recorded digest and where the manifest that records it says to find it.
struct TrackedDigest<'a> {
    label: &'a str,
    kind: &'a str,
    root: &'a Path,
    tracked: &'a BTreeSet<String>,
    manifest_path: &'a Path,
    directory: &'a Path,
    name: &'a str,
    recorded: &'a str,
}

/// Compares one recorded digest against the file inside the milestone it names.
fn verify_tracked_digest(record: &TrackedDigest<'_>, violations: &mut Vec<String>) {
    let TrackedDigest {
        label,
        kind,
        root,
        tracked,
        manifest_path,
        directory,
        name,
        recorded,
    } = *record;
    if name.is_empty() || name.starts_with('/') || name.split('/').any(|part| part == "..") {
        violations.push(format!(
            "{label} records a {kind} outside its own milestone: {name}"
        ));
        return;
    }
    let path = directory.join(name);
    // A manifest cannot carry its own digest. Writing one changes the file
    // it describes, so a check that admitted the record would only ever
    // pass on a value nobody could compute, which is a check that verifies
    // itself rather than the evidence.
    if path == manifest_path {
        violations.push(format!("{label} records its own digest, which cannot bind"));
        return;
    }
    // The digest has to bind a file the repository holds. Hashing the working copy
    // and stopping there let a record be bound to an untracked file, which nobody
    // but the machine that wrote it can obtain — the same failure as naming the
    // artifact in prose, arrived at from the other side.
    let relative = relative_label(root, &path).replace('\\', "/");
    if !tracked.contains(&relative) {
        violations.push(format!(
            "{label} records a {kind} that the repository does not track, so its digest binds \
             nothing a reader can obtain: {name}"
        ));
        return;
    }
    match file_digest(&path) {
        Ok(actual) if actual == recorded => {}
        Ok(actual) => violations.push(format!(
            "{label} records {name} as {recorded} but it hashes {actual}"
        )),
        Err(error) => violations.push(format!("{label} records {name}: {error}")),
    }
}

/// Hashes a tracked file as it stands in the working copy.
fn file_digest(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read the recorded file: {error}"))?;
    Ok(hex_digest(&bytes))
}

/// Renders the SHA-256 of some bytes the way a manifest records it.
fn hex_digest(bytes: &[u8]) -> String {
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(digest, "{byte:02x}").expect("writing to a string cannot fail");
    }
    digest
}

/// Checks every recorded architecture spike result against the M1 gate rules.
///
/// An absent spike tree is not a violation; the M1 milestone gate is what
/// requires results to exist. This check only enforces that recorded results are
/// complete, comparable, and free of substituted values.
#[must_use]
pub fn verify_spikes(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    let spikes = root.join("evidence/m1/spikes");

    for (directory, gate) in SPIKE_GATES {
        let gate_root = spikes.join(directory);
        if !gate_root.is_dir() {
            continue;
        }

        let mut results = fs::read_dir(&gate_root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        results.sort();

        if results.is_empty() {
            violations.push(format!("spike gate {gate} has no recorded result"));
            continue;
        }
        for result in &results {
            verify_spike_result(root, result, gate, &mut violations);
            verify_recorded_change(root, result, &mut violations);
        }
    }

    violations
}

/// Checks that a result names the change that introduced it.
///
/// `trial.revision.id` is a commit, which every jj rewrite abandons, so it cannot
/// be resolved and is not checked. The change id can be, and this is the only
/// reason to prefer it: seven results named the wrong revision for a week because
/// nothing recomputed the claim.
///
/// A checkout with no jj repository cannot resolve a change at all, so the check
/// is skipped there and says so on stdout. It used to return in silence, which is
/// indistinguishable from passing on exactly the machines where the repository's
/// strongest provenance control is absent.
fn verify_recorded_change(root: &Path, path: &Path, violations: &mut Vec<String>) {
    let label = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    if !root.join(".jj").exists() {
        println!(
            "- {label}: the recorded change is not recomputed, because this checkout has no jj \
             repository to resolve it in"
        );
        return;
    }
    let recorded = match jq_value(path, ".trial.revision.changeId") {
        Ok(value) if value != "null" && !value.is_empty() => value,
        // A result with no recorded change is the failure this rule exists for,
        // so it cannot be the branch that returns quietly.
        Ok(_) => {
            violations.push(format!("{label} records no change id"));
            return;
        }
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match introducing_change(root, &label) {
        Ok(owner) => violations.extend(change_mismatch(&label, &recorded, &owner)),
        Err(error) => violations.push(error),
    }
}

/// Resolves the change that introduced a tracked path.
///
/// A lookup that cannot run is an error, not a pass. This is the repository's
/// strongest provenance control, and a control that fails open when its tool is
/// missing verifies nothing on exactly the machines where it matters. Empty
/// output is not an error: an untracked result has no introducing change yet.
fn introducing_change(root: &Path, label: &str) -> Result<String, String> {
    let output = Command::new("jj")
        .args([
            "log",
            "-r",
            &format!("roots(::@ & files('{label}'))"),
            "--no-graph",
            "-T",
            "change_id",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot resolve the change that introduced {label}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot resolve the change that introduced {label}: jj exited {}",
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Checks that a milestone manifest names the change that introduced it.
///
/// The same rule the spike results carry, for the same reason: a commit id is
/// abandoned by every rewrite, so a manifest that records only one cannot be
/// resolved by the reader it is written for.
///
/// As with the spike results, a checkout with no jj repository is skipped and the
/// skip is announced rather than silent.
fn verify_manifest_change(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let Ok(label) = manifest_path.strip_prefix(root) else {
        return;
    };
    let label = label.display().to_string();
    // Contract fixtures are synthetic by design and describe no real revision;
    // only admitted evidence carries a resolvable provenance claim.
    if !label.starts_with("evidence/") {
        return;
    }
    if !root.join(".jj").exists() {
        println!(
            "- {label}: the recorded change is not recomputed, because this checkout has no jj \
             repository to resolve it in"
        );
        return;
    }
    let owner = match introducing_change(root, &label) {
        Ok(owner) => owner,
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    match jq_value(manifest_path, ".revision.changeId") {
        Ok(recorded) if recorded != "null" && !recorded.is_empty() => {
            violations.extend(change_mismatch(&label, &recorded, &owner));
            verify_pinned_commit(root, &label, &recorded, manifest_path, violations);
        }
        Ok(_) => violations.push(format!("{label} records no change id")),
        Err(error) => violations.push(error),
    }
}

/// Requires `revision.id` to be the visible commit `revision.changeId` is at now.
///
/// A commit id is not a name. Every jj rewrite abandons the commit and keeps the
/// change, so a manifest pinned by commit id dangles the moment its own change is
/// revised. The dangling is invisible here and only here: the abandoned commit
/// survives in this working copy's operation log, so `jj file show -r` still reads
/// it, while a clone carries no `refs/jj/*` and holds no such object at all. All four
/// milestone manifests were in that state, and every input digest in all four was
/// unreadable in a fresh clone as a result.
///
/// So the pinned commit has to be one visible history reaches, and it has to be the
/// commit the pinned change is at. A rewrite that moves the change then fails the
/// gate and forces a re-pin with the inputs re-hashed there, which is the only way
/// a recorded commit id stays a true statement about anything.
fn verify_pinned_commit(
    root: &Path,
    label: &str,
    change: &str,
    manifest_path: &Path,
    violations: &mut Vec<String>,
) {
    let recorded = match jq_value(manifest_path, ".revision.id") {
        Ok(value) if value != "null" && !value.is_empty() => value,
        Ok(_) => {
            violations.push(format!(
                "{label} records change {change} but no commit it pins"
            ));
            return;
        }
        Err(error) => {
            violations.push(error);
            return;
        }
    };
    let current = match visible_commit(root, change) {
        Ok(current) => current,
        Err(error) => {
            violations.push(format!("{label} records change {change}, which {error}"));
            return;
        }
    };
    if let Err(error) = visible_commit(root, &recorded) {
        violations.push(format!(
            "{label} pins revision.id {recorded}, which {error}; re-pin it to {current}, the \
             commit change {change} is at now, and re-hash every input there"
        ));
        return;
    }
    violations.extend(pinned_commit_mismatch(label, change, &recorded, &current));
}

/// Compares a pinned commit against the one its change is at.
fn pinned_commit_mismatch(
    label: &str,
    change: &str,
    recorded: &str,
    current: &str,
) -> Option<String> {
    if recorded == current {
        return None;
    }
    Some(format!(
        "{label} pins revision.id {recorded} but change {change} is at {current}; a commit id does \
         not survive a rewrite, so re-pin it and re-hash every input there"
    ))
}

/// Resolves a revision to the one commit visible history reaches through it.
///
/// Resolving is not the check. `jj file show -r` reads an abandoned commit out of
/// this machine's operation log, and `jj log -r <abandoned>` prints it, so neither
/// distinguishes a commit a clone would carry from one only this working copy can
/// see. `::visible_heads()` is that distinction: it is what publishing the stack
/// would put in a clone, and an abandoned commit is outside it.
///
/// The root commit is subtracted, because it is the one commit every repository
/// has and it holds nothing: a pin there names no tree anything could have been
/// measured on. That is not a hypothetical shape. jj's root commit *is* the
/// all-zero commit id and the all-`z` change id, so a record whose
/// `trial.revision` was overwritten with forty zeroes and thirty-two `z`s pinned
/// the root, agreed with itself, and satisfied both tests — which is exactly the
/// substituted value this resolution exists to refuse.
fn visible_commit(root: &Path, revision: &str) -> Result<String, String> {
    let output = Command::new("jj")
        .args([
            "log",
            "-r",
            &format!("(::visible_heads() & ({revision})) ~ root()"),
            "--no-graph",
            "-T",
            r#"commit_id ++ "\n""#,
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot be resolved: {error}"))?;
    let resolved = String::from_utf8_lossy(&output.stdout);
    let mut commits = resolved.lines().filter(|line| !line.trim().is_empty());
    let Some(commit) = commits.next() else {
        return Err("names no commit in visible history".to_owned());
    };
    if commits.next().is_some() {
        return Err("names more than one visible commit".to_owned());
    }
    Ok(commit.to_owned())
}

/// Compares a recorded change against the one that introduced the file.
///
/// An untracked result has no introducing change yet, which is not a violation:
/// the schema still constrains the field, and the next run has a change to check.
/// It is announced rather than passed over, for the reason the `.jj` skips are:
/// a path jj cannot attribute clears this control, and a cleared control that
/// says nothing is indistinguishable from one that agreed.
fn change_mismatch(label: &str, recorded: &str, owner: &str) -> Option<String> {
    if owner.is_empty() {
        println!(
            "- {label}: the recorded change {recorded} is not recomputed, because jj attributes \
             this path to no change yet"
        );
        return None;
    }
    if owner == recorded {
        return None;
    }
    Some(format!(
        "{label} records change {recorded} but was introduced by {owner}"
    ))
}

fn verify_spike_result(root: &Path, path: &Path, gate: &str, violations: &mut Vec<String>) {
    let label = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();

    match spike_schema_matches(root, path) {
        Ok(true) => {}
        Ok(false) => violations.push(format!("{label} does not conform to the spike schema")),
        Err(error) => violations.push(error),
    }

    let rules = [
        (
            ".schemaVersion == 1 and .protocolVersion == \"spike-v1\"",
            "must record spike protocol version 1",
        ),
        (
            &format!(".gate == \"{gate}\""),
            "must record the gate of the directory it belongs to",
        ),
        (
            r#".candidate.disposition == "rejected"
                or ([.failures[].scenario] | sort
                    == ["callback-failure", "crash", "invalid-input", "timeout",
                        "unavailable-runtime"])"#,
            "must exercise every required failure scenario exactly once",
        ),
        (
            // Recording the scenarios says nothing about what happened in them.
            // A candidate that never once failed closed never demonstrated the
            // property this gate exists to check.
            r#".candidate.disposition == "rejected"
                or ([.failures[] | select(.outcome == "failed-closed")] | length > 0)"#,
            "a measured candidate must record at least one failure that closed",
        ),
        (
            r#"[.memory.bootstrapPeakRss, .memory.retainedRss, .memory.retainedClasses,
                .startup.processToReady]
                | all((.status == "unavailable") or (.value > 0))"#,
            "must not report a measured bootstrap or startup value of zero",
        ),
        (
            r#".candidate.disposition == "rejected"
                or .calls.callbackNanoseconds.status == "measured"
                or (.calls.callbackNanoseconds.reason | length > 0)"#,
            "must measure callback cost or record why it is unavailable",
        ),
        (
            r#".deviations | all(.classification != "blocking")"#,
            "must not carry a blocking deviation",
        ),
        (
            r#".trial.measuredInvocations > 0 or .candidate.disposition == "rejected""#,
            "must measure at least one invocation",
        ),
    ];
    for (filter, requirement) in rules {
        match jq_matches(path, filter) {
            Ok(true) => {}
            Ok(false) => violations.push(format!("{label} {requirement}")),
            Err(error) => violations.push(error),
        }
    }

    verify_redaction(path, &label, violations);
}

fn spike_schema_matches(root: &Path, result: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/spike-result.schema.json"))
        .arg(result)
        .output()
        .map_err(|error| format!("cannot validate spike schema: {error}"))?;
    Ok(output.status.success())
}

/// Keeps measurement-only spike crates out of the production dependency graph.
fn verify_experiment_fence(root: &Path, violations: &mut Vec<String>) {
    let production_manifests = CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/Cargo.toml")),
        )
        .chain([
            "tests/conformance/Cargo.toml".to_owned(),
            "tools/ravel-cli/Cargo.toml".to_owned(),
            "tools/ravel-xtask/Cargo.toml".to_owned(),
        ]);

    for relative in production_manifests {
        let Ok(manifest) = fs::read_to_string(root.join(&relative)) else {
            continue;
        };
        if manifest.contains("ravel-spike") || manifest.contains("experiments/") {
            violations.push(format!(
                "production crate depends on an experiment: {relative}"
            ));
        }
        if manifest.contains(r#"unsafe_code = "allow""#) {
            violations.push(format!(
                "unsafe code is permitted outside the experiment boundary: {relative}"
            ));
        }
    }

    verify_testkit_fence(root, violations);

    let mut sources = Vec::new();
    for directory in ["crates", "test-mods", "tools", "tests"] {
        collect_files(&root.join(directory), &mut sources);
    }
    for path in sources {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        // A bare `contains` would match this rule's own literals, so anchor on
        // the attribute position instead of the substring.
        if fs::read_to_string(&path).is_ok_and(|source| {
            source.lines().map(str::trim_start).any(|line| {
                line.starts_with("#![allow(unsafe_code)")
                    || line.starts_with("#[allow(unsafe_code)")
                    || line.starts_with("#![expect(unsafe_code)")
                    || line.starts_with("#[expect(unsafe_code)")
            })
        }) {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            violations.push(format!(
                "unsafe code is permitted outside the experiment boundary: {}",
                relative.display()
            ));
        }
    }
}

/// Keeps the test harness crate out of the production libraries.
///
/// `dependency-rules.md` says testkit may depend on the loader and the public
/// contracts to build process harnesses, but that production libraries never
/// depend on testkit. That was a sentence and nothing else: adding
/// `ravel-testkit.workspace = true` to `crates/ravel-loader/Cargo.toml` left the
/// scaffold at zero. No live violation existed, which is why it went unnoticed.
///
/// The rule is held against the shipped libraries, the test mods, and the
/// examples, which are what a player's or an author's build graph contains.
/// `tools/ravel-cli` and `tests/conformance` are harnesses that legitimately
/// drive testkit, and testkit is not held against itself.
fn verify_testkit_fence(root: &Path, violations: &mut Vec<String>) {
    let fenced = CRATE_NAMES
        .iter()
        .filter(|name| **name != "ravel-testkit")
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/Cargo.toml")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/Cargo.toml")),
        );

    for relative in fenced {
        let Ok(manifest) = fs::read_to_string(root.join(&relative)) else {
            continue;
        };
        if manifest.contains("ravel-testkit") {
            violations.push(format!(
                "production crate depends on the test harness: {relative}"
            ));
        }
    }
}

/// Checks a milestone evidence manifest's identity and minimum gate fields.
#[must_use]
pub fn verify_milestone(root: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    let relative = format!("evidence/{}/manifest.json", milestone.to_ascii_lowercase());
    let manifest_path = root.join(&relative);
    if !manifest_path.is_file() {
        // A milestone with no manifest is not closed. Say that, rather than
        // reporting a filesystem error against an absolute path.
        return vec![format!(
            "{milestone} is not closed: {relative} does not exist"
        )];
    }

    let mut violations = verify_evidence_manifest(root, &manifest_path, milestone);
    verify_admitted_manifest(&manifest_path, milestone, &mut violations);
    violations
}

/// Checks one evidence manifest against the schema and milestone closure rules.
#[must_use]
pub fn verify_evidence_manifest(root: &Path, manifest_path: &Path, milestone: &str) -> Vec<String> {
    if !is_valid_milestone_id(milestone) {
        return vec![format!("invalid milestone ID: {milestone}")];
    }

    match jq_matches(manifest_path, "type == \"object\"") {
        Ok(true) => {}
        Ok(false) => return vec!["milestone manifest must be a JSON object".to_owned()],
        Err(error) => return vec![error],
    }

    let mut violations = Vec::new();
    match evidence_schema_matches(root, manifest_path) {
        Ok(true) => {}
        Ok(false) => {
            violations.push("milestone manifest does not conform to evidence schema".to_owned());
        }
        Err(error) => violations.push(error),
    }
    verify_redaction(manifest_path, "milestone manifest", &mut violations);
    if !jq_matches(manifest_path, ".schemaVersion == 1").unwrap_or(false) {
        violations.push("manifest schemaVersion must equal 1".to_owned());
    }
    let milestone_filter = format!(".milestone == \"{milestone}\"");
    if !jq_matches(manifest_path, &milestone_filter).unwrap_or(false) {
        violations.push(format!("manifest milestone must equal {milestone}"));
    }

    verify_general_closure(manifest_path, milestone, &mut violations);
    // The review-record rule runs here as well as in the every-manifest sweep,
    // because this is the gate a milestone is closed by, and both halves of that
    // rule have already failed open in exactly this way. The verdict half let
    // `verify milestone M1` exit 0 on a manifest claiming pass over a record
    // reading `Result: FAIL`; the digest half let `verify milestone M4` exit 0
    // after `evidence/m4/review.md` was replaced wholesale with five lines saying
    // `Result: PASS` and that no round ever ran, because the bytes the digest
    // attested to were only ever recomputed by `ravel:verify`. A gate that closes
    // a milestone on a review record has to bind the record it closes on.
    match tracked_paths(root) {
        Ok(paths) => verify_recorded_review(
            root,
            manifest_path,
            &paths.into_iter().collect(),
            &mut violations,
        ),
        Err(error) => violations.push(error),
    }
    verify_manifest_change(root, manifest_path, &mut violations);
    verify_finding_anchors(root, manifest_path, milestone, &mut violations);
    if milestone == "M0" {
        verify_m0_manifest(manifest_path, &mut violations);
    }
    if milestone == "M1" {
        verify_m1_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M2" {
        verify_m2_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M3" {
        verify_m3_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M4" {
        verify_m4_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M5" {
        verify_m5_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M6" {
        verify_m6_manifest(root, manifest_path, &mut violations);
    }
    if milestone == "M7" {
        verify_m7_manifest(root, manifest_path, &mut violations);
    }

    violations
}

/// The closure rules the repository decides, whatever the review concluded.
///
/// These run on every `ravel:verify` through `verify_manifest_invariants` as well
/// as in the per-milestone gate, so that the gate every agent is told to run is
/// the gate that checks them. They are one table rather than two copies for the
/// obvious reason: a second statement of the same rule is a second thing to drift.
///
/// The review-record rule is here rather than with the verdicts because what it
/// asks is not a verdict. Whether a review passed is the reviewer's; whether a
/// recorded verdict names a document anyone can read is the repository's, and it
/// sat with the verdicts, so `del(.review.record)` left `verify scaffold` at zero.
/// M0 and M2 record `pass`, so for those two the readable review could be deleted
/// and the gate every agent is told to run stayed green. It is conditioned on a
/// verdict being recorded, because a milestone under construction has no review yet.
const MANIFEST_INVARIANTS: &[(&str, &str)] = &[
    (
        ".revision.dirty == false",
        "milestone revision must be clean",
    ),
    (
        ".review.result == null or (.review | has(\"record\"))",
        "milestone recording a review verdict must record a hashed review record for it",
    ),
    (
        ".inputs | length > 0",
        "milestone must include at least one hashed input",
    ),
    (
        "([.commands[].exitStatus] + [.platformEvidence[].commands[].exitStatus]) | length > 0 and all(. == 0)",
        "milestone commands must all succeed",
    ),
    (
        ".tests.total == (.tests.passed + .tests.failed + .tests.skipped)",
        "test counts must equal passed + failed + skipped",
    ),
    (
        ".tests.total > 0 and .tests.passed > 0",
        "milestone must record at least one passing test",
    ),
    (
        ".tests.failed == 0",
        "milestone tests must have zero failures",
    ),
    (
        ".tests.skipped == 0",
        "milestone tests must have zero skipped tests",
    ),
    (
        ".tests.lineCoveragePercent != null and .tests.lineCoveragePercent >= 80",
        "milestone line coverage must be at least 80%",
    ),
];

/// The closure rules only the independent review's verdict can decide.
///
/// These stay in `ravel:verify-milestone:*`. M1, M2 and M3 record
/// `review.result: "fail"`, which is a true statement about three open milestones
/// rather than a broken repository, so `ravel:verify` cannot ask them and keep
/// exiting zero — and greening them to make it possible would be the failure this
/// whole gate exists to prevent.
///
/// The blocking-finding rule is paired with a cardinality floor in each
/// milestone's own closure, because `all(. != "blocking")` is true of an empty
/// array: with `deviations` and `risks` emptied, this rule passed and the milestone
/// closed on findings that had simply been deleted. Where the floor sits is
/// per milestone, because M0 recorded none and its closure rests on its
/// platform evidence and baseline digests instead.
const MANIFEST_VERDICTS: &[(&str, &str)] = &[
    (
        "([.deviations[].classification] + [.risks[].classification]) | all(. != \"blocking\")",
        "milestone must have no blocking deviations or risks",
    ),
    (
        ".review.independent == true and .review.result == \"pass\"",
        "milestone review must be independent and pass",
    ),
];

fn verify_general_closure(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    let work_package_prefix = format!("{milestone}-W");
    require_manifest_rule(
        manifest_path,
        &format!(".workPackages | length > 0 and all(startswith(\"{work_package_prefix}\"))"),
        "work-package IDs must belong to the manifest milestone",
        violations,
    );
    for (filter, violation) in MANIFEST_INVARIANTS.iter().chain(MANIFEST_VERDICTS) {
        require_manifest_rule(manifest_path, filter, violation, violations);
    }
}

fn verify_admitted_manifest(manifest_path: &Path, milestone: &str, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        r#".evidenceKind == "native""#,
        "milestone closure requires native evidence, not a synthetic contract fixture",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"[.. | strings | ascii_downcase | contains("synthetic")] | any | not"#,
        "native milestone evidence must not contain synthetic placeholders",
        violations,
    );
    if milestone == "M0" {
        require_manifest_rule(
            manifest_path,
            r#".attestations
                | any(
                    .platform == "linux"
                    and .architecture == "x86_64"
                    and (.workloads | sort == ["official-atm11", "vanilla"])
                )"#,
            "M0 closure requires a workload-bound native Linux attestation",
            violations,
        );
    }
}

/// Checks what closing M1 requires beyond the rules every milestone shares.
///
/// M1 decides the architecture, so its closure is about decisions being made on
/// recorded evidence: every gate measured, every decision written down, and the
/// one gate that needs an operator-run launch actually having one.
fn verify_m1_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M1-W1\", \"M1-W2\", \"M1-W3\", \"M1-W4\", \"M1-W5\", \"M1-W6\", \"M1-W7\", \"M1-W8\"]",
        "M1 manifest must include every work package from M1-W1 through M1-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".benchmarks.protocolVersion == \"spike-v1\" and (.benchmarks.results | length > 0)",
        "M1 manifest must record spike results under the spike-v1 protocol",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M1 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    // Every gate needs a recorded result and an accepted decision. A gate with
    // no ADR is a measurement nobody committed to.
    for (directory, gate) in SPIKE_GATES {
        let results = root.join("evidence/m1/spikes").join(directory);
        let recorded = fs::read_dir(&results)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count();
        if recorded == 0 {
            violations.push(format!("M1 closure requires a recorded result for {gate}"));
        }
    }
    for decision in M1_DECISIONS {
        if !root.join("docs/decisions").join(decision).is_file() {
            violations.push(format!(
                "M1 closure requires the decision record {decision}"
            ));
        }
    }

    // M1-W4's Verification clause asks that a generated 26.1.2 binding be
    // reproducible from pinned inputs and fail on drift. Only the drift half was
    // recorded, and nothing asked for the other, so downgrading the one blocking
    // deviation and flipping `review.result` closed M1 with the clause still unmet.
    // What is asked for is the pair of digests rather than the boolean beside
    // them. A boolean can be written by hand into a record that repeated nothing,
    // and it was: adding `repeatedRunIsIdentical` to the M1-era classfile record,
    // which measured one generation, left this rule passing. Two recorded digests
    // that have to be equal are refutable instead — an operator holding the pinned
    // jar re-derives them — so the digests are the check and the boolean is
    // required with them only so that a record cannot state a disagreement its
    // own digests do not show.
    let repeated = fs::read_dir(root.join("evidence/m1/spikes/adg-3"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .any(|path| {
            jq_matches(
                &path,
                r#"(.bindingGeneration.outputSha256 // "") != ""
                   and .bindingGeneration.repeatedOutputSha256 == .bindingGeneration.outputSha256
                   and .bindingGeneration.repeatedRunIsIdentical == true"#,
            )
            .unwrap_or(false)
        });
    if !repeated {
        violations.push(
            "M1 closure requires an ADG-3 result recording bindingGeneration outputSha256 and \
             repeatedOutputSha256 as the same digest, which is the reproducibility half of \
             M1-W4's Verification clause"
                .to_owned(),
        );
    }

    // The ADG-5 launch is operator-run, so no record recomputes it and the
    // attestation is the only thing standing behind it. M0 and M3 require one for
    // the same reason; M1 recorded an empty list while resting on the same kind of
    // launch, which left the strongest unrecomputable claim in the milestone with
    // nobody's name on it.
    require_manifest_rule(
        manifest_path,
        r#".attestations
            | any(.platform == "linux" and .architecture == "x86_64" and (.workloads | length > 0))"#,
        "M1 closure requires a Linux attestation for the operator-run ADG-5 launch, which no \
         tracked record can recompute",
        violations,
    );

    // ADG-5 is the one gate whose answer needs the game to actually start.
    let launched = fs::read_dir(root.join("evidence/m1/spikes/adg-5"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .any(|path| jq_matches(&path, ".prismIntegration.stockLaunch == true").unwrap_or(false));
    if !launched {
        violations.push(
            "M1 closure requires an ADG-5 result recording an observed stock-Prism launch"
                .to_owned(),
        );
    }
}

/// Checks what closing M2 requires beyond the rules every milestone shares.
///
/// M2 builds the pre-world loader, so its closure is about the pipeline having
/// been run rather than described. The manifest names the work; the records
/// under `evidence/m2/` are what happened, and each one is asked here for the
/// claim `ROADMAP.md`'s M2 rows make of it. A manifest that says the loader
/// works and a record that does not show it working is the failure this
/// catches.
fn verify_m2_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M2-W1\", \"M2-W2\", \"M2-W3\", \"M2-W4\", \"M2-W5\", \"M2-W6\", \"M2-W7\", \"M2-W8\"]",
        "M2 manifest must include every work package from M2-W1 through M2-W8 exactly once",
        violations,
    );
    // The same rule M3 carries, for the reason M3 carries it: a record no manifest
    // names is a record no digest covers. `M2_RECORDS` below asks each file to
    // exist and to say the right thing, and neither of those is a binding — with
    // `prism-launch.json` removed from `benchmarks.results`, its
    // `nativeInvocations` and `classesRewritten` were rewritten to invented
    // numbers and `verify scaffold` exited zero, because the digest that would
    // have caught it was no longer recorded.
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m2-loader-budget-v1"
            and ([.benchmarks.results[].name] | sort
                == ["loader-overhead.json", "noop-pipeline.json",
                    "pre-world-gate.json", "prism-launch.json"])"#,
        "M2 manifest must record all four M2 results under the m2-loader-budget-v1 protocol, so \
         that each one is covered by a recomputed digest",
        violations,
    );
    // M2-W5's launch is operator-run, so a person says it happened. The record
    // it points at is checked below; this is the claim that someone stands
    // behind it.
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(.platform == "linux" and .architecture == "x86_64")"#,
        "M2 closure requires a native Linux attestation for the operator-run launch",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M2 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    // The six limits below are validated identically, and three of them the
    // record's own expiry clause voided. Said here rather than left to the
    // roadmap, because a reader who runs this gate and not `ROADMAP.md` otherwise
    // reads a green on three limits that describe a shape which no longer exists.
    // `verify_void_limits` is what keeps this list and the roadmap's the same one.
    for (metric, why) in VOID_LIMITS {
        println!(
            "- evidence/m2/manifest.json: the budget limit {metric} is checked for shape and not \
             for meaning, because it is {why}"
        );
    }

    for (file, rule, requirement) in M2_RECORDS {
        let path = root.join("evidence/m2").join(file);
        if !path.is_file() {
            violations.push(format!("M2 closure requires the record evidence/m2/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M2 closure requires {requirement}"));
        }
        // The records carry what a launcher log and a machine produced, so they
        // are held to the same redaction rule as the manifest rather than to
        // the reader's attention. The two generated records have a conformance
        // test of their own; the overhead and launch records have only this.
        verify_redaction(&path, &format!("evidence/m2/{file}"), violations);
    }
}

/// The records M2 cannot close without, and what each one has to say.
const M2_RECORDS: &[(&str, &str, &str)] = &[
    (
        "noop-pipeline.json",
        // `all(.repeatedRunIsIdentical)` would be a truthiness test, and every
        // non-null value but `false` is truthy in jq, so the string "no" would
        // pass it. The comparison is what makes the field mean what it says.
        ".runs | length > 0 and all(.repeatedRunIsIdentical == true)",
        "a no-op pipeline record whose run is identical when repeated",
    ),
    (
        "pre-world-gate.json",
        ".gate.refusesWorldAccess == true and .gate.ranModCode == false",
        "a pre-world gate record showing invalid packages refused before any mod code ran",
    ),
    (
        "loader-overhead.json",
        // A budget is only a budget if each line says which metric it bounds, in
        // what unit, and sits above the measurement it was derived from. A bare
        // positive number bounds nothing, and two lines naming the same metric
        // are a limit and a contradiction rather than two limits.
        r#".budget.limits
            | length > 0
            and all(
                (.metric | type == "string" and length > 0)
                and (.unit | type == "string" and length > 0)
                and .operator == "less-than-or-equal"
                and (.measured | type == "number")
                and .measured > 0
                and (.limit | type == "number")
                and .limit > .measured
            )
            and (map(.metric) | unique | length) == length"#,
        "a loader overhead record whose budget lines each name one metric and a unit, and set an \
         upper limit above the positive measurement that limit was derived from",
    ),
    (
        "prism-launch.json",
        // The rewrite count and the native invocation count are what M2's launch
        // added over M1's, which could show the client started and nothing more.
        // Each count is type-checked before it is compared: jq orders strings
        // above numbers, so an unguarded `> 0` accepts "not measured". The
        // objection has to be present and null, because a missing key reads as
        // null too and would let a record drop its own objection to pass.
        // `stockLauncher` is the operator's word rather than a measurement, so
        // the record labels it attested and the gate asks for it under that name.
        r#".launch.provesStockLaunch == true
            and (.launch | has("objection"))
            and .launch.objection == null
            and .launch.attestation.stockLauncher == true
            and .ravel.reachedGameProcess == true
            and .ravel.rewrite.reported == true
            and .ravel.rewrite.reachedRavel == true
            and (.ravel.rewrite.classesRewritten | type == "number")
            and .ravel.rewrite.classesRewritten > 0
            and (.ravel.rewrite.methodsRewritten | type == "number")
            and .ravel.rewrite.methodsRewritten > 0
            and (.ravel.rewrite.nativeInvocations | type == "number")
            and .ravel.rewrite.nativeInvocations > 0
            and .instance.withoutAnotherLoader == true"#,
        "a launch record showing the official client reaching Ravel from an attested stock Prism, \
         with the transformer's own count of what it rewrote inside that process and of the hook \
         invocations that reached native code, and no other loader present",
    ),
];

/// Checks what closing M3 requires beyond the rules every milestone shares.
///
/// M3 had none, which is how a mutation closed it: with `review.result` set to
/// `"pass"` and the one blocking risk downgraded, `verify milestone M3` exited zero
/// with every `gateStatus` row set to `"failed"`, eight of the nine work packages
/// deleted, and `benchmarks.results` emptied — so all four M3 records were bound by
/// nothing and the scaffold still passed. The shared rules cannot catch that,
/// because every one of them was still true of what was left.
///
/// So this asks M3's own gate rows for what `ROADMAP.md` says each one produces.
/// The manifest names the work and its own per-package status; the records under
/// `evidence/m3/` are what happened. `benchmarks.results` is required to name all
/// four of them exactly, which is what puts them back under the digest binding
/// `verify_recorded_digests` already performs — a record no manifest names is a
/// record no digest covers.
fn verify_m3_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M3-W0\", \"M3-W1\", \"M3-W2\", \"M3-W3\", \"M3-W4\", \"M3-W5\", \"M3-W6\", \"M3-W7\", \"M3-W8\"]",
        "M3 manifest must include every work package from M3-W0 through M3-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m3-prototype-v1"
            and ([.benchmarks.results[].name] | sort
                == ["class-loader-identity.json", "linux-process-matrix.json",
                    "pack-compile.json", "profiling-overhead.json"])"#,
        "M3 manifest must record all four M3 results under the m3-prototype-v1 protocol, so that \
         each one is covered by a recomputed digest",
        violations,
    );
    // Every work package carries its own status and the evidence it rests on, and
    // M3-W8 is the review itself, whose status is the verdict the milestone gate
    // reads separately. Emptying the table, or reporting a row as anything but
    // closed, is what this refuses.
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M3-W8") | .value]
                 | length == 8
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M3 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    // M3-W7's launch is operator-run, so a person says it happened, and the
    // workload is Ravel itself rather than one of M0's baselines.
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("ravel") != null)
        )"#,
        "M3 closure requires a native Linux attestation for the operator-run Ravel launch",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M3 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    for decision in M3_DECISIONS {
        if !root.join("docs/decisions").join(decision).is_file() {
            violations.push(format!(
                "M3 closure requires the decision record {decision}"
            ));
        }
    }

    for (file, rule, requirement) in M3_RECORDS {
        let path = root.join("evidence/m3").join(file);
        if !path.is_file() {
            violations.push(format!("M3 closure requires the record evidence/m3/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M3 closure requires {requirement}"));
        }
        verify_redaction(&path, &format!("evidence/m3/{file}"), violations);
    }
}

/// The records M3 cannot close without, and what each one has to say.
///
/// Each rule is the `ROADMAP.md` M3 row it answers, read as a question the file
/// has to satisfy rather than as a file that has to exist. Every count is
/// type-checked before it is compared, for the reason M2's launch rule gives: jq
/// orders strings above numbers, so an unguarded `> 0` accepts "not measured".
const M3_RECORDS: &[(&str, &str, &str)] = &[
    (
        // ADG-1's second candidate rests on the loader that resolves the game's
        // classes being the system loader. A launch that rewrote nothing would
        // describe no rewrite, so the counts are what rule that out.
        "class-loader-identity.json",
        r#".milestone == "M3"
            and .observed.sameObject == true
            and .conclusion.candidateTwoIsAvailable == true
            and (.rewrite.classesRewritten | type == "number")
            and .rewrite.classesRewritten > 0
            and (.rewrite.methodsRewritten | type == "number")
            and .rewrite.methodsRewritten > 0
            and .rewrite.reachedNativeCode == true
            and (.rewrite.nativeInvocations | type == "number")
            and .rewrite.nativeInvocations > 0"#,
        "a class-loader identity record showing the loader that resolved a rewritten game class \
         is the system loader, measured in a launch that rewrote classes and reached native code",
    ),
    (
        // ADG-2 chose source compiled on the player's machine, and this is the
        // record that prices it. A cost of zero would mean nothing was compiled.
        "pack-compile.json",
        r#".milestone == "M3"
            and .gate == "ADG-2"
            and .protocol == "m3-pack-compile-v1"
            and (.compile.repeats | type == "number")
            and .compile.repeats > 0
            and (.compile.baselineMilliseconds | type == "number")
            and .compile.baselineMilliseconds > 0
            and (.compile.marginalColdMillisecondsPerMod | type == "number")
            and .compile.marginalColdMillisecondsPerMod > 0
            and (.disk.toolchainBytes | type == "number")
            and .disk.toolchainBytes > 0"#,
        "a pack-compile record pricing ADG-2's source distribution with a repeated measurement, \
         a positive marginal per-mod cost, and the toolchain size the decision accepted",
    ),
    (
        // M3-W6: profiling off has to come in under the limit that was fixed
        // before it was measured, and the unsupported metrics have to be named
        // rather than left implied.
        "profiling-overhead.json",
        r#".milestone == "M3"
            and .workPackage == "M3-W6"
            and .budget.protocol == "m2-loader-budget-v1"
            and .budget.hardwareProfile == "linux-reference-v1"
            and (.budget.limits | any(
                .metric == "profiling.off.perTick"
                and .operator == "less-than-or-equal"
                and (.measured | type == "number")
                and (.limit | type == "number")
                and .measured > 0
                and .measured <= .limit))
            and (.collectors.attributionAccuracy | type == "array" and length > 0)
            and (.collectors.unsupported | type == "array")"#,
        "a profiling record whose off-tick measurement comes in under the limit M3-W6 fixed, with \
         per-mod attribution accuracy stated and the unsupported metrics named",
    ),
    (
        // M3-W7: the whole surface, natively, in one operator-run launch, with the
        // instance scan proving no other loader read the mods.
        "linux-process-matrix.json",
        r#".milestone == "M3"
            and .workPackage == "M3-W7"
            and .result == "pass"
            and .instance.withoutAnotherLoader == true
            and .instance.stockComponentsExact == true
            and .instance.otherLoaderComponents == 0
            and .instance.modJars == 0
            and .instance.recognizedLoaderModJars == 0
            and (.rewrite.classesRewritten | type == "number")
            and .rewrite.classesRewritten > 0
            and .rewrite.transformationFailures == 0
            and (.rewrite.nativeInvocations | type == "number")
            and .rewrite.nativeInvocations > 0
            and .screens.titleMenuButtonPresent == true
            and .screens.allThreeOpenedInThisCampaign == true
            and .screens.settings.persistenceRoundTrip == true
            and .commandAndSavedData.vanillaCommandExecutedThroughInput == true
            and .commandAndSavedData.relaunchVerifiedExactValue == true
            and .worldPersistenceRoundTrip.createSaveExitReload
                .aggregateByteIdenticalAcrossTheRoundTrip == true
            and .worldPersistenceRoundTrip.failClosedOnAnUnreadableWorld.worldRefused == true
            and .worldPersistenceRoundTrip.failClosedOnAnUnreadableWorld
                .integratedServerNeverStarted == true"#,
        "a Linux process-matrix record showing every M3 surface passing natively in an instance \
         the scan proves holds no other loader, with the world round trip preserving bytes and \
         refusing a world it cannot read",
    ),
];

/// Decision records M3 cannot close without.
const M3_DECISIONS: &[&str] = &["adg-6-configuration-ui.md", "adg-7-world-persistence.md"];

/// Checks what closing M4 requires beyond the rules every milestone shares.
///
/// M4 is the SDK breadth milestone, so its closure is about a surface a mod
/// author can reach and a boundary that author is told about. Three things are
/// bound rather than described, because M4's evidence is in-gate throughout and a
/// record of in-gate work is the easiest kind to let drift: the inventory the
/// surface record claims is reconciled against the directories it names, the gate
/// figures the matrix record reports are reconciled against the manifest's own
/// `tests` block, and the matrix has to say in a field rather than in prose that
/// no game process was launched for it. That last one is the difference between
/// this milestone's matrix and M3's, and it is the difference a reader is most
/// likely to miss, so it is a field a gate reads rather than a sentence.
fn verify_m4_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M4-W1\", \"M4-W2\", \"M4-W3\", \"M4-W4\", \"M4-W5\", \"M4-W6\", \"M4-W7\", \"M4-W8\"]",
        "M4 manifest must include every work package from M4-W1 through M4-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m4-surface-v1"
            and ([.benchmarks.results[].name] | sort
                == ["linux-process-matrix.json", "sdk-surface.json"])"#,
        "M4 manifest must record both M4 results under the m4-surface-v1 protocol, so that each \
         one is covered by a recomputed digest",
        violations,
    );
    // M4-W8 is the review itself, whose status is the verdict the milestone gate
    // reads separately. The other seven are implementation work packages and each
    // one carries its own status and the evidence it rests on.
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M4-W8") | .value]
                 | length == 7
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M4 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    // The gate run is native Linux and a person says which machine ran it, the
    // same way M3's launch is attested. What the attestation may not do is stand
    // in for a launch, which is why the matrix's `gameLaunch.occurred` is checked
    // below rather than left to this statement's prose.
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("ravel") != null)
        )"#,
        "M4 closure requires a native Linux attestation for the recorded gate run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M4 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );

    for decision in M4_DECISIONS {
        if !root.join("docs/decisions").join(decision).is_file() {
            violations.push(format!(
                "M4 closure requires the decision record {decision}"
            ));
        }
    }

    for (file, rule, requirement) in M4_RECORDS {
        let path = root.join("evidence/m4").join(file);
        if !path.is_file() {
            violations.push(format!("M4 closure requires the record evidence/m4/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M4 closure requires {requirement}"));
        }
        verify_redaction(&path, &format!("evidence/m4/{file}"), violations);
    }

    verify_m4_surface(root, violations);
    verify_m4_process_matrix(root, violations);
    verify_m4_gate_figures(root, manifest_path, violations);
}

/// The records M4 cannot close without, and what each one has to say.
///
/// Each rule is the `ROADMAP.md` M4-W8 row read as a question the file has to
/// answer rather than as a file that has to exist. Every count is type-checked
/// before it is compared, for the reason M2's launch rule gives: jq orders strings
/// above numbers, so an unguarded `> 0` accepts "not measured".
const M4_RECORDS: &[(&str, &str, &str)] = &[
    (
        // The M4-W8 gate row asks for a global Linux gate and a process matrix.
        // Every row is a process that ran, so a row with no exit status is a row
        // that describes an intention, and `doesNotProve` is required on each
        // because the whole difference between this matrix and M3's is what a
        // gate process cannot reach.
        "linux-process-matrix.json",
        r#".milestone == "M4"
            and .workPackage == "M4-W8"
            and .protocol == "m4-gate-matrix-v1"
            and .result == "pass"
            and .platform.os == "linux"
            and .platform.architecture == "x86_64"
            and .gameLaunch.occurred == false
            and (.gameLaunch.whyItIsNotHere | type == "string" and length > 0)
            and (.gate.exitStatus | type == "number")
            and .gate.exitStatus == 0
            and (.gate.tests.run | type == "number")
            and .gate.tests.run > 0
            and (.processes | type == "array" and length > 0)
            and (.processes | all(
                (.surface | type == "string" and length > 0)
                and (.command | type == "string" and length > 0)
                and (.exitStatus | type == "number")
                and .exitStatus == 0
                and (.proves | type == "string" and length > 0)
                and (.doesNotProve | type == "string" and length > 0)))"#,
        "a Linux process matrix in which every recorded gate process exited zero and says what it \
         does not prove, and which states in a field rather than in prose that no game process \
         was launched for it",
    ),
    (
        // M4's deliverable is a surface, so the record that inventories it is
        // held to naming what each work package shipped and what it left unproven.
        // The lists themselves are held against the tree by `verify_m4_surface`,
        // which fails on a name the repository no longer holds and says nothing
        // about a surface added after M4 closed.
        "sdk-surface.json",
        r#".milestone == "M4"
            and .workPackage == "M4-W8"
            and .protocol == "m4-surface-v1"
            and (.inventory.crates | type == "array" and length > 0)
            and (.inventory.testMods | type == "array" and length > 0)
            and (.inventory.examples | type == "array" and length > 0)
            and (.inventory.guideDocuments | type == "array" and length > 0)
            and (.inventory.conformanceSuites | type == "array" and length > 0)
            and ([.workPackages[].id] | sort
                == ["M4-W1", "M4-W2", "M4-W3", "M4-W4", "M4-W5", "M4-W6", "M4-W7"])
            and (.workPackages | all(
                (.shipped | type == "string" and length > 0)
                and (.provenBy | type == "array" and length > 0)
                and (.notProven | type == "string" and length > 0)))"#,
        "an SDK surface record inventorying what each of M4-W1 through M4-W7 shipped, what proves \
         it, and what it does not prove",
    ),
];

/// Decision records M4 cannot close without.
///
/// ADG-2 is the distribution and execution decision every packaging surface in
/// M4 implements, and ADG-3 is the binding decision M4-W6's version isolation is
/// an argument about. M4 accepted no decision of its own.
const M4_DECISIONS: &[&str] = &["adg-2-mod-execution.md", "adg-3-bindings.md"];

/// Reconciles the recorded SDK surface against the repository it inventories.
///
/// A list in an evidence record is a count in prose wearing different brackets:
/// nothing recomputed it, so a guide page deleted after the record was written
/// left it naming a document the tree no longer holds. Each list is compared by
/// name rather than by length, so a rename fails as loudly as a deletion instead
/// of being absorbed by whatever took the old name's place.
///
/// The comparison runs one way only: every name the record holds has to exist,
/// and a surface the repository holds that the record does not name is not a
/// violation. M4 is closed, and a closed record measures the tree at closure
/// rather than asserting anything about later trees. Holding it to the current
/// tree made every later milestone that adds a crate, test mod, example, guide
/// page or conformance suite edit M4's closed evidence to stay green — a silent
/// amendment with no trail, which is worse than the drift it was catching.
/// Accounting for a newly added surface belongs to the milestone that adds it.
///
/// What that withdraws: for crates, test mods and examples, nothing, because
/// `CRATE_NAMES`, `TEST_MOD_NAMES` and `EXAMPLE_NAMES` are reconciled against
/// their own directories by [`verify_inventory`] and this check only ever
/// inherited that. For `guideDocuments` and `conformanceSuites` there is no such
/// constant, so a guide page or conformance suite added after M4 closed is now
/// checked by nothing here. That coverage is deliberately given up rather than
/// replaced: the milestone that adds the file is the one that owes evidence for
/// it.
fn verify_m4_surface(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m4/sdk-surface.json");
    if !record.is_file() {
        // Its absence is already a violation where the record set is checked.
        return;
    }
    let present = [
        ("crates", listed_names(CRATE_NAMES)),
        ("testMods", listed_names(TEST_MOD_NAMES)),
        ("examples", listed_names(EXAMPLE_NAMES)),
        ("guideDocuments", files_named(root, "docs/guide", "md")),
        (
            "conformanceSuites",
            files_named(root, "tests/conformance/tests", "rs"),
        ),
    ];
    for (field, present) in present {
        let recorded = match jq_value(&record, &format!(".inventory.{field}[]?")) {
            Ok(value) => value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        for extra in recorded.difference(&present) {
            violations.push(format!(
                "evidence/m4/sdk-surface.json names {extra} under inventory.{field}, which the \
                 repository does not hold"
            ));
        }
    }
}

/// The names one maintained list holds.
fn listed_names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// The file names one directory holds with one extension.
fn files_named(root: &Path, directory: &str, extension: &str) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(root.join(directory)) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|found| found == extension)
        })
        .collect()
}

/// The file that declares what `ravel:verify` runs, and the name of that gate.
const GATE_DEFINITION: &str = "nix/common.nix";

/// The record that lists the processes that gate ran.
const M4_PROCESS_MATRIX: &str = "evidence/m4/linux-process-matrix.json";

/// The prefix every row's command carries, and the whole of what makes it a task.
const TASK_COMMAND_PREFIX: &str = "devenv tasks run ";

/// Reconciles the recorded gate processes against the gate that ran them.
///
/// The matrix says it holds every process `devenv tasks run ravel:verify` runs,
/// and until this existed nothing recomputed that: an invented row naming a task
/// that does not exist passed the gate, and so did a row recording a nonzero exit
/// under a record whose own `result` is `pass`. Both are the failure
/// [`verify_m4_surface`] closes for the inventory lists one document over — a list
/// nobody recomputes is a claim about the tree that the tree never answered — and
/// the process list is the one an entire milestone's gate evidence rests on.
///
/// So the list is compared against `nix/common.nix`'s own `after` list in both
/// directions. A recorded process the gate does not run is an error, because that
/// is a row whose exit status came from somewhere other than the run the record
/// heads. A gate sub-task with no recorded row is an error too, because the
/// omission is how a failing process leaves a record that still reads complete.
///
/// `checksOutsideTheGate` is the escape valve and it is held shut: a row there is
/// required to name a task the gate does *not* run, so moving an in-gate task out
/// of `processes` to avoid the reconciliation fails by name instead of passing.
///
/// The exit statuses are held to the record's own verdict rather than to zero, so
/// a record that reports a red run can still be written — what it may not do is
/// report one while calling itself a pass.
///
/// What this cannot do, stated because the reconciliation reads stronger than it
/// is: both sides are still Ravel's own text. It compares a record against the
/// task graph's source, not against a run, so a row's `exitStatus` is an
/// unverifiable assertion about what happened and remains one. What it closes is
/// the drift between the two documents, which is the part a later edit breaks.
fn verify_m4_process_matrix(root: &Path, violations: &mut Vec<String>) {
    let record = root.join(M4_PROCESS_MATRIX);
    if !record.is_file() {
        // Its absence is already a violation where the record set is checked.
        return;
    }
    let gate = match gate_sub_tasks(root) {
        Ok(gate) => gate,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    let Some(recorded) = recorded_tasks(&record, ".processes", violations) else {
        return;
    };
    for extra in recorded.difference(&gate) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} records {extra} under processes, which {GATE_DEFINITION} does not \
             list among the sub-tasks ravel:verify runs; a row the gate never ran took its exit \
             status from somewhere the record does not name"
        ));
    }
    for missing in gate.difference(&recorded) {
        violations.push(format!(
            "{M4_PROCESS_MATRIX} omits {missing} from processes, which {GATE_DEFINITION} lists \
             among the sub-tasks ravel:verify runs; a gate process with no row is how a failing \
             one leaves a record that still reads complete"
        ));
    }

    if let Some(outside) = recorded_tasks(&record, ".checksOutsideTheGate", violations) {
        for inside in outside.intersection(&gate) {
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records {inside} under checksOutsideTheGate while \
                 {GATE_DEFINITION} lists it among the sub-tasks ravel:verify runs; that section is \
                 for checks the gate does not run and is not a way out of the reconciliation"
            ));
        }
    }

    verify_recorded_exit_statuses(&record, violations);
}

/// Every sub-task `ravel:verify` declares in its `after` list.
fn gate_sub_tasks(root: &Path) -> Result<BTreeSet<String>, String> {
    let source = fs::read_to_string(root.join(GATE_DEFINITION))
        .map_err(|error| format!("cannot read {GATE_DEFINITION}: {error}"))?;
    let mut names = BTreeSet::new();
    let mut inside_gate = false;
    let mut inside_list = false;
    for line in source.lines().map(str::trim) {
        if line.starts_with("\"ravel:verify\" =") {
            inside_gate = true;
            continue;
        }
        if !inside_gate {
            continue;
        }
        if !inside_list {
            inside_list = line.starts_with("after = [");
            continue;
        }
        if line.starts_with(']') {
            break;
        }
        let name = line.trim_matches('"');
        if !name.is_empty() {
            names.insert(name.to_owned());
        }
    }
    if names.is_empty() {
        return Err(format!(
            "{GATE_DEFINITION} states no after list for ravel:verify, so nothing says which \
             sub-tasks the gate runs and no record of them can be reconciled"
        ));
    }
    Ok(names)
}

/// The task each row of one array names, or nothing when the array is absent.
fn recorded_tasks(
    record: &Path,
    field: &str,
    violations: &mut Vec<String>,
) -> Option<BTreeSet<String>> {
    let commands = match jq_value(record, &format!("{field}[]?.command")) {
        Ok(commands) => commands,
        Err(error) => {
            violations.push(error);
            return None;
        }
    };
    let mut tasks = BTreeSet::new();
    for command in commands
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        match command.strip_prefix(TASK_COMMAND_PREFIX) {
            Some(task) => {
                tasks.insert(task.trim().to_owned());
            }
            None => violations.push(format!(
                "{M4_PROCESS_MATRIX} records the command `{command}` under {field}, which does not \
                 name a devenv task; a row nothing can resolve to a task is reconciled against \
                 nothing"
            )),
        }
    }
    Some(tasks)
}

/// Refuses a red row under a record that calls itself green.
fn verify_recorded_exit_statuses(record: &Path, violations: &mut Vec<String>) {
    match jq_value(record, ".result") {
        Ok(result) if result == "pass" => {}
        Ok(_) => return,
        Err(error) => {
            violations.push(error);
            return;
        }
    }
    for field in [".processes", ".checksOutsideTheGate"] {
        let failed = match jq_value(
            record,
            &format!("{field}[]? | select(.exitStatus != 0) | .command"),
        ) {
            Ok(failed) => failed,
            Err(error) => {
                violations.push(error);
                continue;
            }
        };
        for command in failed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            violations.push(format!(
                "{M4_PROCESS_MATRIX} records a nonzero exit status for `{command}` under {field} \
                 while its own result is pass; a record that reports a failed process and calls \
                 itself green states two things that cannot both be true"
            ));
        }
    }
}

/// Requires M4's manifest and its matrix record to report the same gate run.
///
/// Both documents state the run's figures, which is one figure written twice and
/// therefore one figure free to go stale in one place. The manifest's block is
/// already held to its own arithmetic and to the coverage floor by
/// [`verify_recorded_run`]; what nothing checked is that it describes the run the
/// record beside it reports. A mismatch here means one of the two was edited
/// without the other, and there is no reading under which that is harmless.
fn verify_m4_gate_figures(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m4/linux-process-matrix.json");
    if !record.is_file() {
        return;
    }
    for (claimed_at, measured_at) in [
        (".tests.total", ".gate.tests.run"),
        (".tests.passed", ".gate.tests.passed"),
        (".tests.failed", ".gate.tests.failed"),
        (".tests.skipped", ".gate.tests.skipped"),
        (".tests.lineCoveragePercent", ".gate.coverage.linesPercent"),
    ] {
        let (Ok(claimed), Ok(measured)) = (
            jq_value(manifest_path, claimed_at),
            jq_value(&record, measured_at),
        ) else {
            violations.push(format!(
                "evidence/m4/manifest.json {claimed_at} and evidence/m4/linux-process-matrix.json \
                 {measured_at} cannot both be read, so nothing reconciles them"
            ));
            continue;
        };
        if claimed != measured {
            violations.push(format!(
                "evidence/m4/manifest.json records {claimed_at} {claimed} while \
                 evidence/m4/linux-process-matrix.json records {measured_at} {measured}; they \
                 describe one gate run"
            ));
        }
    }
}

/// The record M5 cannot close without, and what it has to say.
///
/// M5's deliverable sits in a private workspace, so the public record is an
/// aggregate: counts, verdicts, and digests over files this repository does not
/// hold. Every field is type-checked before it is compared, because jq orders
/// strings above numbers and an unguarded `> 0` accepts the string "not
/// measured".
const M5_RECORDS: &[(&str, &str, &str)] = &[(
    "inventory-summary.json",
    r#".milestone == "M5"
        and .workPackage == "M5-W8"
        and .protocol == "m5-inventory-v1"
        and (.result == "pass" or .result == "provisional")
        and .disclosure.readObservation.bound != null
        and (.disclosure.readObservation.bound | type == "string" and length > 0)
        and (.disclosure.judgedClaims | type == "array" and length > 0)
        and (.disclosure.judgedClaims | all(
            (.claim | type == "string" and length > 0)
            and (.basis == "judged")
            and (.decidedBy | type == "string" and length > 0)))
        and (.artifacts | type == "object")
        and ([.artifacts.total, .artifacts.catalogued, .artifacts.uncatalogued,
              .artifacts.planned]
             | all(type == "number"))
        and (.graph | type == "object")
        and ([.graph.nodes, .graph.resolvedNodes, .graph.blockedNodes, .graph.edges,
              .graph.resolvedEdges, .graph.blockedEdges, .graph.clusters,
              .graph.largestCluster]
             | all(type == "number"))
        and (.rights | type == "object")
        and ([.rights.sourceUse.permitted, .rights.sourceUse.conditional,
              .rights.sourceUse.prohibited, .rights.sourceUse.unresolved,
              .rights.publishable]
             | all(type == "number"))
        and (.surfaces | type == "object")
        and ([.surfaces.classFiles, .surfaces.models, .surfaces.textures,
              .surfaces.recipes, .surfaces.catalogued, .surfaces.unclassified,
              .surfaces.nestedJars]
             | all(type == "number"))
        and (.plans | type == "object")
        and ([.plans.planned, .plans.readingSource, .plans.notReadingSource,
              .plans.dependencyEdges, .plans.canonicalIds]
             | all(type == "number"))
        and (.reads | type == "object")
        and ([.reads.producersWatched, .reads.archiveReads, .reads.entryBodyReads]
             | all(type == "number"))
        and (.boundArtifacts | type == "array" and length > 0)
        and (.boundArtifacts | all(
            (.name | type == "string" and length > 0)
            and (.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
            and (.holds | type == "string" and length > 0)
            and (.class == "material" or .class == "machinery")))
        and ((.boundArtifacts | map(.name) | unique | length)
             == (.boundArtifacts | length))
        and (.primitiveScan.class == "machinery")
        and (.machineryAmendments | type == "array")
        and (.machineryAmendments | all(
            (.milestone | type == "string" and length > 0)
            and (.reason | type == "string" and length > 0)
            and (.entries | type == "array" and length > 0)
            and (.figures | type == "array")))
        and ((.boundArtifacts | map(select(.class == "machinery") | .name)) as $machinery
             | .machineryAmendments | all(.entries | all(. as $e | $machinery | index($e) != null)))"#,
    "an inventory summary carrying every aggregate as a number, a digest over every \
     private file it binds with the class that digest freezes, its trail of machinery \
     amendments, and its own disclosure of what the read observation watches and \
     which of its claims rest on judgement",
)];

/// Decision records M5 cannot close without.
///
/// The rights half of M5's inventory is a judged claim, and the record that
/// decides how a judged claim is written is what makes publishing it legitimate
/// rather than a figure with no basis. It is not an ADG: `SPEC.md` §15 owns those
/// numbers and reserves the unused ones for other subjects.
const M5_DECISIONS: &[&str] = &[
    "claims-without-a-second-reading.md",
    // M5's bound digests freeze two different things, and the record now says
    // which is which per entry. What a class means — permanent for material,
    // amendable in the open for machinery — is decided there rather than in the
    // summary that carries the labels, so M5's record does not resolve without
    // it. It was written for M6 and reaches back into M5's record, which is the
    // amendment that record licenses.
    "machinery-is-not-material.md",
];

/// Checks what closing M5 requires beyond the rules every milestone shares.
///
/// M5 catalogues a fixture it may not publish, so its closure is almost entirely
/// about the boundary: the numbers reconcile with each other, the private files
/// behind them are bound by digest, and the two claims that cannot be recomputed
/// anywhere say so in the record rather than being carried as if they were
/// measurements.
fn verify_m5_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M5-W1\", \"M5-W2\", \"M5-W3\", \"M5-W4\", \"M5-W5\", \"M5-W6\", \"M5-W7\", \"M5-W8\"]",
        "M5 manifest must include every work package from M5-W1 through M5-W8 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m5-inventory-v1"
            and ([.benchmarks.results[].name] == ["inventory-summary.json"])"#,
        "M5 manifest must record its inventory summary under the m5-inventory-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M5-W8") | .value]
                 | length == 7
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M5 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("official-atm11") != null)
        )"#,
        "M5 closure requires a native Linux attestation for the recorded inventory run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M5 recorded deviations and risks, and an empty finding set satisfies every rule phrased \
         as a refusal, so closure requires them to still be there",
        violations,
    );
    // A provisional figure is a legitimate thing for this manifest to carry while
    // the workspace behind it is being repaired — it is what lets the gate's
    // arithmetic be exercised and mutation-proved before the numbers are final.
    // What it may not do is close a milestone. `ravel:verify` accepts the
    // provisional record, because a repository holding one is not broken; only
    // closure refuses it, and it refuses it here rather than leaving a reader to
    // notice that the record and the manifest disagree about how settled they are.
    let record = root.join("evidence/m5/inventory-summary.json");
    if record.is_file() && jq_matches(&record, r#".result == "provisional""#).unwrap_or(false) {
        violations.push(
            "M5 closure requires final figures: evidence/m5/inventory-summary.json records itself \
             as provisional, which is a true statement about an inventory under repair and not \
             something a milestone can close over"
                .to_owned(),
        );
    }

    for decision in M5_DECISIONS {
        if !root.join("docs/decisions").join(decision).is_file() {
            violations.push(format!(
                "M5 closure requires the decision record {decision}"
            ));
        }
    }

    for (file, rule, requirement) in M5_RECORDS {
        let path = root.join("evidence/m5").join(file);
        if !path.is_file() {
            violations.push(format!("M5 closure requires the record evidence/m5/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M5 closure requires {requirement}"));
        }
        verify_redaction(&path, &format!("evidence/m5/{file}"), violations);
    }

    verify_m5_inventory_arithmetic(root, violations);
    verify_m5_carried_figures(root, manifest_path, violations);
}

/// The sums the inventory summary has to close, inside the record itself.
///
/// A headline figure with no parts beside it is a number nothing can contradict.
/// Each of these decomposes one published total into the pieces it was built
/// from, so that changing the total alone, or one piece alone, stops the record
/// agreeing with itself. This is what the public gate can hold: it cannot re-take
/// the measurement, and it says so in the manifest's own deviations rather than
/// implying otherwise here.
const M5_ARITHMETIC: &[(&str, &str)] = &[
    (
        ".artifacts.catalogued + .artifacts.uncatalogued == .artifacts.total",
        "every artifact is either catalogued or not",
    ),
    (
        ".artifacts.planned == .artifacts.total and .artifacts.uncatalogued == 0",
        "M5-W8's gate row asks that every artifact carry a complete plan",
    ),
    (
        ".plans.planned == .artifacts.total",
        "M5-W8's gate row asks for a plan per catalogued artifact",
    ),
    (
        ".plans.readingSource + .plans.notReadingSource == .artifacts.total",
        "a plan either reads source or does not",
    ),
    (
        ".plans.canonicalIds == .artifacts.total",
        "M5-W6 assigns one canonical ID per entry",
    ),
    (
        ".rights.sourceUse.permitted + .rights.sourceUse.conditional
            + .rights.sourceUse.prohibited + .rights.sourceUse.unresolved
            == .artifacts.total",
        "every triaged artifact carries exactly one source-use verdict",
    ),
    (
        ".surfaces.catalogued + .surfaces.unclassified == .artifacts.total",
        "every acquired artifact is catalogued or is counted as unclassified",
    ),
    (
        ".graph.resolvedNodes + .graph.blockedNodes == .graph.nodes",
        "every node resolves or carries a blocking classification",
    ),
    (
        ".graph.resolvedEdges + .graph.blockedEdges == .graph.edges",
        "every edge resolves or carries a blocking classification",
    ),
    (
        ".graph.clusters > 0 and .graph.largestCluster > 0
            and .graph.largestCluster <= .graph.nodes",
        "a cluster is a set of nodes, so the largest cannot exceed the graph",
    ),
    (
        ".reads.producersWatched > 0 and .reads.archiveReads > 0",
        "an observation that classified nothing observed nothing",
    ),
    (
        ".reads.entryBodyReads == 0",
        "reading an entry body is what the metadata-only method excludes",
    ),
];

fn verify_m5_inventory_arithmetic(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m5/inventory-summary.json");
    if !record.is_file() {
        // Its absence is already a violation where the record set is checked.
        return;
    }
    verify_record_arithmetic(
        &record,
        "evidence/m5/inventory-summary.json",
        M5_ARITHMETIC,
        violations,
    );
}

/// Reports every decomposition a record does not close against itself.
fn verify_record_arithmetic(
    record: &Path,
    record_label: &str,
    rules: &[(&str, &str)],
    violations: &mut Vec<String>,
) {
    for (filter, requirement) in rules {
        if !jq_matches(record, filter).unwrap_or(false) {
            violations.push(format!(
                "{record_label} does not satisfy `{filter}`, and {requirement}"
            ));
        }
    }
}

/// The figures the manifest carries, recomputed from the record's own parts.
///
/// The manifest states the headline totals because a reader of the manifest
/// should not have to open a second file to learn what M5 counted. Stating a
/// figure twice is how figures drift, so none of these is compared against a
/// copy: each is compared against a sum or a field the record derives it from,
/// and the record's parts are held to each other by [`M5_ARITHMETIC`]. Changing
/// one number anywhere breaks one of the two.
const M5_CARRIED_FIGURES: &[(&str, &str)] = &[
    (
        ".environment.settings.inventory.artifacts",
        ".artifacts.catalogued + .artifacts.uncatalogued",
    ),
    (
        ".environment.settings.inventory.planned",
        ".artifacts.planned",
    ),
    (
        ".environment.settings.inventory.graphNodes",
        ".graph.resolvedNodes + .graph.blockedNodes",
    ),
    (
        ".environment.settings.inventory.graphEdges",
        ".graph.resolvedEdges + .graph.blockedEdges",
    ),
    (
        ".environment.settings.inventory.clusters",
        ".graph.clusters",
    ),
    (
        ".environment.settings.inventory.sourceProhibited",
        ".rights.sourceUse.prohibited",
    ),
    (
        ".environment.settings.inventory.sourceUnresolved",
        ".rights.sourceUse.unresolved",
    ),
    (
        ".environment.settings.inventory.publishable",
        ".rights.publishable",
    ),
    (
        ".environment.settings.inventory.readingSource",
        ".plans.readingSource",
    ),
    (
        ".environment.settings.inventory.dependencyEdges",
        ".plans.dependencyEdges",
    ),
    (
        ".environment.settings.inventory.archiveReads",
        ".reads.archiveReads",
    ),
    (
        ".environment.settings.inventory.entryBodyReads",
        ".reads.entryBodyReads",
    ),
    (
        ".environment.settings.inventory.digestsBound",
        ".boundArtifacts | length",
    ),
];

fn verify_m5_carried_figures(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m5/inventory-summary.json");
    if !record.is_file() {
        return;
    }
    verify_carried_figures(
        manifest_path,
        "evidence/m5/manifest.json",
        &record,
        "evidence/m5/inventory-summary.json",
        "inventory",
        M5_CARRIED_FIGURES,
        violations,
    );
}

/// Recomputes each figure a manifest carries from the record it came out of.
fn verify_carried_figures(
    manifest_path: &Path,
    manifest_label: &str,
    record: &Path,
    record_label: &str,
    subject: &str,
    figures: &[(&str, &str)],
    violations: &mut Vec<String>,
) {
    for (claimed_at, derived_from) in figures {
        let (Ok(claimed), Ok(derived)) = (
            jq_value(manifest_path, claimed_at),
            jq_value(record, derived_from),
        ) else {
            violations.push(format!(
                "{manifest_label} {claimed_at} and {record_label} \
                 `{derived_from}` cannot both be read, so nothing reconciles them"
            ));
            continue;
        };
        if claimed == "null" || derived == "null" {
            violations.push(format!(
                "{manifest_label} {claimed_at} or the record's `{derived_from}` is \
                 absent; a figure the gate cannot resolve is a figure nothing binds"
            ));
            continue;
        }
        if claimed != derived {
            violations.push(format!(
                "{manifest_label} records {claimed_at} {claimed} while \
                 {record_label} derives {derived} from `{derived_from}`; one \
                 {subject} produced both"
            ));
        }
    }
}

/// The record M6 cannot close without, and what it has to say.
///
/// M6's counterparts are built in the private compatibility workspace, so the
/// public record is again an aggregate: the wave that was frozen, one row per
/// counterpart carrying its observable areas and how many of them nothing runs,
/// and the rewrite-integrity check's own scan and mutation results. Every field
/// is type-checked before it is compared, for the reason [`M5_RECORDS`] gives.
const M6_RECORDS: &[(&str, &str, &str)] = &[(
    "counterpart-summary.json",
    r#".milestone == "M6"
        and .workPackage == "M6-W6"
        and .protocol == "m6-counterpart-v1"
        and .result == "pass"
        and (.wave | type == "object")
        and (.wave.frozen == true)
        and (.wave.entries | type == "array" and length > 0)
        and (.wave.entries | all(
            (.id | type == "string" and length > 0)
            and (.reason | type == "string" and length > 0)
            and (.dependencies | type == "array")))
        and (.counterparts | type == "array" and length > 0)
        and (.counterparts | all(
            (.id | type == "string" and length > 0)
            and (.entry | type == "string" and length > 0)
            and (.specification | type == "object")
            and (.specification.route == "source" or .specification.route == "observed")
            and ([.specification.accepted, .specification.structureReviewed,
                  .specification.carriesUpstreamIdentifier]
                 | all(type == "boolean"))
            and ([.observableAreas, .declarableOnly, .exercised] | all(type == "number"))
            and (.provenance | type == "object")
            and (.provenance.complete | type == "boolean")
            and ([.suites.total, .suites.failed] | all(type == "number"))))
        and ((.counterparts | map(.id) | unique | length) == (.counterparts | length))
        and (.cluster | type == "object")
        and (.cluster.scenario | type == "string" and length > 0)
        and (.cluster.steps | type == "array" and length > 0)
        and (.cluster.steps | all(
            (.name | type == "string" and length > 0)
            and (.result | type == "string" and length > 0)))
        and (.processValidation | type == "object")
        and ([.processValidation.template, .processValidation.checklist,
              .processValidation.evidenceSchema, .processValidation.secondCounterpart]
             | all(type == "string" and length > 0))
        and (.rewriteIntegrity | type == "object")
        and (.rewriteIntegrity.enforcing | type == "boolean")
        and ([.rewriteIntegrity.lexicon, .rewriteIntegrity.identifierSet]
             | all(type == "string" and length > 0))
        and ([.rewriteIntegrity.filesScanned, .rewriteIntegrity.artifactsScanned,
              .rewriteIntegrity.violations, .rewriteIntegrity.specificationViolations]
             | all(type == "number"))
        and (.rewriteIntegrity.mutationProof | type == "object")
        and ([.rewriteIntegrity.mutationProof.sourceInjection.exitStatus,
              .rewriteIntegrity.mutationProof.artifactInjection.exitStatus,
              .rewriteIntegrity.mutationProof.cleanRun.exitStatus]
             | all(type == "number"))
        and (.totals | type == "object")
        and ([.totals.counterparts, .totals.observableAreas, .totals.declarableOnly,
              .totals.exercised]
             | all(type == "number"))"#,
    "a counterpart summary carrying the frozen wave with a reason per entry, one row per \
     counterpart with its observable areas as numbers, the cluster scenario's steps, the \
     process-validation artefacts, and the rewrite-integrity check's scan counts and both \
     mutation exit statuses",
)];

/// Decision records M6 cannot close without.
///
/// Both were accepted inside M6 and both decide something a counterpart is
/// written against rather than something it merely cites: where an upstream name
/// may appear, and how an action a mod asks for reaches the game.
const M6_DECISIONS: &[&str] = &[
    "carrying-an-action-outward.md",
    "counterpart-identity-and-aliases.md",
];

/// The sums and thresholds M6's counterpart summary has to close, inside itself.
///
/// The declarable-only figure is the one this table exists for. ROADMAP.md says
/// it is the denominator M6's green sits over, and a denominator nothing binds is
/// a number a hand can move: each counterpart's areas are held to its own parts,
/// the totals are held to the rows, and a counterpart observing nothing at all is
/// refused rather than counted as fully exercised.
///
/// The rewrite-integrity rules are here rather than in the record predicate above
/// because they are not shapes. ROADMAP.md makes the check M6's obligation to
/// *enforce*, proved by injecting an upstream identifier into a counterpart source
/// file and into a generated artefact, so a clean run that never opened an
/// artefact and a mutation that exited zero are both refused by name.
const M6_ARITHMETIC: &[(&str, &str)] = &[
    (
        ".totals.counterparts == (.counterparts | length)",
        "the total counts the rows the record itself lists",
    ),
    (
        ".totals.observableAreas == ([.counterparts[].observableAreas] | add)",
        "the observable-area total is the sum of the counterparts' own",
    ),
    (
        ".totals.declarableOnly == ([.counterparts[].declarableOnly] | add)",
        "the declarable-only total is the sum of the counterparts' own",
    ),
    (
        ".totals.exercised == ([.counterparts[].exercised] | add)",
        "the exercised total is the sum of the counterparts' own",
    ),
    (
        ".counterparts | all(.declarableOnly + .exercised == .observableAreas)",
        "an observable area is either exercised by a running process or declarable-only",
    ),
    (
        ".totals.declarableOnly + .totals.exercised == .totals.observableAreas",
        "the denominator M6's green sits over is the sum of its two halves",
    ),
    (
        ".counterparts | all(.observableAreas > 0)",
        "a counterpart with no observable area declared nothing to be green about",
    ),
    (
        "([.counterparts[].entry] | unique) == ([.wave.entries[].id] | unique)",
        "M6-W3 asks for one accepted behaviour specification per frozen wave entry, so a \
         counterpart outside the wave and a wave entry with no counterpart are both refused",
    ),
    (
        ".counterparts | all(.specification.accepted == true)",
        "M6-W3 asks that every entry's behaviour specification be accepted",
    ),
    (
        ".counterparts | all(.specification.carriesUpstreamIdentifier == false)",
        "M6-W3 asks that no specification carry an upstream identifier",
    ),
    (
        ".counterparts | all(.specification.structureReviewed == true)",
        "M6-W5 reviews a specification for structure before an implementer sees it, which is \
         what caught the defect the sampling did not",
    ),
    (
        ".counterparts | all(.provenance.complete == true)",
        "M6-W6's gate row asks for complete provenance",
    ),
    (
        ".counterparts | all(.suites.total > 0 and .suites.failed == 0)",
        "M6-W3 asks that each package's suites pass, and a package running none passed none",
    ),
    (
        ".counterparts | length >= 2",
        "M6-W5 validates the authoring process against a second independent counterpart, which \
         needs two",
    ),
    (
        ".processValidation.secondCounterpart as $second
            | [.counterparts[].id] | index($second) != null",
        "M6-W5's second counterpart has to be one the record carries a row for",
    ),
    (
        r#"(["fresh-play", "save", "exit", "reload"] - [.cluster.steps[].name]) == []
            and (.cluster.steps | all(.result == "pass"))"#,
        "M6-W4's verification is a fresh play, save, exit and reload scenario that passes",
    ),
    (
        ".rewriteIntegrity.enforcing == true",
        "M6-W6's gate row asks that the rewrite-integrity check be enforcing rather than advisory",
    ),
    (
        ".rewriteIntegrity.violations == 0",
        "a counterpart carrying an upstream identifier fails the check, so closure cannot record \
         one that it carried",
    ),
    (
        ".rewriteIntegrity.specificationViolations == 0",
        "M6-W3 asks that no specification carry an upstream identifier, and the check reads the \
         specifications too",
    ),
    (
        ".rewriteIntegrity.filesScanned > 0 and .rewriteIntegrity.artifactsScanned > 0",
        "the check reads every counterpart source file and every generated artefact, and one \
         enforcing the rule over source files only has not satisfied the obligation",
    ),
    (
        ".rewriteIntegrity.mutationProof.sourceInjection.exitStatus != 0",
        "the check is proved by mutation: an upstream identifier injected into a counterpart \
         source file must produce a nonzero exit",
    ),
    (
        ".rewriteIntegrity.mutationProof.artifactInjection.exitStatus != 0",
        "the check is proved by mutation: an upstream identifier injected into a generated \
         artefact must produce a nonzero exit",
    ),
    (
        ".rewriteIntegrity.mutationProof.cleanRun.exitStatus == 0",
        "a check that fails the unmutated tree proves nothing about the mutations it also failed",
    ),
];

/// The figures M6's manifest carries, recomputed from the record's own parts.
///
/// Same rule as [`M5_CARRIED_FIGURES`]: nothing here is compared against a copy.
/// The two mutation exit statuses are carried because ROADMAP.md asks the manifest
/// to record them, and they are reconciled here so that recording them is not a
/// second place for them to say something the record does not.
const M6_CARRIED_FIGURES: &[(&str, &str)] = &[
    (
        ".environment.settings.counterparts.counterparts",
        ".counterparts | length",
    ),
    (
        ".environment.settings.counterparts.waveEntries",
        ".wave.entries | length",
    ),
    (
        ".environment.settings.counterparts.observableAreas",
        "[.counterparts[].observableAreas] | add",
    ),
    (
        ".environment.settings.counterparts.declarableOnly",
        "[.counterparts[].declarableOnly] | add",
    ),
    (
        ".environment.settings.counterparts.exercised",
        "[.counterparts[].exercised] | add",
    ),
    (
        ".environment.settings.rewriteIntegrity.filesScanned",
        ".rewriteIntegrity.filesScanned",
    ),
    (
        ".environment.settings.rewriteIntegrity.artifactsScanned",
        ".rewriteIntegrity.artifactsScanned",
    ),
    (
        ".environment.settings.rewriteIntegrity.violations",
        ".rewriteIntegrity.violations",
    ),
    (
        ".environment.settings.rewriteIntegrity.sourceInjectionExitStatus",
        ".rewriteIntegrity.mutationProof.sourceInjection.exitStatus",
    ),
    (
        ".environment.settings.rewriteIntegrity.artifactInjectionExitStatus",
        ".rewriteIntegrity.mutationProof.artifactInjection.exitStatus",
    ),
];

/// Checks what closing M6 requires beyond the rules every milestone shares.
///
/// M6 validates the counterpart-authoring process, so its closure is about the
/// two things a green admitted set cannot say on its own: that the
/// rewrite-integrity check refuses what it claims to refuse, proved by injection
/// rather than asserted, and that every counterpart discloses how much of its
/// behaviour no running process has ever exercised.
fn verify_m6_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M6-W1\", \"M6-W2\", \"M6-W3\", \"M6-W4\", \"M6-W5\", \"M6-W6\"]",
        "M6 manifest must include every work package from M6-W1 through M6-W6 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m6-counterpart-v1"
            and ([.benchmarks.results[].name] == ["counterpart-summary.json"])"#,
        "M6 manifest must record its counterpart summary under the m6-counterpart-v1 protocol, so \
         that the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M6-W6") | .value]
                 | length == 5
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M6 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".environment.settings.rewriteIntegrity.enforcing == true
            and ([.environment.settings.rewriteIntegrity.sourceInjectionExitStatus,
                  .environment.settings.rewriteIntegrity.artifactInjectionExitStatus]
                 | all(type == "number" and . != 0))"#,
        "M6 closure requires the manifest itself to record the rewrite-integrity check as \
         enforcing and to record a nonzero exit status for each of the two injections that \
         prove it",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("official-atm11") != null)
        )"#,
        "M6 closure requires a native Linux attestation for the recorded counterpart run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M6 counts the observable areas nothing runs, and a milestone disclosing that and \
         recording no deviation or risk deleted the finding rather than the defect",
        violations,
    );

    for decision in M6_DECISIONS {
        if !root.join("docs/decisions").join(decision).is_file() {
            violations.push(format!(
                "M6 closure requires the decision record {decision}"
            ));
        }
    }

    for (file, rule, requirement) in M6_RECORDS {
        let path = root.join("evidence/m6").join(file);
        if !path.is_file() {
            violations.push(format!("M6 closure requires the record evidence/m6/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M6 closure requires {requirement}"));
        }
        verify_redaction(&path, &format!("evidence/m6/{file}"), violations);
    }

    verify_m6_counterpart_arithmetic(root, violations);
    verify_m6_carried_figures(root, manifest_path, violations);
}

fn verify_m6_counterpart_arithmetic(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m6/counterpart-summary.json");
    if !record.is_file() {
        // Its absence is already a violation where the record set is checked.
        return;
    }
    verify_record_arithmetic(
        &record,
        "evidence/m6/counterpart-summary.json",
        M6_ARITHMETIC,
        violations,
    );
}

fn verify_m6_carried_figures(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m6/counterpart-summary.json");
    if !record.is_file() {
        return;
    }
    verify_carried_figures(
        manifest_path,
        "evidence/m6/manifest.json",
        &record,
        "evidence/m6/counterpart-summary.json",
        "counterpart set",
        M6_CARRIED_FIGURES,
        violations,
    );
}

/// The record M7 cannot close without, and what it has to say.
///
/// M7's claim is a completed library, which is a claim about a total rather than
/// about any one package: the burn-up ledger, the waves the work was ordered by,
/// and the closure figures M7-W5 audits. The corrected-total branch is a field
/// rather than a licence, because "324 or an evidence-corrected total" is only
/// checkable if the correction says what it corrected and why.
const M7_RECORDS: &[(&str, &str, &str)] = &[(
    "library-summary.json",
    r#".milestone == "M7"
        and .workPackage == "M7-W5"
        and .protocol == "m7-library-v1"
        and .result == "pass"
        and (.entries | type == "object")
        and ([.entries.total, .entries.accepted, .entries.blocked, .entries.unstarted]
             | all(type == "number"))
        and (.correction == null or (
            (.correction | type == "object")
            and ([.correction.from, .correction.total] | all(type == "number"))
            and (.correction.reason | type == "string" and length > 0)))
        and (.waves | type == "array" and length > 0)
        and (.waves | all(
            ([.index, .entries] | all(type == "number"))
            and (.dependsOn | type == "array")
            and (.dependsOn | all(type == "number"))))
        and ([.dependencies.resolved, .dependencies.unresolved,
              .registries.resolved, .registries.unresolved,
              .provenance.present, .provenance.missing,
              .routing.absorbed, .routing.remaining,
              .routing.runnableToday, .routing.blocked,
              .suites.packages.total, .suites.packages.failed,
              .suites.clusters.total, .suites.clusters.failed]
             | all(type == "number"))"#,
    "a library summary carrying the burn-up ledger as numbers, the waves with the waves each \
     depends on, any evidence correction with the figure it replaced and its reason, and the \
     dependency, registry, provenance, routing and suite figures M7-W5 audits",
)];

/// The sums and thresholds M7's library summary has to close, inside itself.
///
/// M7's verification columns are almost all totals, and a total is the figure a
/// milestone is most easily closed on: `accepted + blocked + unstarted` is the
/// ledger M7-W4 reconciles, and M7-W5 then asks that the blocked and unstarted
/// halves be empty rather than merely counted. The wave rules are M7-W1's graph
/// closure written as arithmetic: a wave may depend only on a wave that exists and
/// runs before it.
const M7_ARITHMETIC: &[(&str, &str)] = &[
    (
        ".entries.accepted + .entries.blocked + .entries.unstarted == .entries.total",
        "M7-W4's burn-up ledger asks that the accepted, blocked and unstarted totals equal the \
         verified fixture total",
    ),
    (
        ".entries.blocked == 0 and .entries.unstarted == 0",
        "a wave cannot close with failing or unclassified entries, so the library cannot close \
         with any",
    ),
    (
        ".entries.accepted == .entries.total",
        "M7-W5's audit is the whole fixture, and a counterpart absent from it is not present",
    ),
    (
        r#".entries.total == 324
            or ((.correction | type == "object") and .correction.total == .entries.total)"#,
        "M7-W5 audits 324 of 324 or an evidence-corrected total, and a different figure with no \
         correction beside it is neither",
    ),
    (
        ".dependencies.unresolved == 0",
        "M7-W5's gate row asks for zero unresolved dependencies",
    ),
    (
        ".registries.unresolved == 0",
        "M7-W5's gate row asks for zero unresolved registries",
    ),
    (
        ".provenance.present == .entries.total and .provenance.missing == 0",
        "M7-W5's gate row asks that all provenance be present",
    ),
    (
        "([.waves[].entries] | add) == .entries.total",
        "the waves the work was ordered by carry the whole library between them",
    ),
    (
        "([.waves[].index] | unique | length) == (.waves | length)",
        "two waves sharing an index are one wave counted twice, and order is what a wave is for",
    ),
    (
        ". as $record
            | .waves | all(.dependsOn | all(. as $dependency
                | ([$record.waves[].index] | index($dependency)) != null))",
        "M7-W1's graph closure proves no wave depends on an unavailable wave",
    ),
    (
        ".waves | all(. as $wave | .dependsOn | all(. < $wave.index))",
        "M7-W1's graph closure proves no wave depends on a later wave",
    ),
    (
        ".routing.absorbed + .routing.remaining == .entries.total",
        "M7-W1's routing counts, per entry, how much Ravel already ships and how much remains",
    ),
    (
        ".routing.runnableToday + .routing.blocked == .entries.total",
        "an entry is runnable today or it is blocked, and the two account for the library",
    ),
    (
        ".routing.blocked == 0",
        "a library whose entries cannot be run today is not the complete library M7 delivers",
    ),
    (
        ".suites.packages.total > 0 and .suites.packages.failed == 0",
        "M7-W6's gate row asks that all package suites pass, and a library running none passed \
         none",
    ),
    (
        ".suites.clusters.total > 0 and .suites.clusters.failed == 0",
        "M7-W6's gate row asks that all cluster suites pass, and a library running none passed \
         none",
    ),
];

/// The figures M7's manifest carries, recomputed from the record's own parts.
const M7_CARRIED_FIGURES: &[(&str, &str)] = &[
    (
        ".environment.settings.library.entries",
        ".entries.accepted + .entries.blocked + .entries.unstarted",
    ),
    (
        ".environment.settings.library.accepted",
        ".entries.accepted",
    ),
    (".environment.settings.library.blocked", ".entries.blocked"),
    (
        ".environment.settings.library.unstarted",
        ".entries.unstarted",
    ),
    (".environment.settings.library.waves", ".waves | length"),
    (
        ".environment.settings.library.dependenciesUnresolved",
        ".dependencies.unresolved",
    ),
    (
        ".environment.settings.library.registriesUnresolved",
        ".registries.unresolved",
    ),
    (
        ".environment.settings.library.provenancePresent",
        ".provenance.present",
    ),
    (
        ".environment.settings.library.absorbed",
        ".routing.absorbed",
    ),
    (
        ".environment.settings.library.packageSuites",
        ".suites.packages.total",
    ),
    (
        ".environment.settings.library.clusterSuites",
        ".suites.clusters.total",
    ),
];

/// Checks what closing M7 requires beyond the rules every milestone shares.
///
/// M7 claims the library is complete, and completeness is a claim about a
/// denominator someone else measured. So its closure holds the ledger to M5's
/// inventory rather than to itself: the total M7 audits is the total M5 verified,
/// or it is a correction that says which figure it replaced and why.
fn verify_m7_manifest(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M7-W0\", \"M7-W1\", \"M7-W2\", \"M7-W3\", \"M7-W4\", \"M7-W5\", \"M7-W6\"]",
        "M7 manifest must include every work package from M7-W0 through M7-W6 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".benchmarks.protocolVersion == "m7-library-v1"
            and ([.benchmarks.results[].name] == ["library-summary.json"])"#,
        "M7 manifest must record its library summary under the m7-library-v1 protocol, so that \
         the record carrying every figure is covered by a recomputed digest",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#"(.environment.settings.gateStatus | type == "object")
            and ((.workPackages - (.environment.settings.gateStatus | keys)) == [])
            and ([.environment.settings.gateStatus | to_entries[]
                    | select(.key != "M7-W6") | .value]
                 | length == 6
                 and all(.status == "closed"
                         and (.evidence | type == "string" and length > 0)))"#,
        "M7 closure requires a gateStatus row for every work package, each implementation row \
         closed and naming its evidence",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        r#".attestations | any(
            .platform == "linux"
            and .architecture == "x86_64"
            and (.workloads | index("official-atm11") != null)
        )"#,
        "M7 closure requires a native Linux attestation for the recorded library run",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        "(.deviations | length) > 0 and (.risks | length) > 0",
        "M7 closes a library of 324 counterparts, and an empty finding set satisfies every rule \
         phrased as a refusal",
        violations,
    );

    for (file, rule, requirement) in M7_RECORDS {
        let path = root.join("evidence/m7").join(file);
        if !path.is_file() {
            violations.push(format!("M7 closure requires the record evidence/m7/{file}"));
            continue;
        }
        if !jq_matches(&path, rule).unwrap_or(false) {
            violations.push(format!("M7 closure requires {requirement}"));
        }
        verify_redaction(&path, &format!("evidence/m7/{file}"), violations);
    }

    verify_m7_library_arithmetic(root, violations);
    verify_m7_verified_fixture_total(root, violations);
    verify_m7_carried_figures(root, manifest_path, violations);
}

fn verify_m7_library_arithmetic(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m7/library-summary.json");
    if !record.is_file() {
        return;
    }
    verify_record_arithmetic(
        &record,
        "evidence/m7/library-summary.json",
        M7_ARITHMETIC,
        violations,
    );
}

/// Holds M7's denominator to the inventory that measured it.
///
/// "324/324 or an evidence-corrected total" is only a rule if something owns the
/// 324. M5 does, in `artifacts.total`, so M7 either audits that number or records
/// a correction naming it as the figure it replaced. A ledger that quietly shrinks
/// its own denominator closes at 100% over a library it never counted.
fn verify_m7_verified_fixture_total(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m7/library-summary.json");
    let inventory = root.join("evidence/m5/inventory-summary.json");
    if !record.is_file() || !inventory.is_file() {
        return;
    }
    let (Ok(audited), Ok(verified)) = (
        jq_value(&record, ".entries.total"),
        jq_value(&inventory, ".artifacts.total"),
    ) else {
        violations.push(
            "evidence/m7/library-summary.json `.entries.total` and \
             evidence/m5/inventory-summary.json `.artifacts.total` cannot both be read, so \
             nothing holds M7's denominator to the inventory that measured it"
                .to_owned(),
        );
        return;
    };
    if audited == verified {
        return;
    }
    let corrects_the_inventory = jq_matches(
        &record,
        &format!(
            "(.correction | type == \"object\") and (.correction.from == {verified}) \
             and (.correction.total == .entries.total)"
        ),
    )
    .unwrap_or(false);
    if !corrects_the_inventory {
        violations.push(format!(
            "evidence/m7/library-summary.json audits {audited} entries while \
             evidence/m5/inventory-summary.json verified {verified}; M7-W5 audits the verified \
             fixture total or an evidence correction that names the figure it replaced"
        ));
    }
}

fn verify_m7_carried_figures(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m7/library-summary.json");
    if !record.is_file() {
        return;
    }
    verify_carried_figures(
        manifest_path,
        "evidence/m7/manifest.json",
        &record,
        "evidence/m7/library-summary.json",
        "library",
        M7_CARRIED_FIGURES,
        violations,
    );
}

/// Decision records M1 cannot close without.
const M1_DECISIONS: &[&str] = &[
    "adg-1-jvm-entry.md",
    "adg-2-mod-execution.md",
    "adg-3-bindings.md",
    "adg-4-state-ownership.md",
    "adg-5-prism-integration.md",
];

fn verify_m0_manifest(manifest_path: &Path, violations: &mut Vec<String>) {
    require_manifest_rule(
        manifest_path,
        ".workPackages | sort == [\"M0-W1\", \"M0-W2\", \"M0-W3\", \"M0-W4\", \"M0-W5\", \"M0-W6\", \"M0-W7\", \"M0-W8\", \"M0-W9\"]",
        "M0 manifest must include every work package from M0-W1 through M0-W9 exactly once",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        ".versions.minecraft == \"26.1.2\" and .versions.neoforge == \"26.1.2.78\" and .versions.fixture == \"ATM11 0.2.1\"",
        "M0 manifest must pin Minecraft 26.1.2, NeoForge 26.1.2.78, and ATM11 0.2.1",
        violations,
    );
    require_manifest_rule(
        manifest_path,
        &m0_platform_filter("linux", "x86_64", "linux-reference-v1"),
        "M0 manifest must include native Linux x86_64 launch evidence",
        violations,
    );
}

fn m0_platform_filter(platform: &str, architecture: &str, hardware_profile: &str) -> String {
    format!(
        r#".platformEvidence | any(
            .platform == "{platform}"
            and .architecture == "{architecture}"
            and .native == true
            and .hardwareProfile == "{hardware_profile}"
            and .versions.minecraft == "26.1.2"
            and .versions.neoforge == "26.1.2.78"
            and .versions.prism == "11.0.3"
            and .versions.fixture == "ATM11 0.2.1"
            and .versions.java == "25.0.1"
            and .versions.rust == "1.95.0"
            and .versions.nix == "2.33.3"
            and .versions.devenv == "2.1.2"
            and (.settings | length > 0)
            and any(.commands[]; .command == "devenv test --no-tui")
            and any(.commands[]; .command == "nix flake check --accept-flake-config")
            and any(.baselines[]; .workload == "vanilla")
            and any(.baselines[]; .workload == "official-atm11")
        )"#
    )
}

/// Refuses a document that carries a local path, a credential, or a secret field.
///
/// Every tracked evidence document is held to this, not just the manifest: a
/// record built from a launcher log or a machine's output is where leakage would
/// actually come from. `subject` names the document in the violation, so a
/// reader is told which file to look at.
fn verify_redaction(document: &Path, subject: &str, violations: &mut Vec<String>) {
    let output = Command::new("jq")
        .args(["--raw-output", ".. | strings"])
        .arg(document)
        .output();
    let lower = output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_ascii_lowercase())
        .unwrap_or_default();
    let contains_local_data = [
        "/home/",
        "/users/",
        r"c:\\users\\",
        // A path segment, not a bare suffix: `net.minecraft.util.StringUtil` is
        // a class name that evidence is supposed to contain.
        "/.minecraft",
        "tests/fixtures/local/",
        "evidence/private/",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    let contains_credential_value = [
        "authorization: bearer",
        "authorization= bearer",
        "access_token=",
        "accesstoken=",
        "minecraft_access_token=",
        "tangled_token=",
        "aws_secret_access_key=",
        "atp_app_password=",
        "ghp_",
        "xoxb-",
        "xoxp-",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    let credential_fields_absent = jq_matches(
        document,
        r#"[paths(strings) as $path
            | select(
                any(
                    $path[];
                    type == "string"
                    and (ascii_downcase | test("token|password|secret|credential|authorization"))
                )
                and (getpath($path) | length > 0)
            )] | length == 0"#,
    )
    .unwrap_or(false);

    if contains_local_data || contains_credential_value || !credential_fields_absent {
        violations.push(format!("{subject} contains sensitive or local data"));
    }
}

fn require_manifest_rule(
    manifest_path: &Path,
    filter: &str,
    violation: &str,
    violations: &mut Vec<String>,
) {
    match jq_matches(manifest_path, filter) {
        Ok(true) => {}
        Ok(false) => violations.push(violation.to_owned()),
        Err(error) => violations.push(error),
    }
}

/// The private compatibility workspace's directory name, in two pieces.
///
/// The fence below sweeps every tracked file for this name, so spelling it here
/// as one literal would make `tools/ravel-xtask/src/lib.rs` the first thing it
/// reports. `verify_experiment_fence` has the same shape of problem and solves
/// it the same way: a rule written inside the tree it sweeps must not itself be
/// expressible as the thing it forbids.
const PRIVATE_WORKSPACE_NAME: [&str; 2] = ["ravel-", "registry"];

/// The name a public tracked file may not contain.
fn private_workspace_name() -> String {
    PRIVATE_WORKSPACE_NAME.concat()
}

/// Rejects rights-restricted, secret, and local-runtime paths from a tracked tree.
#[must_use]
pub fn verify_tracked_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let private = private_workspace_name();
    paths
        .into_iter()
        .flat_map(|path| {
            let normalized = path.replace('\\', "/");
            let lower = normalized.to_ascii_lowercase();
            let mut violations = Vec::new();
            if is_forbidden_tracked_path(&normalized) {
                violations.push(format!("forbidden tracked path: {normalized}"));
            }
            if lower.contains(&private) {
                violations.push(format!(
                    "tracked path names the private compatibility workspace: {normalized}"
                ));
            }
            violations
        })
        .collect()
}

/// Rejects private keys, credential assignments, and private-workspace
/// references in tracked text.
///
/// The last of those is M5-W1's own verification criterion, held here rather
/// than asserted in prose. Four rules run, and they are named rather than
/// summarised: a PEM private-key header, a credential assignment, the private
/// compatibility workspace's name as a lowercase substring, and a Cargo `path =`
/// value that walks out of this checkout. The name is refused everywhere rather
/// than only in manifests — `docs/architecture/public-private-boundary.md`
/// describes the private workspace without naming it for exactly this reason,
/// and a document that started naming it would be the first sign the split had
/// stopped being a split.
///
/// This used to say those were "the two shapes a leak could arrive as". That
/// was a completeness claim, written once by the hand relying on it, and the
/// tree falsified it: a member mod's catalogue ids sat verbatim in a test
/// fixture, and `evidence/m5/review.md` had already written the counterexample
/// down. `docs/decisions/claims-without-a-second-reading.md` is the record that
/// forbids the shape, so what is not enforced is stated here instead.
///
/// **Not enforced, deliberately.** An absolute path naming a user's home
/// directory is not refused: this repository's redaction machinery and its
/// tests have to carry that shape to redact it, so the rule would need an
/// exclusion list, and an exclusion list is the hole the rule exists to not
/// have. An upstream identifier drawn from the private inventory is not
/// refused either: deciding one would mean holding the inventory here, which is
/// the thing the split forbids. Both were adjudicated: a public catalogue
/// identifier is public, and the private side sweeps this checkout for its own
/// identifiers on every run of its gate, which is where that question is
/// answered. `contents_scan_discloses_what_it_does_not_refuse` holds this
/// paragraph to the code, so adding either rule fails a test rather than
/// leaving the disclosure quietly false.
#[must_use]
pub fn verify_tracked_contents(root: &Path, paths: &[String]) -> Vec<String> {
    let private = private_workspace_name();
    let mut violations = Vec::new();
    for relative in paths {
        // Reading as UTF-8 and skipping what failed meant every non-UTF-8 tracked
        // file was exempt from the credential scan, which is the one file class a
        // key or a keystore would actually arrive as. Bytes are scanned lossily
        // instead, and a file that cannot be read at all is reported rather than
        // passed over. A symlink's tracked content is its target text, which is
        // what jj records and therefore what is scanned.
        let path = root.join(relative);
        let read = if path.is_symlink() {
            fs::read_link(&path).map(|target| target.to_string_lossy().into_owned())
        } else {
            fs::read(&path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        };
        let source = match read {
            Ok(source) => source,
            Err(error) => {
                violations.push(format!("cannot scan tracked file {relative}: {error}"));
                continue;
            }
        };

        let private_key = [
            "PRIVATE KEY-----",
            "RSA PRIVATE KEY-----",
            "OPENSSH PRIVATE KEY-----",
        ]
        .iter()
        .any(|suffix| source.contains(&format!("{}{}", "-----BEGIN ", suffix)));
        let credential = source.lines().any(has_credential_assignment);
        if private_key || credential {
            violations.push(format!(
                "tracked file contains credential material: {relative}"
            ));
        }

        if source.to_ascii_lowercase().contains(&private) {
            violations.push(format!(
                "tracked file names the private compatibility workspace: {relative}"
            ));
        }
        if relative.rsplit('/').next() == Some("Cargo.toml") {
            for value in escaping_path_dependencies(relative, &source) {
                violations.push(format!(
                    "tracked manifest depends on a path outside this repository: {relative} -> {value}"
                ));
            }
        }
    }
    violations
}

/// Every `path = "…"` value in a Cargo manifest that resolves outside the
/// repository root.
///
/// The workspace is full of legitimate `../` dependencies — one crate reaching
/// a sibling — so the rule cannot be "no `..`". It is where the path lands:
/// resolved lexically against the manifest's own directory, a dependency that
/// walks above the root reaches a tree this repository does not contain, and
/// the private workspace is a sibling directory exactly one such walk away.
fn escaping_path_dependencies<'a>(relative_manifest: &str, source: &'a str) -> Vec<&'a str> {
    let directory = relative_manifest
        .rsplit_once('/')
        .map_or("", |(head, _)| head);
    source
        .lines()
        .flat_map(manifest_path_values)
        .filter(|value| resolves_outside_root(directory, value))
        .collect()
}

/// Reads every `path = "…"` value out of one manifest line.
///
/// A dependency is written inline — `ravel-core = { path = "../ravel-core" }` —
/// so `quoted_value` cannot be reused: it anchors on the line starting with the
/// key and on the line ending with the closing quote, and neither holds here.
fn manifest_path_values(line: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find("path") {
        let at = cursor + offset;
        cursor = at + "path".len();
        // `rustc-dep-of-std` and `crate-path` are not this key.
        if line[..at]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        {
            continue;
        }
        let Some(rest) = line[cursor..].trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        values.push(&rest[..end]);
    }
    values
}

/// Whether a manifest-relative path escapes the repository root.
fn resolves_outside_root(directory: &str, value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    if normalized.starts_with('/') || normalized.chars().nth(1) == Some(':') {
        return true;
    }
    let mut stack = directory
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    for component in normalized.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if stack.pop().is_none() {
                    return true;
                }
            }
            other => stack.push(other),
        }
    }
    false
}

/// Every Rust crate root the unsafe prohibition is swept over.
///
/// The three name lists drive the enumerated part. A crate's own `examples/`
/// directory is a fourth kind of root and is read off the filesystem instead,
/// because it is a directory an author adds a file to rather than a list an
/// author edits: `crates/ravel-pack/examples/example_pack.rs` was the first such
/// target and sat outside every hand-maintained list until it was discovered
/// here.
fn maintained_rust_roots(root: &Path) -> Vec<String> {
    CRATE_NAMES
        .iter()
        .map(|name| format!("crates/{name}/src/lib.rs"))
        .chain([
            "tests/conformance/src/lib.rs".to_owned(),
            "tools/ravel-cli/src/lib.rs".to_owned(),
            "tools/ravel-cli/src/main.rs".to_owned(),
            "tools/ravel-xtask/src/lib.rs".to_owned(),
            "tools/ravel-xtask/src/main.rs".to_owned(),
        ])
        .chain(
            TEST_MOD_NAMES
                .iter()
                .map(|name| format!("test-mods/{name}/src/lib.rs")),
        )
        .chain(
            EXAMPLE_NAMES
                .iter()
                .map(|name| format!("examples/{name}/src/lib.rs")),
        )
        .chain(crate_example_roots(root))
        .collect()
}

/// Returns every `crates/<crate>/examples/*.rs` target, in path order.
fn crate_example_roots(root: &Path) -> Vec<String> {
    let mut roots = Vec::new();
    for name in CRATE_NAMES {
        let directory = root.join("crates").join(name).join("examples");
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                let file = entry.file_name().to_string_lossy().into_owned();
                roots.push(format!("crates/{name}/examples/{file}"));
            }
        }
    }
    roots.sort();
    roots
}

fn verify_ignored_boundaries(root: &Path, violations: &mut Vec<String>) {
    let gitignore_path = root.join(".gitignore");
    let Ok(gitignore) = fs::read_to_string(&gitignore_path) else {
        violations.push("cannot read .gitignore".to_owned());
        return;
    };

    for pattern in ["/tests/fixtures/local/**", "/evidence/private/**", ".env"] {
        if !gitignore.lines().any(|line| line.trim() == pattern) {
            violations.push(format!("missing restricted-content ignore: {pattern}"));
        }
    }
}

/// The metrics `evidence/m2/loader-overhead.json`'s own expiry clause voided,
/// and why each one is not compared.
///
/// The record says its three build limits "describe a linked pack" and that if
/// M3 selected anything but linking they "describe a shape that no longer exists
/// and are remeasured rather than carried". ADG-2 selected source compiled on the
/// player's machine, so the condition fired exactly as written and `ROADMAP.md`
/// records it.
///
/// They are listed here rather than removed from the record because a recorded
/// measurement is not edited after the fact — the clause inside it is what does
/// the voiding. They are listed at all, rather than skipped, because a limit that
/// is quietly not compared is indistinguishable from one that passed, and that is
/// how three breaches of 18x and 29x sat in this repository unremarked.
const VOID_LIMITS: &[(&str, &str)] = &[
    (
        "build.packCompile",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; the build cost of source distribution is evidence/m3/pack-compile.json",
    ),
    (
        "build.relink",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; nothing relinks a pack under the selected mechanism",
    ),
    (
        "build.binaryGrowthPerMod",
        "voided by the record's own expiry clause when ADG-2 chose source distribution over \
         linking; per-mod binary growth describes an artefact the mechanism does not produce",
    ),
];

/// The budget protocol this comparison understands.
///
/// A record declaring a different protocol is refused rather than compared. Two
/// records can carry the same metric names and mean different things by them, and
/// a budget that silently compared across protocols would be the same failure this
/// mechanism exists to end.
const BUDGET_PROTOCOL: &str = "m2-loader-budget-v1";

/// The records whose limits every later run is compared against.
///
/// The budget is their union rather than any one of them, because a limit is
/// set by whichever measurement was addressed to it: M2 bounded what the loader
/// costs, and M3-W6 bounds what profiling costs a tick when it is off. Two
/// records may not state the same metric — a metric with two ceilings has none —
/// and that is checked rather than resolved.
const BUDGET_RECORDS: &[&str] = &[
    "evidence/m2/loader-overhead.json",
    "evidence/m3/profiling-overhead.json",
];

/// What comparing a run against the regression budget found.
///
/// Violations and notes are separate because they answer different questions. A
/// violation stops a milestone; a note says what was *not* checked, which is the
/// half that has no other way to be seen. `budget.enforcement` in the record
/// records that "comparing a later run against them is M3's to build, and until it
/// does the rule above is a sentence rather than a mechanism" — notes are what
/// keep the mechanism from quietly becoming a sentence again.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BudgetReport {
    /// Limits the candidate run exceeded, or could not be compared against.
    pub violations: Vec<String>,
    /// What was deliberately not compared, and why.
    pub notes: Vec<String>,
}

/// One limit as a record states it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Limit {
    metric: String,
    /// The value a measurement must not exceed, as the record spells it.
    ceiling: String,
    measured: String,
    operator: String,
}

/// Compares the `candidates`' measurements against the recorded regression
/// budget.
///
/// The join key is the metric name, and both sides come from the same
/// `budget.limits` shape: a re-run writes a new record carrying its own measured
/// values, so nothing here needs a table mapping a metric to a field. That
/// matters more than it looks — a hand-written path table is a second description
/// of the record that can drift from it, and the drift would show up as a limit
/// silently not compared.
///
/// The candidates are taken together rather than one at a time. The budget spans
/// more than one measurement now, and no single run measures all of it: asking
/// each run for every limit would fail a loader run for not having profiled and
/// a profiling run for not having loaded. A limit is satisfied when some run
/// measured it and came in under it, and no limit may go unmeasured.
///
/// The hardware profile has to match. Each record's own restatement says its
/// limits "hold on linux-reference-v1 only" and that "a budget without the
/// machine it was measured on is not a budget", so a run from another machine is
/// refused rather than compared and passed.
#[must_use]
pub fn verify_budget(root: &Path, candidates: &[&Path]) -> BudgetReport {
    let mut report = BudgetReport::default();
    if candidates.is_empty() {
        report.violations.push(
            "no candidate run was given, and a budget compared against nothing passes \
                   vacuously"
                .to_owned(),
        );
        return report;
    }

    let owned = BUDGET_RECORDS
        .iter()
        .map(|record| root.join(record))
        .collect::<Vec<_>>();
    let baselines = owned
        .iter()
        .map(std::path::PathBuf::as_path)
        .collect::<Vec<_>>();

    for (path, label) in baselines
        .iter()
        .map(|path| (*path, "a budget record"))
        .chain(candidates.iter().map(|path| (*path, "a candidate run")))
    {
        match jq_value(path, ".budget.protocol") {
            Ok(protocol) if protocol == BUDGET_PROTOCOL => {}
            Ok(protocol) => report.violations.push(format!(
                "{label}, {}, declares budget protocol `{protocol}`, and this comparison \
                 understands only `{BUDGET_PROTOCOL}`",
                redacted_path(path)
            )),
            Err(error) => report.violations.push(error),
        }
    }
    if !report.violations.is_empty() {
        return report;
    }

    let required = match hardware_profile(&baselines) {
        Ok(profile) => profile,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    for candidate in candidates {
        match jq_value(candidate, ".identity.hardwareProfile") {
            Ok(actual) if actual == required => {}
            Ok(actual) => report.violations.push(format!(
                "the budget holds on hardware profile `{required}` and {} was measured on \
                 `{actual}`, so it is remeasured rather than compared",
                redacted_path(candidate)
            )),
            Err(error) => report.violations.push(error),
        }
    }
    if !report.violations.is_empty() {
        return report;
    }

    let baseline_limits = match union(&baselines, "budget records") {
        Ok(limits) => limits,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };
    let candidate_limits = match union(candidates, "candidate runs") {
        Ok(limits) => limits,
        Err(error) => {
            report.violations.push(error);
            return report;
        }
    };

    for limit in &baseline_limits {
        if let Some((_, reason)) = VOID_LIMITS
            .iter()
            .find(|(metric, _)| *metric == limit.metric)
        {
            report
                .notes
                .push(format!("`{}` is not compared: {reason}", limit.metric));
            continue;
        }
        compare(limit, &candidate_limits, &mut report.violations);
    }

    for measured in &candidate_limits {
        if !baseline_limits
            .iter()
            .any(|limit| limit.metric == measured.metric)
        {
            report.notes.push(format!(
                "`{}` is measured by the candidate run and the budget sets no limit for it, so \
                 nothing here can pass or fail it",
                measured.metric
            ));
        }
    }

    report
}

/// Compares one live limit against whatever the candidate measured for it.
///
/// The ceiling and the comparison both come from the budget, and only the
/// measurement comes from the candidate. A run that could restate the operator or
/// the limit it is judged by would be grading itself, which is a softer version of
/// the same silence this whole check exists to end.
fn compare(limit: &Limit, candidate: &[Limit], violations: &mut Vec<String>) {
    let Some(measured) = candidate.iter().find(|entry| entry.metric == limit.metric) else {
        violations.push(format!(
            "the budget limits `{}` and the candidate run does not measure it, so the limit \
             cannot be said to hold",
            limit.metric
        ));
        return;
    };

    // An operator this comparison does not implement is refused rather than
    // assumed. Guessing that an unknown comparison meant "at most" is how a
    // budget starts passing things it was written to stop.
    if limit.operator != "less-than-or-equal" {
        violations.push(format!(
            "`{}` uses comparison `{}`, which this check does not implement",
            limit.metric, limit.operator
        ));
        return;
    }

    match (
        limit.ceiling.parse::<f64>(),
        measured.measured.parse::<f64>(),
    ) {
        (Ok(ceiling), Ok(value)) if value <= ceiling => {}
        (Ok(ceiling), Ok(value)) => violations.push(format!(
            "`{}` measured {value} against a limit of {ceiling}, which is a regression",
            limit.metric
        )),
        _ => violations.push(format!(
            "`{}` has a limit or a measurement that is not a number",
            limit.metric
        )),
    }
}

/// The one hardware profile every budget record holds on.
///
/// Two budget records that hold on different machines are not one budget, and a
/// candidate cannot be on both. Refusing that is cheaper than deciding which
/// half of an incoherent budget a run should be judged by.
fn hardware_profile(baselines: &[&Path]) -> Result<String, String> {
    let mut required: Option<String> = None;
    for baseline in baselines {
        let profile = jq_value(baseline, ".budget.hardwareProfile")?;
        match &required {
            None => required = Some(profile),
            Some(first) if *first == profile => {}
            Some(first) => {
                return Err(format!(
                    "the budget records hold on different hardware profiles, `{first}` and \
                     `{profile}`, so together they are not a budget; {} is the second",
                    redacted_path(baseline)
                ));
            }
        }
    }
    required.ok_or_else(|| "no budget record was named".to_owned())
}

/// Reads every limit a set of records states, refusing a metric two of them
/// state.
///
/// A metric with two ceilings has none: which one applies would be decided by
/// the order the files happened to be read in. Refusing is the only answer that
/// does not make the budget depend on that.
fn union(paths: &[&Path], role: &str) -> Result<Vec<Limit>, String> {
    let mut all: Vec<Limit> = Vec::new();
    for path in paths {
        for limit in limits(path)? {
            if all.iter().any(|seen| seen.metric == limit.metric) {
                return Err(format!(
                    "`{}` is stated by two {role}, so it has two ceilings and therefore none; the \
                     second is in {}",
                    limit.metric,
                    redacted_path(path)
                ));
            }
            all.push(limit);
        }
    }
    Ok(all)
}

/// Reads every limit a record states, in the order it states them.
fn limits(path: &Path) -> Result<Vec<Limit>, String> {
    let rows = jq_value(
        path,
        r#".budget.limits[] | "\(.metric)\t\(.limit)\t\(.measured)\t\(.operator)""#,
    )?;
    if rows.is_empty() {
        return Err(format!("{} states no budget limits", redacted_path(path)));
    }

    rows.lines()
        .map(|row| {
            let mut fields = row.split('\t');
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some(metric), Some(ceiling), Some(measured), Some(operator)) => Ok(Limit {
                    metric: metric.to_owned(),
                    ceiling: ceiling.to_owned(),
                    measured: measured.to_owned(),
                    operator: operator.to_owned(),
                }),
                _ => Err(format!(
                    "{} states a limit that is not complete",
                    redacted_path(path)
                )),
            }
        })
        .collect()
}

fn verify_schema_documents(root: &Path, violations: &mut Vec<String>) {
    for schema in [
        "configuration",
        "configuration-override",
        "evidence",
        "fixture",
        "pack",
        "package",
        "performance-collector",
        "performance-result",
        "provenance",
        "runtime-inputs",
        "saved-data",
        "spike-result",
    ] {
        let relative = format!("schemas/{schema}.schema.json");
        let path = root.join(&relative);
        match jq_matches(
            &path,
            "type == \"object\" and .\"$schema\" == \"https://json-schema.org/draft/2020-12/schema\"",
        ) {
            Ok(true) => {}
            Ok(false) => violations.push(format!("invalid JSON schema document: {relative}")),
            Err(error) => violations.push(error),
        }
    }
}

/// Holds `EXAMPLE_NAMES` to what `examples/` actually contains.
///
/// `required_scaffold_paths` reads the list forwards, so a name in it with no
/// directory already fails. Nothing read it backwards, and the fences, the
/// unsafe-prohibition sweep, and the maintained Rust roots are all driven from
/// the list — so an example added without editing it would sit outside every one
/// of them.
fn verify_example_inventory(root: &Path, violations: &mut Vec<String>) {
    verify_inventory(root, "examples", EXAMPLE_NAMES, "EXAMPLE_NAMES", violations);
}

/// The same, for `test-mods/`, which had no backwards reading at all.
///
/// `CLAUDE.md` tells an author adding a test mod to edit `TEST_MOD_NAMES`, and
/// until this existed nothing checked that they had: the forwards reading fails
/// on a listed name with no directory, and an unlisted directory passed in
/// silence, outside the unsafe-prohibition sweep and the maintained Rust roots
/// exactly as an unlisted example would have been. Found while landing
/// `test-mods/waystone-ledger` by deleting its entry and watching the gate stay
/// green.
fn verify_test_mod_inventory(root: &Path, violations: &mut Vec<String>) {
    verify_inventory(
        root,
        "test-mods",
        TEST_MOD_NAMES,
        "TEST_MOD_NAMES",
        violations,
    );
}

/// Reports every crate directory under `directory` that `listed` does not name.
fn verify_inventory(
    root: &Path,
    directory: &str,
    listed: &[&str],
    list: &str,
    violations: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(root.join(directory)) else {
        violations.push(format!("cannot read {directory}"));
        return;
    };

    for entry in entries.flatten() {
        if !entry.path().join("Cargo.toml").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !listed.contains(&name.as_str()) {
            violations.push(format!("{directory}/{name} is not listed in {list}"));
        }
    }
}

/// What the version-boundary sweep reads, said in every violation it reports.
///
/// `crates/` is what "general crate" means and what
/// `docs/architecture/dependency-rules.md` states the rule over. `tests/` and
/// `test-mods/` are excluded on purpose — a conformance test or a test mod may
/// legitimately pin the version it was written against — and saying so in the
/// failure text is what keeps the exclusion a decision rather than a hole
/// nobody noticed.
const SWEPT_SCOPE: &str =
    "the sweep reads crates/ only; tests/ and test-mods/ are excluded by design";

/// Game class names a file under `crates/` may spell, each with its reason.
///
/// Silence is not tolerance here: every occurrence the sweep finds is either
/// corrected — the adapter's and `ravel-minecraft`'s invented fixtures were,
/// which is why they are not listed — or named below with a reason someone can
/// disagree with. Every entry is one file and one class, and an entry that stops
/// matching is reported as stale, so an excuse cannot outlive its occurrence and
/// a file's excuse does not cover the next class added to it.
///
/// All three entries are the same shape: a test that reads the pinned version's
/// own data and asserts on what that data says. Such a test cannot be written
/// against invented names, because the names are the thing under test.
const VERSION_CLASS_EXCUSES: [(&str, &[&str], &str); 3] = [
    (
        "crates/ravel-minecraft-adapter/src/generate/mod.rs",
        &["net/minecraft/client/gui/screens/Screen"],
        "generates from the tracked pinned inputs and asserts on the stub tree \
         they produce, so it names what they name",
    ),
    (
        "crates/ravel-minecraft-adapter/src/generate/resolve.rs",
        &["net/minecraft/util/StringUtil"],
        "resolves the pinned inventory's first hook and asserts the class that \
         inventory names it against",
    ),
    (
        "crates/ravel-minecraft-adapter/src/generate/stubs.rs",
        &[
            "net/minecraft/client/gui/components/AbstractSliderButton",
            "net/minecraft/client/gui/components/AbstractWidget",
            "net/minecraft/client/gui/components/Button",
            "net/minecraft/client/gui/components/EditBox",
            "net/minecraft/client/gui/components/events/GuiEventListener",
            "net/minecraft/client/gui/screens/Screen",
            "net/minecraft/network/chat/Component",
            "net/minecraft/network/chat/FormattedText",
        ],
        "asserts which supertype edges the pinned jar's own stub set keeps and \
         which it drops, and an edge is a fact about those classes",
    ),
];

fn verify_version_boundary(root: &Path, violations: &mut Vec<String>) {
    let crates = root.join("crates");
    if let Ok(entries) = fs::read_dir(&crates) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(suffix) = name.strip_prefix("ravel-minecraft-")
                && suffix != "adapter"
            {
                violations.push(format!("unexpected per-version Minecraft crate: {name}"));
            }
        }
    }

    // Both version directories, the pinned one and M4-W6's synthetic second one.
    // A crate that names either is welded to a version, and the synthetic one is
    // the half that catches the mistake nobody makes on purpose: a general crate
    // written against whichever version the author happened to have.
    let versions = ["minecraft/versions", "tests/fixtures/public/versions"]
        .into_iter()
        .flat_map(|directory| fs::read_dir(root.join(directory)).into_iter().flatten())
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let classes = version_class_names(root);
    let excuses = VERSION_CLASS_EXCUSES
        .iter()
        .flat_map(|(file, excused, _)| excused.iter().map(move |class| (*file, *class)))
        .collect::<Vec<_>>();
    let mut excuse_matched = vec![false; excuses.len()];

    let mut files = Vec::new();
    collect_files(&root.join("crates"), &mut files);
    for path in files {
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml")
        ) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if versions.iter().any(|version| source.contains(version)) {
            violations.push(format!(
                "hardcoded Minecraft version outside version data ({SWEPT_SCOPE}): {relative}"
            ));
        }

        for class in &classes {
            let dotted = class.replace('/', ".");
            if !source.contains(class.as_str()) && !source.contains(&dotted) {
                continue;
            }
            if let Some(index) = excuses
                .iter()
                .position(|(file, excused)| *file == relative && *excused == class.as_str())
            {
                excuse_matched[index] = true;
                continue;
            }
            violations.push(format!(
                "game class name from version data outside version data ({SWEPT_SCOPE}): \
                 {relative} names {class}"
            ));
        }
    }

    for (index, matched) in excuse_matched.iter().enumerate() {
        if !matched {
            let (file, class) = excuses[index];
            violations.push(format!(
                "stale version-class excuse: {file} no longer names {class}, \
                 so the excuse is unearned and has to go"
            ));
        }
    }
}

/// Every fully-qualified game class name a version directory supplies, as an
/// internal name with slashes.
///
/// The names come out of the version directory's own data: each `class` entry in
/// `hooks.toml`, `stubs.toml`, `mappings.lock` and `reflection.toml`, plus every
/// class a descriptor in those files refers to, kept only when it sits under a
/// package root a `class` entry declared. The reflected set is swept for the same
/// reason as the other three and only became sweepable when it stopped being a
/// Rust constant: a general crate spelling `net.minecraft.resources.Identifier`
/// is welded to 26.1.2 whether the generator reads that file or not. That last filter is what keeps `java/lang/String` out
/// of a set that is about the game rather than the platform.
///
/// Member names are deliberately not swept. `width`, `tick` and `flush` are
/// ordinary English, and a sweep that fires on them would be turned off within a
/// week. A class name is long, unambiguous, and the thing a general crate has no
/// business spelling.
fn version_class_names(root: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    for directory in ["minecraft/versions", "tests/fixtures/public/versions"] {
        let Ok(entries) = fs::read_dir(root.join(directory)) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            for file in ["hooks.toml", "stubs.toml", "mappings.lock", REFLECTION_FILE] {
                if let Ok(text) = fs::read_to_string(entry.path().join(file)) {
                    texts.push(text);
                }
            }
        }
    }

    let mut classes = BTreeSet::new();
    for text in &texts {
        for line in text.lines() {
            if let Some(value) = line.trim().strip_prefix("class = \"")
                && let Some(value) = value.strip_suffix('"')
            {
                classes.insert(value.replace('.', "/"));
            }
        }
    }

    let roots = classes
        .iter()
        .filter_map(|class| {
            let mut segments = class.split('/');
            Some(format!("{}/{}/", segments.next()?, segments.next()?))
        })
        .collect::<BTreeSet<_>>();

    for text in &texts {
        for reference in descriptor_classes(text) {
            if roots.iter().any(|prefix| reference.starts_with(prefix)) {
                classes.insert(reference);
            }
        }
    }

    classes.into_iter().collect()
}

/// Every class a descriptor in `text` refers to, as an internal name.
///
/// A descriptor spells an object type `Lnet/minecraft/util/StringUtil;`, so this
/// reads each `L`-to-`;` run of descriptor-legal characters. A run without a
/// slash is not a package-qualified class and is dropped, which is what keeps
/// the `L` in ordinary prose from being read as a type.
fn descriptor_classes(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    for (start, _) in text.match_indices('L') {
        let name = start + 1;
        let mut end = name;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'/' | b'$' | b'_'))
        {
            end += 1;
        }
        if bytes.get(end) == Some(&b';') && text[name..end].contains('/') {
            found.push(text[name..end].to_owned());
        }
    }
    found
}

fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    if root.join(".jj").exists() {
        let output = Command::new("jj")
            .args(["file", "list"])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot list tracked files with jj: {error}"))?;
        if !output.status.success() {
            return Err("cannot list tracked files with jj".to_owned());
        }
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect());
    }

    let mut files = Vec::new();
    collect_files(root, &mut files);
    Ok(files
        .iter()
        .filter_map(|path| path.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect())
}

fn collect_files(root: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            if !matches!(
                name.to_str(),
                Some(".git" | ".jj" | ".devenv" | ".direnv" | "target")
            ) {
                collect_files(&path, files);
            }
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn is_forbidden_tracked_path(path: &str) -> bool {
    if path == "tests/fixtures/local/README.md" {
        return false;
    }

    let lower = path.to_ascii_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    let components = lower.split('/').collect::<Vec<_>>();
    let forbidden_prefix = [
        "tests/fixtures/local/",
        "evidence/private/",
        "private/",
        "restricted/",
        ".minecraft/",
        "prism-instances/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let forbidden_component = components
        .iter()
        .any(|component| matches!(*component, "saves" | "logs" | "crash-reports"));
    let forbidden_evidence_component = lower.starts_with("evidence/")
        && components
            .iter()
            .any(|component| matches!(*component, "raw" | "captures"));
    let forbidden_basename = matches!(
        basename,
        ".env"
            | "credentials.json"
            | "secrets.json"
            | "fabric.mod.json"
            | "quilt.mod.json"
            | "neoforge.mods.toml"
            | "mods.toml"
    ) || (basename.starts_with(".env.") && basename != ".env.example")
        || basename.starts_with("hs_err_pid");
    let forbidden_extension = [
        ".jar",
        ".mrpack",
        ".pem",
        ".p12",
        ".pfx",
        ".key",
        ".keystore",
        ".jfr",
        ".hprof",
        ".trace",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension));

    forbidden_prefix
        || forbidden_component
        || forbidden_evidence_component
        || forbidden_basename
        || forbidden_extension
}

fn is_valid_milestone_id(milestone: &str) -> bool {
    matches!(
        milestone,
        "M0" | "M1"
            | "M2"
            | "M3"
            | "M4"
            | "M5"
            | "M6"
            | "M7"
            | "M8"
            | "M9"
            | "M10"
            | "M11"
            | "M12"
            | "M13"
    )
}

/// Reads one scalar out of a JSON file, for claims that are compared rather than
/// asserted.
fn jq_value(path: &Path, filter: &str) -> Result<String, String> {
    let output = Command::new("jq")
        .args(["-r", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot read JSON with jq: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot read {filter} from {}", redacted_path(path)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Names a path the way gate output has to name one.
///
/// `verify_milestone` already refuses to report a filesystem error against an
/// absolute path, and the repository redacts local paths from everything it
/// records — but `jq_value` printed `root.join(..)` straight into a violation, so
/// the same gate leaked what the same gate elsewhere took care to hide. This makes
/// the two agree.
fn redacted_path(path: &Path) -> String {
    if path.is_relative() {
        return path.display().to_string();
    }
    std::env::current_dir()
        .ok()
        .and_then(|directory| path.strip_prefix(directory).ok().map(Path::to_path_buf))
        .map_or_else(
            || {
                path.file_name().map_or_else(
                    || "an unnamed path".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                )
            },
            |relative| relative.display().to_string(),
        )
}

/// Answers one yes-or-no question about a JSON document, or refuses to answer.
///
/// Any nonzero jq status used to mean "no", which put a parse error and a false
/// answer in the same box: an unparseable manifest made every check phrased as a
/// refusal quietly succeed, and the gate had no way to say it had failed to look.
/// jq exits 1 when the filter's last output was false or null and 4 when it
/// produced none — those are answers. Everything else, including the 5 an
/// unparseable document produces, is jq failing to run at all.
fn jq_matches(path: &Path, filter: &str) -> Result<bool, String> {
    let output = Command::new("jq")
        .args(["--exit-status", filter])
        .arg(path)
        .output()
        .map_err(|error| format!("cannot validate JSON with jq: {error}"))?;
    if output.status.success() {
        return Ok(true);
    }
    match output.status.code() {
        Some(1 | 4) => Ok(false),
        Some(code) => Err(format!(
            "cannot evaluate {filter} against {}: jq exited {code}",
            redacted_path(path)
        )),
        None => Err(format!(
            "cannot evaluate {filter} against {}: jq was terminated by a signal",
            redacted_path(path)
        )),
    }
}

fn evidence_schema_matches(root: &Path, manifest: &Path) -> Result<bool, String> {
    let output = Command::new("check-jsonschema")
        .arg("--schemafile")
        .arg(root.join("schemas/evidence.schema.json"))
        .arg(manifest)
        .output()
        .map_err(|error| format!("cannot validate evidence schema: {error}"))?;
    Ok(output.status.success())
}

fn has_credential_assignment(line: &str) -> bool {
    let Some((name, value)) = line.split_once('=') else {
        return false;
    };
    if !matches!(
        name.trim(),
        "ATP_APP_PASSWORD" | "AWS_SECRET_ACCESS_KEY" | "MINECRAFT_ACCESS_TOKEN" | "TANGLED_TOKEN"
    ) {
        return false;
    }

    let value = value.trim().trim_matches(['\'', '"']);
    !value.is_empty()
        && !value.contains('<')
        && !value.eq_ignore_ascii_case("example")
        && !value.eq_ignore_ascii_case("placeholder")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        AGENT_JAR_SOURCES, CRATE_NAMES, CampaignInventory, DECISION_RECORDS, EXAMPLE_NAMES,
        GATE_DEFINITION, GUIDE_DIRECTORY, HOOK_DOCUMENT, HookCounts, JVM_ENTRY_DECISION,
        MILESTONE_MANIFESTS, NETWORK_SOURCE, REFLECTION_FILE, SCREEN_JAR_SOURCES, SWEPT_SCOPE,
        TEST_MOD_NAMES, TRANSFORMER_SHAPE_RECORD, VERSION_CLASS_EXCUSES, VERSION_DATA,
        VERSION_FILES, VERSION_PROSE_FILE, campaign_document_violations, change_mismatch,
        declared_hook_names, declares_unsafe_prohibition, denied_hook_names, documented_versions,
        game_class_literals, hook_counts, hook_document_violations, hook_inventories,
        numbers_before, pinned_commit_mismatch, redacted_path, reflective_surfaces,
        required_scaffold_paths, round_count_restatements, verify_campaign_inventory,
        verify_decision_hook_claims, verify_evidence_records, verify_experiment_fence,
        verify_guide_enumerations, verify_harness_lifecycles, verify_hook_inventory_document,
        verify_hook_name_literals, verify_ignored_boundaries, verify_m4_gate_figures,
        verify_m4_process_matrix, verify_m4_surface, verify_manifest_percentages, verify_milestone,
        verify_persistence_reflection, verify_resolved_class_figure, verify_review_round_counts,
        verify_scaffold, verify_screen_templates, verify_spikes, verify_testkit_fence,
        verify_tracked_contents, verify_tracked_paths, verify_transformer_shape,
        verify_version_boundary, verify_version_directories, verify_void_limits,
        version_class_names, visible_commit,
    };
    use super::{
        M6_ARITHMETIC, M6_CARRIED_FIGURES, M6_DECISIONS, M7_ARITHMETIC, M7_CARRIED_FIGURES,
        verify_m6_carried_figures, verify_m6_counterpart_arithmetic, verify_m6_manifest,
        verify_m7_carried_figures, verify_m7_library_arithmetic, verify_m7_manifest,
        verify_m7_verified_fixture_total,
    };
    use super::{
        ROUND_COUNT_EXEMPT, defined_rust_symbols, distinguishable_symbol, named_rust_symbols,
        round_count_subjects, sources_naming, verify_m5_carried_figures,
        verify_m5_inventory_arithmetic,
    };

    /// A passing gate run, which every fixture below records.
    ///
    /// A manifest carrying neither `commands` nor `tests` is now a violation of
    /// its own, because both being absent used to clear the run check in silence.
    /// The fixtures below are about digests, so they record a run that agrees with
    /// itself and leave the run rules to the tests written for them.
    const RECORDED_RUN: &str = r#""commands":[{"exitStatus":0}],
        "tests":{"total":9,"passed":9,"failed":0,"skipped":0,"lineCoveragePercent":85.97}"#;

    /// Writes a milestone manifest recording one result and returns the root.
    fn recorded_result_root(label: &str, result: &str, digest: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("result.json"), result).expect("result should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"benchmarks":{{"protocolVersion":"v1",
                    "results":[{{"name":"result.json","sha256":"{digest}"}}]}}}}"#
            ),
        )
        .expect("manifest should be writable");
        root
    }

    /// The digest of `{}\n`, which is what the tests below record.
    const EMPTY_OBJECT_DIGEST: &str =
        "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356";

    #[test]
    fn a_recorded_result_digest_is_recomputed_against_the_file() {
        let root = recorded_result_root("records-match", "{}\n", EMPTY_OBJECT_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_result_digest_that_binds_nothing_is_reported() {
        // The defect this exists for: a manifest validated against its schema
        // while four of its digests named content no tracked file ever held.
        let root = recorded_result_root("records-drift", "{\"changed\": true}\n", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json records result.json as ")
                && violations[0].contains(" but it hashes "),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_manifest_cannot_record_its_own_digest_or_a_result_it_does_not_hold() {
        let root = temporary_root("records-self");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"benchmarks":{{"protocolVersion":"v1","results":[
                    {{"name":"manifest.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    {{"name":"../m8/borrowed.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    {{"name":"absent.json","sha256":"{EMPTY_OBJECT_DIGEST}"}}]}}}}"#
            ),
        )
        .expect("manifest should be writable");

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json records its own digest, which cannot bind".to_owned(),
                "evidence/m9/manifest.json records a result outside its own milestone: \
                 ../m8/borrowed.json"
                    .to_owned(),
                "evidence/m9/manifest.json records a result that the repository does not \
                 track, so its digest binds nothing a reader can obtain: absent.json"
                    .to_owned(),
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A review record stating the verdict the manifests below claim.
    const PASS_RECORD: &str = "Result: PASS\n";

    /// The digest of [`PASS_RECORD`].
    const PASS_RECORD_DIGEST: &str =
        "d2bcd3b6eb96afdab9d8fa0b8333538ff07cfd0764afc5a3854976209bef21e2";

    /// Writes a manifest recording one review record and returns the root.
    fn recorded_review_root(label: &str, record: &str, name: &str, digest: &str) -> PathBuf {
        reviewed_root(label, "pass", record, name, digest)
    }

    /// Writes a manifest recording one review verdict and its record.
    fn reviewed_root(label: &str, result: &str, record: &str, name: &str, digest: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("review.md"), record).expect("record should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"review":{{"reviewer":"an independent reviewer",
                    "independent":true,
                    "result":"{result}","record":{{"name":"{name}","sha256":"{digest}"}}}}}}"#
            ),
        )
        .expect("manifest should be writable");
        root
    }

    #[test]
    fn a_review_record_stating_another_verdict_than_the_manifest_is_reported() {
        // The defect this exists for, in both directions: `review.result` was set
        // to pass while the bound record read `Result: FAIL`, and the milestone
        // gate closed on the field without reading the document it names.
        for (result, record, digest) in [
            (
                "pass",
                "Result: FAIL\n",
                "5fcc61bd66ab279caaa7ec4d95d65cc532deee32d8a74ce93a04cb0cdd4ded44",
            ),
            ("fail", PASS_RECORD, PASS_RECORD_DIGEST),
        ] {
            let root = reviewed_root("review-verdict", result, record, "review.md", digest);

            let violations = verify_evidence_records(&root);

            let stated = if result == "pass" { "fail" } else { "pass" };
            assert_eq!(
                violations,
                [format!(
                    "evidence/m9/manifest.json records review.result {result} while the review \
                     record it binds states {stated}"
                )],
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn a_review_record_stating_no_verdict_is_reported() {
        let root = recorded_review_root(
            "review-silent",
            "a record with no verdict\n",
            "review.md",
            "2f3630fd670787672c297ef4a9f7a4acb23eaa18c86ab35bc8940ca3363bde62",
        );

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json binds a review record that states no verdict, so \
                 nothing checks that review.result pass is what the review concluded"
                    .to_owned()
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_review_digest_is_recomputed_against_the_tracked_record() {
        let root =
            recorded_review_root("review-match", PASS_RECORD, "review.md", PASS_RECORD_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_review_digest_that_binds_nothing_is_reported() {
        let root = recorded_review_root("review-drift", PASS_RECORD, "review.md", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json records review.md as ")
                && violations[0].ends_with(&format!(" but it hashes {PASS_RECORD_DIGEST}")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_review_record_named_by_title_is_reported() {
        // The defect this exists for: three manifests named the review by
        // title, so a recorded sha256 read as proof of a review while binding
        // no artifact a reader could obtain.
        let root = recorded_review_root(
            "review-title",
            PASS_RECORD,
            "M9 independent closure review",
            PASS_RECORD_DIGEST,
        );

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json records a review record by title rather than by \
                 path, so its digest binds nothing obtainable: M9 independent closure review"
                    .to_owned()
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_manifest_recording_no_review_record_is_left_to_the_milestone_gate() {
        // A milestone under construction has no review yet, and the milestone
        // gate is what refuses to close on that.
        let root = temporary_root("review-absent");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"review":{{"reviewer":"an independent reviewer",
                    "independent":true,"result":"fail"}}}}"#
            ),
        )
        .expect("manifest should be writable");

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes a manifest recording one baseline result and one private
    /// validation, and returns the root.
    fn recorded_baseline_root(label: &str, result: &str, digest: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("launch.json"), result).expect("record should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"platformEvidence":[{{"baselines":[
                    {{"workload":"vanilla",
                    "result":{{"name":"launch.json","sha256":"{digest}"}},
                    "validation":{{"name":"private launch validation",
                        "sha256":"{}"}}}}]}}]}}"#,
                "0".repeat(64)
            ),
        )
        .expect("manifest should be writable");
        root
    }

    #[test]
    fn a_recorded_baseline_digest_is_recomputed_against_the_tracked_file() {
        // The private validation digest names a capture outside the repository
        // and is announced rather than checked, so it cannot be a violation.
        let root = recorded_baseline_root("baseline-match", "{}\n", EMPTY_OBJECT_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_baseline_digest_that_binds_nothing_is_reported() {
        // The hole this exists for: two M0 baseline digests naming tracked
        // evidence files that no gate recomputed.
        let root = recorded_baseline_root("baseline-drift", "{}\n", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json records launch.json as ")
                && violations[0].ends_with(&format!(" but it hashes {EMPTY_OBJECT_DIGEST}")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_baseline_naming_a_file_the_milestone_does_not_hold_is_reported() {
        let root = temporary_root("baseline-absent");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"platformEvidence":[{{"baselines":[
                    {{"workload":"vanilla",
                    "result":{{"name":"absent.json","sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    "validation":{{"name":"a private launch validation",
                        "sha256":"{EMPTY_OBJECT_DIGEST}"}}}}]}}]}}"#
            ),
        )
        .expect("manifest should be writable");

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json records a baseline that the repository does not track, \
              so its digest binds nothing a reader can obtain: absent.json"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A jj repository with nothing in it yet, for the pin tests below.
    fn pinned_record_root(label: &str) -> PathBuf {
        let root = temporary_root(label);
        let initialised = Command::new("jj")
            .args(["git", "init"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(initialised.status.success(), "{initialised:?}");
        root
    }

    /// Writes one admitted record carrying `pin`, with its digest bound.
    ///
    /// `revision.id` is here because a manifest with no revision at all is a
    /// violation of the input rule; it resolves nothing, because the manifest
    /// records no inputs to hash there. What these tests are about is the pin
    /// inside the record.
    fn write_admitted(root: &Path, pin: &str, disclosures: &str) {
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        let record = format!("{{\"trial\":{{\"revision\":{pin}}}}}\n");
        fs::write(milestone.join("record.json"), &record).expect("record should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"revision":{{"id":"@"}},{disclosures}
                    "benchmarks":{{"protocolVersion":"v1",
                    "results":[{{"name":"record.json","sha256":"{}"}}]}}}}"#,
                super::hex_digest(record.as_bytes())
            ),
        )
        .expect("manifest should be writable");
    }

    /// Commits everything and answers the commit and change that now hold it.
    ///
    /// A described commit is what a pin can name truthfully: editing the working
    /// copy afterwards moves `@` and leaves this one where it is.
    fn committed_pin(root: &Path, message: &str) -> (String, String) {
        let committed = Command::new("jj")
            .args(["commit", "-m", message])
            .current_dir(root)
            .output()
            .expect("jj should be available");
        assert!(committed.status.success(), "{committed:?}");
        let output = Command::new("jj")
            .args([
                "log",
                "-r",
                "@-",
                "--no-graph",
                "-T",
                r#"commit_id ++ " " ++ change_id"#,
            ])
            .current_dir(root)
            .output()
            .expect("jj should be available");
        assert!(output.status.success(), "{output:?}");
        let resolved = String::from_utf8_lossy(&output.stdout);
        let mut fields = resolved.split_whitespace();
        (
            fields.next().expect("a commit id").to_owned(),
            fields.next().expect("a change id").to_owned(),
        )
    }

    /// A pin naming one commit and one change, as a record records it.
    fn pin(id: &str, change: &str) -> String {
        format!(r#"{{"id":"{id}","changeId":"{change}","dirty":false}}"#)
    }

    /// One disclosure of an unobtainable pin, as a manifest records it.
    fn disclosure(id: &str, change: &str, reason: &str) -> String {
        format!(
            r#""unobtainableRevisions":[{{"record":"record.json","id":"{id}",
                "changeId":"{change}","reason":"{reason}"}}],"#
        )
    }

    #[test]
    fn a_record_pin_that_no_visible_commit_answers_is_reported() {
        // The defect this exists for: a record a manifest admits could pin
        // anything at all, because the visible-commit test was applied to the
        // manifest's own revision and never to a record's.
        let root = pinned_record_root("pin-dangles");
        write_admitted(&root, &pin(&"1".repeat(40), &"k".repeat(32)), "");

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("evidence/m9/manifest.json admits record.json")
                && violations[0].contains("names no commit in visible history"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_record_pin_at_the_repository_root_is_refused() {
        // Forty zeroes and thirty-two `z`s are not arbitrary garbage: they are jj's
        // root commit and its change, so a pin substituted with them resolves,
        // agrees with itself, and satisfied both tests until the root was
        // subtracted from what counts as a visible commit.
        let root = pinned_record_root("pin-root");
        write_admitted(&root, &pin(&"0".repeat(40), &"z".repeat(32)), "");

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("names no commit in visible history"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_record_pin_its_own_change_has_moved_off_is_reported() {
        let root = pinned_record_root("pin-moved");
        write_admitted(&root, r#"{"dirty":false}"#, "");
        let (older, _) = committed_pin(&root, "the tree the trial measured");
        fs::write(root.join("later.txt"), "later\n").expect("a later file should be writable");
        let (_, moved) = committed_pin(&root, "a later change");
        // Both halves resolve and they resolve to different commits, which is the
        // state every rewrite leaves a recorded pin in and the one a pin that
        // merely resolves cannot be told apart from a true one.
        write_admitted(&root, &pin(&older, &moved), "");

        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("no longer holds"), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_true_record_pin_passes_and_a_disclosure_of_it_does_not() {
        let root = pinned_record_root("pin-true");
        write_admitted(&root, r#"{"dirty":false}"#, "");
        let (commit, change) = committed_pin(&root, "the tree the trial measured");
        write_admitted(&root, &pin(&commit, &change), "");

        assert!(verify_evidence_records(&root).is_empty());

        // The same pin, disclosed as unobtainable while it resolves. An excuse the
        // defect has outlived is what lets the next real one hide behind it.
        write_admitted(
            &root,
            &pin(&commit, &change),
            &disclosure(&commit, &change, "stale"),
        );
        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("remove the disclosure"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_disclosed_record_pin_is_permitted_and_a_disclosure_naming_no_pin_is_not() {
        let dangling = pin(&"1".repeat(40), &"k".repeat(32));
        let root = pinned_record_root("pin-disclosed");
        write_admitted(
            &root,
            &dangling,
            &disclosure(
                &"1".repeat(40),
                &"k".repeat(32),
                "the trial ran on a working copy jj abandoned",
            ),
        );

        assert!(verify_evidence_records(&root).is_empty());

        // A disclosure quoting a pin no record carries excuses nothing, and
        // leaving it would let the undisclosed pin beside it pass under its name.
        write_admitted(
            &root,
            &dangling,
            &disclosure(&"2".repeat(40), &"k".repeat(32), "quotes another commit"),
        );
        let violations = verify_evidence_records(&root);

        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("a disclosure that names no pin excuses")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_tracked_record_its_manifest_does_not_name_is_reported() {
        // With `prism-launch.json` removed from `benchmarks.results`, that record's
        // counts were rewritten to invented numbers and the scaffold stayed at
        // zero, because the digest that would have caught it was no longer
        // recorded.
        let root = pinned_record_root("record-unnamed");
        write_admitted(&root, r#"{"dirty":false}"#, "");
        fs::write(root.join("evidence/m9/unnamed.json"), "{}\n")
            .expect("record should be writable");

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json names no record unnamed.json, which its own milestone \
                 directory tracks; a record no manifest names is a record no digest covers"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The digest of `hello\n`, which the pinned-input tests below record.
    const HELLO_DIGEST: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";

    /// Initialises a jj repository holding one input and returns it with the
    /// revision that holds it.
    ///
    /// The input is edited afterwards so its recorded content survives only in
    /// the pinned revision, which is the position every closed milestone is in:
    /// a check that hashed the working copy would see the later content.
    fn pinned_input_root(label: &str) -> (PathBuf, String) {
        let root = temporary_root(label);
        let initialised = Command::new("jj")
            .args(["git", "init"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(initialised.status.success(), "{initialised:?}");
        fs::write(root.join("input.txt"), "hello\n").expect("input should be writable");
        let pinned = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "commit_id"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(pinned.status.success(), "{pinned:?}");
        fs::write(root.join("input.txt"), "moved on\n").expect("input should be rewritable");
        (
            root,
            String::from_utf8_lossy(&pinned.stdout).trim().to_owned(),
        )
    }

    /// Records one input against a pinned revision in an M9 manifest.
    fn write_input_manifest(root: &Path, revision: &str, name: &str, digest: &str) {
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"revision":{{"id":"{revision}"}},
                    "inputs":[{{"name":"{name}","sha256":"{digest}"}}]}}"#
            ),
        )
        .expect("manifest should be writable");
    }

    #[test]
    fn an_input_digest_is_recomputed_against_the_revision_the_manifest_pins() {
        let (root, revision) = pinned_input_root("inputs-match");
        write_input_manifest(&root, &revision, "input.txt", HELLO_DIGEST);

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_input_digest_that_binds_nothing_at_the_pinned_revision_is_reported() {
        // The hole this exists for: an input digest that matched the file at no
        // revision at all, recorded beside four that did and read as verified.
        let (root, revision) = pinned_input_root("inputs-drift");
        write_input_manifest(&root, &revision, "input.txt", &"0".repeat(64));

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [format!(
                "evidence/m9/manifest.json records input input.txt as {} but revision {revision} \
                 holds {HELLO_DIGEST}",
                "0".repeat(64)
            )]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_input_the_pinned_revision_does_not_hold_is_reported() {
        // A file the repository holds now and the pinned revision does not, so the
        // existence question passes and only the digest question can speak.
        let (root, revision) = pinned_input_root("inputs-absent");
        fs::write(root.join("later.txt"), "hello\n").expect("input should be writable");
        write_input_manifest(&root, &revision, "later.txt", HELLO_DIGEST);

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [format!(
                "evidence/m9/manifest.json records input later.txt: cannot read it at revision \
                 {revision}"
            )]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_input_the_repository_no_longer_holds_is_reported() {
        // The other half, and the hole it exists for: M0 went on binding two fixture
        // descriptors the ATM10 retarget deleted, and every digest still matched,
        // because a deleted file hashes at the revision that predates the deletion
        // exactly as it always did.
        let (root, revision) = pinned_input_root("inputs-deleted");
        fs::remove_file(root.join("input.txt")).expect("input should be removable");
        write_input_manifest(&root, &revision, "input.txt", HELLO_DIGEST);

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/m9/manifest.json records input input.txt, which the repository no longer \
              holds, so the digest binds a path no reader can resolve"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_pinned_commit_that_is_the_change_head_agrees() {
        assert_eq!(pinned_commit_mismatch("m9", "kkxp", "abc", "abc"), None);
    }

    #[test]
    fn a_pinned_commit_the_change_has_moved_off_is_reported() {
        // The hole this exists for: all four manifests pinned a commit their own
        // change no longer held, and every one of them resolved here and nowhere
        // else, so twenty input digests bound nothing in a clone.
        let reported = pinned_commit_mismatch("evidence/m9/manifest.json", "kkxp", "abc", "def")
            .expect("a moved change should be reported");

        assert!(
            reported.starts_with(
                "evidence/m9/manifest.json pins revision.id abc but change kkxp is at def"
            ),
            "{reported}"
        );
    }

    #[test]
    fn an_abandoned_commit_is_not_visible_although_jj_still_reads_it() {
        // `jj file show -r` reads an abandoned commit out of the operation log, so
        // resolving one is not evidence a clone could. This is the distinction the
        // whole re-pin rests on.
        let (root, abandoned) = pinned_input_root("visible-abandoned");
        let described = Command::new("jj")
            .args(["describe", "-m", "moves the change off that commit"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(described.status.success(), "{described:?}");

        let readable = Command::new("jj")
            .args(["file", "show", "-r", &abandoned, "--", "input.txt"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(readable.status.success(), "{readable:?}");
        assert_eq!(
            visible_commit(&root, &abandoned),
            Err("names no commit in visible history".to_owned())
        );

        let current = visible_commit(&root, "@").expect("the working copy should be visible");
        assert_ne!(current, abandoned);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_revision_naming_nothing_is_not_visible() {
        // All-zeros is jj's root commit and is visible, so the bogus id here is a
        // hash no object carries.
        let (root, _) = pinned_input_root("visible-bogus");

        assert_eq!(
            visible_commit(&root, &"1".repeat(40)),
            Err("names no commit in visible history".to_owned())
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes a manifest recording one gate run and returns the root.
    fn recorded_run_root(label: &str, run: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("manifest.json"), run).expect("manifest should be writable");
        root
    }

    #[test]
    fn a_recorded_run_whose_numbers_agree_is_accepted() {
        let root = recorded_run_root(
            "run-consistent",
            r#"{"commands":[{"exitStatus":0}],
                "tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                    "lineCoveragePercent":85.97}}"#,
        );

        assert!(verify_evidence_records(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_run_whose_counts_do_not_sum_is_reported() {
        // A manifest attests a run nothing can replay, so the arithmetic between
        // its own numbers is the part a gate can hold it to.
        let root = recorded_run_root(
            "run-arithmetic",
            r#"{"commands":[{"exitStatus":0}],
                "tests":{"total":8,"passed":9,"failed":0,"skipped":0,
                    "lineCoveragePercent":85.97}}"#,
        );

        assert_eq!(
            verify_evidence_records(&root),
            ["evidence/m9/manifest.json records test counts that do not sum to its total"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_run_with_no_test_result_and_no_coverage_is_reported() {
        let root = recorded_run_root("run-untested", r#"{"commands":[{"exitStatus":1}]}"#);

        assert_eq!(
            verify_evidence_records(&root),
            [
                "evidence/m9/manifest.json records a gate run but no test result",
                "evidence/m9/manifest.json records a gate run that did not succeed",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_run_below_the_coverage_floor_is_reported() {
        let root = recorded_run_root(
            "run-coverage",
            r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                "lineCoveragePercent":79.9}}"#,
        );

        assert_eq!(
            verify_evidence_records(&root),
            ["evidence/m9/manifest.json records line coverage below the floor the gate enforces"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_absent_spike_tree_is_not_a_violation() {
        let root = temporary_root("spikes-absent");

        assert!(verify_spikes(&root).is_empty());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn spike_results_must_cover_every_failure_scenario() {
        let root = temporary_root("spikes-failures");
        let gate = root.join("evidence/m1/spikes/adg-1");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        fs::write(
            gate.join("candidate.json"),
            r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
                "candidate":{"disposition":"measured"},
                "failures":[{"scenario":"crash"}],
                "memory":{"bootstrapPeakRss":{"status":"measured","value":1},
                    "retainedRss":{"status":"measured","value":1},
                    "retainedClasses":{"status":"measured","value":1}},
                "startup":{"processToReady":{"status":"measured","value":1}},
                "calls":{"callbackNanoseconds":{"status":"measured"}},
                "trial":{"measuredInvocations":1},"deviations":[]}"#,
        )
        .expect("spike result should be writable");

        let violations = verify_spikes(&root);

        assert!(violations.iter().any(|violation| {
            violation.ends_with("must exercise every required failure scenario exactly once")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// `ravel-test` skips a version whose client mapping is absent, and the
    /// generator is what decides that a mapping is needed at all.
    ///
    /// Two files state one rule, so this holds them together. The script greps
    /// `adapter.toml` for a `client.txt` path; the adapter decides a mapping is
    /// required by declaring `official-proguard-mappings`. If a version ever
    /// declares that source without such a path, or declares the path under a
    /// source that reads none, the script would skip a version it should test or
    /// test one it cannot build, and it would do it silently.
    #[test]
    fn ravel_test_versions_agree_with_the_generator() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let versions = root.join(VERSION_DATA);
        let mut checked = 0;
        for entry in fs::read_dir(&versions).expect("the version data should be readable") {
            let directory = entry
                .expect("a version directory should be readable")
                .path();
            if !directory.is_dir() {
                continue;
            }
            let adapter = fs::read_to_string(directory.join("adapter.toml"))
                .expect("every version declares adapter inputs");
            let translating = adapter.contains("mappingSource = \"official-proguard-mappings\"");
            // The same shape the script's `sed` matches.
            let declares_mapping = adapter
                .lines()
                .any(|line| line.starts_with("path = \"") && line.ends_with("client.txt\""));
            assert_eq!(
                translating,
                declares_mapping,
                "{} reads a published mapping ({translating}) but declares a client.txt path \
                 ({declares_mapping}); ravel-test decides what to skip from that path",
                directory.display()
            );
            checked += 1;
        }
        assert!(checked > 0, "the repository tracks a version");
    }

    #[test]
    fn a_claimed_stock_launch_without_the_redacted_log_hash_is_refused() {
        let root = temporary_root("spikes-launch-log");
        fs::create_dir_all(root.join("schemas")).expect("schema directory should be creatable");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/spike-result.schema.json"),
            root.join("schemas/spike-result.schema.json"),
        )
        .expect("spike schema should be copied");
        let gate = root.join("evidence/m1/spikes/adg-5");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        let recorded = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../evidence/m1/spikes/adg-5/adg5-stock-jvm-arguments.json"),
        )
        .expect("the recorded ADG-5 result should be readable");
        let target = gate.join("candidate.json");

        // The recorded result is the control: only the log artifact's name differs
        // between the two writes, so nothing but the new rule can explain the refusal.
        fs::write(&target, &recorded).expect("spike result should be writable");
        assert!(
            verify_spikes(&root).is_empty(),
            "the recorded result should pass unaltered"
        );

        fs::write(
            &target,
            recorded.replace("redacted-instance-launch.log", "some-other-capture.log"),
        )
        .expect("altered spike result should be writable");
        assert!(
            verify_spikes(&root)
                .iter()
                .any(|violation| violation.ends_with("does not conform to the spike schema")),
            "a stock launch with no redacted log hash should be refused"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn spike_results_must_not_substitute_zero_for_a_missing_measurement() {
        let root = temporary_root("spikes-zero");
        let gate = root.join("evidence/m1/spikes/adg-1");
        fs::create_dir_all(&gate).expect("spike directory should be creatable");
        fs::write(
            gate.join("candidate.json"),
            r#"{"schemaVersion":1,"protocolVersion":"spike-v1","gate":"ADG-1",
                "candidate":{"disposition":"measured"},
                "failures":[{"scenario":"callback-failure"},{"scenario":"crash"},
                    {"scenario":"invalid-input"},{"scenario":"timeout"},
                    {"scenario":"unavailable-runtime"}],
                "memory":{"bootstrapPeakRss":{"status":"measured","value":0},
                    "retainedRss":{"status":"measured","value":1},
                    "retainedClasses":{"status":"measured","value":1}},
                "startup":{"processToReady":{"status":"measured","value":1}},
                "calls":{"callbackNanoseconds":{"status":"measured"}},
                "trial":{"measuredInvocations":1},"deviations":[]}"#,
        )
        .expect("spike result should be writable");

        let violations = verify_spikes(&root);

        assert!(violations.iter().any(|violation| {
            violation.ends_with("must not report a measured bootstrap or startup value of zero")
        }));
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn production_crates_cannot_depend_on_measurement_experiments() {
        let root = temporary_root("fence");
        let crate_root = root.join("crates/ravel-loader");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[dependencies]\nravel-spike-harness.workspace = true\n",
        )
        .expect("manifest should be writable");
        let mut violations = Vec::new();

        verify_experiment_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["production crate depends on an experiment: crates/ravel-loader/Cargo.toml"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_result_naming_a_change_that_did_not_introduce_it_is_reported() {
        // The defect this exists for: seven results named a revision no rewrite
        // had updated, and nothing recomputed the claim until a human read them.
        let wrong = change_mismatch(
            "evidence/m1/spikes/adg-3/adg3-javap-extraction.json",
            "wtvoqolwmrxtpwsxspnlwnsnkoslxxlt",
            "ysvpslstqmmmkkzknzmozqkpnozlworn",
        );
        assert_eq!(
            wrong.as_deref(),
            Some(
                "evidence/m1/spikes/adg-3/adg3-javap-extraction.json records change \
                 wtvoqolwmrxtpwsxspnlwnsnkoslxxlt but was introduced by \
                 ysvpslstqmmmkkzknzmozqkpnozlworn"
            )
        );
        assert_eq!(
            change_mismatch("r.json", "wtvoqolwmrxt", "wtvoqolwmrxt"),
            None
        );
        // An untracked result has no introducing change and is not yet checkable,
        // so the check clears without a verdict and has to say so rather than
        // read as agreement.
        assert_eq!(change_mismatch("r.json", "wtvoqolwmrxt", ""), None);
    }

    #[test]
    fn unsafe_code_cannot_be_permitted_outside_experiments() {
        let root = temporary_root("fence-unsafe");
        let crate_root = root.join("crates/ravel-bootstrap/src");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("lib.rs"),
            "#![allow(unsafe_code)]\npub fn boundary() {}\n",
        )
        .expect("source should be writable");
        let mut violations = Vec::new();

        verify_experiment_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["unsafe code is permitted outside the experiment boundary: \
                 crates/ravel-bootstrap/src/lib.rs"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn missing_files_are_reported_by_relative_path() {
        let root = temporary_root("missing");

        let violations = verify_scaffold(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation == "missing required file: Cargo.toml")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_required_path_is_unique() {
        let mut paths = required_scaffold_paths();
        let original_len = paths.len();
        paths.sort_unstable();
        paths.dedup();

        assert_eq!(paths.len(), original_len);
    }

    #[test]
    fn restricted_content_ignores_are_required() {
        let root = temporary_root("ignores");
        fs::write(root.join(".gitignore"), "# intentionally incomplete\n")
            .expect("test gitignore should be writable");
        let mut violations = Vec::new();

        verify_ignored_boundaries(&root, &mut violations);

        assert_eq!(violations.len(), 3);
        assert!(
            violations
                .iter()
                .all(|violation| violation.starts_with("missing restricted-content ignore:"))
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn restricted_tracked_paths_are_rejected() {
        let violations = verify_tracked_paths([
            "tests/fixtures/local/secret.jar",
            "evidence/private/capture.json",
            "evidence/m0/raw/process.log",
            "evidence/m0/captures/frame.trace",
            "config/credentials.json",
            "docs/README.md",
        ]);

        assert_eq!(violations.len(), 5);
        assert!(
            violations
                .iter()
                .all(|violation| violation.starts_with("forbidden tracked path:"))
        );
    }

    #[test]
    fn tracked_credential_material_is_rejected() {
        let root = temporary_root("credentials");
        fs::write(
            root.join("notes.txt"),
            format!("{}{}\n", "-----BEGIN OPENSSH ", "PRIVATE KEY-----"),
        )
        .expect("credential canary should be writable");

        let violations = verify_tracked_contents(&root, &["notes.txt".to_owned()]);

        assert_eq!(
            violations,
            ["tracked file contains credential material: notes.txt"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Holds `verify_tracked_contents`'s disclosure to what it does.
    ///
    /// The doc comment says two shapes are deliberately not refused. A sentence
    /// like that is exactly the claim this project keeps finding false, and the
    /// only thing that makes it not a claim is something that fails when it
    /// stops being true. So the two shapes are planted here and required to
    /// pass: whoever adds either rule breaks this test and has to delete the
    /// paragraph that says the rule is absent, in the same edit.
    #[test]
    fn contents_scan_discloses_what_it_does_not_refuse() {
        let root = temporary_root("disclosure");
        fs::write(
            root.join("notes.txt"),
            "cache lives at /home/someone/.local/state/ravel\n",
        )
        .expect("absolute path sample should be writable");
        // Shaped like an upstream mod id and belonging to nothing; the scan has
        // no identifier rule at all, so no real one is needed to show that.
        fs::write(
            root.join("fixture.json"),
            "{\"files\": [{\"projectID\": 1, \"modId\": \"someupstreammod\"}]}\n",
        )
        .expect("identifier sample should be writable");

        let violations =
            verify_tracked_contents(&root, &["notes.txt".to_owned(), "fixture.json".to_owned()]);

        assert_eq!(
            violations,
            Vec::<String>::new(),
            "an absolute home path and an upstream-shaped identifier are disclosed as not refused; \
             if either now fails, update the disclosure on verify_tracked_contents and in \
             evidence/m5/manifest.json rather than this assertion"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn milestone_manifest_must_match_requested_milestone() {
        let root = temporary_root("milestone");
        let evidence = root.join("evidence/m0");
        fs::create_dir_all(&evidence).expect("evidence directory should be creatable");
        fs::write(
            evidence.join("manifest.json"),
            r#"{"schemaVersion":1,"milestone":"M1"}"#,
        )
        .expect("manifest should be writable");

        let violations = verify_milestone(&root, "M0");

        assert!(
            violations
                .iter()
                .any(|violation| violation == "manifest milestone must equal M0")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes a milestone manifest binding one review record and returns the root.
    fn milestone_review_root(label: &str, record: &str, digest: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m0");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("review.md"), record).expect("record should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{"schemaVersion":1,"milestone":"M0",{RECORDED_RUN},
                    "review":{{"reviewer":"an independent reviewer","independent":true,
                    "result":"pass","record":{{"name":"review.md","sha256":"{digest}"}}}}}}"#
            ),
        )
        .expect("manifest should be writable");
        root
    }

    #[test]
    fn the_milestone_gate_recomputes_the_review_record_it_closes_on() {
        // The defect this exists for: the review record a closed milestone binds
        // was replaced wholesale with five lines reading `Result: PASS` and saying
        // no round ever ran, `review.record.sha256` was left at its real value,
        // and `verify milestone M4` exited 0, because the digest that makes the
        // record obtainable was recomputed only by the every-manifest sweep.
        let substituted = "Result: PASS\nNo round ever ran.\n";
        let hashes = "4f682a1f88a7dbba60f3046cc667ef8fcc6876c6eda74b90dc97a4ddca688091";
        let root = milestone_review_root("milestone-review", substituted, &"0".repeat(64));

        let violations = verify_milestone(&root, "M0");

        assert!(
            violations.iter().any(|violation| {
                violation.starts_with("evidence/m0/manifest.json records review.md as ")
                    && violation.ends_with(&format!(" but it hashes {hashes}"))
            }),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn the_milestone_gate_accepts_a_review_record_that_hashes_as_recorded() {
        // The control for the mutation above: a record whose bytes are the ones
        // the manifest attested to raises nothing about its digest, so the check
        // reports substitution rather than the presence of a review record.
        let root = milestone_review_root("milestone-review-match", PASS_RECORD, PASS_RECORD_DIGEST);

        let violations = verify_milestone(&root, "M0");

        assert!(
            !violations
                .iter()
                .any(|violation| violation.contains("review.md")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn malformed_milestone_ids_are_rejected() {
        let root = temporary_root("milestone-id");

        let violations = verify_milestone(&root, "M14");

        assert_eq!(violations, ["invalid milestone ID: M14"]);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn version_specific_crates_and_production_literals_are_rejected() {
        let root = temporary_root("versions");
        fs::create_dir_all(root.join("minecraft/versions/test-version"))
            .expect("version directory should be creatable");
        // A `class` entry declares the package root, and the descriptor beside it
        // is how a second class reaches the swept set without a `class` line of
        // its own. `java/lang/String` is there to be ignored: it is not under a
        // root the data declared.
        fs::write(
            root.join("minecraft/versions/test-version/hooks.toml"),
            "class = \"com.testgame.Thing\"\n\
             descriptor = \"(Lcom/testgame/Other;Ljava/lang/String;)V\"\n",
        )
        .expect("version data should be writable");
        fs::create_dir_all(root.join("crates/ravel-minecraft-test-version/src"))
            .expect("crate directory should be creatable");
        fs::write(
            root.join("crates/ravel-minecraft-test-version/src/lib.rs"),
            "const VERSION: &str = \"test-version\";\n",
        )
        .expect("test source should be writable");
        fs::create_dir_all(root.join("crates/ravel-core/src"))
            .expect("crate directory should be creatable");
        fs::write(
            root.join("crates/ravel-core/src/lib.rs"),
            "// com.testgame.Thing, com/testgame/Other, java/lang/String\n",
        )
        .expect("test source should be writable");
        let mut violations = Vec::new();

        verify_version_boundary(&root, &mut violations);

        assert!(
            violations
                .iter()
                .any(|violation| violation.starts_with("unexpected per-version Minecraft crate:")),
            "{violations:?}"
        );
        let swept = violations
            .iter()
            .filter(|violation| violation.contains(SWEPT_SCOPE))
            .collect::<Vec<_>>();
        assert!(
            swept.iter().any(|violation| violation
                .contains("hardcoded Minecraft version outside version data")
                && violation.contains("ravel-minecraft-test-version")),
            "{violations:?}"
        );
        assert!(
            swept
                .iter()
                .any(|violation| violation.contains("names com/testgame/Thing")),
            "a class the data declares is a name no general crate may spell: {violations:?}"
        );
        assert!(
            swept
                .iter()
                .any(|violation| violation.contains("names com/testgame/Other")),
            "a class only a descriptor names is in the set too: {violations:?}"
        );
        assert!(
            !violations
                .iter()
                .any(|violation| violation.contains("java/lang/String")),
            "the platform is not the game: {violations:?}"
        );
        assert_eq!(swept.len(), 3, "{violations:?}");

        // Every excuse names a file this root does not hold, so every excuse is
        // unearned here. The count is recomputed rather than written down.
        let excused = VERSION_CLASS_EXCUSES
            .iter()
            .map(|(_, classes, _)| classes.len())
            .sum::<usize>();
        assert_eq!(
            violations
                .iter()
                .filter(|violation| violation.starts_with("stale version-class excuse:"))
                .count(),
            excused,
            "{violations:?}"
        );
        assert_eq!(violations.len(), 4 + excused, "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_milestone_manifest_is_required_by_name() {
        // The defect this exists for: the sweep discovered manifests by scanning
        // `evidence/`, so moving all four away left the scaffold reporting zero
        // manifests, zero violations, and "verification passed".
        let required = required_scaffold_paths();

        for (relative, _) in MILESTONE_MANIFESTS {
            assert!(
                required.iter().any(|path| path == relative),
                "{relative} must be required by name"
            );
        }
    }

    #[test]
    fn an_evidence_tree_holding_no_manifest_is_reported() {
        let root = temporary_root("records-none");
        fs::create_dir_all(root.join("evidence")).expect("evidence directory should be creatable");

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [
                "evidence/ holds no milestone manifest, and a manifest sweep over nothing passes \
              vacuously"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_manifest_recording_neither_a_run_nor_a_test_result_is_reported() {
        // The defect this exists for: `del(.tests, .commands)` left this check
        // with nothing to say and the scaffold at zero.
        let root = recorded_run_root("run-silent", r#"{"milestone":"M9"}"#);

        assert_eq!(
            verify_evidence_records(&root),
            [
                "evidence/m9/manifest.json records neither a gate run nor a test result, so there is \
              nothing here a gate can hold it to"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_baseline_result_named_by_description_is_reported() {
        // The defect this exists for: rewording a result name to prose failed the
        // path heuristic, and failing it bought an announced skip rather than a
        // violation, so a digest could be detached from its file by editing a
        // string. The validation beside it names a private capture and stays a
        // skip, which is what makes the two distinguishable at all.
        let root = temporary_root("baseline-prose");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"platformEvidence":[{{"baselines":[
                    {{"workload":"vanilla",
                    "result":{{"name":"the Linux vanilla launch summary",
                        "sha256":"{EMPTY_OBJECT_DIGEST}"}},
                    "validation":{{"name":"a private launch validation",
                        "sha256":"{EMPTY_OBJECT_DIGEST}"}}}}]}}]}}"#
            ),
        )
        .expect("manifest should be writable");

        assert_eq!(
            verify_evidence_records(&root),
            [
                "evidence/m9/manifest.json records a baseline result by description rather than by \
              path, so its digest binds nothing obtainable: the Linux vanilla launch summary"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_digest_binding_an_untracked_file_is_reported() {
        // The defect this exists for: the digest was recomputed against the working
        // copy and nothing asked whether the repository held the file, so a record
        // could be bound to something only the machine that wrote it can obtain.
        let root = temporary_root("records-untracked");
        let initialised = Command::new("jj")
            .args(["git", "init"])
            .current_dir(&root)
            .output()
            .expect("jj should be available");
        assert!(initialised.status.success(), "{initialised:?}");
        fs::write(root.join(".gitignore"), "ignored.json\n").expect("ignore should be writable");
        let milestone = root.join("evidence/m9");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("ignored.json"), "{}\n").expect("result should be writable");
        fs::write(
            milestone.join("manifest.json"),
            format!(
                r#"{{{RECORDED_RUN},"revision":{{"id":"@"}},
                    "benchmarks":{{"protocolVersion":"v1",
                    "results":[{{"name":"ignored.json",
                        "sha256":"{EMPTY_OBJECT_DIGEST}"}}]}}}}"#
            ),
        )
        .expect("manifest should be writable");

        assert_eq!(
            verify_evidence_records(&root),
            [
                "evidence/m9/manifest.json records a result that the repository does not track, so \
              its digest binds nothing a reader can obtain: ignored.json"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_unparseable_manifest_cannot_answer_a_check() {
        // The defect this exists for: jq's nonzero exit was read as the answer
        // "no", so a document it could not parse quietly satisfied every check
        // phrased as a refusal.
        let root = recorded_run_root("records-unparseable", "{not json\n");

        let violations = verify_evidence_records(&root);

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("jq exited 5")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn the_unsafe_prohibition_must_be_the_attribute_and_not_a_doc_comment() {
        // The defect this exists for: `crates/ravel-bootstrap/src/lib.rs` quotes
        // the attribute in a doc comment, so a substring test passed that crate
        // with the real attribute deleted and a working `unsafe` block added.
        assert!(declares_unsafe_prohibition("#![forbid(unsafe_code)]\n"));
        assert!(declares_unsafe_prohibition(
            "//! a crate\n\n    #![forbid(unsafe_code)]\n"
        ));
        assert!(!declares_unsafe_prohibition(
            "//! this crate keeps `#![forbid(unsafe_code)]` verbatim\npub fn f() {}\n"
        ));
    }

    #[test]
    fn production_crates_cannot_depend_on_the_test_harness() {
        // The defect this exists for: `dependency-rules.md` forbids it and nothing
        // enforced it, so adding the dependency left the scaffold at zero.
        let root = temporary_root("fence-testkit");
        let crate_root = root.join("crates/ravel-loader");
        fs::create_dir_all(&crate_root).expect("crate directory should be creatable");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[dependencies]\nravel-testkit.workspace = true\n",
        )
        .expect("manifest should be writable");
        let mut violations = Vec::new();

        verify_testkit_fence(&root, &mut violations);

        assert_eq!(
            violations,
            ["production crate depends on the test harness: crates/ravel-loader/Cargo.toml"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_tracked_file_that_is_not_utf8_is_still_scanned_for_credentials() {
        // The defect this exists for: the scan read every tracked file as UTF-8 and
        // skipped what failed, exempting exactly the binary files a key arrives as.
        let root = temporary_root("credentials-binary");
        let mut bytes = format!("{}{}\n", "-----BEGIN OPENSSH ", "PRIVATE KEY-----").into_bytes();
        bytes.push(0xff);
        fs::write(root.join("keystore.bin"), bytes).expect("canary should be writable");

        let violations = verify_tracked_contents(&root, &["keystore.bin".to_owned()]);

        assert_eq!(
            violations,
            ["tracked file contains credential material: keystore.bin"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn gate_output_never_names_an_absolute_path() {
        // The repository redacts local paths from everything it records, and
        // `verify_milestone` already refuses to report one; `jq_value` printed one
        // straight into a violation.
        let absolute = std::env::current_dir()
            .expect("the current directory must be available")
            .join("evidence/m0/manifest.json");

        assert_eq!(redacted_path(&absolute), "evidence/m0/manifest.json");
        assert_eq!(
            redacted_path(Path::new("evidence/m0/manifest.json")),
            "evidence/m0/manifest.json"
        );
        assert_eq!(
            redacted_path(Path::new("/somewhere/else/manifest.json")),
            "manifest.json"
        );
    }

    /// One `[[hook]]` block, in the shape `hooks.toml` writes them.
    fn hook_entry(kind: &str, descriptor: &str) -> String {
        format!(
            "\n[[hook]]\nid = \"an.id\"\nclass = \"net.minecraft.Thing\"\nmember = \"member\"\n\
             descriptor = \"{descriptor}\"\nkind = \"{kind}\"\ncapability = \"Do something.\"\n"
        )
    }

    #[test]
    fn a_hook_inventory_is_counted_by_kind_and_by_handle_parameter() {
        let inventory = format!(
            "schemaVersion = 1\n{}{}{}{}",
            // A string is rendered as a string, so it is not a handle; an array of
            // strings is a reference and is one.
            hook_entry("static-method", "(Ljava/lang/String;Z)Ljava/lang/String;"),
            hook_entry(
                "instance-method",
                "([Ljava/lang/String;[JLjava/util/List;D)V"
            ),
            hook_entry("instance-method", "(I)Z"),
            hook_entry("instance-field", "Z"),
        );

        let counts = hook_counts(&inventory).expect("the inventory should count");

        assert_eq!(counts.entries, 4);
        assert_eq!(counts.methods, 3);
        assert_eq!(counts.fields, 1);
        assert_eq!(counts.handles, 3);
    }

    #[test]
    fn the_hook_document_fails_on_every_count_that_no_longer_matches_the_inventory() {
        let counts = HookCounts {
            entries: 37,
            methods: 33,
            fields: 4,
            handles: 27,
        };
        let current = "`hooks.toml` therefore grew from 6 entries to 37 — 33 methods and 4 fields,\n\
                       carrying 27 handle parameters between them.";
        let stale = "`hooks.toml` therefore grew from 6 entries to 35 — 31 methods and 4 fields,\n\
                     carrying 23 handle parameters between them.";

        let inventory = "minecraft/versions/26.1.2/hooks.toml";

        assert_eq!(
            hook_document_violations(current, inventory, &counts),
            Vec::<String>::new()
        );
        assert_eq!(
            hook_document_violations(stale, inventory, &counts),
            [
                format!("{HOOK_DOCUMENT} states 35 hook entries where {inventory} declares 37"),
                format!("{HOOK_DOCUMENT} states 31 method entries where {inventory} declares 33"),
                format!(
                    "{HOOK_DOCUMENT} states 23 handle parameters where {inventory} declares 27"
                ),
            ]
        );
    }

    #[test]
    fn a_hook_document_stating_no_count_at_all_is_not_read_as_agreement() {
        let counts = HookCounts {
            entries: 37,
            methods: 33,
            fields: 4,
            handles: 27,
        };

        let violations = hook_document_violations(
            "The inventory grew.",
            "minecraft/versions/26.1.2/hooks.toml",
            &counts,
        );

        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            violations
                .iter()
                .all(|violation| violation.contains("no longer writes a count against")),
            "{violations:?}"
        );
    }

    /// The prose figures are checked at every occurrence, not the first.
    #[test]
    fn a_campaign_document_stating_the_wrong_inventory_anywhere_is_reported() {
        let derived = CampaignInventory {
            classes: 15,
            targets: 33,
            prior: 31,
            launches: 3,
        };

        let violations = campaign_document_violations(
            "Rewriting all 33 callable targets, over the same 15 classes in the reviewed \
             inventory. The count moved from 31 to 33, over three surface launches. A later \
             launch at 34 callable targets over 16 classes in the reviewed inventory.",
            &derived,
        );

        assert_eq!(
            violations,
            [
                "evidence/m3/README.md states 34 callable targets where \
                 evidence/m3/linux-process-matrix.json records 33"
                    .to_owned(),
                "evidence/m3/README.md states 16 classes in the reviewed inventory where \
                 evidence/m3/linux-process-matrix.json records 15"
                    .to_owned(),
            ]
        );
    }

    /// A figure the markdown wrapped mid-phrase still has to be checked.
    #[test]
    fn a_campaign_figure_split_across_a_wrapped_line_is_still_checked() {
        let derived = CampaignInventory {
            classes: 15,
            targets: 33,
            prior: 31,
            launches: 3,
        };

        let violations = campaign_document_violations(
            "The count moved from 31 to 33 callable targets over the same 16 classes in the\n  \
             reviewed inventory, across three surface launches.",
            &derived,
        );

        assert_eq!(
            violations,
            [
                "evidence/m3/README.md states 16 classes in the reviewed inventory where \
                 evidence/m3/linux-process-matrix.json records 15"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn a_campaign_document_stating_no_count_at_all_is_not_read_as_agreement() {
        let derived = CampaignInventory {
            classes: 15,
            targets: 33,
            prior: 31,
            launches: 3,
        };

        let violations = campaign_document_violations("The campaigns are recorded.", &derived);

        assert_eq!(violations.len(), 4, "{violations:?}");
        assert!(
            violations
                .iter()
                .all(|violation| violation.contains("no longer writes a count against")),
            "{violations:?}"
        );
    }

    /// A count reaches its noun through the adjectives the stale sites used.
    #[test]
    fn a_count_is_read_against_its_noun_however_it_was_spelled_or_qualified() {
        assert_eq!(
            numbers_before("six independent rounds have run", &["rounds"]),
            [6]
        );
        assert_eq!(
            numbers_before("the five tracked independent M3-W8 rounds", &["rounds"]),
            [5]
        );
        assert_eq!(numbers_before("5 rounds are tracked", &["rounds"]), [5]);
        assert_eq!(
            numbers_before("over nine world lifecycles", &["world", "lifecycles"]),
            [9]
        );
        // An ordinal is a reference into the record's numbering rather than a
        // count of it, and prose that states no figure states none.
        assert!(numbers_before("the fourth round of review", &["round"]).is_empty());
        assert!(numbers_before("every round returned FAIL", &["rounds"]).is_empty());
    }

    /// The tail a round walked the count checks around by spelling it.
    ///
    /// `forty` and `twenty-one` both left the gate at zero against a document the
    /// sweep reads, so both are held here by the spelling that escaped.
    #[test]
    fn a_count_spelled_above_twenty_is_read_up_to_ninety_nine() {
        assert_eq!(numbers_before("forty M3 review rounds", &["rounds"]), [40]);
        assert_eq!(
            numbers_before("twenty-one M3 review rounds", &["rounds"]),
            [21]
        );
        assert_eq!(numbers_before("ninety-nine rounds", &["rounds"]), [99]);
        assert_eq!(numbers_before("thirty rounds", &["rounds"]), [30]);

        // The stated ceiling, held so that moving it is a decision rather than an
        // accident: a hundred and above is read by nothing, a compound without its
        // hyphen reads as its parts, and a hyphenated pair that is not a count is
        // not invented into one.
        assert!(numbers_before("a hundred rounds", &["rounds"]).is_empty());
        assert!(numbers_before("thirty-thousand-entry rounds", &["rounds"]).is_empty());
        assert!(numbers_before("twenty-eleven rounds", &["rounds"]).is_empty());
        assert_eq!(numbers_before("twenty one rounds", &["rounds"]), [1]);
    }

    /// The restatements an independent round walked the round-count check around.
    ///
    /// Each of these left the gate at zero: two called a round something the noun
    /// set had missed, and one put its figure after the noun where a backward
    /// window could not reach it. They are held here by the phrasing that escaped
    /// rather than by a paraphrase of it.
    #[test]
    fn a_round_count_is_caught_whichever_noun_it_chose_and_whichever_side_it_sat() {
        for restatement in [
            "Five independent reviews have run against this milestone.",
            "The independent rounds recorded here number five in total.",
            "There have been five review iterations.",
            "Five independent rounds have run.",
            "There are 5 verdicts recorded below.",
        ] {
            assert_eq!(
                round_count_restatements(restatement)
                    .into_iter()
                    .map(|(count, _)| count)
                    .collect::<Vec<_>>(),
                [5],
                "{restatement}"
            );
        }

        // A numeral against the noun that labels or divides rather than counts
        // rounds, which is why the word beside the noun is walked past.
        for innocent in [
            "the profiling overhead rounds to `0.05` and not one digit better",
            "Round 5 returned FAIL with four blocking findings.",
            "every round so far has been an independent one",
        ] {
            assert_eq!(round_count_restatements(innocent), Vec::new(), "{innocent}");
        }
    }

    #[test]
    fn the_tracked_m3_records_leave_the_round_count_to_the_record_that_counts() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_review_round_counts(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// Every tracked manifest leaves its percentages to the records that measured
    /// them, which is the state the stale guard.rs figure was removed to reach.
    #[test]
    fn no_tracked_manifest_writes_a_percentage_into_its_prose() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_manifest_percentages(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// The void set, the record it was read out of and the roadmap that declares
    /// it agree, so no void limit is left standing and no standing limit is void.
    #[test]
    fn the_void_budget_limits_are_the_ones_the_roadmap_declares() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_void_limits(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    #[test]
    fn the_tracked_m3_records_agree_with_the_harnesss_declared_world_lifecycles() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_harness_lifecycles(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// The shipped agent jar names no game class outside its declared reflective
    /// surface and the pinned hook inventory, and every declared name is in use.
    #[test]
    fn the_agent_jars_game_class_names_are_the_declared_and_the_pinned_ones() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_persistence_reflection(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// Writes one well-formed version directory, minus the files named.
    ///
    /// Well formed means what the sweep asks for and no more: every file declares
    /// the version its directory is named for, and the inventory carries one
    /// countable entry. Both used to be a bare `schemaVersion` line, which made
    /// the fixture satisfy the sweep by being emptier than a real directory can
    /// be — the shape of a version this fixture must never accidentally model.
    fn version_directory(root: &Path, version: &str, without: &[&str]) {
        let directory = root.join(format!("{VERSION_DATA}/{version}"));
        fs::create_dir_all(&directory).expect("version directory should be creatable");
        for file in VERSION_FILES {
            if without.contains(&file) {
                continue;
            }
            let text = match file {
                VERSION_PROSE_FILE => format!("# Minecraft {version}\n"),
                "hooks.toml" => format!(
                    "schemaVersion = 1\nminecraft = \"{version}\"\n\n[[hook]]\nid = \
                     \"level.tick\"\nkind = \"instance-method\"\nmember = \
                     \"tick\"\ndescriptor = \"()V\"\n"
                ),
                REFLECTION_FILE => format!(
                    "schemaVersion = 1\nminecraft = \"{version}\"\n\n[[reflected]]\nname = \
                     \"ONE\"\nclass = \"net.minecraft.client.One\"\nruntime = \
                     \"net.minecraft.client.One\"\n"
                ),
                _ => format!("schemaVersion = 1\nminecraft = \"{version}\"\n"),
            };
            fs::write(directory.join(file), text).expect("version data should be writable");
        }
    }

    /// The defect the per-version sweep exists for: the contract named one
    /// directory's six files by path, so a second directory arriving without its
    /// locks was asked for nothing at all.
    #[test]
    fn a_version_directory_missing_a_required_file_is_reported() {
        let root = temporary_root("version-files");
        version_directory(&root, "26.1.2", &[]);
        version_directory(&root, "99.0.0-synthetic", &["stubs.lock"]);
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(
            violations,
            ["missing required file: minecraft/versions/99.0.0-synthetic/stubs.lock"]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Countability is swept per version because it is the only check that reads an
    /// inventory's structure, and under one directory it ran only by the document
    /// check's accident.
    #[test]
    fn a_version_whose_inventory_cannot_be_counted_is_reported() {
        let root = temporary_root("version-counts");
        version_directory(&root, "26.1.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
            "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[hook]]\nid = \"an.id\"\n",
        )
        .expect("version data should be writable");
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "cannot count minecraft/versions/26.1.2/hooks.toml: a `[[hook]]` entry declares no \
              kind or no descriptor"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// An inventory truncated to its header counted as four zeroes and read as
    /// countable, so the version contributed no hook name any decision record
    /// could be falsified by and no figure any document could be held to.
    #[test]
    fn a_version_whose_inventory_declares_no_hook_is_reported() {
        let root = temporary_root("version-empty-inventory");
        version_directory(&root, "26.1.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
            "schemaVersion = 1\nminecraft = \"26.1.2\"\n",
        )
        .expect("version data should be writable");
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "cannot count minecraft/versions/26.1.2/hooks.toml: the inventory declares no \
                 `[[hook]]` entry"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The copy-paste the sweep exists to refuse: a directory duplicated from
    /// another version and never re-pinned is present, countable, and serving the
    /// source version's data under the new version's name.
    #[test]
    fn a_version_directory_declaring_another_versions_data_is_reported() {
        let root = temporary_root("version-declaration");
        version_directory(&root, "26.1.2", &[]);
        version_directory(&root, "1.21.1", &[]);
        for file in VERSION_FILES {
            if file == VERSION_PROSE_FILE {
                continue;
            }
            let source = root.join(format!("{VERSION_DATA}/26.1.2/{file}"));
            let text = fs::read_to_string(source).expect("version data should be readable");
            fs::write(root.join(format!("{VERSION_DATA}/1.21.1/{file}")), text)
                .expect("version data should be writable");
        }
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(violations.len(), VERSION_FILES.len() - 1, "{violations:?}");
        for violation in &violations {
            assert!(
                violation.contains("declares Minecraft 26.1.2 while its directory is named 1.21.1"),
                "{violations:?}"
            );
        }
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A version-data file that declares no version at all is held to nothing, so
    /// the absence is the violation rather than the excuse.
    #[test]
    fn a_version_file_declaring_no_version_is_reported() {
        let root = temporary_root("version-undeclared");
        version_directory(&root, "26.1.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/stubs.toml")),
            "schemaVersion = 1\n",
        )
        .expect("version data should be writable");
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "minecraft/versions/26.1.2/stubs.toml declares no `minecraft` version, so nothing \
                 holds it to the version minecraft/versions/26.1.2 is named for"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// An empty version tree is the vacuity the sweep replaced a literal list with,
    /// so it is a violation rather than nothing to say.
    #[test]
    fn a_version_tree_holding_no_version_is_not_read_as_agreement() {
        let root = temporary_root("version-empty");
        fs::create_dir_all(root.join(VERSION_DATA)).expect("version tree should be creatable");
        let mut violations = Vec::new();

        verify_version_directories(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("holds no version directory"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A hook document's subject is the version directory it links into, and the
    /// bare directory in prose names none.
    #[test]
    fn a_documents_subject_version_comes_from_the_directories_it_links_into() {
        let document = "Pinned in [adapter.toml](../../minecraft/versions/26.1.2/adapter.toml)\n\
                        beside minecraft/versions/26.1.2/hooks.toml, while minecraft/versions/ \
                        holds the rest.";

        let named = documented_versions(document);

        assert_eq!(named.into_iter().collect::<Vec<_>>(), ["26.1.2"]);
    }

    /// Writes one agent source and one figure, so a reflection test asks only
    /// about the surface.
    fn reflection_root(label: &str, source: &str, figure: usize) -> PathBuf {
        let root = temporary_root(label);
        version_directory(&root, "26.1.2", &[]);
        let sources = root.join(AGENT_JAR_SOURCES).join("dev/ravel/jvm");
        fs::create_dir_all(&sources).expect("agent sources should be creatable");
        fs::write(sources.join("Content.java"), source).expect("agent source should be writable");
        fs::create_dir_all(root.join(DECISION_RECORDS)).expect("records should be creatable");
        fs::write(
            root.join(JVM_ENTRY_DECISION),
            format!("The adapters resolve {figure} classes by name.\n"),
        )
        .expect("record should be writable");
        root
    }

    /// The defect the generated names close: `jvm/bootstrap` is one source tree
    /// for every tracked version, so a game name written there is one version's
    /// spelling asserted over all of them and is the obfuscated name of none.
    #[test]
    fn a_hand_written_game_literal_in_the_agent_sources_is_refused() {
        let root = reflection_root(
            "version-reflection-literal",
            "String identifier = \"net.minecraft.resources.Identifier\";\n    String one = \
             GameNames.ONE;\n",
            1,
        );
        let mut violations = Vec::new();

        verify_persistence_reflection(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0]
                .contains("spells the game class net.minecraft.resources.Identifier as a literal"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A role one version answers and another does not is a shared source that
    /// compiles against one game and not the other, which is what a per-version
    /// declared set has to catch.
    #[test]
    fn a_role_a_version_does_not_declare_is_refused() {
        let root = reflection_root(
            "version-reflection-role",
            "String one = GameNames.ONE;\n    String other = GameNames.STORAGE;\n",
            1,
        );
        let mut violations = Vec::new();

        verify_persistence_reflection(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("resolves the game class GameNames.STORAGE"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The other direction: a declared role nothing reaches says the surface
    /// drifted rather than that it is pinned.
    #[test]
    fn a_declared_role_no_source_resolves_is_refused() {
        let root = reflection_root("version-reflection-unused", "String nothing = null;\n", 1);
        let mut violations = Vec::new();

        verify_persistence_reflection(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("declares the role ONE for net.minecraft.client.One"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A table short of one of its three names is a role that resolves to nothing
    /// or a name held to nothing, so it is refused rather than dropped from the
    /// set — dropping it silently is how a surface goes unchecked while the count
    /// still looks right.
    #[test]
    fn a_reflected_table_missing_a_name_is_refused_rather_than_skipped() {
        let root = reflection_root(
            "version-reflection-partial",
            "String one = GameNames.ONE;\n",
            1,
        );
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/{REFLECTION_FILE}")),
            "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[reflected]]\nname = \"ONE\"\nclass = \
             \"net.minecraft.client.One\"\n",
        )
        .expect("version data should be writable");
        let mut violations = Vec::new();

        verify_persistence_reflection(&root, &mut violations);

        assert!(
            violations.iter().any(|violation| violation.contains(
                "declares a reflected table without all three of name, class and \
                           runtime"
            )),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Deleting one `reflection.toml` used to be reported as a missing-file line
    /// that reads like it is about something else, while the both-directions
    /// comparison and the figure anchored to it went quiet.
    #[test]
    fn a_version_without_a_reflection_file_disarms_no_check_silently() {
        let root = temporary_root("version-reflection-missing");
        version_directory(&root, "26.1.2", &[REFLECTION_FILE]);
        let mut violations = Vec::new();

        verify_persistence_reflection(&root, &mut violations);

        assert!(
            violations.iter().any(|violation| violation
                .contains("cannot read minecraft/versions/26.1.2/reflection.toml")),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The transformer decides which of ADG-7's seams a rewritten target is by
    /// comparing the generated hook name against a literal, and nothing bound
    /// those literals to a hook id: a version declaring the seam under another id
    /// renders another name, the comparison is false for every target, and the
    /// transform is silently not applied to a jar that still builds.
    #[test]
    fn a_hook_name_no_version_declares_is_refused() {
        let root = reflection_root(
            "version-hook-name",
            "boolean seam = target.hookMethod().equals(\"worldSave\");\n    String one = \
             GameNames.ONE;\n",
            1,
        );
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_hook_name_literals(&root, &inventories, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("matches the generated hook worldSave"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The same literal against the id it is rendered from, which is the whole
    /// coupling: the comparison holds across the separators the generator drops.
    #[test]
    fn a_hook_name_every_version_declares_is_accepted() {
        let root = reflection_root(
            "version-hook-name-declared",
            "boolean seam = target.hookMethod().equals(\"levelTick\");\n    String one = \
             GameNames.ONE;\n",
            1,
        );
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_hook_name_literals(&root, &inventories, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes one version whose inventory declares exactly the seams given.
    fn seam_root(label: &str, seams: &str) -> PathBuf {
        let root = temporary_root(label);
        version_directory(&root, "26.1.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/hooks.toml")),
            format!("schemaVersion = 1\nminecraft = \"26.1.2\"\n{seams}"),
        )
        .expect("version data should be writable");
        root
    }

    /// One entry, at the shape `Transformer.java`'s sequences are written for.
    fn seam(id: &str, kind: &str, descriptor: &str) -> String {
        format!(
            "\n[[hook]]\nid = \"{id}\"\nclass = \"net.minecraft.client.One\"\nmember = \
             \"one\"\nkind = \"{kind}\"\ndescriptor = \"{descriptor}\"\n"
        )
    }

    /// The three seams as every tracked version declares them.
    fn declared_seams() -> String {
        seam(
            "world.open-persistence",
            "instance-method",
            "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;Z)V",
        ) + &seam("world.save-persistence", "instance-method", "(ZZZ)Z")
            + &seam("screen.title-menu-options", "instance-method", "(II)V")
    }

    /// The defect this check was written for: 1.21.1's `doWorldLoad` takes four
    /// parameters where 26.1.2 takes five, and the transform loaded an int at the
    /// position the fifth used to sit in. Nothing could see it, because the
    /// transformer compiles against no version and only a launch would verify the
    /// rewritten class.
    #[test]
    fn a_seam_whose_loaded_parameter_moved_is_refused() {
        let root = seam_root(
            "transformer-shape-parameter",
            &(seam(
                "world.open-persistence",
                "instance-method",
                "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;)V",
            ) + &seam("world.save-persistence", "instance-method", "(ZZZ)Z")
                + &seam("screen.title-menu-options", "instance-method", "(II)V")),
        );
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_transformer_shape(&root, &inventories, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("the transformer loads a boolean there"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The second defect: the title-menu bridge injected at an int return, and
    /// 1.21.1 returns nothing, so the sequence matched no return instruction and
    /// the mods button was silently never added.
    #[test]
    fn a_seam_returning_what_no_sequence_is_written_for_is_refused() {
        let root = seam_root(
            "transformer-shape-return",
            &(seam(
                "world.open-persistence",
                "instance-method",
                "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;Z)V",
            ) + &seam("world.save-persistence", "instance-method", "(ZZZ)Z")
                + &seam(
                    "screen.title-menu-options",
                    "instance-method",
                    "(II)Ljava/lang/String;",
                )),
        );
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_transformer_shape(&root, &inventories, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("injects at its return for an int or nothing"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A receiver the transform loads that the target does not have. Local zero
    /// is the first argument on a static method, so the adapter would be handed
    /// an argument in place of the server the seam is about.
    #[test]
    fn a_seam_declared_static_where_the_receiver_is_loaded_is_refused() {
        let root = seam_root(
            "transformer-shape-static",
            &(seam(
                "world.open-persistence",
                "instance-method",
                "(Lnet/minecraft/One;Lnet/minecraft/Two;Lnet/minecraft/Three;Z)V",
            ) + &seam("world.save-persistence", "static-method", "(ZZZ)Z")
                + &seam("screen.title-menu-options", "instance-method", "(II)V")),
        );
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_transformer_shape(&root, &inventories, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("the transformer loads its receiver"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The shapes the sequences are written for, and the record that says which
    /// version was actually held to them. `devenv tasks run` prints nothing for a
    /// task that succeeds, so a check that only printed would pass invisibly.
    #[test]
    fn the_shapes_the_transformer_injects_against_are_recorded_by_version() {
        let root = seam_root("transformer-shape-held", &declared_seams());
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_transformer_shape(&root, &inventories, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
        let record = fs::read_to_string(root.join(TRANSFORMER_SHAPE_RECORD))
            .expect("the record should be written");
        assert_eq!(
            record,
            "held: Minecraft 26.1.2 against 3 transformer seams\n"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A sweep with no subject passes for the same reason a correct one does, and
    /// every seam here is named by data a version can stop declaring.
    #[test]
    fn a_run_no_version_names_a_seam_in_is_refused() {
        let root = seam_root(
            "transformer-shape-vacuous",
            &seam("level.tick", "instance-method", "()V"),
        );
        let inventories = hook_inventories(&root);
        let mut violations = Vec::new();

        verify_transformer_shape(&root, &inventories, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("checked against nothing"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Writes one screen template, and a version to hold it against.
    fn screen_root(label: &str, template: &str, mapping_source: &str) -> PathBuf {
        let root = temporary_root(label);
        version_directory(&root, "26.1.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/adapter.toml")),
            format!(
                "schemaVersion = 1\nminecraft = \"26.1.2\"\nmappingSource = \
                 \"{mapping_source}\"\n"
            ),
        )
        .expect("version data should be writable");
        let templates = root.join(SCREEN_JAR_SOURCES);
        fs::create_dir_all(&templates).expect("screen templates should be creatable");
        fs::write(templates.join("Content.java.in"), template)
            .expect("screen template should be writable");
        root
    }

    /// A template is one source for every tracked version, so a game name written
    /// into one is refused whatever the versions declare — the same rule the
    /// agent jar's sources are held to, for the same reason.
    #[test]
    fn a_template_that_spells_a_game_name_is_refused() {
        let root = screen_root(
            "screen-spelled",
            "import net.minecraft.client.One;\n",
            "unobfuscated-class-files",
        );
        let mut violations = Vec::new();

        verify_screen_templates(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("spells the game name net.minecraft.client.One"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The same name inside a placeholder is what the generator renders, so it is
    /// the shape the rule asks for rather than a second violation.
    #[test]
    fn a_template_that_names_a_game_class_in_a_placeholder_is_accepted() {
        let root = screen_root(
            "screen-placeholder",
            "{{import net.minecraft.client.One}}\nOne one = ({{net.minecraft.client.One}}) held;\n",
            "official-proguard-mappings",
        );
        let mut violations = Vec::new();

        verify_screen_templates(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A directory with no template at all would pass every line rule while
    /// generating no screen, so the absence is the violation.
    #[test]
    fn a_screen_template_directory_with_nothing_in_it_is_refused() {
        let root = temporary_root("screen-empty");
        fs::create_dir_all(root.join(SCREEN_JAR_SOURCES)).expect("creatable");
        let mut violations = Vec::new();

        verify_screen_templates(&root, &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains("holds no screen template"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The record owes one figure per distinct surface size. One figure held
    /// against every surface's length was unsatisfiable across two versions of
    /// different sizes, so the rule is set equality and both directions fail.
    #[test]
    fn the_resolved_class_figure_is_owed_once_per_distinct_surface_size() {
        let root = temporary_root("version-figures");
        for (version, declarations) in [
            (
                "26.1.2",
                "[[reflected]]\nname = \"ONE\"\nclass = \"net.minecraft.client.One\"\nruntime = \
                 \"net.minecraft.client.One\"\n",
            ),
            (
                "99.0.0-synthetic",
                "[[reflected]]\nname = \"ONE\"\nclass = \"net.minecraft.client.One\"\nruntime = \
                 \"net.minecraft.client.One\"\n\n[[reflected]]\nname = \"TWO\"\nclass = \
                 \"net.minecraft.client.Two\"\nruntime = \"net.minecraft.client.Two\"\n",
            ),
        ] {
            version_directory(&root, version, &[]);
            fs::write(
                root.join(format!("{VERSION_DATA}/{version}/{REFLECTION_FILE}")),
                format!("schemaVersion = 1\nminecraft = \"{version}\"\n\n{declarations}"),
            )
            .expect("version data should be writable");
        }
        fs::create_dir_all(root.join(DECISION_RECORDS)).expect("records should be creatable");
        fs::write(
            root.join(JVM_ENTRY_DECISION),
            "The adapters resolve 1 classes by name, and 3 classes by name at the other.\n",
        )
        .expect("record should be writable");
        let mut violations = Vec::new();

        let surfaces = reflective_surfaces(&root, &mut violations);
        verify_resolved_class_figure(&root, &surfaces, &mut violations);

        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations[0].contains("99.0.0-synthetic/reflection.toml declares 2 classes"),
            "{violations:?}"
        );
        assert!(
            violations[1].contains("resolve 3 classes by name while no tracked version declares"),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A document states one inventory's counts, so a document that links into no
    /// version directory, or into two, names no subject and is refused rather than
    /// held to whichever inventory happened to be first.
    #[test]
    fn a_hook_document_naming_no_single_version_is_refused() {
        for (label, links, expected) in [
            ("document-none", "It links nowhere.\n", "links into 0"),
            (
                "document-many",
                "See minecraft/versions/26.1.2/hooks.toml and \
                 minecraft/versions/99.0.0-synthetic/hooks.toml.\n",
                "links into 2",
            ),
        ] {
            let root = temporary_root(label);
            version_directory(&root, "26.1.2", &[]);
            let document = root.join(HOOK_DOCUMENT);
            fs::create_dir_all(
                document
                    .parent()
                    .expect("the document sits inside a directory"),
            )
            .expect("document directory should be creatable");
            fs::write(&document, links).expect("document should be writable");
            let mut violations = Vec::new();

            verify_hook_inventory_document(&root, &hook_inventories(&root), &mut violations);

            assert_eq!(violations.len(), 1, "{violations:?}");
            assert!(violations[0].contains(expected), "{violations:?}");
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    /// A document whose subject is a version nothing tracks is refused too: the
    /// counts it states are held to an inventory that does not exist.
    #[test]
    fn a_hook_document_naming_an_untracked_version_is_refused() {
        let root = temporary_root("document-untracked");
        version_directory(&root, "26.1.2", &[]);
        let document = root.join(HOOK_DOCUMENT);
        fs::create_dir_all(
            document
                .parent()
                .expect("the document sits inside a directory"),
        )
        .expect("document directory should be creatable");
        fs::write(
            &document,
            "Counted from minecraft/versions/99.0.0-synthetic/hooks.toml.\n",
        )
        .expect("document should be writable");
        let mut violations = Vec::new();

        verify_hook_inventory_document(&root, &hook_inventories(&root), &mut violations);

        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].contains(
                "states the counts of minecraft/versions/99.0.0-synthetic, which is not a tracked \
                 version"
            ),
            "{violations:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A decision record denies a mechanism outright, so the denial is false as
    /// soon as *any* tracked version declares the hook. The version the reader is
    /// not on must not excuse the sentence, and the message names the inventory
    /// that falsifies it.
    #[test]
    fn a_denial_is_falsified_by_any_tracked_versions_inventory() {
        let root = temporary_root("decision-union");
        version_directory(&root, "26.1.2", &[]);
        version_directory(&root, "99.0.0-synthetic", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/99.0.0-synthetic/hooks.toml")),
            "schemaVersion = 1\nminecraft = \"99.0.0-synthetic\"\n\n[[hook]]\nid = \
             \"screen.extract-render-state\"\nkind = \"instance-method\"\nmember = \
             \"extractRenderState\"\ndescriptor = \"()V\"\n",
        )
        .expect("version data should be writable");
        fs::create_dir_all(root.join(DECISION_RECORDS)).expect("records should be creatable");
        fs::write(
            root.join(format!("{DECISION_RECORDS}/adg-6-example.md")),
            "Ravel does **not** hook `extractRenderState`.\n",
        )
        .expect("record should be writable");
        let mut violations = Vec::new();

        verify_decision_hook_claims(&root, &hook_inventories(&root), &mut violations);

        assert_eq!(
            violations,
            [
                "docs/decisions/adg-6-example.md says `extractRenderState` is not hooked while \
                 minecraft/versions/99.0.0-synthetic/hooks.toml declares it"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// The reflected names became version data in the same change that made them
    /// version data, so the version-boundary sweep reads them: a general crate
    /// spelling one is welded to that version whether the generator reads the file
    /// or not.
    #[test]
    fn a_reflected_class_name_is_version_data_the_boundary_sweep_reads() {
        let root = temporary_root("version-classes");
        version_directory(&root, "26.1.2", &[]);
        fs::write(
            root.join(format!("{VERSION_DATA}/26.1.2/{REFLECTION_FILE}")),
            "schemaVersion = 1\nminecraft = \"26.1.2\"\n\n[[reflected]]\nclass = \
             \"net.minecraft.resources.Identifier\"\n",
        )
        .expect("version data should be writable");

        let classes = version_class_names(&root);

        assert!(
            classes.contains(&"net/minecraft/resources/Identifier".to_owned()),
            "{classes:?}"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// Both spellings of a game class name are read out of a literal, and neither a
    /// doc comment nor an unpaired quotation mark in prose is read as one.
    #[test]
    fn a_game_class_name_counts_from_a_literal_and_not_from_prose() {
        let source = "\
            /**\n\
             * Resolves \"net.minecraft.world.level.storage.LevelResource\" in prose,\n\
             * and mentions net.minecraft.client.Minecraft without quoting it.\n\
             */\n\
            private static final String STORAGE = \"net.minecraft.world.level.storage.SavedDataStorage\";\n\
            // A commented-out \"net.minecraft.util.datafix.DataFixers\".\n\
            return target.internalClass().equals(\"net/minecraft/client/Minecraft\");\n\
            String other = \"dev.ravel.jvm.screen.RavelWorldData\";\n";

        assert_eq!(
            game_class_literals(source),
            [
                "net.minecraft.world.level.storage.SavedDataStorage",
                "net.minecraft.client.Minecraft"
            ]
        );
    }

    #[test]
    fn the_tracked_campaign_document_agrees_with_the_tracked_launch_record() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_campaign_inventory(&root, &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// No accepted decision denies a hook the pinned inventory declares.
    #[test]
    fn the_tracked_decision_records_deny_no_hook_the_inventory_declares() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_decision_hook_claims(&root, &hook_inventories(&root), &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// The two shapes the stale ADG-6 sentences had, against the names the pinned
    /// inventory actually declares.
    #[test]
    fn a_denial_of_a_declared_hook_is_found_by_id_and_by_member() {
        let inventory = "\
            [[hook]]\n\
            id = \"screen.extract-render-state\"\n\
            member = \"extractRenderState\"\n";
        let declared = declared_hook_names(inventory);

        assert_eq!(
            declared,
            ["screen.extract-render-state", "extractRenderState"]
        );
        assert_eq!(
            denied_hook_names("It does **not** hook `extractRenderState`.", &declared),
            ["extractRenderState"]
        );
        assert_eq!(
            denied_hook_names(
                "The record does not hook screen.extract-render-state.",
                &declared
            ),
            ["screen.extract-render-state"]
        );
        // The inflected passive, and the order that puts the name first. Round 10
        // wrote this exact sentence into ADG-6 and the gate stayed at zero.
        assert_eq!(
            denied_hook_names(
                "The `extractRenderState` seam is not hooked by Ravel.",
                &declared
            ),
            ["extractRenderState"]
        );
        assert_eq!(
            denied_hook_names(
                "Ravel is not hooking `screen.extract-render-state` in this version.",
                &declared
            ),
            ["screen.extract-render-state"]
        );
        // A denial of something the inventory does not declare is the record's
        // business, and a declared hook named without a denial is ordinary prose.
        assert!(denied_hook_names("It does not hook `Screen.render`.", &declared).is_empty());
        assert!(
            denied_hook_names(
                "The inventory carries `extractRenderState`, the per-frame seam.",
                &declared
            )
            .is_empty()
        );
    }

    #[test]
    fn the_tracked_hook_document_agrees_with_the_tracked_inventory() {
        let root = std::env::current_dir()
            .expect("the current directory must be available")
            .join("../..");
        let mut violations = Vec::new();

        verify_hook_inventory_document(&root, &hook_inventories(&root), &mut violations);

        assert_eq!(violations, Vec::<String>::new());
    }

    /// A root holding one M4 surface record over a tree of known shape.
    ///
    /// The guide and conformance directories are given one file each rather than
    /// the repository's own, so the expectation the check computes is small
    /// enough to write out and the test does not go stale when a guide page is
    /// added.
    fn surface_root(label: &str, guide: &[&str], suites: &[&str], record: &str) -> PathBuf {
        let root = temporary_root(label);
        for (directory, files) in [("docs/guide", guide), ("tests/conformance/tests", suites)] {
            let directory = root.join(directory);
            fs::create_dir_all(&directory).expect("directory should be creatable");
            for file in files {
                fs::write(directory.join(file), "").expect("file should be writable");
            }
        }
        let milestone = root.join("evidence/m4");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("sdk-surface.json"), record).expect("record should be writable");
        root
    }

    /// The surface record naming exactly what a `surface_root` tree holds.
    fn surface_record(guide: &[&str], suites: &[&str]) -> String {
        let quoted = |names: &[&str]| {
            names
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            r#"{{"inventory":{{"crates":[{}],"testMods":[{}],"examples":[{}],
                "guideDocuments":[{}],"conformanceSuites":[{}]}}}}"#,
            quoted(CRATE_NAMES),
            quoted(TEST_MOD_NAMES),
            quoted(EXAMPLE_NAMES),
            quoted(guide),
            quoted(suites),
        )
    }

    #[test]
    fn a_surface_inventory_naming_what_the_repository_holds_is_accepted() {
        let root = surface_root(
            "surface-agrees",
            &["README.md"],
            &["one.rs"],
            &surface_record(&["README.md"], &["one.rs"]),
        );

        let mut violations = Vec::new();
        verify_m4_surface(&root, &mut violations);

        assert!(violations.is_empty(), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_surface_inventory_naming_what_the_repository_lost_is_reported() {
        // A guide page and a conformance suite the record names and the tree
        // does not: the deletion falsifies what M4 measured, and an inventory
        // checked by length alone would report neither.
        let root = surface_root(
            "surface-drifted",
            &["README.md"],
            &["one.rs"],
            &surface_record(&["README.md", "removed.md"], &["one.rs", "deleted.rs"]),
        );

        let mut violations = Vec::new();
        verify_m4_surface(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "evidence/m4/sdk-surface.json names removed.md under inventory.guideDocuments, \
                 which the repository does not hold",
                "evidence/m4/sdk-surface.json names deleted.rs under inventory.conformanceSuites, \
                 which the repository does not hold",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_surface_inventory_omitting_what_a_later_milestone_added_is_accepted() {
        // The record measured M4's tree; a guide page and a conformance suite
        // added afterwards are the adding milestone's to account for. Reporting
        // them here would make every later milestone edit M4's closed evidence.
        let root = surface_root(
            "surface-later-additions",
            &["README.md", "added-later.md"],
            &["one.rs", "added_later.rs"],
            &surface_record(&["README.md"], &["one.rs"]),
        );

        let mut violations = Vec::new();
        verify_m4_surface(&root, &mut violations);

        assert!(violations.is_empty(), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A root holding an M4 manifest and matrix record with the given figures.
    fn gate_figures_root(label: &str, manifest: &str, matrix: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m4");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("manifest.json"), manifest).expect("manifest should be writable");
        fs::write(milestone.join("linux-process-matrix.json"), matrix)
            .expect("record should be writable");
        root
    }

    #[test]
    fn a_manifest_and_a_matrix_reporting_the_same_run_are_accepted() {
        let root = gate_figures_root(
            "gate-figures-agree",
            r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                "lineCoveragePercent":85.97}}"#,
            r#"{"gate":{"tests":{"run":9,"passed":9,"failed":0,"skipped":0},
                "coverage":{"linesPercent":85.97}}}"#,
        );

        let mut violations = Vec::new();
        verify_m4_gate_figures(
            &root,
            &root.join("evidence/m4/manifest.json"),
            &mut violations,
        );

        assert!(violations.is_empty(), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_manifest_and_a_matrix_reporting_different_runs_are_reported() {
        // One run written down twice is one figure free to go stale in one place.
        let root = gate_figures_root(
            "gate-figures-differ",
            r#"{"tests":{"total":9,"passed":9,"failed":0,"skipped":0,
                "lineCoveragePercent":85.97}}"#,
            r#"{"gate":{"tests":{"run":8,"passed":9,"failed":0,"skipped":0},
                "coverage":{"linesPercent":85.97}}}"#,
        );

        let mut violations = Vec::new();
        verify_m4_gate_figures(
            &root,
            &root.join("evidence/m4/manifest.json"),
            &mut violations,
        );

        assert_eq!(
            violations,
            ["evidence/m4/manifest.json records .tests.total 9 while \
                 evidence/m4/linux-process-matrix.json records .gate.tests.run 8; they describe \
                 one gate run",]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A root holding one network crate and one guide page.
    fn guide_enumeration_root(label: &str, source: &str, guide: &str) -> PathBuf {
        let root = temporary_root(label);
        fs::create_dir_all(root.join("crates/ravel-network/src"))
            .expect("directory should be creatable");
        fs::write(root.join(NETWORK_SOURCE), source).expect("source should be writable");
        fs::create_dir_all(root.join(GUIDE_DIRECTORY)).expect("directory should be creatable");
        fs::write(root.join(GUIDE_DIRECTORY).join("running.md"), guide)
            .expect("guide should be writable");
        root
    }

    /// A network source declaring both enums, with `WireError` given `variants`.
    fn network_source(variants: &[&str]) -> String {
        let mut source = String::from("pub enum WireError {\n");
        for variant in variants {
            source.push_str("    /// A refusal.\n    ");
            source.push_str(variant);
            source.push_str(" {\n        /// A field.\n        Needed: usize,\n    },\n");
        }
        source.push_str("}\n\npub enum DeliveryRefused {\n    /// A refusal.\n    WrongSide,\n}\n");
        source
    }

    #[test]
    fn a_guide_enumerating_every_variant_the_code_declares_is_accepted() {
        let root = guide_enumeration_root(
            "guide-enumeration-agrees",
            &network_source(&["Truncated", "NotUtf8"]),
            "`WireError` is the whole of those refusals: `WireError::Truncated` and \
             `WireError::NotUtf8`.\n\nSeparately, `DeliveryRefused::WrongSide`.\n",
        );

        let mut violations = Vec::new();
        verify_guide_enumerations(&root, &mut violations);

        assert!(violations.is_empty(), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_guide_enumeration_that_has_drifted_from_the_code_is_reported() {
        // Both directions: a variant the guide invents and one it leaves out. The
        // omission is the half the guide's own false completeness claim hid.
        let root = guide_enumeration_root(
            "guide-enumeration-drifted",
            &network_source(&["Truncated", "NotUtf8"]),
            "`WireError` is the whole of those refusals: `WireError::Truncated` and \
             `WireError::NotABool`.\n",
        );

        let mut violations = Vec::new();
        verify_guide_enumerations(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "docs/guide names WireError::NotABool and crates/ravel-network/src/lib.rs declares \
                 no such variant; an author told to match on a name that does not exist finds out \
                 at the compiler rather than here",
                "docs/guide/running.md's enumeration of WireError omits WireError::NotUtf8, which \
                 crates/ravel-network/src/lib.rs declares; a variant left out of a list an author \
                 reads as complete is the one their match arm will not have, and naming it on \
                 another page does not complete this one",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_variant_named_on_another_page_does_not_complete_the_enumeration() {
        // The escape a round found: the omitted variant is pasted somewhere else
        // under docs/guide, so a check reading the concatenated directory sees a
        // complete set while the passage an author reads is short one.
        let root = guide_enumeration_root(
            "guide-enumeration-elsewhere",
            &network_source(&["Truncated", "NotUtf8"]),
            "`WireError` is the whole of those refusals: `WireError::Truncated`.\n",
        );
        fs::write(
            root.join(GUIDE_DIRECTORY).join("limits.md"),
            "In passing, `WireError::NotUtf8` is also refused.\n",
        )
        .expect("guide should be writable");

        let mut violations = Vec::new();
        verify_guide_enumerations(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "docs/guide/running.md's enumeration of WireError omits WireError::NotUtf8, which \
              crates/ravel-network/src/lib.rs declares; a variant left out of a list an author \
              reads as complete is the one their match arm will not have, and naming it on \
              another page does not complete this one",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_guide_that_drops_its_completeness_claim_is_reported() {
        // Deleting the sentence would otherwise retire the obligation while the
        // list an author reads stays exactly where it was.
        let root = guide_enumeration_root(
            "guide-enumeration-claimless",
            &network_source(&["Truncated", "NotUtf8"]),
            "`WireError::Truncated` and `WireError::NotUtf8` are refusals.\n",
        );

        let mut violations = Vec::new();
        verify_guide_enumerations(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "docs/guide/running.md no longer holds the passage saying WireError \"is the whole \
              of those refusals\", so no list an author reads as complete is held to \
              crates/ravel-network/src/lib.rs; the claim is what this check exists for and \
              dropping it while keeping the list is not a way out of it",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_guide_naming_part_of_an_enum_it_does_not_enumerate_is_accepted() {
        // The asymmetry is the point: the guide has no reason to list every
        // delivery refusal, so silence about one is not a violation while a name
        // that does not exist still is.
        let root = guide_enumeration_root(
            "guide-enumeration-partial",
            &network_source(&["Truncated"]),
            "`WireError` is the whole of those refusals: `WireError::Truncated`, and nothing \
             about `DeliveryRefused` at all.\n",
        );

        let mut violations = Vec::new();
        verify_guide_enumerations(&root, &mut violations);

        assert!(violations.is_empty(), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A root holding a gate definition that runs `tasks`, and one matrix record.
    fn process_matrix_root(label: &str, tasks: &[&str], matrix: &str) -> PathBuf {
        let root = temporary_root(label);
        let mut after = String::new();
        for task in tasks {
            after.push_str("          \"");
            after.push_str(task);
            after.push_str("\"\n");
        }
        fs::create_dir_all(root.join("nix")).expect("directory should be creatable");
        fs::write(
            root.join(GATE_DEFINITION),
            format!(
                "  tasks = {{\n    \"ravel:verify-scaffold\".exec = \"x\";\n\
                 \n    \"ravel:verify\" = {{\n      after = [\n{after}      ];\n\
                 \x20     exec = \"true\";\n    }};\n  }};\n"
            ),
        )
        .expect("gate definition should be writable");
        let milestone = root.join("evidence/m4");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("linux-process-matrix.json"), matrix)
            .expect("record should be writable");
        root
    }

    #[test]
    fn a_process_matrix_naming_exactly_the_gates_sub_tasks_is_accepted() {
        let root = process_matrix_root(
            "matrix-agrees",
            &["ravel:fmt", "ravel:audit"],
            r#"{"result":"pass","processes":[
                {"command":"devenv tasks run ravel:fmt","exitStatus":0},
                {"command":"devenv tasks run ravel:audit","exitStatus":0}]}"#,
        );

        let mut violations = Vec::new();
        verify_m4_process_matrix(&root, &mut violations);

        assert!(violations.is_empty(), "{violations:?}");
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_process_matrix_that_has_drifted_from_the_gate_is_reported() {
        // Both directions in one record, for the reason the surface inventory is
        // read both ways: a fabricated row and an omitted one cancel out under
        // any check that compares lengths.
        let root = process_matrix_root(
            "matrix-drifted",
            &["ravel:fmt", "ravel:audit"],
            r#"{"result":"pass","processes":[
                {"command":"devenv tasks run ravel:fmt","exitStatus":0},
                {"command":"devenv tasks run ravel:this-task-does-not-exist","exitStatus":0}]}"#,
        );

        let mut violations = Vec::new();
        verify_m4_process_matrix(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "evidence/m4/linux-process-matrix.json records ravel:this-task-does-not-exist \
                 under processes, which nix/common.nix does not list among the sub-tasks \
                 ravel:verify runs; a row the gate never ran took its exit status from somewhere \
                 the record does not name",
                "evidence/m4/linux-process-matrix.json omits ravel:audit from processes, which \
                 nix/common.nix lists among the sub-tasks ravel:verify runs; a gate process with \
                 no row is how a failing one leaves a record that still reads complete",
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_failed_process_under_a_record_that_calls_itself_a_pass_is_reported() {
        let root = process_matrix_root(
            "matrix-red-row",
            &["ravel:fmt"],
            r#"{"result":"pass","processes":[
                {"command":"devenv tasks run ravel:fmt","exitStatus":1}]}"#,
        );

        let mut violations = Vec::new();
        verify_m4_process_matrix(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "evidence/m4/linux-process-matrix.json records a nonzero exit status for `devenv \
                 tasks run ravel:fmt` under .processes while its own result is pass; a record that \
                 reports a failed process and calls itself green states two things that cannot \
                 both be true"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_gate_sub_task_recorded_as_a_check_outside_the_gate_is_reported() {
        // The section exists so a hand-run check can be recorded honestly. It
        // would also be the obvious way to evade the reconciliation above, which
        // is why membership of it is checked rather than trusted.
        let root = process_matrix_root(
            "matrix-parked-row",
            &["ravel:fmt"],
            r#"{"result":"pass",
                "processes":[{"command":"devenv tasks run ravel:fmt","exitStatus":0}],
                "checksOutsideTheGate":[
                    {"command":"devenv tasks run ravel:fmt","exitStatus":0}]}"#,
        );

        let mut violations = Vec::new();
        verify_m4_process_matrix(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "evidence/m4/linux-process-matrix.json records ravel:fmt under checksOutsideTheGate \
                 while nix/common.nix lists it among the sub-tasks ravel:verify runs; that section \
                 is for checks the gate does not run and is not a way out of the reconciliation"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_recorded_command_that_names_no_task_is_reported() {
        let root = process_matrix_root(
            "matrix-bare-command",
            &["ravel:fmt"],
            r#"{"result":"pass","processes":[
                {"command":"devenv tasks run ravel:fmt","exitStatus":0},
                {"command":"cargo test","exitStatus":0}]}"#,
        );

        let mut violations = Vec::new();
        verify_m4_process_matrix(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "evidence/m4/linux-process-matrix.json records the command `cargo test` under \
                 .processes, which does not name a devenv task; a row nothing can resolve to a \
                 task is reconciled against nothing"
            ]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    /// A summary record whose parts close against every total.
    fn balanced_summary() -> String {
        r#"{"artifacts":{"total":10,"catalogued":10,"uncatalogued":0,"planned":10},
            "graph":{"nodes":12,"resolvedNodes":11,"blockedNodes":1,"edges":20,
                "resolvedEdges":20,"blockedEdges":0,"clusters":3,"largestCluster":5},
            "rights":{"sourceUse":{"permitted":4,"conditional":3,"prohibited":2,
                "unresolved":1},"publishable":0},
            "surfaces":{"catalogued":10,"unclassified":0},
            "plans":{"planned":10,"readingSource":4,"notReadingSource":6,
                "canonicalIds":10,"dependencyEdges":7},
            "reads":{"producersWatched":2,"archiveReads":99,"entryBodyReads":0},
            "boundArtifacts":[{"name":"a.json","sha256":"0","holds":"x"}]}"#
            .to_owned()
    }

    fn summary_root(label: &str, summary: &str) -> PathBuf {
        let root = temporary_root(label);
        let milestone = root.join("evidence/m5");
        fs::create_dir_all(&milestone).expect("evidence directory should be creatable");
        fs::write(milestone.join("inventory-summary.json"), summary)
            .expect("summary should be writable");
        root
    }

    #[test]
    fn a_balanced_inventory_summary_closes_its_own_arithmetic() {
        let root = summary_root("m5-arithmetic-balanced", &balanced_summary());
        let mut violations = Vec::new();

        verify_m5_inventory_arithmetic(&root, &mut violations);

        assert!(
            violations.is_empty(),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn moving_one_part_of_an_inventory_total_stops_it_summing() {
        // The whole point of recording a decomposition: the total and its pieces
        // cannot drift apart quietly, whichever of them a hand edits.
        let root = summary_root(
            "m5-arithmetic-drift",
            &balanced_summary().replace(r#""prohibited":2"#, r#""prohibited":3"#),
        );
        let mut violations = Vec::new();

        verify_m5_inventory_arithmetic(&root, &mut violations);

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("exactly one source-use verdict")),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_entry_body_read_is_refused_however_the_record_reads() {
        let root = summary_root(
            "m5-arithmetic-reads",
            &balanced_summary().replace(r#""entryBodyReads":0"#, r#""entryBodyReads":1"#),
        );
        let mut violations = Vec::new();

        verify_m5_inventory_arithmetic(&root, &mut violations);

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("metadata-only method excludes")),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_manifest_figure_must_follow_from_the_record_it_summarises() {
        let root = summary_root("m5-carried-figures", &balanced_summary());
        let manifest = root.join("evidence/m5/manifest.json");
        fs::write(
            &manifest,
            r#"{"environment":{"settings":{"inventory":{"artifacts":11,"planned":10,
                "graphNodes":12,"graphEdges":20,"clusters":3,"sourceProhibited":2,
                "sourceUnresolved":1,"publishable":0,"readingSource":4,
                "dependencyEdges":7,"archiveReads":99,"entryBodyReads":0,
                "digestsBound":1}}}}"#,
        )
        .expect("manifest should be writable");
        let mut violations = Vec::new();

        verify_m5_carried_figures(&root, &manifest, &mut violations);

        assert_eq!(
            violations.len(),
            1,
            "violations:\n{}",
            violations.join("\n")
        );
        assert!(
            violations[0].contains("records .environment.settings.inventory.artifacts 11"),
            "violation was: {}",
            violations[0]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn a_symbol_a_finding_could_not_have_meant_is_not_a_symbol() {
        // The detection floor this rule discloses, exercised rather than
        // described: `open` is defined all over this tree and is also a word.
        assert!(!distinguishable_symbol("open"));
        assert!(!distinguishable_symbol("run"));
        assert!(distinguishable_symbol("verify_tracked_contents"));
        assert!(distinguishable_symbol("LoaderRuntime"));
    }

    #[test]
    fn the_tree_decides_what_a_finding_named_rather_than_its_author() {
        let root = temporary_root("m5-defined-symbols");
        fs::write(
            root.join("source.rs"),
            "fn verify_something_narrow() {}\nstruct WidgetHandle;\nfn go() {}\n",
        )
        .expect("source should be writable");
        let sources = vec!["source.rs".to_owned()];

        let defined = defined_rust_symbols(&root, &sources);

        assert!(defined.contains("verify_something_narrow"));
        assert!(defined.contains("WidgetHandle"));
        assert!(
            !defined.contains("go"),
            "a two-letter item is below the floor"
        );
        assert_eq!(
            named_rust_symbols("the WidgetHandle is never built", &defined),
            vec!["WidgetHandle".to_owned()]
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn an_anchor_goes_stale_when_a_caller_appears_outside_its_path_set() {
        let root = temporary_root("m5-anchor-stale");
        fs::create_dir_all(root.join("elsewhere")).expect("directory should be creatable");
        fs::write(root.join("source.rs"), "fn verify_something_narrow() {}\n")
            .expect("source should be writable");
        fs::write(
            root.join("elsewhere/caller.rs"),
            "fn call() { verify_something_narrow(); }\n",
        )
        .expect("source should be writable");
        let sources = vec!["source.rs".to_owned(), "elsewhere/caller.rs".to_owned()];

        let naming = sources_naming(&root, &sources, "verify_something_narrow");

        assert_eq!(naming, vec!["source.rs", "elsewhere/caller.rs"]);
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn the_round_count_rule_sweeps_every_milestone_it_is_not_exempt_from() {
        let subjects = round_count_subjects();

        // Derived from the manifest list, so a milestone cannot be forgotten into
        // it — which is exactly how M4 escaped the rule its own record cited.
        assert_eq!(
            subjects.len(),
            MILESTONE_MANIFESTS.len() - ROUND_COUNT_EXEMPT.len()
        );
        assert!(
            subjects
                .iter()
                .any(|(record, _)| record == "evidence/m5/review.md"),
            "M5 must be swept: {subjects:?}"
        );
        assert!(
            !subjects
                .iter()
                .any(|(record, _)| record == "evidence/m0/review.md"),
            "M0 is exempt: {subjects:?}"
        );
    }

    /// The head of a counterpart summary, up to the parts a mutation moves.
    const M6_SUMMARY_HEAD: &str = r#"{"milestone":"M6","workPackage":"M6-W6",
        "protocol":"m6-counterpart-v1","result":"pass","#;

    /// The frozen wave, its counterparts, and the totals over them.
    const M6_TWO_COUNTERPARTS: &str = r#""wave":{"frozen":true,"entries":[
            {"id":"e1","reason":"foundation value","dependencies":[]},
            {"id":"e2","reason":"cluster partner","dependencies":["e1"]}]},
        "counterparts":[
            {"id":"c1","entry":"e1","specification":{"accepted":true,
                "structureReviewed":true,"carriesUpstreamIdentifier":false,"route":"source"},
             "observableAreas":6,"declarableOnly":4,"exercised":2,
             "provenance":{"complete":true},"suites":{"total":12,"failed":0}},
            {"id":"c2","entry":"e2","specification":{"accepted":true,
                "structureReviewed":true,"carriesUpstreamIdentifier":false,"route":"observed"},
             "observableAreas":4,"declarableOnly":3,"exercised":1,
             "provenance":{"complete":true},"suites":{"total":8,"failed":0}}],
        "processValidation":{"template":"counterpart-template.md",
            "checklist":"counterpart-checklist.md",
            "evidenceSchema":"counterpart.schema.json","secondCounterpart":"c2"},
        "totals":{"counterparts":2,"observableAreas":10,"declarableOnly":7,"exercised":3},"#;

    /// The same wave with the second counterpart never written.
    const M6_ONE_COUNTERPART: &str = r#""wave":{"frozen":true,"entries":[
            {"id":"e1","reason":"foundation value","dependencies":[]}]},
        "counterparts":[
            {"id":"c1","entry":"e1","specification":{"accepted":true,
                "structureReviewed":true,"carriesUpstreamIdentifier":false,"route":"source"},
             "observableAreas":6,"declarableOnly":4,"exercised":2,
             "provenance":{"complete":true},"suites":{"total":12,"failed":0}}],
        "processValidation":{"template":"counterpart-template.md",
            "checklist":"counterpart-checklist.md",
            "evidenceSchema":"counterpart.schema.json","secondCounterpart":"c1"},
        "totals":{"counterparts":1,"observableAreas":6,"declarableOnly":4,"exercised":2},"#;

    /// The cluster scenario and the rewrite-integrity check's own results.
    const M6_SUMMARY_TAIL: &str = r#""cluster":{"scenario":"smallest gameplay cluster","steps":[
            {"name":"fresh-play","result":"pass"},{"name":"save","result":"pass"},
            {"name":"exit","result":"pass"},{"name":"reload","result":"pass"}]},
        "rewriteIntegrity":{"enforcing":true,"lexicon":"lexicon.json",
            "identifierSet":"upstream-identifiers.json","filesScanned":214,
            "artifactsScanned":18,"violations":0,"specificationViolations":0,
            "mutationProof":{"sourceInjection":{"exitStatus":1},
                "artifactInjection":{"exitStatus":1},"cleanRun":{"exitStatus":0}}}}"#;

    fn balanced_counterpart_summary() -> String {
        [M6_SUMMARY_HEAD, M6_TWO_COUNTERPARTS, M6_SUMMARY_TAIL].concat()
    }

    fn balanced_m6_manifest() -> String {
        r#"{"workPackages":["M6-W1","M6-W2","M6-W3","M6-W4","M6-W5","M6-W6"],
            "benchmarks":{"protocolVersion":"m6-counterpart-v1",
                "results":[{"name":"counterpart-summary.json"}]},
            "environment":{"settings":{
                "gateStatus":{
                    "M6-W1":{"status":"closed","evidence":"wave.json"},
                    "M6-W2":{"status":"closed","evidence":"sdk-gaps.md"},
                    "M6-W3":{"status":"closed","evidence":"specifications.md"},
                    "M6-W4":{"status":"closed","evidence":"cluster.json"},
                    "M6-W5":{"status":"closed","evidence":"process.md"},
                    "M6-W6":{"status":"open","evidence":""}},
                "rewriteIntegrity":{"enforcing":true,"filesScanned":214,
                    "artifactsScanned":18,"violations":0,
                    "sourceInjectionExitStatus":1,"artifactInjectionExitStatus":1},
                "counterparts":{"counterparts":2,"waveEntries":2,"observableAreas":10,
                    "declarableOnly":7,"exercised":3}}},
            "attestations":[{"platform":"linux","architecture":"x86_64",
                "workloads":["official-atm11","vanilla"]}],
            "deviations":[{"classification":"minor"}],
            "risks":[{"classification":"minor"}]}"#
            .to_owned()
    }

    fn library_summary(total: u32, correction: &str) -> String {
        format!(
            r#"{{"milestone":"M7","workPackage":"M7-W5","protocol":"m7-library-v1",
                "result":"pass",
                "entries":{{"total":{total},"accepted":{total},"blocked":0,"unstarted":0}},
                "correction":{correction},
                "waves":[{{"index":0,"entries":{first},"dependsOn":[]}},
                    {{"index":1,"entries":120,"dependsOn":[0]}},
                    {{"index":2,"entries":104,"dependsOn":[0,1]}}],
                "dependencies":{{"resolved":900,"unresolved":0}},
                "registries":{{"resolved":400,"unresolved":0}},
                "provenance":{{"present":{total},"missing":0}},
                "routing":{{"absorbed":40,"remaining":{remaining},
                    "runnableToday":{total},"blocked":0}},
                "suites":{{"packages":{{"total":{total},"failed":0}},
                    "clusters":{{"total":12,"failed":0}}}}}}"#,
            first = total - 224,
            remaining = total - 40,
        )
    }

    fn balanced_m7_manifest() -> String {
        r#"{"workPackages":["M7-W0","M7-W1","M7-W2","M7-W3","M7-W4","M7-W5","M7-W6"],
            "benchmarks":{"protocolVersion":"m7-library-v1",
                "results":[{"name":"library-summary.json"}]},
            "environment":{"settings":{
                "gateStatus":{
                    "M7-W0":{"status":"closed","evidence":"sdk.md"},
                    "M7-W1":{"status":"closed","evidence":"waves.json"},
                    "M7-W2":{"status":"closed","evidence":"packages.json"},
                    "M7-W3":{"status":"closed","evidence":"clusters.json"},
                    "M7-W4":{"status":"closed","evidence":"ledger.json"},
                    "M7-W5":{"status":"closed","evidence":"audit.md"},
                    "M7-W6":{"status":"open","evidence":""}},
                "library":{"entries":324,"accepted":324,"blocked":0,"unstarted":0,
                    "waves":3,"dependenciesUnresolved":0,"registriesUnresolved":0,
                    "provenancePresent":324,"absorbed":40,"packageSuites":324,
                    "clusterSuites":12}}},
            "attestations":[{"platform":"linux","architecture":"x86_64",
                "workloads":["official-atm11","vanilla"]}],
            "deviations":[{"classification":"minor"}],
            "risks":[{"classification":"minor"}]}"#
            .to_owned()
    }

    /// One closure-clause case: its label, the manifest rewrites it applies, and
    /// the refusal the rewritten manifest has to draw.
    type ClosureCase<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);

    /// Applies each replacement in turn, refusing one the fixture does not carry.
    fn mutate(document: &str, replacements: &[(&str, &str)]) -> String {
        let mut mutated = document.to_owned();
        for (from, to) in replacements {
            assert!(
                mutated.contains(from),
                "the fixture does not carry `{from}`, so replacing it mutates nothing"
            );
            mutated = mutated.replace(from, to);
        }
        mutated
    }

    fn m6_root(label: &str, summary: &str, manifest: &str) -> PathBuf {
        let root = temporary_root(label);
        fs::create_dir_all(root.join("evidence/m6")).expect("evidence should be creatable");
        fs::create_dir_all(root.join("docs/decisions")).expect("decisions should be creatable");
        for decision in M6_DECISIONS {
            fs::write(root.join("docs/decisions").join(decision), "# record\n")
                .expect("decision should be writable");
        }
        fs::write(root.join("evidence/m6/counterpart-summary.json"), summary)
            .expect("summary should be writable");
        fs::write(root.join("evidence/m6/manifest.json"), manifest)
            .expect("manifest should be writable");
        root
    }

    fn m7_root(label: &str, summary: &str, manifest: &str) -> PathBuf {
        let root = temporary_root(label);
        fs::create_dir_all(root.join("evidence/m7")).expect("evidence should be creatable");
        fs::create_dir_all(root.join("evidence/m5")).expect("evidence should be creatable");
        fs::write(
            root.join("evidence/m5/inventory-summary.json"),
            r#"{"artifacts":{"total":324}}"#,
        )
        .expect("inventory should be writable");
        fs::write(root.join("evidence/m7/library-summary.json"), summary)
            .expect("summary should be writable");
        fs::write(root.join("evidence/m7/manifest.json"), manifest)
            .expect("manifest should be writable");
        root
    }

    /// Rewrites one field of a manifest to a figure no record derives.
    fn misstate_figure(manifest: &Path, path: &str) {
        let output = Command::new("jq")
            .arg(format!("{path} = 987654"))
            .arg(manifest)
            .output()
            .expect("jq should run");
        assert!(output.status.success(), "jq could not set {path}");
        fs::write(manifest, output.stdout).expect("manifest should be writable");
    }

    #[test]
    fn a_balanced_counterpart_summary_closes_its_own_arithmetic() {
        let root = m6_root(
            "m6-arithmetic-balanced",
            &balanced_counterpart_summary(),
            &balanced_m6_manifest(),
        );
        let mut violations = Vec::new();

        verify_m6_counterpart_arithmetic(&root, &mut violations);

        assert!(
            violations.is_empty(),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn m6_area_arithmetic_mutations(balanced: &str) -> Vec<(usize, String)> {
        vec![
            (
                0,
                mutate(balanced, &[(r#""counterparts":2"#, r#""counterparts":3"#)]),
            ),
            (
                1,
                mutate(
                    balanced,
                    &[(r#""observableAreas":10"#, r#""observableAreas":11"#)],
                ),
            ),
            (
                2,
                mutate(
                    balanced,
                    &[(r#""declarableOnly":7"#, r#""declarableOnly":6"#)],
                ),
            ),
            (
                3,
                mutate(balanced, &[(r#""exercised":3"#, r#""exercised":4"#)]),
            ),
            (
                4,
                mutate(
                    balanced,
                    &[(
                        r#""observableAreas":6,"declarableOnly":4,"exercised":2"#,
                        r#""observableAreas":6,"declarableOnly":4,"exercised":1"#,
                    )],
                ),
            ),
            (
                5,
                mutate(
                    balanced,
                    &[
                        (
                            r#""observableAreas":6,"declarableOnly":4"#,
                            r#""observableAreas":7,"declarableOnly":4"#,
                        ),
                        (r#""observableAreas":10"#, r#""observableAreas":11"#),
                    ],
                ),
            ),
            (
                6,
                mutate(
                    balanced,
                    &[
                        (
                            r#""observableAreas":4,"declarableOnly":3,"exercised":1"#,
                            r#""observableAreas":0,"declarableOnly":0,"exercised":0"#,
                        ),
                        (r#""observableAreas":10"#, r#""observableAreas":6"#),
                        (r#""declarableOnly":7"#, r#""declarableOnly":4"#),
                        (r#""exercised":3"#, r#""exercised":2"#),
                    ],
                ),
            ),
        ]
    }

    fn m6_specification_and_provenance_mutations(balanced: &str) -> Vec<(usize, String)> {
        vec![
            (7, mutate(balanced, &[(r#""id":"e2""#, r#""id":"e9""#)])),
            (
                8,
                mutate(balanced, &[(r#""accepted":true"#, r#""accepted":false"#)]),
            ),
            (
                9,
                mutate(
                    balanced,
                    &[(
                        r#""carriesUpstreamIdentifier":false"#,
                        r#""carriesUpstreamIdentifier":true"#,
                    )],
                ),
            ),
            (
                10,
                mutate(
                    balanced,
                    &[(
                        r#""structureReviewed":true"#,
                        r#""structureReviewed":false"#,
                    )],
                ),
            ),
            (
                11,
                mutate(balanced, &[(r#""complete":true"#, r#""complete":false"#)]),
            ),
            (
                12,
                mutate(
                    balanced,
                    &[(
                        r#""suites":{"total":12,"failed":0}"#,
                        r#""suites":{"total":12,"failed":1}"#,
                    )],
                ),
            ),
        ]
    }

    fn m6_process_and_integrity_mutations(balanced: &str) -> Vec<(usize, String)> {
        vec![
            (
                13,
                mutate(balanced, &[(M6_TWO_COUNTERPARTS, M6_ONE_COUNTERPART)]),
            ),
            (
                14,
                mutate(
                    balanced,
                    &[(r#""secondCounterpart":"c2""#, r#""secondCounterpart":"c9""#)],
                ),
            ),
            (
                15,
                mutate(
                    balanced,
                    &[(
                        r#"{"name":"reload","result":"pass"}"#,
                        r#"{"name":"restart","result":"pass"}"#,
                    )],
                ),
            ),
            (
                16,
                mutate(balanced, &[(r#""enforcing":true"#, r#""enforcing":false"#)]),
            ),
            (
                17,
                mutate(balanced, &[(r#""violations":0,"#, r#""violations":2780,"#)]),
            ),
            (
                18,
                mutate(
                    balanced,
                    &[(
                        r#""specificationViolations":0"#,
                        r#""specificationViolations":12"#,
                    )],
                ),
            ),
            (
                19,
                mutate(
                    balanced,
                    &[(r#""artifactsScanned":18"#, r#""artifactsScanned":0"#)],
                ),
            ),
            (
                20,
                mutate(
                    balanced,
                    &[(
                        r#""sourceInjection":{"exitStatus":1}"#,
                        r#""sourceInjection":{"exitStatus":0}"#,
                    )],
                ),
            ),
            (
                21,
                mutate(
                    balanced,
                    &[(
                        r#""artifactInjection":{"exitStatus":1}"#,
                        r#""artifactInjection":{"exitStatus":0}"#,
                    )],
                ),
            ),
            (
                22,
                mutate(
                    balanced,
                    &[(
                        r#""cleanRun":{"exitStatus":0}"#,
                        r#""cleanRun":{"exitStatus":1}"#,
                    )],
                ),
            ),
        ]
    }

    #[test]
    fn every_m6_arithmetic_clause_refuses_a_record_that_breaks_it() {
        let balanced = balanced_counterpart_summary();
        let cases: Vec<(usize, String)> = [
            m6_area_arithmetic_mutations(&balanced),
            m6_specification_and_provenance_mutations(&balanced),
            m6_process_and_integrity_mutations(&balanced),
        ]
        .concat();

        let covered: Vec<usize> = cases.iter().map(|(clause, _)| *clause).collect();
        assert_eq!(
            covered,
            (0..M6_ARITHMETIC.len()).collect::<Vec<usize>>(),
            "a clause with no mutation behind it is a clause nothing has seen refuse"
        );

        for (clause, summary) in cases {
            let root = m6_root(
                &format!("m6-arithmetic-{clause}"),
                &summary,
                &balanced_m6_manifest(),
            );
            let mut violations = Vec::new();

            verify_m6_counterpart_arithmetic(&root, &mut violations);

            let requirement = M6_ARITHMETIC[clause].1;
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(requirement)),
                "clause {clause} was not refused; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn a_complete_m6_record_set_leaves_m6_closure_with_nothing_to_report() {
        let root = m6_root(
            "m6-manifest-balanced",
            &balanced_counterpart_summary(),
            &balanced_m6_manifest(),
        );
        let manifest = root.join("evidence/m6/manifest.json");
        let mut violations = Vec::new();

        verify_m6_manifest(&root, &manifest, &mut violations);

        assert!(
            violations.is_empty(),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_m6_closure_clause_refuses_a_manifest_that_breaks_it() {
        let manifest_cases: &[ClosureCase] = &[
            (
                "work-packages",
                &[(r#","M6-W6"]"#, "]")],
                "M6 manifest must include every work package from M6-W1 through M6-W6 exactly once",
            ),
            (
                "protocol",
                &[(
                    r#""protocolVersion":"m6-counterpart-v1""#,
                    r#""protocolVersion":"m6-counterpart-v0""#,
                )],
                "M6 manifest must record its counterpart summary under the m6-counterpart-v1",
            ),
            (
                "gate-status",
                &[(
                    r#""M6-W3":{"status":"closed""#,
                    r#""M6-W3":{"status":"open""#,
                )],
                "M6 closure requires a gateStatus row for every work package",
            ),
            (
                "advisory-check",
                &[(r#""enforcing":true"#, r#""enforcing":false"#)],
                "M6 closure requires the manifest itself to record the rewrite-integrity check",
            ),
            (
                "zero-exit-injection",
                &[(
                    r#""artifactInjectionExitStatus":1"#,
                    r#""artifactInjectionExitStatus":0"#,
                )],
                "M6 closure requires the manifest itself to record the rewrite-integrity check",
            ),
            (
                "attestation",
                &[(r#""architecture":"x86_64""#, r#""architecture":"aarch64""#)],
                "M6 closure requires a native Linux attestation",
            ),
            (
                "findings",
                &[(
                    r#""deviations":[{"classification":"minor"}]"#,
                    r#""deviations":[]"#,
                )],
                "recording no deviation or risk deleted the finding rather than the defect",
            ),
        ];

        for (label, replacements, expected) in manifest_cases {
            let root = m6_root(
                &format!("m6-closure-{label}"),
                &balanced_counterpart_summary(),
                &mutate(&balanced_m6_manifest(), replacements),
            );
            let mut violations = Vec::new();

            verify_m6_manifest(
                &root,
                &root.join("evidence/m6/manifest.json"),
                &mut violations,
            );

            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(expected)),
                "{label} was not refused; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn m6_closure_refuses_a_missing_decision_record() {
        for decision in M6_DECISIONS {
            let root = m6_root(
                &format!("m6-decision-{decision}"),
                &balanced_counterpart_summary(),
                &balanced_m6_manifest(),
            );
            fs::remove_file(root.join("docs/decisions").join(decision))
                .expect("decision should be removable");
            let mut violations = Vec::new();

            verify_m6_manifest(
                &root,
                &root.join("evidence/m6/manifest.json"),
                &mut violations,
            );

            assert!(
                violations.contains(&format!(
                    "M6 closure requires the decision record {decision}"
                )),
                "violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn m6_closure_refuses_a_summary_that_is_absent_or_the_wrong_shape() {
        let root = m6_root(
            "m6-record-shape",
            &mutate(
                &balanced_counterpart_summary(),
                &[(r#""workPackage":"M6-W6""#, r#""workPackage":"M6-W5""#)],
            ),
            &balanced_m6_manifest(),
        );
        let mut violations = Vec::new();

        verify_m6_manifest(
            &root,
            &root.join("evidence/m6/manifest.json"),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("M6 closure requires a counterpart summary")),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");

        let root = m6_root(
            "m6-record-absent",
            &balanced_counterpart_summary(),
            &balanced_m6_manifest(),
        );
        fs::remove_file(root.join("evidence/m6/counterpart-summary.json"))
            .expect("summary should be removable");
        let mut violations = Vec::new();

        verify_m6_manifest(
            &root,
            &root.join("evidence/m6/manifest.json"),
            &mut violations,
        );

        assert!(
            violations.contains(
                &"M6 closure requires the record evidence/m6/counterpart-summary.json".to_owned()
            ),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_figure_m6_carries_is_recomputed_from_the_record() {
        for (claimed_at, _) in M6_CARRIED_FIGURES {
            let label = claimed_at.replace('.', "-");
            let root = m6_root(
                &format!("m6-carried{label}"),
                &balanced_counterpart_summary(),
                &balanced_m6_manifest(),
            );
            let manifest = root.join("evidence/m6/manifest.json");
            misstate_figure(&manifest, claimed_at);
            let mut violations = Vec::new();

            verify_m6_carried_figures(&root, &manifest, &mut violations);

            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(&format!("records {claimed_at} 987654"))),
                "{claimed_at} was not reconciled; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn a_balanced_library_summary_closes_its_own_arithmetic() {
        let root = m7_root(
            "m7-arithmetic-balanced",
            &library_summary(324, "null"),
            &balanced_m7_manifest(),
        );
        let mut violations = Vec::new();

        verify_m7_library_arithmetic(&root, &mut violations);

        assert!(
            violations.is_empty(),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn m7_ledger_mutations(balanced: &str) -> Vec<(usize, String)> {
        vec![
            (
                0,
                mutate(balanced, &[(r#""accepted":324"#, r#""accepted":323"#)]),
            ),
            (
                1,
                mutate(
                    balanced,
                    &[(
                        r#""accepted":324,"blocked":0"#,
                        r#""accepted":319,"blocked":5"#,
                    )],
                ),
            ),
            (
                2,
                mutate(
                    balanced,
                    &[(
                        r#""accepted":324,"blocked":0"#,
                        r#""accepted":319,"blocked":5"#,
                    )],
                ),
            ),
            (3, library_summary(300, "null")),
            (
                4,
                mutate(
                    balanced,
                    &[(
                        r#""dependencies":{"resolved":900,"unresolved":0}"#,
                        r#""dependencies":{"resolved":900,"unresolved":3}"#,
                    )],
                ),
            ),
            (
                5,
                mutate(
                    balanced,
                    &[(
                        r#""registries":{"resolved":400,"unresolved":0}"#,
                        r#""registries":{"resolved":400,"unresolved":7}"#,
                    )],
                ),
            ),
            (6, mutate(balanced, &[(r#""missing":0"#, r#""missing":2"#)])),
        ]
    }

    fn m7_wave_routing_and_suite_mutations(balanced: &str) -> Vec<(usize, String)> {
        vec![
            (
                7,
                mutate(
                    balanced,
                    &[(r#""index":1,"entries":120"#, r#""index":1,"entries":121"#)],
                ),
            ),
            (8, mutate(balanced, &[(r#""index":2"#, r#""index":1"#)])),
            (
                9,
                mutate(
                    balanced,
                    &[(r#""dependsOn":[0,1]"#, r#""dependsOn":[0,5]"#)],
                ),
            ),
            (
                10,
                mutate(
                    balanced,
                    &[(
                        r#""index":0,"entries":100,"dependsOn":[]"#,
                        r#""index":0,"entries":100,"dependsOn":[2]"#,
                    )],
                ),
            ),
            (
                11,
                mutate(balanced, &[(r#""absorbed":40"#, r#""absorbed":50"#)]),
            ),
            (
                12,
                mutate(
                    balanced,
                    &[(r#""runnableToday":324"#, r#""runnableToday":323"#)],
                ),
            ),
            (
                13,
                mutate(
                    balanced,
                    &[(
                        r#""runnableToday":324,"blocked":0"#,
                        r#""runnableToday":0,"blocked":324"#,
                    )],
                ),
            ),
            (
                14,
                mutate(
                    balanced,
                    &[(
                        r#""packages":{"total":324,"failed":0}"#,
                        r#""packages":{"total":324,"failed":3}"#,
                    )],
                ),
            ),
            (
                15,
                mutate(
                    balanced,
                    &[(
                        r#""clusters":{"total":12,"failed":0}"#,
                        r#""clusters":{"total":12,"failed":1}"#,
                    )],
                ),
            ),
        ]
    }

    #[test]
    fn every_m7_arithmetic_clause_refuses_a_record_that_breaks_it() {
        let balanced = library_summary(324, "null");
        let cases: Vec<(usize, String)> = [
            m7_ledger_mutations(&balanced),
            m7_wave_routing_and_suite_mutations(&balanced),
        ]
        .concat();

        let covered: Vec<usize> = cases.iter().map(|(clause, _)| *clause).collect();
        assert_eq!(
            covered,
            (0..M7_ARITHMETIC.len()).collect::<Vec<usize>>(),
            "a clause with no mutation behind it is a clause nothing has seen refuse"
        );

        for (clause, summary) in cases {
            let root = m7_root(
                &format!("m7-arithmetic-{clause}"),
                &summary,
                &balanced_m7_manifest(),
            );
            let mut violations = Vec::new();

            verify_m7_library_arithmetic(&root, &mut violations);

            let requirement = M7_ARITHMETIC[clause].1;
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(requirement)),
                "clause {clause} was not refused; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn m7_audits_the_total_the_inventory_verified_or_a_correction_that_names_it() {
        let corrected = r#"{"from":324,"total":300,"reason":"two entries were one release"}"#;
        let cases: &[(&str, String, bool)] = &[
            ("matching", library_summary(324, "null"), true),
            ("corrected", library_summary(300, corrected), true),
            ("uncorrected", library_summary(300, "null"), false),
            (
                "corrected-from-elsewhere",
                library_summary(
                    300,
                    r#"{"from":301,"total":300,"reason":"a figure nothing verified"}"#,
                ),
                false,
            ),
        ];

        for (label, summary, reconciles) in cases {
            let root = m7_root(
                &format!("m7-fixture-total-{label}"),
                summary,
                &balanced_m7_manifest(),
            );
            let mut violations = Vec::new();

            verify_m7_verified_fixture_total(&root, &mut violations);

            assert_eq!(
                violations.is_empty(),
                *reconciles,
                "{label}; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn a_complete_m7_record_set_leaves_m7_closure_with_nothing_to_report() {
        let root = m7_root(
            "m7-manifest-balanced",
            &library_summary(324, "null"),
            &balanced_m7_manifest(),
        );
        let mut violations = Vec::new();

        verify_m7_manifest(
            &root,
            &root.join("evidence/m7/manifest.json"),
            &mut violations,
        );

        assert!(
            violations.is_empty(),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_m7_closure_clause_refuses_a_manifest_that_breaks_it() {
        let manifest_cases: &[ClosureCase] = &[
            (
                "work-packages",
                &[(r#","M7-W6"]"#, "]")],
                "M7 manifest must include every work package from M7-W0 through M7-W6 exactly once",
            ),
            (
                "protocol",
                &[(
                    r#""protocolVersion":"m7-library-v1""#,
                    r#""protocolVersion":"m7-library-v0""#,
                )],
                "M7 manifest must record its library summary under the m7-library-v1",
            ),
            (
                "gate-status",
                &[(
                    r#""M7-W4":{"status":"closed""#,
                    r#""M7-W4":{"status":"open""#,
                )],
                "M7 closure requires a gateStatus row for every work package",
            ),
            (
                "attestation",
                &[(r#""architecture":"x86_64""#, r#""architecture":"aarch64""#)],
                "M7 closure requires a native Linux attestation",
            ),
            (
                "findings",
                &[(r#""risks":[{"classification":"minor"}]"#, r#""risks":[]"#)],
                "an empty finding set satisfies every rule phrased as a refusal",
            ),
        ];

        for (label, replacements, expected) in manifest_cases {
            let root = m7_root(
                &format!("m7-closure-{label}"),
                &library_summary(324, "null"),
                &mutate(&balanced_m7_manifest(), replacements),
            );
            let mut violations = Vec::new();

            verify_m7_manifest(
                &root,
                &root.join("evidence/m7/manifest.json"),
                &mut violations,
            );

            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(expected)),
                "{label} was not refused; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    #[test]
    fn m7_closure_refuses_a_summary_that_is_absent_or_the_wrong_shape() {
        let root = m7_root(
            "m7-record-shape",
            &mutate(
                &library_summary(324, "null"),
                &[(r#""result":"pass""#, r#""result":"provisional""#)],
            ),
            &balanced_m7_manifest(),
        );
        let mut violations = Vec::new();

        verify_m7_manifest(
            &root,
            &root.join("evidence/m7/manifest.json"),
            &mut violations,
        );

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("M7 closure requires a library summary")),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");

        let root = m7_root(
            "m7-record-absent",
            &library_summary(324, "null"),
            &balanced_m7_manifest(),
        );
        fs::remove_file(root.join("evidence/m7/library-summary.json"))
            .expect("summary should be removable");
        let mut violations = Vec::new();

        verify_m7_manifest(
            &root,
            &root.join("evidence/m7/manifest.json"),
            &mut violations,
        );

        assert!(
            violations.contains(
                &"M7 closure requires the record evidence/m7/library-summary.json".to_owned()
            ),
            "violations:\n{}",
            violations.join("\n")
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn every_figure_m7_carries_is_recomputed_from_the_record() {
        for (claimed_at, _) in M7_CARRIED_FIGURES {
            let label = claimed_at.replace('.', "-");
            let root = m7_root(
                &format!("m7-carried{label}"),
                &library_summary(324, "null"),
                &balanced_m7_manifest(),
            );
            let manifest = root.join("evidence/m7/manifest.json");
            misstate_figure(&manifest, claimed_at);
            let mut violations = Vec::new();

            verify_m7_carried_figures(&root, &manifest, &mut violations);

            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(&format!("records {claimed_at} 987654"))),
                "{claimed_at} was not reconciled; violations:\n{}",
                violations.join("\n")
            );
            fs::remove_dir_all(root).expect("temporary root should be removable");
        }
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-xtask-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
