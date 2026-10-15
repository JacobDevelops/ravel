# Guide snippets

Every Rust block in [`docs/guide/`](../../docs/guide) is a copy of a run of lines
in [`src/lib.rs`](src/lib.rs). This crate is where they compile.

This is not an installable mod. It depends only on `ravel-sdk`, matching a mod
author's dependency boundary. Its tests exercise the snippets' behavior as well
as compiling them.

`tests/conformance/tests/integration/guide_snippets.rs` checks that every guide Rust block
occurs in this source.
