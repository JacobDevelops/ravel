# World fixtures

The saved-data half of a Ravel world, as M3-W4 drives it. Each directory is one
world in one condition, and every one of them is original Ravel content: no
official Minecraft world, no region file, no third-party asset, and no local
path. A real Minecraft world cannot live here — `ROADMAP.md`'s constraints keep
private worlds and official assets out of this repository entirely — so a
fixture records the part Ravel owns and nothing else.

That restriction is not a limitation of the fixture. `SPEC.md` §14 makes the
saved-data decision a thing that happens *before* a world opens, so what decides
whether a world may be opened is the versions each mod's data carries and the
packages it names. That is exactly what `world.json` holds.

## What a world holds

| Field | Meaning |
| --- | --- |
| `schemaVersion` | The fixture format, so a later shape can be told apart from this one. |
| `aggregateFormatVersion` | The format the world's canonical aggregate was written in. Optional; a world that omits it was written in the format this build reads. |
| `savedData[].owner` | The canonical mod id that owns the data, which `SPEC.md` §14 ties ownership to. |
| `savedData[].version` | The version the world was written under. Non-zero, because zero is what an absent version reads as. |
| `savedData[].envelopeSchemaVersion` | The schema this owner's envelope was written in, which is separate from the data version inside it. Optional, and omitted means the schema this build reads. |
| `savedData[].requiredPackages` | The packages this owner's data needs installed, each at one exact version, to be interpreted at all. Optional and empty by default. |
| `savedData[].state` | Opaque to everything but the owning mod. Ravel decides whether it may be read; it does not interpret it. |

Which packages are *installed* is not among them. That is a property of the
installation reading the world rather than of the world, so a fixture states
only what its data requires and the test states what is installed against it.

There is no schema under `schemas/` for this. A fixture format is test input
rather than a product contract, and adding one would put a test convenience in
the place `SPEC.md` reserves for things a package must satisfy.

## The worlds

Each one exists for a case in M3-W4's verification column, and the seven
together are that column: "create/save/exit/reload preserves state; schema,
package, migration and size incompatibilities fail closed."

| World | Stored version | What it proves |
| --- | --- | --- |
| [`current`](current/world.json) | 3 | Reload preserves state. The world is already at the version the mod writes, so it opens with no migration step. |
| [`migratable`](migratable/world.json) | 1 | An older world opens, and the plan names every version it steps through. |
| [`future`](future/world.json) | 9 | Fails closed. The world was written by a build that knows a format this one has never seen, and guessing at it is the silent continuation §14 forbids. |
| [`absent`](absent/world.json) | — | Initializes fresh. The mod is installed and required, the world holds no data it owns, and ADG-7 gives absence one policy for every world rather than reading it as loss. |
| [`unsupported-format`](unsupported-format/world.json) | 3 | Fails closed on the aggregate itself. Every owner inside it is readable, and the container they arrive in is not, so the refusal names no mod to blame. |
| [`unsupported-schema`](unsupported-schema/world.json) | 3 | Fails closed on one owner's envelope. The data version is one this build reads, which is what makes the envelope schema the thing being decided. |
| [`conflicting-package`](conflicting-package/world.json) | 3 | Fails closed on a package that *is* installed, at a version the stored data cannot be read under. Absence is already a different refusal; this is the one a partial upgrade produces. |

The versions assume the save test mod declares 1, 2 and 3 and writes 3. That
pairing is the fixture's only coupling to a mod, and it is stated here rather
than inferred so a change to either side is visible from both.

The last three all hold a data version the mod reads and a state it could
interpret, so nothing but the field each one exists for can be what refused
them.

`absent` deliberately holds an empty `savedData` array rather than omitting the
field, so the fixture states that the world was read and held nothing rather
than leaving a reader to guess. ADG-7 opens both the same way, but a fixture
that cannot say which one it is could not record the case it exists for.
