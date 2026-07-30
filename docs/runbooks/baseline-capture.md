# Baseline capture

Status: scaffold; M0-W5 must define the numeric protocol before capture.

1. Select a versioned performance profile and verify its hash.
2. Record the exact game, fixture, Java, Prism, Ravel, platform, hardware,
   graphics, simulation, and system settings.
3. Verify the canonical world, route, warm-up, duration, repetitions, active
   content, and sampling intervals.
4. Capture process RSS, system and unified-memory pressure, frame-time median,
   p95 and p99, integrated-server tick time, startup, world load, and relevant
   GPU allocation data.
5. Record collector versions, command start times, durations, exit status, and
   hashes for every raw artifact.
6. Store raw captures outside public evidence. Admit only sanitized summaries,
   hashes, and the schema-valid milestone manifest.

Official and Ravel runs must use the same profile revision. A profile migration
requires review and reruns of both baselines.
