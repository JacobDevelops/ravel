#![forbid(unsafe_code)]

//! Every Rust block in the author's guide, held to the crate that compiles it.
//!
//! `docs/guide/` is prose full of code and markdown compiles nowhere, so a block
//! in it is a claim nothing recomputes. `examples/guide-snippets` is where those
//! blocks actually live: it depends on `ravel-sdk` alone, so it compiles as an
//! author's crate compiles, and its tests drive each one. This file is what makes
//! the guide's copy the same code — every fenced Rust block in `docs/guide/`
//! has to appear in that crate's source.
//!
//! # Why the comparison ignores whitespace
//!
//! The crate is formatted by `rustfmt` and the guide is not. A block sitting
//! inside a module is indented; the same block in markdown is not, and rustfmt
//! wraps a long line where the guide leaves it alone. None of that is drift. What
//! would be drift is a changed name, a changed call, a dropped line or a comment
//! that no longer says what the code does, and all of those survive having the
//! whitespace taken out of both sides.
//!
//! Language-tagged blocks other than `rust` are not checked here. A `console`
//! block is a command and a `json` block is a document; the JSON the guide shows
//! is validated against the schemas it names by the same gate that validates
//! every other document in the repository.

use std::fs;
use std::path::{Path, PathBuf};

/// The directory whose markdown is the author's guide.
const GUIDE: &str = "docs/guide";

/// The crate every Rust block in the guide is a copy of.
const SNIPPETS: &str = "examples/guide-snippets/src";

/// The fence language a block has to carry to be checked.
const CHECKED_LANGUAGE: &str = "rust";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

/// Every file in a directory, sorted, so a run is the same run twice.
fn files_in(directory: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(directory)
        .expect("the directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// One fenced block, with where it came from.
struct Block {
    document: String,
    /// The 1-based line the opening fence sits on, which is what an editor shows.
    line: usize,
    body: String,
}

fn rust_blocks(path: &Path) -> Vec<Block> {
    let document = path
        .file_name()
        .expect("a named file")
        .to_string_lossy()
        .into_owned();
    let text = fs::read_to_string(path).expect("a readable document");
    let mut blocks = Vec::new();
    // A fence in any other language still opens a block, because its contents
    // must not be mistaken for the next one; only a `rust` fence is collected.
    let mut open: Option<(usize, bool, Vec<&str>)> = None;
    for (index, line) in text.lines().enumerate() {
        match (&mut open, line.strip_prefix("```")) {
            (None, Some(language)) => {
                open = Some((index + 1, language.trim() == CHECKED_LANGUAGE, Vec::new()));
            }
            (Some(_), Some(_)) => {
                let (line, checked, body) = open.take().expect("a block is open");
                if checked && !body.is_empty() {
                    blocks.push(Block {
                        document: document.clone(),
                        line,
                        body: body.join("\n"),
                    });
                }
            }
            (Some((_, _, body)), None) => body.push(line),
            (None, None) => {}
        }
    }
    assert!(
        open.is_none(),
        "{document} leaves a code fence open, so what this file read as a block is the rest of \
         the document"
    );
    blocks
}

/// The text with every whitespace character removed.
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The crate's whole source, compacted, as one haystack.
fn snippet_source() -> String {
    files_in(&repository_root().join(SNIPPETS))
        .iter()
        .map(|path| fs::read_to_string(path).expect("a readable source file"))
        .map(|text| compact(&text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every Rust block in the guide, in reading order.
fn guide_blocks() -> Vec<Block> {
    files_in(&repository_root().join(GUIDE))
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .flat_map(|path| rust_blocks(path))
        .collect()
}

/// A parser that found nothing passes the comparison below by having nothing to
/// compare, so what it found is counted against a second, dumber reading of the
/// same files: one block per fence line that opens one.
#[test]
fn the_parser_finds_every_rust_block_a_plain_search_finds() {
    let mut expected = 0;
    for path in files_in(&repository_root().join(GUIDE)) {
        if path.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("a readable document");
        expected += text
            .lines()
            .filter(|line| line.trim_end() == "```rust")
            .count();
    }
    assert!(expected > 0, "the guide shows no Rust at all");
    assert_eq!(guide_blocks().len(), expected);
}

#[test]
fn every_rust_block_in_the_guide_is_a_copy_of_the_crate_that_compiles_it() {
    let haystack = snippet_source();
    for block in guide_blocks() {
        assert!(
            haystack.contains(&compact(&block.body)),
            "`{GUIDE}/{}` line {} shows Rust that `{SNIPPETS}` does not contain, so nothing \
             compiles it:\n{}",
            block.document,
            block.line,
            block.body
        );
    }
}

/// A `rust` block whose fence lies about its language would be skipped silently.
///
/// The guide's other fences are `console`, `json` and `toml`. Anything else is
/// either a new kind of block someone should decide about or a `rust` block
/// mistyped into invisibility, and both are worth a failure.
#[test]
fn every_fence_in_the_guide_names_a_language_this_file_knows() {
    const KNOWN: [&str; 4] = ["rust", "console", "json", "toml"];
    for path in files_in(&repository_root().join(GUIDE)) {
        if path.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("a readable document");
        let mut inside = false;
        for line in text.lines() {
            let Some(language) = line.strip_prefix("```") else {
                continue;
            };
            if inside {
                inside = false;
                continue;
            }
            inside = true;
            assert!(
                KNOWN.contains(&language.trim()),
                "{} opens a fence tagged `{}`, which nothing here checks",
                path.display(),
                language.trim()
            );
        }
    }
}
