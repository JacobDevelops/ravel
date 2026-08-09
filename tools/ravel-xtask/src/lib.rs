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
    "ravel-bootstrap",
    "ravel-composed-host",
    "ravel-minecraft",
    "ravel-minecraft-adapter",
    "ravel-config",
    "ravel-data",
    "ravel-save",
    "ravel-network",
    "ravel-observability",
    "ravel-pack",
    "ravel-testkit",
];

const TEST_MOD_NAMES: &[&str] = &[
    "client-only",
    "commands",
    "configuration",
    "dependency-base",
    "dependency-consumer",
    "events",
    "failure-cases",
    "inter-mod-service",
    "networking",
    "noop",
    "registries",
    "resources",
    "saved-data",
    "waystone-ledger",
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
    "jvm/screen/src/dev/ravel/jvm/screen/RavelModsScreen.java",
    "jvm/screen/src/dev/ravel/jvm/screen/RavelSettingsScreen.java",
    "jvm/screen/src/dev/ravel/jvm/screen/Wire.java",
    "minecraft/README.md",
    "minecraft/versions/26.1.2/README.md",
    "minecraft/versions/26.1.2/adapter.toml",
    "minecraft/versions/26.1.2/hooks.toml",
    "minecraft/versions/26.1.2/mappings.lock",
    "minecraft/versions/26.1.2/stubs.lock",
    "minecraft/versions/26.1.2/stubs.toml",
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
    verify_hook_inventory_document(root, &mut violations);
    verify_decision_hook_claims(root, &mut violations);
    verify_campaign_inventory(root, &mut violations);
    verify_review_round_counts(root, &mut violations);
    verify_manifest_percentages(root, &mut violations);
    verify_void_limits(root, &mut violations);
    verify_harness_lifecycles(root, &mut violations);
    verify_persistence_reflection(root, &mut violations);
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

/// The reviewed hook inventory for the pinned version.
const HOOK_INVENTORY: &str = "minecraft/versions/26.1.2/hooks.toml";

/// The document that describes the inventory above, and M3-W0's closing evidence.
const HOOK_DOCUMENT: &str = "docs/architecture/m3-hook-targets.md";

/// The launch record that enumerates the inventory M3's campaigns reviewed.
const CAMPAIGN_MATRIX: &str = "evidence/m3/linux-process-matrix.json";

/// The prose that restates that record's inventory figures.
const CAMPAIGN_DOCUMENT: &str = "evidence/m3/README.md";

/// The manifest that records M3's gate state, and restates review prose.
const M3_MANIFEST: &str = "evidence/m3/manifest.json";

/// The one record that says how many independent M3-W8 rounds have run.
const REVIEW_RECORD: &str = "evidence/m3/review.md";

/// M2's evidence README, which summarises the audits its record holds.
const M2_CAMPAIGN_DOCUMENT: &str = "evidence/m2/README.md";

/// M2's admission artifact.
const M2_MANIFEST: &str = "evidence/m2/manifest.json";

/// The record that counts M2's independent audits, for the reason
/// [`REVIEW_RECORD`] counts M3's.
const M2_REVIEW_RECORD: &str = "evidence/m2/review.md";

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
fn verify_hook_inventory_document(root: &Path, violations: &mut Vec<String>) {
    let inventory = match fs::read_to_string(root.join(HOOK_INVENTORY)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!("cannot read {HOOK_INVENTORY}: {error}"));
            return;
        }
    };
    let document = match fs::read_to_string(root.join(HOOK_DOCUMENT)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!("cannot read {HOOK_DOCUMENT}: {error}"));
            return;
        }
    };
    let counts = match hook_counts(&inventory) {
        Ok(counts) => counts,
        Err(error) => {
            violations.push(format!("cannot count {HOOK_INVENTORY}: {error}"));
            return;
        }
    };
    violations.extend(hook_document_violations(&document, &counts));
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
fn verify_decision_hook_claims(root: &Path, violations: &mut Vec<String>) {
    let inventory = match fs::read_to_string(root.join(HOOK_INVENTORY)) {
        Ok(text) => text,
        Err(error) => {
            violations.push(format!("cannot read {HOOK_INVENTORY}: {error}"));
            return;
        }
    };
    let declared = declared_hook_names(&inventory);
    let per_frame = declared.iter().any(|name| name == PER_FRAME_MEMBER);

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
            violations.push(format!(
                "{DECISION_RECORDS}/{name} says `{denied}` is not hooked while {HOOK_INVENTORY} \
                 declares it"
            ));
        }
        if per_frame
            && record
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
                .contains(NO_PER_FRAME)
        {
            violations.push(format!(
                "{DECISION_RECORDS}/{name} says \"{NO_PER_FRAME}\" while {HOOK_INVENTORY} \
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
fn hook_document_violations(document: &str, counts: &HookCounts) -> Vec<String> {
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
                 held to {HOOK_INVENTORY}'s {expected} {subject}"
            ));
            continue;
        }
        for count in stated {
            if count != expected {
                violations.push(format!(
                    "{HOOK_DOCUMENT} states {count} {subject} where {HOOK_INVENTORY} declares \
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

/// How a count of things can be spelled in this repository's prose.
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

/// One word read as a number, whether it was written in digits or spelled out.
fn numeral(word: &str) -> Option<usize> {
    let word = word
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_ascii_lowercase();
    if !word.is_empty() && word.chars().all(|character| character.is_ascii_digit()) {
        return word.parse().ok();
    }
    CARDINALS.iter().position(|spelled| *spelled == word)
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
    // M2 is held to the same rule as M3, and for the same reason rather than by
    // analogy: its record now carries successive audits too, its manifest and
    // README both summarise them, and the figure that went stale in M3 went stale
    // in five sites of exactly that kind. Applying the rule before a count is
    // written is the only time it costs nothing.
    //
    // M0 and M1 are deliberately outside this, and the choice is recorded here so it
    // is not read as an oversight: both are closed, their reviews are over, and the
    // counts their records restate — `evidence/m1/manifest.json` and
    // `evidence/m1/README.md` each restate one — cannot go stale because no further
    // round will run against them. Bringing them in would mean rewording a closed
    // milestone's prose to satisfy a rule written after it.
    for (record, documents) in [
        (REVIEW_RECORD, [CAMPAIGN_DOCUMENT, M3_MANIFEST]),
        (M2_REVIEW_RECORD, [M2_CAMPAIGN_DOCUMENT, M2_MANIFEST]),
    ] {
        if !root.join(record).is_file() {
            violations.push(format!("missing required file: {record}"));
        }
        for relative in documents {
            match fs::read_to_string(root.join(relative)) {
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

/// Every game class the agent jar resolves reflectively, as a declared set.
///
/// ADG-7's persistence adapter is the only source that resolves one. Adding a name
/// to that source without adding it here fails the scaffold, which is the whole
/// point of the list.
const PERSISTENCE_REFLECTED_CLASSES: [&str; 5] = [
    "net.minecraft.client.Minecraft",
    "net.minecraft.server.MinecraftServer",
    "net.minecraft.util.datafix.DataFixers",
    "net.minecraft.world.level.storage.LevelResource",
    "net.minecraft.world.level.storage.SavedDataStorage",
];

/// Pins the game class names the always-present agent jar's own sources carry.
///
/// ADG-1 forbids that jar from *declaring* a game type and `build-agent.sh`
/// enforces it by construction, compiling with no `-cp` at all. ADG-7's persistence
/// adapter resolves the game classes it needs by name instead, and ADG-1's
/// amendment said for a while that the agent jar "names no game type", which was
/// false of the shipped jar. This holds the jar's game-class literals to a set that
/// is either declared here or declared as a hook target in `hooks.toml`, and it
/// fails in both directions: an undeclared name in the jar, and a declared name no
/// source resolves any more.
///
/// **What it cannot do, because the gap is the reason it exists.** It cannot detect
/// a game-side rename. Both sides of the comparison are Ravel's own — the jar's
/// literals and the list above — and neither comes from the pinned client jar, so a
/// class renamed in the next pinned version passes here and fails at runtime as a
/// refused world load. Hook target classes are the exception and only because
/// `hooks.toml` and `mappings.lock` are re-pinned against the jar; the reflective
/// set is in no lock. Nor does this reach anything below a class: the member names
/// [`method`] resolves by name and arity, the two `LevelResource` field constants,
/// and the three-argument `SavedDataStorage` constructor are checked only by
/// `jvm/harness/persistence/net/minecraft/**`, which are stand-ins Ravel wrote, so
/// that harness asserts Ravel's stubs against Ravel's reflection.
///
/// Two further limits, stated because a gate that promises more than it enforces is
/// the defect this one was written to close. It sees only the prefixes
/// [`GAME_PACKAGES`] lists, so a game name under any other package escapes it
/// entirely. And it reads only the tracked sources under [`AGENT_JAR_SOURCES`],
/// while `build-agent.sh` compiles those *and* the generated hook tree: generated
/// `Hook.java` carries an internal game name for every class it transforms, and
/// nothing here reads it. That half is `hooks.toml`-derived by construction and so
/// cannot drift from the pinned inventory without the generator drifting, which is
/// why this function does not reach for a build output — but it means "the agent
/// jar's game names" is wider than what this reads. A name assembled by
/// concatenation, or held in a constant declared elsewhere, is outside it too.
///
/// `evidence/m3/manifest.json` records the surface as a named deviation and
/// `ROADMAP.md` records bringing it under `stubs.toml` and `mappings.lock` as owed
/// work, which needs the re-pin ceremony against the pinned jar.
fn verify_persistence_reflection(root: &Path, violations: &mut Vec<String>) {
    let inventory = match fs::read_to_string(root.join(HOOK_INVENTORY)) {
        Ok(inventory) => inventory,
        Err(error) => {
            violations.push(format!("cannot read {HOOK_INVENTORY}: {error}"));
            return;
        }
    };
    let pinned: Vec<&str> = inventory
        .lines()
        .map(str::trim)
        .filter_map(|line| quoted_value(line, "class"))
        .collect();

    let mut sources = Vec::new();
    collect_files(&root.join(AGENT_JAR_SOURCES), &mut sources);
    sources.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "java")
    });
    sources.sort();
    if sources.is_empty() {
        violations.push(format!(
            "{AGENT_JAR_SOURCES} holds no Java source, so no game class the agent jar resolves is \
             held to a declared set"
        ));
        return;
    }

    let mut resolved: Vec<String> = Vec::new();
    for path in &sources {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                violations.push(format!("cannot read {relative}: {error}"));
                continue;
            }
        };
        for name in game_class_literals(&source) {
            if PERSISTENCE_REFLECTED_CLASSES.contains(&name.as_str()) {
                resolved.push(name);
                continue;
            }
            if pinned.contains(&name.as_str()) {
                continue;
            }
            violations.push(format!(
                "{relative} names the game class {name}, which is neither in the agent jar's \
                 declared reflective surface nor a hook target {HOOK_INVENTORY} pins; a game name \
                 in the always-present jar that no lock and no declared set covers is coupled to \
                 the pinned version by nothing"
            ));
        }
    }

    for declared in PERSISTENCE_REFLECTED_CLASSES {
        if !resolved.iter().any(|name| name == declared) {
            violations.push(format!(
                "the agent jar's declared reflective surface names {declared} and no source under \
                 {AGENT_JAR_SOURCES} resolves it; a declared name nothing uses says the surface \
                 drifted rather than that it is pinned"
            ));
        }
    }
}

/// Refuses the claim that an artifact *names* no game type, in tracked text.
///
/// Two M3 repair passes each swept for the sentence "names no game type" by hand
/// and each left sites behind, and a third pass found more. A grep finds them all
/// at once, which is why this is a gate rather than a fourth sweep. The claim is
/// false of two shipped artifacts: `jvm/bootstrap/dev/ravel/jvm/Transformer.java`
/// holds game class names as string constants and compares against them, and
/// generated `Hook.java` carries the internal name of every class it transforms.
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
/// parity of the rest of the file. Both spellings count: the transformer writes hook
/// targets in internal `net/minecraft/…` form and the persistence adapter writes
/// binary `net.minecraft.…` form, and they are the same coupling.
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
    let mut rest = descriptor.strip_prefix('(')?.split_once(')')?.0;
    let mut handles = 0;
    while !rest.is_empty() {
        let arrays = rest.len() - rest.trim_start_matches('[').len();
        rest = &rest[arrays..];
        if let Some(named) = rest.strip_prefix('L') {
            let (name, tail) = named.split_once(';')?;
            if arrays > 0 || name != "java/lang/String" {
                handles += 1;
            }
            rest = tail;
        } else {
            if !matches!(
                rest.as_bytes().first()?,
                b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z'
            ) {
                return None;
            }
            if arrays > 0 {
                handles += 1;
            }
            rest = &rest[1..];
        }
    }
    Some(handles)
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
        verify_recorded_inputs(root, manifest, &mut violations);
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
    if milestone == "M4" {
        // These two ask whether two documents in the tree agree with each other
        // and with the repository, which is what this function's doc comment
        // calls a rule the repository decides. A review verdict has no bearing on
        // either, so holding them in the opt-in milestone task alone would leave
        // the gate every agent is told to run unable to see an inventory that had
        // gone stale. The milestone gate runs them too, and the two never run in
        // the same invocation.
        verify_m4_surface(root, violations);
        verify_m4_gate_figures(root, manifest_path, violations);
    }
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
/// The revision is resolved through jj, and a checkout without a jj repository is
/// skipped rather than failed, because there is nothing there to resolve a revision
/// with. That is the one gap left: a clone of this repository does reach the pinned
/// commits, since the colocated repository exports the visible stack to git HEAD, so
/// `jj git init --colocate` in a clone recomputes every digest — but a plain git tree
/// cannot, and reporting every input unreadable there would turn a provenance check
/// into noise. The skip says so on stdout, because a whole provenance check that
/// evaporates in silence reads exactly like one that passed.
fn verify_recorded_inputs(root: &Path, manifest_path: &Path, violations: &mut Vec<String>) {
    let label = manifest_path
        .strip_prefix(root)
        .unwrap_or(manifest_path)
        .display()
        .to_string();
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
    let records = match jq_value(manifest_path, r#".inputs[]? | .name + " " + .sha256"#) {
        Ok(records) => records,
        Err(error) => {
            violations.push(error);
            return;
        }
    };

    for record in records.lines().filter(|record| !record.trim().is_empty()) {
        let Some((name, recorded)) = record.rsplit_once(' ') else {
            violations.push(format!("{label} records an input with no digest: {record}"));
            continue;
        };
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

/// The record a manifest binds, when it binds one by path.
///
/// A name that reads as prose is reported where the digests are recomputed, so
/// there is nothing for this to say about one.
fn bound_review_record(manifest_path: &Path) -> Option<PathBuf> {
    let name = jq_value(manifest_path, r#"(.review.record? | .name?) // """#).ok()?;
    if name.is_empty() || name == "null" || !names_a_path(&name) {
        return None;
    }
    manifest_path.parent().map(|directory| directory.join(name))
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
    // The verdict rule runs here as well as in the every-manifest sweep, because
    // this is the gate a milestone is closed by, and the defect it exists for was
    // exactly that `verify milestone M1` exited 0 on a manifest claiming pass over
    // a record reading `Result: FAIL`.
    if let Some(record) = bound_review_record(manifest_path) {
        verify_recorded_verdict(
            &relative_label(root, manifest_path),
            manifest_path,
            &record,
            &mut violations,
        );
    }
    verify_manifest_change(root, manifest_path, &mut violations);
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
        // The lists themselves are reconciled against the tree by
        // `verify_m4_surface`, because a list nothing recomputes is a count in
        // different brackets.
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
/// nothing recomputed it, so a crate added or a guide page deleted after the
/// record was written left it describing a tree that no longer exists. Each list
/// is compared by name rather than by length, in both directions, so a rename
/// fails as loudly as an addition and a deletion is not absorbed by an addition
/// somewhere else in the same list.
///
/// The crate, test-mod and example lists are compared against the constants
/// `CLAUDE.md` already makes an author maintain, and those constants are
/// themselves compared against their directories by [`verify_inventory`], so this
/// check inherits that reconciliation rather than restating it.
fn verify_m4_surface(root: &Path, violations: &mut Vec<String>) {
    let record = root.join("evidence/m4/sdk-surface.json");
    if !record.is_file() {
        // Its absence is already a violation where the record set is checked.
        return;
    }
    let expected = [
        ("crates", listed_names(CRATE_NAMES)),
        ("testMods", listed_names(TEST_MOD_NAMES)),
        ("examples", listed_names(EXAMPLE_NAMES)),
        ("guideDocuments", files_named(root, "docs/guide", "md")),
        (
            "conformanceSuites",
            files_named(root, "tests/conformance/tests", "rs"),
        ),
    ];
    for (field, expected) in expected {
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
        for missing in expected.difference(&recorded) {
            violations.push(format!(
                "evidence/m4/sdk-surface.json omits {missing} from inventory.{field}, which the \
                 repository holds"
            ));
        }
        for extra in recorded.difference(&expected) {
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

/// Rejects rights-restricted, secret, and local-runtime paths from a tracked tree.
#[must_use]
pub fn verify_tracked_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    paths
        .into_iter()
        .filter_map(|path| {
            let normalized = path.replace('\\', "/");
            is_forbidden_tracked_path(&normalized)
                .then(|| format!("forbidden tracked path: {normalized}"))
        })
        .collect()
}

/// Rejects private keys and non-placeholder credential assignments in tracked text.
#[must_use]
pub fn verify_tracked_contents(root: &Path, paths: &[String]) -> Vec<String> {
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
    }
    violations
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
/// The names come out of the same data the generator reads: each `class` entry in
/// `hooks.toml`, `stubs.toml` and `mappings.lock`, plus every class a descriptor
/// in those files refers to, kept only when it sits under a package root a
/// `class` entry declared. That last filter is what keeps `java/lang/String` out
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
            for file in ["hooks.toml", "stubs.toml", "mappings.lock"] {
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
        CRATE_NAMES, CampaignInventory, EXAMPLE_NAMES, HOOK_DOCUMENT, HOOK_INVENTORY, HookCounts,
        MILESTONE_MANIFESTS, SWEPT_SCOPE, TEST_MOD_NAMES, VERSION_CLASS_EXCUSES,
        campaign_document_violations, change_mismatch, declared_hook_names,
        declares_unsafe_prohibition, denied_hook_names, game_class_literals, hook_counts,
        hook_document_violations, numbers_before, pinned_commit_mismatch, redacted_path,
        required_scaffold_paths, round_count_restatements, verify_campaign_inventory,
        verify_decision_hook_claims, verify_evidence_records, verify_experiment_fence,
        verify_harness_lifecycles, verify_hook_inventory_document, verify_ignored_boundaries,
        verify_m4_gate_figures, verify_m4_surface, verify_manifest_percentages, verify_milestone,
        verify_persistence_reflection, verify_review_round_counts, verify_scaffold, verify_spikes,
        verify_testkit_fence, verify_tracked_contents, verify_tracked_paths,
        verify_version_boundary, verify_void_limits, visible_commit,
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
        let (root, revision) = pinned_input_root("inputs-absent");
        write_input_manifest(&root, &revision, "absent.txt", HELLO_DIGEST);

        let violations = verify_evidence_records(&root);

        assert_eq!(
            violations,
            [format!(
                "evidence/m9/manifest.json records input absent.txt: cannot read it at revision \
                 {revision}"
            )]
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

        assert_eq!(
            hook_document_violations(current, &counts),
            Vec::<String>::new()
        );
        assert_eq!(
            hook_document_violations(stale, &counts),
            [
                format!(
                    "{HOOK_DOCUMENT} states 35 hook entries where {HOOK_INVENTORY} declares 37"
                ),
                format!(
                    "{HOOK_DOCUMENT} states 31 method entries where {HOOK_INVENTORY} declares 33"
                ),
                format!(
                    "{HOOK_DOCUMENT} states 23 handle parameters where {HOOK_INVENTORY} declares 27"
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

        let violations = hook_document_violations("The inventory grew.", &counts);

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

        verify_decision_hook_claims(&root, &mut violations);

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

        verify_hook_inventory_document(&root, &mut violations);

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
    fn a_surface_inventory_that_has_drifted_from_the_repository_is_reported() {
        // Both directions, in one record: a guide page the tree holds and the
        // record omits, and a suite the record names and the tree does not. An
        // inventory checked by length alone would report neither.
        let root = surface_root(
            "surface-drifted",
            &["README.md", "limits.md"],
            &["one.rs"],
            &surface_record(&["README.md"], &["one.rs", "deleted.rs"]),
        );

        let mut violations = Vec::new();
        verify_m4_surface(&root, &mut violations);

        assert_eq!(
            violations,
            [
                "evidence/m4/sdk-surface.json omits limits.md from inventory.guideDocuments, \
                 which the repository holds",
                "evidence/m4/sdk-surface.json names deleted.rs under inventory.conformanceSuites, \
                 which the repository does not hold",
            ]
        );
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

    fn temporary_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ravel-xtask-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
