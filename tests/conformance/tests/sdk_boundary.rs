#![forbid(unsafe_code)]

//! What the public SDK boundary is, recomputed from the files that set it.
//!
//! `examples/minimal-mod/src/boundary.rs` holds M4-W1's compile-pass/fail suite,
//! and every refusal in it rests on two facts about the build rather than on the
//! suite's own word: the example crate sees exactly what a mod author sees, and
//! `ravel-sdk`'s dependency closure holds no bootstrap, JVM, loader, or
//! version-specific crate. Both facts are cheap to state and easy to break by
//! editing one line of TOML, so this file derives them instead.
//!
//! # Why the closure comes from `Cargo.lock`
//!
//! An earlier version of this file walked `crates/*/Cargo.toml` and read the
//! `dependencies`, `dev-dependencies` and `build-dependencies` tables. Cargo has
//! a fourth place to declare an edge — `[target.'cfg(...)'.dependencies]` — and a
//! review put `jni` there, in a crate two hops from the facade, and watched every
//! test in this file pass while `cargo tree -p ravel-example-minimal-mod -e
//! normal` showed Java in a mod's compiled graph. A hand-maintained list of
//! table names is the same defect one level down: the next table Cargo adds
//! breaks the walk again, silently.
//!
//! `Cargo.lock` is the resolver's own answer and needs no such list. It records
//! every edge from every table, it is not filtered to one platform — so a
//! `cfg(windows)` edge is visible from a Linux run — it names the real package
//! behind a renamed dependency, and it carries third-party subtrees, which the
//! manifest walk never followed either.
//!
//! Nothing here reads the SDK's source. A re-export can only reach a crate the
//! closure contains, so the closure is the thing worth checking, and the
//! doctests cover the second half — that the facade does not re-export what the
//! closure does contain.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use toml::{Table, Value};

/// The crate whose closure is the mod-facing boundary.
const FACADE: &str = "ravel-sdk";

/// The example whose manifest is a mod author's manifest.
const EXAMPLE_MANIFEST: &str = "examples/minimal-mod/Cargo.toml";

/// The directory holding every crate whose manifest is an author's manifest.
const EXAMPLES: &str = "examples";

/// The compile-pass/fail suite whose cases this file stands behind.
const EXAMPLE_SUITE: &str = "examples/minimal-mod/src/boundary.rs";

/// The one dependency a mod author declares.
const AUTHOR_DEPENDENCY: &str = "ravel-sdk";

/// The one source a third-party package in the mod-facing closure may come from.
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// Crates and libraries no mod may reach, and why each one is named.
///
/// `docs/architecture/dependency-rules.md` states the first three as rules about
/// the SDK: it must not expose bootstrap details, generated JVM types, or
/// Minecraft-version internals, and `ravel-minecraft` is the highest crate that
/// may name a JNI type. The rest follow from the same sentence — the SDK is an
/// author-facing facade, so the loader's machinery, the assembled host, the
/// report surfaces, and the test harnesses are not part of what a mod compiles
/// against.
const FORBIDDEN: &[(&str, &str)] = &[
    ("ravel-bootstrap", "the JVM entry crate"),
    ("ravel-minecraft", "the only crate that names a JNI type"),
    ("ravel-minecraft-adapter", "the pinned version's internals"),
    ("ravel-loader", "the lifecycle machine and claim table"),
    ("ravel-composed-host", "the assembled host"),
    ("ravel-observability", "the report and profiling surfaces"),
    (
        "ravel-testkit",
        "a test harness, never a production dependency",
    ),
    ("jni", "Java itself"),
];

/// Returns the repository root, from the manifest directory this test compiles in.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the conformance crate sits two directories below the root")
        .to_path_buf()
}

/// Reads one tracked TOML file as a table, naming the file when it cannot be read.
fn document(relative: &str) -> Table {
    let path = repository_root().join(relative);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    text.parse::<Table>()
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

/// Returns the dependency names one table declares in `section`.
fn declared(table: &Table, section: &str) -> BTreeSet<String> {
    table
        .get(section)
        .and_then(Value::as_table)
        .map(|section| section.keys().cloned().collect())
        .unwrap_or_default()
}

/// Every dependency name a manifest declares, from every table Cargo reads.
///
/// The three plain tables, plus the same three under every `[target.*]` entry.
/// This is the list `Cargo.lock` exists to avoid maintaining, and it is kept
/// here only for the example's own manifest, where the question is what an
/// author declared rather than what the resolver made of it.
fn every_declared_dependency(manifest: &Table) -> BTreeSet<String> {
    const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

    let plain = SECTIONS
        .into_iter()
        .flat_map(|section| declared(manifest, section));
    let per_target = manifest
        .get("target")
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(Table::values)
        .filter_map(Value::as_table)
        .flat_map(|target| {
            SECTIONS
                .into_iter()
                .flat_map(|section| declared(target, section))
                .collect::<Vec<_>>()
        });

    plain.chain(per_target).collect()
}

/// Maps every package the lock file names to the packages it depends on.
///
/// A lock entry's dependency is `"name"` or `"name version"` or
/// `"name version (source)"`, so the first field is the package name. Two
/// versions of one package share a name here and their edges are merged: this
/// file asks which packages a mod compiles, and a second version of a forbidden
/// crate is as forbidden as the first.
fn locked_dependencies() -> BTreeMap<String, BTreeSet<String>> {
    let lock = document("Cargo.lock");
    let packages = lock
        .get("package")
        .and_then(Value::as_array)
        .expect("Cargo.lock lists packages");

    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in packages {
        let package = package
            .as_table()
            .expect("every Cargo.lock package is a table");
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .expect("every Cargo.lock package is named");
        let edges = package
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|entry| entry.split_whitespace().next().unwrap_or(entry).to_owned());
        graph.entry(name.to_owned()).or_default().extend(edges);
    }
    graph
}

/// Maps every package the lock file names to the sources it is resolved from.
///
/// A lock entry carries `source` when the package came from a registry or a git
/// remote and carries none when it is a local path — a workspace member, or a
/// package a `[patch]` or `[replace]` entry redirected to a directory. The set is
/// per name for the reason the graph above is: two versions of one package share
/// a name, and a substituted source on either of them is a substitution.
fn locked_sources() -> BTreeMap<String, BTreeSet<Option<String>>> {
    let lock = document("Cargo.lock");
    let packages = lock
        .get("package")
        .and_then(Value::as_array)
        .expect("Cargo.lock lists packages");

    let mut sources: BTreeMap<String, BTreeSet<Option<String>>> = BTreeMap::new();
    for package in packages {
        let package = package
            .as_table()
            .expect("every Cargo.lock package is a table");
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .expect("every Cargo.lock package is named");
        let source = package
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_owned);
        sources.entry(name.to_owned()).or_default().insert(source);
    }
    sources
}

/// Returns every package reachable from [`FACADE`], the facade included.
fn closure() -> BTreeSet<String> {
    let graph = locked_dependencies();
    assert!(
        graph.contains_key(FACADE),
        "Cargo.lock does not name `{FACADE}`, so a closure computed from it would be empty and \
         every check below would pass without reading the boundary at all"
    );

    let mut reached = BTreeSet::new();
    let mut pending = vec![FACADE.to_owned()];
    while let Some(name) = pending.pop() {
        if !reached.insert(name.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&name) {
            pending.extend(dependencies.iter().cloned());
        }
    }
    reached
}

/// One fenced code block from [`EXAMPLE_SUITE`]'s module documentation.
struct Case {
    /// The fence's info string, such as `compile_fail,E0433`.
    info: String,
    /// The block's source, as rustdoc would receive it.
    body: String,
    /// The line the fence opened on, for a message that can be navigated to.
    line: usize,
}

impl Case {
    /// The error code the case pins, if it pins one.
    fn pinned_code(&self) -> Option<&str> {
        self.info.split(',').nth(1).map(str::trim)
    }

    /// The case's source as rustdoc compiles it, wrapped in `main` if it needs to be.
    fn as_rustdoc_compiles_it(&self) -> String {
        if self.body.contains("fn main") {
            self.body.clone()
        } else {
            format!("fn main() {{\n{}}}\n", self.body)
        }
    }
}

/// Returns every `compile_fail` doctest in [`EXAMPLE_SUITE`].
fn refusals() -> Vec<Case> {
    let path = repository_root().join(EXAMPLE_SUITE);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

    let mut cases = Vec::new();
    let mut open: Option<Case> = None;
    for (index, line) in source.lines().enumerate() {
        let Some(documentation) = line.trim_start().strip_prefix("//!") else {
            continue;
        };
        let documentation = documentation.strip_prefix(' ').unwrap_or(documentation);

        if let Some(case) = open.as_mut() {
            if documentation.starts_with("```") {
                if case.info.starts_with("compile_fail") {
                    cases.push(open.take().expect("the block is open"));
                } else {
                    open = None;
                }
            } else {
                case.body.push_str(documentation);
                case.body.push('\n');
            }
        } else if let Some(info) = documentation.strip_prefix("```") {
            open = Some(Case {
                info: info.trim().to_owned(),
                body: String::new(),
                line: index + 1,
            });
        }
    }
    assert!(
        open.is_none(),
        "an unterminated code fence in {EXAMPLE_SUITE} would hide every case after it"
    );
    cases
}

/// Compiles `source` alone and returns the error codes rustc reported.
///
/// No `--extern` and no `-L`, which is what makes this faithful for a case whose
/// whole content is a name that does not resolve: rustdoc gives the doctest one
/// extern, `ravel_sdk`, and that extern has no bearing on whether `jni` or
/// `ravel_loader` resolves. A case that names `ravel_sdk` is a different
/// question and [`the_pinned_error_codes_are_the_codes_rustc_reports`] refuses
/// to pin a code on one.
fn reported_codes(label: &str, source: &str) -> BTreeSet<String> {
    let workspace = document("Cargo.toml");
    let edition = workspace
        .get("workspace")
        .and_then(Value::as_table)
        .and_then(|workspace| workspace.get("package"))
        .and_then(Value::as_table)
        .and_then(|package| package.get("edition"))
        .and_then(Value::as_str)
        .expect("the workspace pins one edition")
        .to_owned();

    let directory = std::env::temp_dir().join(format!("ravel-sdk-boundary-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("a temporary directory for one snippet");
    let file = directory.join(format!("{label}.rs"));
    fs::write(&file, source).expect("a snippet is writable");

    let output = Command::new("rustc")
        .arg("--edition")
        .arg(&edition)
        .args(["--crate-type", "bin", "--emit", "metadata"])
        .arg("--error-format=json")
        .arg("-o")
        .arg(directory.join(format!("{label}.meta")))
        .arg(&file)
        .output()
        .expect("rustc runs inside the devenv shell");
    let _ = fs::remove_dir_all(&directory);

    assert!(
        !output.status.success(),
        "{EXAMPLE_SUITE} calls this case `compile_fail`, but it compiles on its own:\n{source}"
    );

    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|diagnostic| {
            diagnostic
                .get("code")
                .and_then(|code| code.get("code"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn the_example_mod_declares_exactly_the_one_dependency_an_author_declares() {
    assert_eq!(
        every_declared_dependency(&document(EXAMPLE_MANIFEST)),
        BTreeSet::from([AUTHOR_DEPENDENCY.to_owned()]),
        "{EXAMPLE_MANIFEST} is the crate whose doctests are the compile-fail suite, so its import \
         scope has to stay an author's import scope. A second dependency — in any table, including \
         a `[target.*]` one, and including a dev- or build-dependency a doctest is compiled with in \
         scope — makes every refusal in {EXAMPLE_SUITE} prove less than it says."
    );
}

#[test]
fn every_example_declares_only_the_one_dependency_an_author_declares() {
    // The test above is about the crate whose doctests are the compile-fail
    // suite. This is about the rest: every example is a mod written against the
    // public surface, so an example that quietly grew a second dependency would
    // be demonstrating a manifest no author can write. The examples are
    // discovered rather than listed, because a list is a thing the next example
    // can be left out of.
    let root = repository_root().join(EXAMPLES);
    let mut examined = Vec::new();

    for entry in fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()))
    {
        let entry = entry.expect("a readable directory entry");
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let relative = format!(
            "{EXAMPLES}/{}/Cargo.toml",
            entry.file_name().to_string_lossy()
        );
        assert_eq!(
            every_declared_dependency(&document(&relative)),
            BTreeSet::from([AUTHOR_DEPENDENCY.to_owned()]),
            "{relative} is an example a mod author reads as a model manifest, so it declares \
             `{AUTHOR_DEPENDENCY}` and nothing else — in any table, including a `[target.*]` one \
             and including a dev- or build-dependency."
        );
        examined.push(relative);
    }

    examined.sort();
    assert!(
        examined.contains(&EXAMPLE_MANIFEST.to_owned()),
        "{EXAMPLE_MANIFEST} was not among the examples this test found, so the walk read the \
         wrong directory and every assertion above it was vacuous. Found: {examined:?}"
    );
}

#[test]
fn the_facade_closure_reaches_no_crate_a_mod_is_forbidden_to_reach() {
    let reached = closure();

    for (forbidden, reason) in FORBIDDEN {
        assert!(
            !reached.contains(*forbidden),
            "`{FACADE}`'s dependency closure reaches `{forbidden}` ({reason}). \
             docs/architecture/dependency-rules.md makes the SDK a facade that exposes no \
             bootstrap detail, no generated JVM type, and no Minecraft-version internal, and a \
             crate a mod cannot see is the only way that holds. Closure: {reached:?}"
        );
    }
}

#[test]
fn the_facade_closure_is_the_set_this_test_believes_it_is() {
    // Naming the whole closure is what stops the check above from passing
    // vacuously. A typo in `FORBIDDEN`, or a walk that silently reached nothing,
    // satisfies "holds no forbidden crate" perfectly.
    //
    // The list is every package, Ravel's and otherwise, because `jni` is one of
    // the forbidden entries and a set holding only Ravel crates could not have
    // caught it — and because a third-party subtree is where a forbidden edge
    // would hide from a Ravel-only check. `proc-macro2`, `quote`, `syn` and
    // `unicode-ident` are `ravel-sdk-macros`' dependencies, so they run at a
    // mod's compile time and are not linked into it; they are listed rather than
    // filtered out because deciding which edge is a build edge is exactly the
    // judgement this test should not be making.
    //
    // The set over-approximates what a mod links, deliberately. A
    // dev-dependency of a crate in here is in the lock's graph and so is in this
    // set, while `cargo tree -p ravel-example-minimal-mod -e normal` never shows
    // it — erring wide is right for a boundary check, and deciding which edge is
    // a real one is the judgement this test should not be making.
    //
    // So this set moves for more than a version bump that adds a transitive
    // package: a dev-dependency or a build-dependency added to any crate already
    // in the closure moves it too. That is the intended cost. The lock is
    // tracked, so every entry here arrived by a decision somebody made.
    let expected: BTreeSet<String> = [
        "itoa",
        "memchr",
        "proc-macro2",
        "quote",
        "ravel-core",
        // M4-W2 put the declaration model a mod writes — registry entries, tag
        // memberships, supplied resources, and the pack they generate — behind
        // `content`, `tags`, `resources` and `generate`. The model is
        // `ravel-data`'s, because `ravel-pack` reads the same declarations in
        // M4-W5 and a model living in the facade would invert that. It reaches
        // nothing new: `ravel-data` depends on `ravel-core` and `serde_json`,
        // both already here. `RelativePath` moved down to `ravel-core` for that
        // reason — `ravel-data` naming it through `ravel-package` would have put
        // that crate's SHA-256 implementation and its `digest` subtree into every
        // mod's build graph, for a validated path newtype.
        "ravel-data",
        "ravel-host-api",
        "ravel-save",
        "ravel-sdk",
        "ravel-sdk-macros",
        "semver",
        "serde",
        "serde_core",
        "serde_derive",
        "serde_json",
        "syn",
        "unicode-ident",
        "zmij",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    assert_eq!(
        closure(),
        expected,
        "the mod-facing closure changed. This set is every package `Cargo.lock` reaches from \
         `{FACADE}`, which over-approximates what a mod links: a dev- or build-dependency of a \
         crate already here moves it without entering a mod's compiled graph. An addition is \
         still a decision about the public surface, so decide it — justify the new edge and add \
         the package here, or remove the edge. Narrowing what this set holds is not the third \
         option: `jni` is third-party and reached the facade through a table nobody read, so a \
         set filtered to Ravel crates, or to normal dependencies, is the defect rather than the \
         fix."
    );
}

#[test]
fn every_package_in_the_closure_comes_from_the_source_its_name_implies() {
    // The closure is keyed on package name and the two tests above read no source
    // at all, so a `[patch]` or `[replace]` redirecting a name already in the
    // expected set changes nothing either of them looks at: `serde` is still
    // `serde`, the expected set still matches, and a mod compiles substituted code.
    // `deny.toml`'s `unknown-git = "deny"` closes the git half inside
    // `ravel-audit`, and it treats a path package as local, so a path redirect is
    // the residual gap and the `source` field is what closes it.
    let sources = locked_sources();
    let root = repository_root();

    for name in closure() {
        let resolved = sources
            .get(&name)
            .unwrap_or_else(|| panic!("`{name}` is in the closure, so Cargo.lock names it"));
        for source in resolved {
            match source {
                Some(source) => assert_eq!(
                    source.as_str(),
                    CRATES_IO,
                    "`{name}` is in the mod-facing closure and Cargo.lock resolves it from \
                     `{source}` rather than crates.io. A substituted source for a package whose \
                     name is already expected is invisible to every other check here, because \
                     they read names."
                ),
                None => assert!(
                    root.join("crates").join(&name).join("Cargo.toml").is_file(),
                    "`{name}` is in the mod-facing closure and Cargo.lock gives it no source, \
                     which means a local directory, but `crates/{name}` is not a crate in this \
                     workspace. A `[patch]` or `[replace]` pointing an expected name at a path is \
                     what this refuses: the name a mod compiles would be the same and the code \
                     would not."
                ),
            }
        }
    }
}

#[test]
fn every_forbidden_crate_this_test_names_still_exists_to_be_forbidden() {
    // A renamed or deleted crate would make its row above unfalsifiable, and the
    // check would keep passing while the rule it stands for went unenforced.
    let root = repository_root();
    for (forbidden, reason) in FORBIDDEN {
        if *forbidden == "jni" {
            let workspace = document("Cargo.toml");
            let table = workspace
                .get("workspace")
                .and_then(Value::as_table)
                .expect("the root manifest declares a workspace");
            assert!(
                declared(table, "dependencies").contains("jni"),
                "`jni` is no longer a workspace dependency, so forbidding it proves nothing"
            );
            continue;
        }
        assert!(
            root.join("crates")
                .join(forbidden)
                .join("Cargo.toml")
                .is_file(),
            "`{forbidden}` ({reason}) is named as forbidden but no longer exists"
        );
    }
}

#[test]
fn every_forbidden_crate_is_refused_by_a_case_an_author_can_read() {
    // The closure check and the compile-fail suite are two halves of the same
    // claim, and they were written independently: `FORBIDDEN` named crates the
    // suite refused nothing about, and nothing noticed. This is what makes the
    // next addition to `FORBIDDEN` arrive with the refusal that demonstrates it.
    //
    // Only this direction is checked. The suite also refuses `ravel-host-api`,
    // which cannot be in `FORBIDDEN` because the facade depends on it: a mod
    // does not name it, but a mod does compile it.
    let refusals = refusals();
    for (forbidden, reason) in FORBIDDEN {
        let import = format!("use {}::", forbidden.replace('-', "_"));
        assert!(
            refusals.iter().any(|case| case.body.contains(&import)),
            "`{forbidden}` ({reason}) is forbidden by this file but no `compile_fail` case in \
             {EXAMPLE_SUITE} writes `{import}`, so nothing shows an author what the refusal looks \
             like"
        );
    }
}

#[test]
fn the_pinned_error_codes_are_the_codes_rustc_reports() {
    // Stable rustdoc ignores the code after `compile_fail`: a review set one to
    // `E0999`, which does not exist, and the doctests stayed at exit 0. So a
    // pinned code is a literal nothing recomputes unless something like this
    // does, and this recomputes it by compiling the case and reading the
    // diagnostic.
    //
    // A case that names `ravel_sdk` cannot be checked this way — it would need
    // the facade's own rlib, and the deps directory holds one per feature set
    // that has ever been built, so there is no path to it a test can name. Those
    // cases carry no code, and this asserts that rather than trusting it: their
    // teeth are elsewhere, in the passing case each privacy refusal is paired
    // with, and in the facade's own absence of a re-export.
    let refusals = refusals();
    assert!(
        refusals.len() > 1,
        "no `compile_fail` case was found in {EXAMPLE_SUITE}, so this test read nothing"
    );

    // Which cases must pin is decided per case rather than counted. An earlier
    // version of this counted the cases that pinned a code and required at least
    // `FORBIDDEN.len()` of them. More cases pin than `FORBIDDEN` has rows — the
    // suite also refuses `ravel-host-api`, which cannot be a row — so a code
    // could be dropped and the suite stayed at exit 0, which a review
    // demonstrated. A count leaves that slack by construction. This has none:
    // a case this test can compile has to pin a code, and a case it cannot has to
    // pin none.
    for case in &refusals {
        if case.body.contains("ravel_sdk") {
            assert!(
                case.pinned_code().is_none(),
                "{EXAMPLE_SUITE}:{} pins `{}` on a case that names `ravel_sdk`, which this test \
                 cannot compile on its own. A code nothing recomputes is the defect this test \
                 exists for: drop the code and leave the case `compile_fail`.",
                case.line,
                case.pinned_code().unwrap_or_default()
            );
            continue;
        }

        let Some(code) = case.pinned_code() else {
            panic!(
                "{EXAMPLE_SUITE}:{} is a `compile_fail` case this test can compile, and it pins \
                 no error code, so nothing recomputes why it fails. Pin the code rustc reports. \
                 Only a case that names `ravel_sdk` may pin none.",
                case.line
            );
        };

        let label = format!("case-{}", case.line);
        let reported = reported_codes(&label, &case.as_rustdoc_compiles_it());
        assert!(
            reported.contains(code),
            "{EXAMPLE_SUITE}:{} pins `{code}` but rustc reports {reported:?} for it",
            case.line
        );
    }
}
