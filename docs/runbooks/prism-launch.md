# Prism launch

Status: M0-W6/M0-W7 Linux pinned-input and launch-proof procedure. Native Mac
launch attestations are deferred until after Linux M10.

The pins below are M0's, and they are the ones this procedure was run against:
Minecraft `26.1.2` under `java-runtime-epsilon`, machine-readable in
[`runtime-inputs-26.1.2.json`](../../tests/fixtures/descriptors/runtime-inputs-26.1.2.json).
They are not the proof workload Ravel targets now. Change
`wulksnsyvkkwwxrsuroytzkwmmqtpnxs` retargeted that to ATM10 To the Sky 2.0.2 on
Minecraft `1.21.1` under `java-runtime-delta` `21.0.7`, pinned in
[`runtime-inputs-1.21.1.json`](../../tests/fixtures/descriptors/runtime-inputs-1.21.1.json),
and a launch of the current workload takes its versions from that descriptor and
the rest of its procedure from here. Rewriting these pins to the retargeted ones
would say M0 launched a runtime it did not; `evidence/m0/manifest.json` records
which one it launched and this document agrees with it. New upstream releases do
not change either set of inputs without a reviewed profile revision.

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
12. Leave Prism's pre-launch and post-exit commands empty. A new M0 run also
    leaves the wrapper empty. The already completed focus-free official-pack
    run used the project `supervise-initial-linux` boundary with a live private
    marker socket; its launch admission records the wrapper-command hash but
    admits no performance result. Any other wrapper is rejected. M9 owns any
    resulting performance capture and does not require this M0 run to repeat.

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
