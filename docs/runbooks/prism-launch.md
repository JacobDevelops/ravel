# Prism launch

Status: M0-W6/M0-W7 Linux pinned-input and launch-proof procedure. Native Mac
launch attestations are deferred until after Linux M10.

The pins below are M0's, and they are the ones this procedure was run against:
Minecraft `26.1.2` under `java-runtime-epsilon`, machine-readable in
[`runtime-inputs-26.1.2.json`](../../tests/fixtures/descriptors/runtime-inputs-26.1.2.json).
They are not the forward proof workload. The intermediate ATM10 To the Sky 2.0.2
workload on Minecraft `1.21.1` retains its `java-runtime-delta` `21.0.7` pins in
[`runtime-inputs-1.21.1.json`](../../tests/fixtures/descriptors/runtime-inputs-1.21.1.json).
Select that descriptor explicitly when replaying historical ATM10 evidence.
Forward proof work selects `sevtech-ages-3.2.3` and Minecraft `1.12.2`, with the
JDK 27 execution target described below. Rewriting M0's pins would say it
launched a runtime it did not; `evidence/m0/manifest.json` remains authoritative
for that launch. New upstream releases do not silently change any workload.

## Forward SevTech proof procedure

Status: acquisition inputs are pinned; live Ravel launch acceptance is not yet
demonstrated. WP2 must supply the complete 1.12.2 adapter and WP3 must prove the
runtime cutover before this procedure can produce a successful launch record.

1. Select the immutable
   [`sevtech-ages-3.2.3` descriptor](../../tests/fixtures/descriptors/sevtech-ages-3.2.3.json)
   and verify the publisher archive before extracting or acquiring its inputs.
   The reference is Minecraft `1.12.2`, pack `3.2.3`, and
   `forge-14.23.5.2860`; that Forge identity describes the comparison workload,
   not a loader installed in the Ravel instance.
2. Verify the official client/server inputs against
   [`runtime-inputs-1.12.2.json`](../../tests/fixtures/descriptors/runtime-inputs-1.12.2.json).
   Its Mojang Java 8 baseline is historical metadata, not the execution choice.
   Select the explicit Eclipse Temurin `27.0.0+35` JDK pinned by `mise.toml`
   in the isolated Prism instance; record the actual `java -version` output.
   Do not let automatic legacy Java selection silently substitute Java 8.
3. Keep authentication and entitlement with Prism, outside tracked files.
   Select the matching 1.12.2 Ravel artifacts, not the default 26.1.2 build.
   Install no Forge or Cleanroom host and no reference Java mod jars into the
   Ravel instance. Native Rust counterparts are admitted only after rights
   clearance; archive acquisition alone does not grant publication rights.
4. Use WP3's reviewed LWJGL3 runtime/library pins and capability-selected graphics
   backend. No LWJGL2 path is retained. Record native-access grants and any
   narrowly reviewed module opens rather than inventing blanket flags here.
5. Capture menu, fresh-world creation, save/reopen, input/audio, resize/fullscreen
   and clean-exit results on the exact selected JDK. Capture dedicated-server
   start/save/stop separately without client natives. Failed or unavailable
   acceptance remains a failed or unavailable result, not a completed WP3.

Record reference count, bundled jar count and runtime-loaded containers
separately. This release has 254 publisher references and five bundled jars;
259 top-level jar inputs are not evidence of 259 loaded containers.

`cargo make ravel:registry-verify` selects the SevTech WP0 phase. The fixture
variant also verifies locally acquired bytes. Missing legacy graph derivation,
surface, plan and observed-read evidence is reported as named `NOT-YET-MET`
obligations, not as completed later phases. Request `--phase full` through
`registry/sevtech/verify.sh` to require those later obligations.

Explicit historical archive replay remains available:

```console
mise exec -- cargo run -q -p ravel-xtask -- fixture verify tests/fixtures/descriptors/atm10-tts-2.0.2.json tests/fixtures/local/atm10-tts-2.0.2/source/atm10-tts-2.0.2.zip
mise exec -- env RAVEL_FIXTURE_ID=atm10-tts-2.0.2 bash registry/checks/verify-inventory.sh --fixture
mise exec -- env RAVEL_FIXTURE_ID=atm10-tts-2.0.2 bash registry/sevtech/verify.sh --fixture --phase full
```

Use the historical runtime descriptor and admission inputs for historical
launch replay; neither these commands nor a new JDK 27 launch relabel M0–M7.

## Pinned inputs, as M0 ran them

- Minecraft Java Edition `26.1.2`, release metadata SHA-1
  `a5ba42111e0de7712d677c458bd0dbfc9f36482e` and SHA-256
  `c0befcb001356cf53de2ee7cfa8fa796afcd3e2fe4530274c66d473cc576c98b`.
- Mojang Java component `java-runtime-epsilon`, major version `25`; the pinned
  runtime release is `25.0.1`.
- Prism Launcher `11.0.3`.
- Linux x86_64 AppImage SHA-256
  `0823a0e5f53694f0ff65a9523b6f221e6f10565b904941fa9522d89e8e7c69d1`.

The Minecraft version metadata, Mojang runtime manifests, and Prism artifacts
must be fetched from the URLs in the descriptor and checked before use.

## Clean-machine procedure

1. Verify the host matches the Linux M0 hardware profile. Do not run native
   Apple Silicon work for M0.
2. Download the pinned Prism artifact from its recorded GitHub release URL and
   verify its byte length and SHA-256 before opening it.
3. Set an isolated Prism data root outside the Ravel repository. Use a path
   dedicated to the evidence run; never place launcher metadata, accounts,
   assets, libraries, instances, or managed Java under the repository.
4. Start Prism with `-d <isolated-prism-root>`.
5. Authenticate through Prism. Authentication, entitlement, and account data
   remain Prism-owned and must not appear in commands, captures, logs, or public
   evidence.
6. Enable Java version autodetection and Mojang Java auto-download. Confirm the
   selected runtime is `java-runtime-epsilon` `25.0.1`. If Prism cannot acquire
   that runtime, stop and record a blocking deviation rather than substituting
   Java 21 or another convenient JDK.
7. Create an instance named `ravel-m0-vanilla-26.1.2` with Minecraft
   `26.1.2`. Select no mod loader and install no mods.
8. Inspect the instance version components and filesystem. The instance must
   contain no NeoForge, Fabric, Forge, Quilt, or Ravel bootstrap component and
   no `mods` payload.
9. Launch once from the Prism UI, reach the title screen, create the canonical
   M0 vanilla world, enter it, exit to title, and stop the game cleanly.
10. Re-launch the same instance using
    `prismlauncher -d <isolated-prism-root> -l ravel-m0-vanilla-26.1.2
    -a <prism-profile>`. The profile value is supplied interactively or from
    local launcher state and is redacted from public evidence.
11. Capture the successful launch and world-open summary only in the
    ignored/private evidence boundary; do not repeat the launch or run
    performance capture during M0.
12. Leave Prism's pre-launch, post-exit, and wrapper commands empty. The already
    completed focus-free official-pack run used the now-retired
    `supervise-initial-linux` boundary with a live private marker socket. Its
    launch admission records the wrapper-command hash but admits no performance
    result; it is historical evidence, not a wrapper to configure for a new run.

## Admission record

For each platform, record:

- platform, architecture, hardware-profile ID, OS and kernel version;
- Prism version, artifact name, size, and SHA-256;
- Minecraft metadata URL, SHA-1, and SHA-256;
- selected Java component, runtime version, runtime-manifest hashes, and
  `java -version` output hash;
- sanitized instance ID and exact graphics, simulation, memory, and JVM
  settings;
- UI and CLI launch command status, start time, duration, and redacted log hash;
- title-screen and fresh-world results;
- explicit expected-loader identity and confirmation that no unexpected loader
  component exists (vanilla also has no mod payload).

M0 requires the Linux record only. macOS records are separate post-Linux-M10
evidence and must not block M0 through M10.

This M0 procedure proves only the pinned vanilla host and does not select a
bootstrap. Ravel's stock Prism integration is described in the
[architecture mechanisms](../architecture/overview.md#mechanisms).
