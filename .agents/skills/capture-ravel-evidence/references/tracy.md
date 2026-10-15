# Tracy capture contract

## Pinned path

- Minecraft 26.1.2 bundles Mojang jtracy 1.0.37.
- Ravel pins Tracy 0.13.1 and patches `tracy-csvexport` to emit named,
  discontinuous frame sets.
- Minecraft `--tracy` exposes `Client Tick` around each client loop and
  `Server Tick` around each integrated-server tick.
- The exporter omits continuous, unnamed, VSync, and incomplete frame events.

Run `cargo make ravel:tracy` once before capture. It builds the patched tools
in `target/tools/tracy/bin/`, which mise adds to PATH.

## Capture

Start the capture collector independently of the game window:

```console
cargo run --locked -p ravel-cli -- \
  performance capture-tracy SECONDS \
  evidence/private/MILESTONE/capture.tracy
```

The duration starts after localhost attachment. Ensure the Minecraft process
outlives the requested capture. Include startup allowance plus all 1,320 seconds
of warm-up, measurement, and cool-down.

## Export

Use Ravel's wrapper. It creates the destination exclusively, passes the exact
filter syntax, validates both streams, and removes invalid partial output:

```console
cargo run --locked -p ravel-cli -- \
  performance export-tracy-frames \
  evidence/private/MILESTONE/capture.tracy \
  evidence/private/MILESTONE/tracy-frames.csv
```

The upstream parser declares `--filter` as an optional long argument. If
debugging manually, use `--filter= Tick`; `--filter " Tick"` is misparsed as a
second positional path.

The CSV contract is:

```text
frame_set,frame_index,start_ns,end_ns,duration_ns
```

Indices must begin at zero and remain contiguous within each frame set.
`duration_ns` must equal `end_ns - start_ns`.

## Known recovery

- `patch: malformed patch`: regenerate the patch from the exact pinned source
  with `diff -u`; do not hand-count hunk lengths.
- Patched tool still behaves like the previous build: rebuild it and check
  that the command resolves to `target/tools/tracy/bin/`:

  ```console
  cargo make ravel:tracy
  command -v tracy-csvexport
  tracy-csvexport --help
  ```

- `PRId64` compile failure: include `<inttypes.h>` in the patched C++ source.
- No named frames: confirm Minecraft launched with `--tracy`, the capture
  attached before the measurement window, and export uses `--frames`.
- Missing one required stream: reject the run. Do not synthesize or estimate
  client frames or integrated-server ticks.
