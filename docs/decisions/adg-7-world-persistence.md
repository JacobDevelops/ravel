# ADG-7 — World persistence adapter

Status: accepted before M3-W4. The pinned 26.1.2 seams and fixer were measured;
the Rust ownership, failure, and size contracts below are selected for Phase C.

## Decision

Minecraft stores one Ravel-owned `SavedData` object. Its payload is one opaque
Rust-owned aggregate containing every owner's saved-data envelope. Java neither
parses that aggregate nor knows how many owners it contains. `ravel-save` owns the
aggregate format, its version marker, ordering, admission, migrations, and size
limit; each contained envelope conforms to
[`saved-data.schema.json`](../../schemas/saved-data.schema.json).

The aggregate is canonical UTF-8 JSON with aggregate format version 1 and an
owner-sorted list of envelopes. The decompressed canonical aggregate byte sequence
that crosses JNI is limited to exactly **1 MiB**. Before parsing, `ravel-save`
enforces that bound by measuring the complete slice — not with a limiting reader,
which an earlier form of this sentence named and which the Phase C selection below
rejected with its reason. It refuses an oversized incoming
aggregate during open before it enters staged or long-lived state, and refuses an
oversized replacement before its bytes reach Java. This is M3's fixed safety
ceiling: large enough for the measured persistence proof without turning unbounded
world input into native allocation, and deliberately not configurable. Raising it
requires a new memory, decode-cost, and pinned-fixer measurement.

One version-specific Java adapter translates between that opaque byte array and
the pinned game's `SavedData` API. Exactly two batched JNI calls cross through the
generated `dev.ravel.jvm.Hook`, which remains the sole native declaration surface:

1. Open supplies `absent` or the complete aggregate bytes, the selected world's
   identity, and Minecraft's advisory new-world marker. Rust admits or refuses the
   aggregate and stages admitted state; this crossing never installs it directly
   into the live `LoaderRuntime`.
2. Save supplies the receiver's world identity and asks once for the complete
   current aggregate. It may receive unchanged, replacement bytes, or a reported
   production failure.

Each open replaces the prior staged entry. The stage may hold fully resolved mod
state internally; staged means only that it is not yet bound to save emission. At a
save prepend, a live identity match emits the live aggregate even when another
world is staged. A staged identity match with no live match promotes that stage to
live and emits it. No match skips replacement and reports through observation.
World identities come from `LevelStorageAccess.getLevelPath(ROOT)` at open and
`MinecraftServer.getWorldPath(ROOT)` at save. This makes the prior world's teardown
save emit only its live state and promotes the incoming world on its first save;
a crash before that save leaves only a replaceable stage. There is no teardown or
autosave distinction. There is no field JNI and no JNI call while the loader
runtime mutex is held.

## The pinned lifecycle seams

The open inventory entry is `world.open-persistence`, prepended to this exact
instance member from the pinned 26.1.2 unobfuscated class files:

```text
net.minecraft.client.Minecraft#doWorldLoad
(Lnet/minecraft/world/level/storage/LevelStorageSource$LevelStorageAccess;Lnet/minecraft/server/packs/repository/PackRepository;Lnet/minecraft/server/WorldStem;Ljava/util/Optional;Z)V
```

Both creation and existing-world paths converge there before the original body's
first persistent mutation. The method can be entered while a prior integrated
server still exists, but the original body's first operation synchronously
disconnects, halts that server, and waits for its server thread to complete
`stopServer()`, including the prior world's save and resource closes, before the
first incoming-world mutation.
The final boolean is useful context, but it is advisory: absence, not that marker,
initializes fresh state.

The single receiver-aware persistence-save seam is:

```text
net.minecraft.server.MinecraftServer#saveAllChunks
(ZZZ)Z
```

Autosave calls `saveEverything(ZZZ)Z`, which calls `saveAllChunks(ZZZ)Z` once.
`stopServer()V` calls `saveAllChunks(ZZZ)Z` once and `saveEverything(ZZZ)Z` zero
times, then closes levels, saved-data storage, and level access. One prepend at
`saveAllChunks` therefore covers autosave and shutdown. The existing
`save.save-everything` native observation hook remains distinct and does not
become a persistence adapter.

The mapping source is `unobfuscated-class-files`: these names and descriptors are
both named and official, with no intermediary identifier. The 26.1.2 inventory
and generated mapping lock are the authoritative pins. Phase C gives both seams
the complete M3-W0 inventory, independent-resolution, lock, and architecture-trace
treatment; this decision does not substitute for that enforcement.

## Admission and ownership

`SavedDataPlan::open` and `SavedDataPlan::resolve`, extended within `ravel-save`,
are the sole admission decision. They validate the aggregate marker and bound,
then derive every refusal from the per-owner schema and compatibility contract:
owner and data versions, migration availability, and `requiredPackages` under
`SPEC.md` §14. Java reports bytes or absence and never reproduces that taxonomy.

- Absent data initializes a fresh aggregate for any world. The new-world marker
  does not create a second policy.
- Present unreadable data, invalid aggregate or envelope data, or any refused
  owner prevents world access.
- Deleting Ravel's saved-data file is the defined recovery from unreadable data
  and intentionally produces fresh initialization on the next open.

The installed `Arc<LoaderRuntime>` is the sole synchronized persistence
participant. It owns the lifecycle machine and the same mod values used for
open, visit, migration, and save; no mirror or parallel registry exists.
`LifecycleMachine::request_save` gates production. A `PartialStateRefused` result
or any other envelope-production failure prevents replacement bytes from reaching
Java.

## Failure invariants

Persistence capability is required even though screen capability is optional.
Before any world load, a runtime marker or handshake must prove that the
`doWorldLoad` prologue was actually installed. Jar or class availability alone is
not proof: a transformer can return the original class or match zero methods.
An absent marker refuses world load. Phase C selects and tests the marker
mechanism.

An open refusal must clean up the selected `LevelStorageAccess` and `WorldStem`
and route to a Ravel-owned screen using ADG-6's Java drawing mechanism. It must
not leak a raw exception, retain the world lock, or crash the callback. Rust still
owns the diagnostic; Java owns only cleanup and presentation.

Save has the opposite failure priority. The adapter catches its own persistence
failure and never aborts vanilla player or chunk saving. Before production, Rust
compares the save receiver's world identity with the identity carried by the live
admitted state. A mismatch skips replacement, reports through the existing
observation and log path, and never aborts vanilla saving. Unchanged state also
skips replacement, so idle autosaves do not rewrite the Ravel object. Production
failure, including `request_save` returning `PartialStateRefused`, preserves the
previous on-disk aggregate and reports through the same path. The same rules apply
during shutdown. Losing one Ravel checkpoint is acceptable; losing the player's
vanilla save is not. The next open reconciles the old aggregate through
`SavedDataPlan`.

The pinned `SavedDataStorage.set(SavedDataType, T)` implementation marks the
replacement dirty exactly once. The adapter must not duplicate that call.

## Vanilla owns files, with vanilla's failure exposure

Ravel does not parse or construct NBT and never writes, renames, or flushes a
world file directly. Minecraft owns serialization and the normal save path.
Consequently this decision makes no stronger atomicity claim than vanilla does:
a torn or otherwise unreadable write refuses on the next open. The defined
operator recovery is deletion of the Ravel-owned file, which intentionally resets
Ravel state while leaving the rest of the world untouched.

Fresh state becomes durable at the first successful vanilla save. No eager write
exists merely to create the file sooner.

## Reused mechanisms only

Implementation extends the existing `HookTarget`,
`Transformer.PrependTransform`, generated `Hook`, guarded JNI exports, sibling
Java artifact, build script, generated stubs, and lock pipeline. It must not add a
second transformer, target registry, JNI exporter, or native declaration class.
The sibling artifact can remain optional for screens while its persistence
capability and installed-prologue handshake are required.

The receiver-aware prepend is the narrow ADG-1 amendment linked from this record.
ADG-6 contributes only the Ravel-owned refusal screen. Nothing here decides a
general Rust-to-JVM mechanism.

## Evidence

The fixer result below is a measurement taken once, not a regression gate. Nothing
in `ravel:verify` runs it, no devenv task wraps it, and this record previously
called it a gate while nothing regressed on it. It needs the restricted pinned
client jar and a real Minecraft library root for the fixer's own dependencies, and
the gate graph can assume neither; the fixture-dependent `ravel:spike-adg-*` tasks
sit outside the gate for exactly that reason. Runtime is not why — the whole run
takes about two seconds.

Take it with:

```console
./jvm/harness/run-datafix-byte-preservation.sh
```

What it measures is the shape Ravel actually stores, and that shape is taken from
production rather than restated. `jvm/harness/DataFixBytePreservationMain.java`
compiles `dev.ravel.jvm.screen.RavelWorldData` — the class Minecraft stores —
against the pinned jar and uses that class's own `SavedDataType`: its codec builds
the payload, its `dataFixType()` selects the fixer, and its byte accessor reads the
aggregate back. The wrapping around the payload is the pinned `SavedDataStorage`'s,
read from the pinned class files: writing puts the codec's output under `data` in a
fresh compound and adds `DataVersion`, and reading hands the whole root compound to
`DataFixTypes.update` and then parses `root.get("data")`. The fixer therefore sees
the nesting rather than the payload alone.

At change `muuxlrom` the run reported:

```text
datafix-byte-preservation=passed fixer=SAVED_DATA_COMMAND_STORAGE
shape=data.{aggregate:StringTag} range=1343..4790 versions=3448 payloads=8
cases=27584
```

That is one line in the terminal, wrapped here. Every payload came back
byte-identical: 3,448 source versions from data version 1343 through 4790
inclusive, eight payload classes, and 27,584 of 27,584 cases. Payload lengths were
0, 1, 7, 8, 31, 256, 257, and 4,097 bytes, including leading zeroes, all byte
values, and invalid UTF-8. `shape` is printed off an encoded payload rather than
asserted against a copy, and it is the field that says the bytes measured were the
base64 string under `aggregate` that production writes. This admits the selected
fixer for the pinned version and for the object Ravel stores.

An earlier run of this harness measured a different shape: `Codec.BYTE_BUFFER`
encoded directly under `data`, which is an NBT byte array where production writes a
base64 string one level deeper under `aggregate`. That run passed and its result was
not false, but a fixer result over a shape production never writes cannot admit the
fixer for the object Ravel stores, so it has been re-measured rather than reworded.
The conclusion is unchanged because the real shape is preserved too — and, on the
way to it, so was the same payload set under the key `contents`, which is the one
key the selected fixer's own vanilla data uses.

The figures above are a historical record of one run, so they are anchored to the
files that produced them. At that change:

| File | sha256 |
| --- | --- |
| `jvm/harness/DataFixBytePreservationMain.java` | `cf793a7796dff3420e06141c622ef173e6711af53db36e0ed1d906e7b9de667c` |
| `jvm/harness/run-datafix-byte-preservation.sh` | `8d9c4d4ab171cceade6649a87b3e795c05a225d885fc81e1f06f8bf2a461742e` |
| `jvm/screen/src/dev/ravel/jvm/screen/RavelWorldData.java` | `3923d3a40b26a2627045f5dc92380185b258c49d4ca56f8d5030cae56753bec8` |

`sha256sum` over those three paths recomputes them. Editing any of the three makes
the reported line a record of a different instrument, and nothing in the repository
notices: the provenance of this measurement is prose, which is what not being a gate
costs. The client jar the run linked against is the one
`minecraft/versions/26.1.2/adapter.toml` pins by digest, and that pin is checked.

The limit of the evidence is explicit: it covers only 26.1.2's current range and
payloads through 4,097 bytes. The 1 MiB decompressed canonical aggregate ceiling
is an admission safety policy, not a claim that every byte length up to the ceiling
was passed individually through the fixer.

### What would make it a gate, and who owns that

M3-W4 owns this evidence: it is the first item in that work package's Verification
column. Making it a gate needs one thing that is not available — the pinned class
files and the fixer's libraries reachable from `ravel:verify` on a clean checkout.
The client jar is restricted material that `tests/fixtures/local/` keeps ignored and
`ravel-verify-scaffold` refuses to let into the tracked tree, and the library root
belongs to a launcher installation outside the repository. So no work package owns
wiring this in today, and M3-W4 rests on a recorded measurement instead. An
independent M3-W8 round marked that Verification item proven by citing this section
while it described the earlier shape; the round's transcript in
`evidence/m3/review.md` stays as it was written, and this section is the correction
a later round should read.

The same pinned-class inspection established the common `saveAllChunks` seam and
that `SavedDataStorage.set` dirties exactly once. Phase C turns those measured
facts into generated bindings and process tests; official create/save/reload and
refusal evidence remains a later M3 evidence phase.

## Rejected alternatives

- One Java object per owner duplicates Rust's aggregate admission and makes JNI
  traffic scale with mod count.
- `saveEverything` misses direct shutdown saving; a second shutdown injection
  would create two writers where one common seam exists.
- Treating absence in an existing world as corruption invents policy from file
  identity instead of asking `SavedDataPlan`.
- Direct NBT or file writes take serialization and failure behavior away from
  Minecraft and create another atomicity mechanism to own.
- Throwing out of save protects one Ravel checkpoint by risking the player's
  chunks and player data.
- Checking only for a sibling jar or class permits silent operation after a zero-
  match or failed transformation.

## Phase C selections

This decision deferred three things to Phase C. Two are now selected and the
third is blocked on a measurement recorded below.

### The installed-prologue marker

`Transformer` keeps a per-target rewritten set, collected unconditionally rather
than under `-Dravel.report.targets`. The existing counters and the reported
target table are diagnostics and stay diagnostics; the marker is separate state
whose only reader is the open path, because a marker that a launch flag can turn
off is not a marker.

The open crossing firing is itself proof that the open seam was rewritten: a
prologue that never ran cannot ask. The save seam has no such self-proof at open
time, and `MinecraftServer` may not be loaded yet, so the open path forces
`Class.forName("net.minecraft.server.MinecraftServer")` — which runs the
transformer if it has not already run for that class — and then queries the
rewritten set for the save target. An absent entry refuses the world load. This
is what closes the hole the rejected alternative names: a jar or class that is
present but whose transformation matched zero methods.

The refusal itself is exercised. `jvm/harness/run-harness.sh`'s
`missing-save-seam` lifecycle installs the open seam and not the save seam, so
`MinecraftServer` is present and loads while its transformation matches nothing,
and the harness asserts the world is refused with this diagnostic before the open
crossing is made at all. Without that case the false branch of the marker query
never ran anywhere in the gate, and a marker check that is never observed failing
is indistinguishable from one that always answers true.

### The crossing shape

The two crossings carry an opaque `byte[]`, which is the first byte-carrying
native in the generated `Hook`; every existing payload native carries `String`.
Open reports absence or the complete aggregate together with the advisory
new-world marker and returns a verdict. Save reports the saving world's identity
and returns replacement bytes, unchanged, or skip. Java holds one `SavedData`
object and never sees the structure inside the bytes.

The bound is enforced by measuring the complete slice before parsing rather than
by wrapping it in a limiting reader. The aggregate crosses as one complete array,
so there is nothing to stream and the guarantee is identical; a reader here would
be ceremony rather than a bound.

### Adapter placement

Placement is already settled above: implementation extends the sibling Java
artifact, its build script, the generated stubs, and the lock pipeline, and that
artifact's persistence capability is required even though its screens are
optional. This section records why the alternative that was considered — a
reflection-only adapter inside the agent jar, declaring no game type and seeding
no stub — cannot be built. The pinned 26.1.2 class files refuse it:

```text
public abstract class net.minecraft.world.level.saveddata.SavedData
public net.minecraft.world.level.saveddata.SavedDataType(
    net.minecraft.resources.Identifier,
    java.util.function.Supplier<T>,
    com.mojang.serialization.Codec<T>,
    net.minecraft.util.datafix.DataFixTypes)
```

`SavedData` is abstract, so storing a Ravel-owned one means defining a concrete
subclass of a game type, and `SavedDataType` additionally requires a
`com.mojang.serialization.Codec` for that subclass. Reflection calls methods; it
neither defines a subclass nor builds a codec. ADG-1 forbids the agent jar from
*declaring* a game type — it compiles with no client jar on the class path — and
defining a subclass of one is a declaration, so the storage half cannot live there.
Resolving a game class by name at runtime is a different thing, it is permitted,
and the agent jar's own persistence adapter does exactly that; what it cannot do
that way is define a subclass.

It lives in the sibling artifact, compiled against generated stubs like the
screens already are, and the stub seeds and lock re-pin are the ordinary
ceremony rather than an exception to it. The split is by what has to be
compiled, not by convenience: the open and save control flow stays reflective in
the agent jar, and only the `SavedData` subclass and its codec are compiled.

The one behaviour this placement adds is the failure asymmetry the section above
names. A missing sibling artifact leaves a working game with no screens, and
leaves no persistence at all — so an absent or unusable artifact refuses world
load, where an absent screen merely draws nothing.

### A second class source for the stub pipeline

`com.mojang.serialization.Codec` is not in the client jar — the only `com/mojang`
content there is `blaze3d` — so the stub pipeline, which had exactly one class
source, could not produce the type `SavedDataType`'s constructor requires. It
gains a second: `com.mojang:datafixerupper:9.0.19`, pinned in `adapter.toml` as
`inputs.dataFixerJar` and generated from exactly like the client jar. Hook target
resolution still reads the client jar alone; only stub generation searches both,
client jar first.

This extends the existing trust root rather than adding one. `version.json` is
already a pinned input and already declares the coordinates, URL, SHA-1 and size
of that library, so the jar is a declaration this repository had already accepted
being materialized, and the fetch is checked against the hash `version.json`
carries. It lives beside the client jar under `tests/fixtures/local/`, which is
ignored, so it is a local input and never tracked. `stubs.lock` records both
sources' digests, because a stub whose provenance names one of two jars does not
say where it came from.

## Consequences

- One bounded aggregate crosses once per lifecycle operation, independent of mod
  count.
- Rust owns every compatibility decision and all long-lived state; Java remains a
  narrow version adapter.
- Missing Ravel data is recoverable by reset, while present incompatible data is
  fail-closed.
- Vanilla save continuity outranks one Ravel checkpoint, and the previous durable
  aggregate remains the fallback.
- Supporting another Minecraft version requires new inventory pins and the same
  byte-preservation and control-flow measurements.
