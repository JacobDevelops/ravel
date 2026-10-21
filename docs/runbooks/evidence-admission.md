# Evidence admission

1. Validate the milestone manifest against
   `schemas/evidence.schema.json`.
2. Confirm every input, command, summary, and review record is complete and
   hashed where required.
3. Scan for credentials, launcher tokens, identifying absolute paths, and
   private-world data.
4. Verify that referenced raw artifacts remain outside tracked public paths.
5. Confirm deviations and risks are classified as blocking or non-blocking.
6. Confirm nontrivial work has an independent review result and any required
   manual attestation is explicit.
7. Add only the sanitized manifest and small decision-relevant summaries.

Any failed schema, redaction, integrity, or review check blocks admission.

The verifier admits milestone summaries into typed records before applying their
arithmetic and carried-figure rules. M2 through M7 counts are whole, nonnegative
integers; checked sums reject overflow. Profiling budgets remain finite, positive
measurements. M3 campaign inventories, M4 process and gate records, and M7's carried
M5 inventory total use the admitted models rather than parallel JSON-pointer rules.
Summary and review verdicts use the closed vocabulary `pass`, `fail`,
`provisional`, and `not-required`; each record's policy still determines which
verdicts it accepts. Input-shape diagnostics retain JSON pointers, including array
indices, while rule diagnostics name the failed contract.
Admission failures are reported once during integrity verification. Cached closure
rules are evaluated against the admitted record but reported only when closure is
requested, so an open milestone may retain a truthful failing verdict. Closure
always checks the record's milestone identity, including cached records.

Command failures print the outer error once. Errors with a source include that
cause in their display text; source chains remain available to programmatic callers.
