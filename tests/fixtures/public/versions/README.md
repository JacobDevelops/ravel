# Synthetic version data

M4-W6 proves Ravel is not welded to one Minecraft version. The proof needs a
second version's data, and this directory holds it.

Everything in it is invented. The version it is named for does not exist, the
classes and members its inventory reviews do not exist, and the jar hashes its
adapter inputs pin describe files that are never present. That is deliberate on
two counts. Tracking a real game artefact is forbidden, and a real second
version would not make the proof stronger: what is being tested is that the
generator reads version data rather than carrying a version, and data it has
never seen before answers that question exactly as well as data from a real
build would.

What is not invented is the shape. The directory is a drop-in replacement for
`minecraft/versions/<version>/` — the same files, the same schema, the same
pins — so the generator reads it through the same code path that reads the
pinned version, with no branch and no fixture-only argument.

Two properties are worth stating because a reader will otherwise wonder:

- **The hook ids are the pinned version's own.** `ravel-minecraft` says a hook id
  is the only name that survives a game update, so a second version of the same
  capability surface reviews the same ids and maps each onto its own class and
  member. That is what makes a general crate version-neutral: it names ids, and
  ids are what two versions share.
- **It is a fixture rather than a second directory under
  `minecraft/versions/`.** The generator refuses two pinned versions on purpose,
  because two would need two hook classes and two exported symbol sets and ADG-3
  decided neither. Staging this one as if it were the pinned version asks the
  question M4-W6 asks without answering a question ADG-3 left open.

The gate that reads it is
`crates/ravel-minecraft-adapter/src/generate/mod.rs`'s
`the_generator_reads_a_second_version_without_naming_the_pinned_one`, and
`ravel-verify-scaffold` treats a directory name here as a version string no crate
may spell, and the class names its inventory reviews as names no crate may spell
either — the same two rules the pinned version's data already has. That sweep
reads `crates/` and nothing else, which is why this directory may name its own
classes freely.
