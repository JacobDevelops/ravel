# Evidence

Each completed milestone stores a sanitized
`evidence/m<N>/manifest.json` conforming to
[`schemas/evidence.schema.json`](../schemas/evidence.schema.json), plus small
public summaries whose inclusion is necessary to audit a decision.

Evidence records exact inputs, hashes, commands, outcomes, platform details,
deviations, risks, and review. It must not contain credentials, launcher tokens,
identifying local paths, official fixture content, private worlds,
rights-sensitive research, or unrestricted raw logs. Large captures and
sensitive records stay outside the public repository; public manifests retain
their cryptographic hashes and redacted summaries.

`evidence/private/`, per-milestone `raw/`, and `captures/` directories are
ignored. Sanitization and redaction checks must pass before evidence is
admitted.
