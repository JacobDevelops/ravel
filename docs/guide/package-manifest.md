# The package manifest

A crate is not yet a mod. A mod is a **package**: the crate's source, plus a
`package.json` declaring what it is and a provenance record saying where it came
from. The manifest is the only place the loader learns a mod's identity, its
compatibility, and the sides it may load on, and package metadata is validated
before any of a mod's code runs.

**A launched game reads your manifest and still will not run your mod.** A launch
discovers `ravel/mods`, validates every installed manifest against the schema
below, and verifies the digests each one records — so a malformed or altered
package stops the launch by name. What it will not do is admit what it found: the
mods a launch runs are the ones its host was compiled from, and an installed
package that host does not name refuses the launch rather than joining it.
`ravel-cli host build` is what compiles a host that names them. Until a launch
runs that itself, write the manifest as though it were read, because it is. See
[What does not work yet](limits.md).

[`schemas/package.schema.json`](../../schemas/package.schema.json) is the
authority and is machine-checkable — the schema and the Rust parser are held to
the same verdict on the same candidate strings, in both directions, so anything
the schema accepts the loader accepts.

[`examples/minimal-mod`](../../examples/minimal-mod) is the shortest complete
package: a crate, a `package.json`, and a `provenance.json`. Copy it.

## A complete manifest

This one uses every optional field, which no real mod does:

```json
{
  "schemaVersion": 1,
  "id": "copper_lanterns",
  "version": "0.1.0",
  "displayName": "Copper Lanterns",
  "description": "A lantern that takes copper and joins the c:lanterns tag.",
  "compatibility": {
    "ravel": ">=0.0.0, <1.0.0",
    "minecraft": ["26.1.2"]
  },
  "sides": ["client", "integrated-server", "dedicated-server"],
  "dependencies": [
    {
      "id": "example_library",
      "relationship": "required",
      "version": "^1.0.0",
      "order": "after"
    }
  ],
  "resources": ["assets/copper_lanterns/models/block/copper_lantern.json"],
  "configurationSchemas": ["configuration.json"],
  "savedDataVersions": [1, 2],
  "authors": [{ "name": "Your Name", "url": "https://example.invalid/you" }],
  "license": "MIT OR Apache-2.0",
  "source": {
    "repository": "https://example.invalid/copper-lanterns",
    "files": [
      {
        "path": "Cargo.toml",
        "sha256": "cc3be6bf1ac361cb00433df7b8604fe9fecfa0a6d178f0ed3fca353b3b713e68"
      },
      {
        "path": "src/lib.rs",
        "sha256": "82e9c63cbd3843fc57a11fd3ab60f2214e477f06482f9d2603986236c6edc4f3"
      }
    ]
  },
  "provenance": {
    "record": "provenance.json",
    "sha256": "682b7ace97fe064be867b56fbef2e57510be1d2020bceb2971a1a46efa68ba1e"
  }
}
```

Those digests are of course wrong for your files; recompute them, as below. The
example's manifest carries real ones, and the repository's own gate recomputes
them, so a stale digest there is a failing build rather than something an author
copies.

Required: `schemaVersion`, `id`, `version`, `compatibility`, `sides`, `authors`,
`license`, `source`, `provenance`. Everything else is optional and defaults to
empty. An optional field you have nothing to say about is a field you leave out
rather than one you write empty, which is why `examples/minimal-mod` is so much
shorter than the manifest above.

The schema refuses any property it does not define, at every level. A typo in a
field name is a refusal, not a field that is quietly ignored.

## The fields worth explaining

**`id`** is lowercase and matches `^[a-z][a-z0-9]*(?:[-_.][a-z0-9]+)*$`.
`Copper_Lanterns` is refused; so is a leading digit, a trailing separator, and
two separators in a row. This id is your content namespace, and it must be the
same string you pass to `#[declare_mod(id = ...)]` — see the pitfall about that
in [What "conforming" is checked to mean](conforming.md).

**`version`** is strict three-part semver, optionally with a pre-release and a
build suffix. `1.4` is refused.

**`compatibility.minecraft`** is a non-empty list of the game versions the mod
supports. Each entry is either an exact version (`26.1.2`) or an explicit range
(`>=26.1, <26.2`); mix them freely, e.g. a range for the current line plus an
exact pin for an older release. A range is your tested claim about a span of
releases — the loader honours it and never infers support for a version you did
not declare, and a range is only evaluated against a semver-shaped running
version (a snapshot must be pinned exactly). Today the reference target is
`26.1.2`. **`compatibility.ravel`** is a range; a mod outside it is refused at
admission with both versions named.

**`sides`** is a non-empty subset of `client`, `integrated-server`,
`dedicated-server`. A side is a role within a process, not a process kind:
single-player runs a client and an integrated server together, so a mod present
in a single-player session is driven twice, once per side. A mod not declaring a
side is parked before any of its code runs there.

**`dependencies`** each name an `id`, a `relationship` — `required`, `optional`,
`incompatible`, `discouraged` — and a version range. `order` is `before` or
`after` and is how you get deterministic load ordering when ordering actually
matters; declare it only when it does. `sides` narrows a dependency to particular
sides.

An `optional` relationship is an ordering preference and not a requirement, so
your mod is admitted whether or not that mod is there. That leaves you needing to
know which happened, and `Registrar::loads` is the question: pass the other mod's
id during `register` and it answers whether this launch is running it. Claim the
integration's content inside that branch and it exists only when the thing it
integrates with does.

```rust
fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
    if registrar.loads(&self.lanterns) {
        claim_all(&integration(), registrar)?;
    }
    Ok(())
}
```

`false` is three different absences and does not tell them apart: the mod is not
installed, or it is installed and its `sides` list parks it on this side, or it
ran and failed. Integrating with any of those would register content backed by
nothing, so one answer covers all three. An off switch is yours to add on top —
declare a boolean setting and check it beside the `loads` call — because a player
turning an integration off is a different question from the target being absent.

There is a second way to depend on a mod optionally, and it answers a different
question. [Using a mod only if it is installed](state.md#using-a-mod-only-if-it-is-installed)
declares an optional *service* and branches on whether anything answered the
call. Use that when the integration is you calling into the other mod, and it
only works when that mod offers a service. Use `loads` when the integration is
content you register, because registration closes before services are called and
because a mod that answers nothing is still a mod you can build a recipe around.

**`savedDataVersions`** lists every version of world data this build can read.
It must agree with what your `Mod::saved_data_declaration` declares.

**`source`** is what makes this a source-only package. It names the repository and
at least one portable source file, each with its SHA-256 digest. **There is no
artifact field.** A compiled mod is not a thing Ravel accepts: not a static
library, not a dynamic library, not an executable. Schema v1 refuses a `targets`
property outright, which is the old shape.

Every path in the manifest is package-relative, forward-slashed, and may not
escape the package: a leading `/`, any `..` segment, a backslash, or an embedded
line break is refused by both the schema and the parser.

## Digests, and the part that will bite you

Before a package loads, every declared source file is hashed and compared to the
digest the manifest records, and so is the provenance record. A mismatch refuses
the package. Truncation, alteration and extension are all the same failure.

That means **every edit to a declared source file invalidates the manifest.**
Your mod lives outside this repository, so the way to refresh a digest is

```console
sha256sum Cargo.toml src/lib.rs provenance.json
```

and pasting the lowercase hex in. Uppercase hex is refused. Any SHA-256 tool
does — `shasum -a 256` on macOS, `Get-FileHash` on Windows — because the digest
is of the file's bytes and nothing about it is Ravel's.

There is a subcommand, `cargo xtask package digests <package directory>`, which
rewrites a manifest's digests from the files on disk, leaves everything else in
the file exactly as you wrote it, and tells you when they were already current;
`--check` verifies without rewriting, which is the form a CI job wants. It is an
in-tree tool of this repository: reaching it means a checkout of Ravel and
running `cargo xtask` there pointed at your package directory elsewhere. That is
worth doing if you already keep a checkout for the `path` dependency — you do —
and it is not something published for you to install.

When validation does refuse, the message names the file and gives you the digest
it actually has — `` `lib.rs` hashes <actual> but the manifest records
<declared>`` — so a single stale digest is a copy-and-paste fix. The loader names
a file and never a path, deliberately and everywhere: no published message names
a filesystem path, which [Conforming](conforming.md) states as a rule. So a
package with two files of the same base name in different directories gets two
refusals that read alike, and the declaration order below is what tells them
apart. The subcommand has no such constraint and prints the full relative path,
which is the other reason to reach for it.

It reports the first failure in declaration order only, so several edited files
take several rounds by hand. That is what the subcommand is for.

**Only what `source.files` lists is verified.** A configuration schema or a
resource named elsewhere in the manifest is not hashed and not checked. If you
want your `configuration.json` and your supplied resources integrity-protected —
and you do — list them in `source.files` as well as in their own field.

## Checking a manifest against the schema

There is no `ravel validate` and no subcommand that runs the schemas for you.
[`schemas/package.schema.json`](../../schemas/package.schema.json),
[`schemas/provenance.schema.json`](../../schemas/provenance.schema.json) and
[`schemas/configuration.schema.json`](../../schemas/configuration.schema.json)
are plain JSON Schema 2020-12 documents, so any validator that speaks that
dialect checks your files: `check-jsonschema`, `ajv`, Python's `jsonschema`,
whichever you already have installed.

```console
check-jsonschema --schemafile <path to ravel>/schemas/package.schema.json package.json
```

Doing this catches the class of mistake the loader would otherwise report one
field at a time, and it catches it before you have a digest to refresh. The
schema and the Rust parser are held to the same verdict on the same candidates,
in both directions, so a document a validator accepts is a document the loader
accepts.

## The provenance record

`provenance.json` is required and validated against
[`schemas/provenance.schema.json`](../../schemas/provenance.schema.json). It
records where the mod's content came from and whether it may be published. A
record for original work is short:

```json
{
  "schemaVersion": 1,
  "subject": { "kind": "ravel-package", "id": "copper_lanterns", "version": "0.1.0" },
  "implementationMethod": "original",
  "sources": [],
  "rights": [
    { "surface": "code", "status": "cleared", "basis": "original work" },
    { "surface": "textures", "status": "not-applicable" }
  ],
  "attribution": [],
  "publication": { "status": "cleared", "reviewedBy": "Your Name" }
}
```

`implementationMethod` is one of `original`, `source-informed`,
`behaviour-only-clean-room`, `permission-based`, `metadata-only`,
`not-applicable`. If your mod reimplements someone else's behaviour, say so here
and record the source you worked from; that is what the field is for.
