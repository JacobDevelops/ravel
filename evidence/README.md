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

Performance/resource tooling is retired. Closed measurement records, historical
commands, logs, reviews, and manifests are preserved, but are not a current
capture procedure. Any retained performance input bound by `inputs[].name`
(including `tests/performance/hardware/linux-reference.toml`) stays at that path
so the gate can recompute its digest at `revision.id`. Do not delete those paths
or hand-edit recorded digests to match the working tree.

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

`revision.id` is the revision every `inputs[]` digest is recomputed at, and it must
be the commit `revision.changeId` is at and one visible history reaches.
`verify_pinned_commit` enforces both on every `ravel:verify`. A commit id does not
survive a rewrite — jj abandons the commit and keeps the change — so a manifest
pinned by commit id dangles the moment its change is revised, and the dangling is
invisible in the working copy that rewrote it because the operation log still
resolves the abandoned commit. All four manifests were in that state until a cold
reviewer cloned the repository and found every recorded input digest unreadable. A
rewrite now costs a re-pin and a re-hash, which is the only way the field stays true.

What `revision.id` does not identify is the tree the recorded commands ran on. A
manifest cannot contain the hash of the commit that contains it, so `commands` and
`tests` are recorded against an earlier commit and then the manifest is rewritten
above it. `revision.dirty` records that source tree's state at execution time. No
gate recomputes those figures — the arithmetic between them and the coverage floor
are all that bind — so they are the operator's attestation that the gate ran and
passed, and each manifest says so in a deviation. The digests are the part a reader
recomputes.

That recompute needs a jj repository, not this working copy. A clone of this
repository reaches the pinned commits through git HEAD, which the colocated
repository exports over the visible stack, so `jj git init --colocate` in a clone is
enough to recompute every input digest. Without a jj repository both provenance
checks announce that they are skipping rather than report a pass. Each manifest
records those limits, and the fact that nothing in the stack has been pushed, as a
deviation.

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
