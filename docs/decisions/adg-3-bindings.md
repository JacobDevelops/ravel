# ADG-3 — Minecraft bindings

Status: accepted at M1-W4; amended while scoping the 1.21.1 mapping source,
ahead of the version-directory work package, to admit Mojang's own published
mapping file as a second source. The amendment is recorded under "Amendment: a
second mapping source" below and each sentence it corrects is marked where it
appears. Nothing about where a binding comes from changed: it is still the pinned
jar, resolved rather than transcribed, pinned by a generated lock.

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
tracked. Where the names in those class files come from is what `mappingSource`
decides: under `unobfuscated-class-files` they are the names, because 26.1.2
ships unobfuscated and publishes no mapping file, and under
`official-proguard-mappings` the jar carries obfuscated names and the version's
own published mapping file is what relates them to the names the inventory
reviews. **Amended:** this sentence read "names come from the class files
themselves" without a condition. See "Amendment: a second mapping source".

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
cargo make ravel:spike-adg-3
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
  change would have to be explicit. **Amended:** the version that brings it back
  arrived. This rejection was for 26.1.2 and on the premise this bullet states,
  and the reversal condition it named is the one that fired. See "Amendment: a
  second mapping source".
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

## Amendment: a second mapping source

Amended while scoping the 1.21.1 mapping source, ahead of the version-directory
work package. `mappingSource` admits a second value, `official-proguard-mappings`,
which reads the version's own published client mapping file to relate the names
the reviewed inventory carries to the names an obfuscated jar declares. The
generator refuses any other value at parse, so the set is closed rather than
descriptive.

Everything the original decision selected is untouched. Bindings still come from
the pinned jar rather than from a file that describes it; every entry still has
to resolve exactly once, with the declared shape, or nothing is written; the
lock is still generated and tracked, so a rename still shows up in a diff; and
drift still fires on an input hash before a class is read.

### Why this is the branch this record already authorised

The rejection above is stated for 26.1.2 and on a premise 26.1.2 satisfies:
"publishes no client mapping download, and its class files carry real names, so
there is nothing to translate". It then names the reversal condition — "an
obfuscated future version brings this back" — and the mechanism that would carry
it, `mappingSource` in `adapter.toml`. 1.21.1 is that version. Its client jar
holds 8269 class files of which 26 keep a `net/minecraft/...` path, and the same
version manifest that publishes the jar publishes a `client_mappings` download
hashed beside it. So the premise of the rejection is false for that version, and
the mechanism this record chose for the case is the one being used.

### Why Mojang's own file is not the mapping sets this record rejected

A reader who stops at the shape of the thing — a mapping file, fetched, applied
between the jar and the binding — will read this as the second rejection above
rather than the first. It is not, and the difference is every reason that
rejection gives.

- **No external cadence.** The community sets are "a per-version external
  dependency, with its own cadence". Mojang's file ships in the same version
  manifest as the jar, at the same moment, for the same build. There is no
  second release to wait for and no second project to track.
- **No second hash to trust.** The manifest that pins the jar's `sha1` pins the
  mapping file's, in the same document, and that document is already an
  `adapter.toml` input. The two artefacts are hashed together, so a jar and a
  mapping file cannot be paired out of step without a hash refusing them.
- **No human-curated layer.** The objection was "a human-curated layer between
  the pinned jar and the binding that can drift silently". The proguard file is
  emitted by the build that emitted the jar. Nobody names anything in it, so
  there is nothing in it to drift.
- **Same names, not different ones.** A community set supplies names its authors
  chose. This file supplies the names Mojang's own source uses, which are the
  names `hooks.toml` already reviews. It translates; it does not rename.

What the two rejections share is a caution about anything standing between the
pinned jar and a binding. This amendment keeps that caution intact by treating
the mapping file exactly as the jar is treated: an input pinned by hash, verified
before a class is read, refused on drift.

### The mapping file is an untracked local fixture pinned by hash

`adapter.toml` declares it as `inputs.clientMappings`, with a `sha256` and a
path under `tests/fixtures/local/`, and it is never tracked. Three reasons, and
the first two are the jar's own:

- It is upstream material this repository does not own, and the repository
  tracks no game artefact. The rule that keeps the jar out keeps this out.
- It is derived from the build that produced the jar, so tracking it stores a
  second copy of something the jar already determines.
- It is 9.6 MB of generated text per version, which is larger than every tracked
  file in `minecraft/versions/` put together and would grow by that much again
  on each version added.

What is tracked is its hash, which is the same trade the jar makes and is what
the consequence above is amended to say. A machine without the fixture takes the
lock path exactly as it does without the jar, because the mapping file is only
ever read on the path that reads the jar.

### What this costs

- A resolution now has two names per target: the Mojang name the reviewed
  inventory carries and a human reads, and the runtime name the JVM links. The
  generated doc comments and the inventory keep the first; the Rust target table
  and the Java `HookTarget` table carry the second. Under
  `unobfuscated-class-files` the two are equal, which is why adopting this
  changed no generated byte for 26.1.2.
- The lock records the runtime name only where it differs from the Mojang name,
  so a jar-less machine can still generate under either source and a lock
  written under `unobfuscated-class-files` is unchanged.
- Everything outside the generator that spells a game name is unaffected by this
  record and is not made correct by it. The agent jar resolves game classes
  reflectively by string literals in `jvm/`, and those are Mojang names; under an
  obfuscated source they are wrong at runtime. That is a separate work package
  and this amendment does not pretend to close it.

### Limits of this amendment

- No obfuscated version is pinned yet. What is decided here is the source and
  the mechanism; which version directory uses it, and whether it replaces or
  joins 26.1.2, is not decided by this record.
- Nothing here was measured on a launched game. The claim is about generation.
