#![forbid(unsafe_code)]

//! SDK dependency, compile-fail, and copyable documentation boundaries.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use toml::{Table, Value};

const FACADE: &str = "ravel-sdk";

const EXAMPLE_MANIFEST: &str = "examples/minimal-mod/Cargo.toml";

const EXAMPLES: &str = "examples";

const EXAMPLE_SUITE: &str = "examples/minimal-mod/src/boundary.rs";

const AUTHOR_DEPENDENCY: &str = "ravel-sdk";

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

const FORBIDDEN: &[(&str, &str)] = &[
    ("ravel-bootstrap", "the JVM entry crate"),
    ("ravel-minecraft", "the only crate that names a JNI type"),
    ("ravel-minecraft-adapter", "the pinned version's internals"),
    ("ravel-minecraft-generate", "build-time source generation"),
    ("ravel-loader", "the lifecycle machine and claim table"),
    ("ravel-composed-host", "the assembled host"),
    ("ravel-observability", "the report and profiling surfaces"),
    (
        "ravel-testkit",
        "a test harness, never a production dependency",
    ),
    ("jni", "Java itself"),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the conformance crate sits two directories below the root")
        .to_path_buf()
}

fn document(relative: &str) -> Table {
    let path = repository_root().join(relative);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    text.parse::<Table>()
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

fn declared(table: &Table, section: &str) -> BTreeSet<String> {
    table
        .get(section)
        .and_then(Value::as_table)
        .map(|section| section.keys().cloned().collect())
        .unwrap_or_default()
}

// Manifest declarations include target-specific, dev, and build dependencies.
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
                .flat_map(move |section| declared(target, section))
        });

    plain.chain(per_target).collect()
}

// Merge versions by package name: any edge to a forbidden package violates the boundary.
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

// A missing source denotes a local path, including path-based patches and replacements.
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

struct Case {
    info: String,
    body: String,
    // One-based source line for diagnostics.
    line: usize,
}

impl Case {
    fn pinned_code(&self) -> Option<&str> {
        self.info.split(',').nth(1).map(str::trim)
    }

    fn as_rustdoc_compiles_it(&self) -> String {
        if self.body.contains("fn main") {
            self.body.clone()
        } else {
            format!("fn main() {{\n{}}}\n", self.body)
        }
    }
}

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

// No externs are needed for snippets that only name unavailable crates.
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
    fs::remove_dir_all(&directory).expect("the snippet directory is removable");

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
    // Include dev/build edges and third-party subtrees to avoid under-approximating the boundary.
    let expected: BTreeSet<String> = [
        "itoa",
        "memchr",
        "proc-macro2",
        "quote",
        // Configuration declarations and Mod::configure share these types.
        "ravel-config",
        "ravel-core",
        // The content model is shared with ravel-pack without depending on the facade.
        "ravel-data",
        "ravel-host-api",
        // Authors implement Packet; the host shares its model through ravel-host-api.
        "ravel-network",
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
    // Matching package names alone cannot detect a substituted source.
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
    // A renamed or deleted crate would leave an unfalsifiable forbidden entry.
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
    // The suite also refuses direct imports of allowed transitive crates.
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
    // Stable rustdoc ignores pinned error codes; check standalone snippets with rustc.
    let refusals = refusals();
    assert!(
        refusals.len() > 1,
        "no `compile_fail` case was found in {EXAMPLE_SUITE}, so this test read nothing"
    );

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

const FACADE_SOURCE: &str = "crates/ravel-sdk/src";

// Authors can name the facade, language paths, and compiler-provided crates.
const NAMEABLE_ROOTS: &[&str] = &[
    "ravel_sdk",
    "crate",
    "self",
    "super",
    "Self",
    "std",
    "core",
    "alloc",
];

const PRIMITIVES: &[&str] = &[
    "bool", "char", "str", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16",
    "u32", "u64", "u128", "usize",
];

struct DocLine {
    file: String,
    line: usize,
    code: String,
}

fn files_in(directory: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(directory)
        .expect("the facade's source directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    files
}

// Include text/ignored fences too: authors may copy them even when rustdoc skips them.
fn doc_example_lines() -> Vec<DocLine> {
    let mut lines = Vec::new();
    for path in files_in(&repository_root().join(FACADE_SOURCE)) {
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let file = path
            .file_name()
            .expect("a named file")
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(&path).expect("a readable source file");
        let mut inside = false;
        for (index, raw) in text.lines().enumerate() {
            let trimmed = raw.trim_start();
            let Some(rest) = trimmed
                .strip_prefix("//!")
                .or_else(|| trimmed.strip_prefix("///"))
            else {
                continue;
            };
            let body = rest.strip_prefix(' ').unwrap_or(rest);
            if body.trim_start().starts_with("```") {
                inside = !inside;
                continue;
            }
            if inside {
                lines.push(DocLine {
                    file: file.clone(),
                    line: index + 1,
                    code: body.to_owned(),
                });
            }
        }
        assert!(
            !inside,
            "{file} leaves a doc fence open, so what this file read as an example is the rest of \
             the file"
        );
    }
    lines
}

// A root precedes `::` and is not itself preceded by a path separator or field access.
fn path_roots(code: &str) -> Vec<String> {
    fn is_word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    let bytes = code.as_bytes();
    let mut roots = Vec::new();
    let mut start = None;
    for index in 0..=bytes.len() {
        let word = index < bytes.len() && is_word(bytes[index]);
        match (word, start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                start = None;
                // Byte scanning tolerates non-ASCII prose without slicing inside a character.
                let preceded = from >= 2 && bytes[from - 2] == b':' && bytes[from - 1] == b':'
                    || from >= 1 && bytes[from - 1] == b'.';
                let followed = bytes[index..].starts_with(b"::");
                if !preceded && followed {
                    roots.push(code[from..index].to_owned());
                }
            }
            (true, Some(_)) | (false, None) => {}
        }
    }
    roots
}

#[test]
fn every_doc_example_in_the_facade_names_only_crates_a_mod_can_name() {
    let lines = doc_example_lines();
    assert!(
        !lines.is_empty(),
        "no doc example was found in {FACADE_SOURCE}, so this test read nothing"
    );

    for line in &lines {
        for root in path_roots(&line.code) {
            if NAMEABLE_ROOTS.contains(&root.as_str()) || PRIMITIVES.contains(&root.as_str()) {
                continue;
            }
            // Uppercase roots are imported type or trait names.
            if root
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
            {
                continue;
            }
            panic!(
                "{FACADE_SOURCE}/{}:{} shows `{root}::` in a doc example. A mod crate lists \
                 `{AUTHOR_DEPENDENCY}` and nothing else, so it cannot name `{root}`, and the \
                 example does not compile where a reader will paste it. Reach the name through \
                 `ravel_sdk`.\n{}",
                line.file, line.line, line.code
            );
        }
    }
}
