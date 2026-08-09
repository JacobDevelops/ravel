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

Schema tests use `evidenceKind: synthetic`. A milestone manifest must use
`evidenceKind: native`, contain no synthetic placeholders, and include the
required native attestations. Relabelling a contract fixture cannot close a
milestone.

`review.record` names the independent review the milestone is closed on, and it is
the one hashed artifact that must be obtainable: `name` is a path relative to the
milestone directory, and `ravel-verify-scaffold` recomputes its digest against the
tracked file. A title is refused. The field used to be free text, and three of the
four manifests named the review by title, so the recorded digest read as proof that
a review happened while binding nothing anyone could read. A private capture may
stay outside the repository because a reader can still be told what it was; a
review that no reader can obtain is not evidence of a review. Absence is legal —
a milestone under construction has no review yet — and the milestone gate is what
refuses to close without one.

`revision.id` identifies the source revision exercised by the recorded commands,
and `revision.dirty` records that source tree's state at execution time. The
revision can precede the commit that admits its manifest, because a tracked
manifest cannot contain the hash of the commit that contains itself.

`platformEvidence` records each native platform's exact versions, relevant
settings, verification commands, and hashed launch, baseline, or validation
summaries as required by that milestone. M0 requires Linux x86_64 entries with
vanilla and official-ATM11 launch-proof summaries. macOS aarch64 evidence is
post-Linux-M10 scope. Cross-evaluation does not satisfy the `native`
requirement once a platform is required.

The top-level `environment` and `versions` describe the host that assembled and
admitted the manifest. Per-platform records are the source of truth for native
test and baseline execution context, so their platform-specific tool versions
and settings may differ.

`evidence/private/`, per-milestone `raw/`, and `captures/` directories are
ignored. Sanitization and redaction checks must pass before evidence is
admitted.
