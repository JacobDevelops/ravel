# ADG-3 — Minecraft bindings

Status: accepted at M1-W4.

## Decision

Bindings are generated at build time by reading the pinned client jar's class
files directly. The reviewed hook inventory in
[`minecraft/versions/26.1.2/hooks.toml`](../../minecraft/versions/26.1.2/hooks.toml)
names each target by class, member, and exact JVM descriptor; the generator
resolves every entry against the jar named in
[`adapter.toml`](../../minecraft/versions/26.1.2/adapter.toml) and writes a Rust
binding plus
[`mappings.lock`](../../minecraft/versions/26.1.2/mappings.lock), which pins what
it resolved.

The generator refuses to write anything unless every entry resolves exactly once,
with the shape the inventory declares, from inputs whose hashes match what is
tracked. Names come from the class files themselves, because 26.1.2 ships
unobfuscated and publishes no mapping file.

## Context

`ravel-minecraft-adapter` needs the exact class, member, and descriptor of every
hook target, per Minecraft version, and needs to notice when a game update moves
one. The question is where those names come from and how a build learns they have
changed.

26.1.2 answers half of it: the class files carry real names, and the pinned
version manifest publishes client and server downloads and no mapping file at
all. There is nothing to deobfuscate. What remains is who reads the jar.

## Evidence

Measured on `x86_64-linux`, hardware profile `linux-reference-v1`. Reproduce with:

```console
devenv tasks run ravel:spike-adg-3
```

Results live in [`evidence/m1/spikes/adg-3/`](../../evidence/m1/spikes/adg-3/)
and are checked by `ravel:verify-spikes`.

| Extractor | Generation | Output | Retained RSS | Peak RSS | Targets | Drift refused |
| --- | --- | --- | --- | --- | --- | --- |
| Class files, read directly | 33 ms | 3417 B | 2.9 MiB | 39.1 MiB | 6 | yes |
| `javap`, asked per class | 601 ms | 3413 B | 2.9 MiB | 39.0 MiB | 6 | yes |

Retained and peak are both reported because they differ by an order of magnitude
here and only one of them is the number that matters. Generation reads the jar,
so peak is dominated by that read and is nearly identical for both extractors;
what a build step leaves behind is the retained figure.

- **Both extractors resolved all six targets identically.** That is the finding
  that matters most, because it is the only cheap evidence that either reading is
  right. Each trial re-resolves every target with the other extractor and reports
  agreement or disagreement; both reported agreement.
- **Reading the class files is about eighteen times faster.** 33 ms against
  601 ms, because `javap` starts a JVM per class. At six targets the absolute
  numbers are irrelevant; at the hundreds of targets M3 onward will carry, one
  JVM per class is not a build step anyone wants. The ratio is the stable part of
  this: it is per-class process startup, so it grows with the target count while
  the class-file reader amortizes one jar open.
- **Two derivations from the pinned jar produced the same bytes.** Each trial
  resolves every target against the jar twice and generates from each resolution,
  and records both digests as `bindingGeneration.outputSha256` and
  `repeatedOutputSha256`; the measurement fails if they differ, and the M1 closure
  rule requires the pair to be equal. What that observes is that reading the jar
  and rendering it are deterministic within one run on one machine — it is not a
  claim about another machine, another day, or another jar. Two earlier shapes of
  this claim were weaker: the instrument first only refused to report on a
  mismatch, which kept the property in its control flow, and then repeated the
  render over an already-resolved target list, which could not disagree. The two
  extractors' outputs differ only by the bytes of the extractor's own name
  recorded in the lock.
- **A drifted input is refused, not worked around.** Presented with the same jar
  under a hash that does not match the tracked pin, the generator refuses before
  reading a single class. Observed in both trials.
- **Everything fails closed.** An inventory entry naming a descriptor the class
  does not declare, an entry declaring the wrong shape for a member that does
  exist, and an absent jar are each refused with a message naming the entry. A
  generator killed mid-run leaves no half-written output, because it writes to a
  temporary name and renames.

## Why the class files rather than `javap`

Both readings agree, so correctness does not decide it. Three things do.

**No JDK on the build machine.** `javap` means a JDK is a build dependency for
anyone who builds Ravel, for a job that is reading bytes out of an archive.

**`javap` prints prose, not data.** Its output is source-shaped: a constructor
appears as the class name rather than `<init>`, so the name the JVM actually uses
is not in the output at all and has to be reconstructed. Generics, varargs, and
annotations are rendered for humans. Parsing it means depending on the formatting
of a tool that has no compatibility promise about it. The class file format, by
contrast, is versioned, documented, and stable enough that a 200-line reader
handles it.

**Speed compounds.** Eighteen times slower at six targets is nothing; at the hook
inventory this repository will actually carry it is a build everyone waits for.

`javap` keeps one job: it stays in the experiment as the independent second
reading, and the generator reports whether the two agree. That cross-check is
worth more than either extractor alone, and it costs half a second.

## Rejected

- **Mojang's published mapping file.** 26.1.2 publishes no client mapping
  download, and its class files carry real names, so there is nothing to
  translate. Recorded in `adg3-official-mappings.json`. An obfuscated future
  version brings this back; `adapter.toml` records `mappingSource`, so that
  change would have to be explicit.
- **Community mapping sets.** A per-version external dependency, with its own
  cadence and licence, to translate names the jar already carries, plus a
  human-curated layer between the pinned jar and the binding that can drift
  silently. Recorded in `adg3-third-party-mappings.json`.
- **Runtime reflection.** Needs no generation step, which is its appeal, and
  defers every version error to the moment a player triggers the hook. Ravel has
  to refuse an incompatible mod before a world opens, which is the opposite.
  Recorded in `adg3-runtime-reflection.json`.
- **Hand-written version data.** Cannot detect drift, because there is nothing to
  compare it against except the jar it was meant to describe. Recorded in
  `adg3-hand-written-version-data.json`.

## Consequences

- Version data is tracked files per version rather than jars: the reviewed hook
  inventory, the reviewed stub seeds, the adapter inputs with their upstream
  hashes, and the generated mapping and stub locks. The set grew after this record
  was written — `adapter.toml` names the two stub files as well — which is why the
  count is not restated here; `adapter.toml`'s `inputs` block is the inventory and
  nothing else should try to be. The mechanism is unaffected: whatever the
  generation reads from is tracked and hashed, and the jar itself is never
  tracked; its hash is.
- The build needs a zip reader. This experiment shells out to `unzip`, which is
  fine for a measurement and not fine for production: M2-W5 picks a Rust archive
  dependency, and that is a dependency decision with its own `cargo deny` review.
- A game update changes the jar hash, so the next generation refuses until the
  adapter inputs are updated deliberately. That is the drift gate: it fires on the
  input, before any name is read.
- The hook inventory is reviewed by humans and consumed by machines. It carries a
  capability sentence per entry for the review, and an exact descriptor for the
  resolution — including for the two `filterText` overloads, which share a name
  and are told apart only by descriptor.
- `mappings.lock` is generated, never edited. It is tracked so that a change to
  it shows up in a diff, which is what makes a silent rename visible.

## Limits of this evidence

- Six targets. The inventory exists to prove the extraction and drift contract on
  real members of the pinned jar, not to enumerate a hook surface. Nothing here
  says how a few hundred targets behave, beyond the ratio between the two
  extractors, which is per class and will hold.
- The class-file extractor reads jar entries through the shell's `unzip`, one
  process per class, so its 33 ms includes process starts that production would
  not pay. That makes the measured gap over `javap` an understatement, not an
  overstatement.
- The memory figures cover the generator process only. `javap` starts a JVM per
  class and none of those JVMs are counted, so its 2.9 MiB retained and 39.0 MiB
  peak understate what that extractor actually costs a build machine. Recorded as a
  deviation on the result. The class-file reader has no equivalent uncounted cost,
  which makes the memory columns less comparable than they look.
- The generated Rust binding is a target table, not the adapter's API. What a
  hook does with a resolved target is ADG-1's mechanism and M2's work; ADG-3 only
  answers where the target came from and how a build knows it is still there.
- Generation was measured on the development JDK's `javap` for the second
  extractor. A different JDK would print differently, which is itself part of the
  argument against depending on that output.
