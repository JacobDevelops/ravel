#![forbid(unsafe_code)]

//! Guide Rust blocks must match the SDK-only guide-snippets crate, ignoring whitespace.
//! Other fence languages and guide prose are outside this comparison.

use std::fs;
use std::path::{Path, PathBuf};

const GUIDE: &str = "docs/guide";

const SNIPPETS: &str = "examples/guide-snippets/src";

const CHECKED_LANGUAGE: &str = "rust";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two directories below the repository root")
        .to_path_buf()
}

fn files_in(directory: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(directory)
        .expect("the directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    files
}

struct Block {
    document: String,
    // Opening fence line, 1-based for editor navigation.
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
    // Track other languages too, so their contents cannot open a Rust block.
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

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn snippet_source() -> String {
    files_in(&repository_root().join(SNIPPETS))
        .iter()
        .map(|path| fs::read_to_string(path).expect("a readable source file"))
        .map(|text| compact(&text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn guide_blocks() -> Vec<Block> {
    files_in(&repository_root().join(GUIDE))
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .flat_map(|path| rust_blocks(path))
        .collect()
}

// Independently count opening fences to catch a parser that silently skips blocks.
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

// Unknown tags may be mistyped Rust fences and must not silently skip comparison.
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
