# Performance profiles

`initial.toml` is protocol `initial-v1`. A result identifies the profile by ID,
integer version, and SHA-256 hash; the filename alone is not identity.

Changing a world input, route, duration, setting, active-content count, sampling
cadence, definition, threshold, or calculation creates a new profile version.
Do not edit a profile version after evidence has been accepted. A reviewed
migration reruns both the official baseline and the compared Ravel result.

The route totals 900 seconds. Its steady-state and transient phases determine
which samples feed each calculation; warm-up samples never feed frame or
steady-state acceptance. The complete-pack content counts apply to official-pack
and Ravel-pack subjects. Vanilla uses the common and vanilla tables.

Profile version 3 runs each subject once: one official-pack comparison followed
by one Ravel-pack acceptance. It intentionally makes no run-to-run
reproducibility claim. Dry runs remain marked `synthetic = true` and cannot
produce a pass.
