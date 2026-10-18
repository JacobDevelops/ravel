//! Copyable SDK documentation must use only author-visible crate paths.

use std::fs;
use std::path::{Path, PathBuf};

use super::{AUTHOR_DEPENDENCY, repository_root};

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
