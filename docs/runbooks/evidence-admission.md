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
arithmetic and carried-figure rules. M5, M6, and M7 counts are whole, nonnegative
integers; checked sums reject overflow. Summary and review verdicts use the closed
vocabulary `pass`, `fail`, `provisional`, and `not-required`; each record's policy
still determines which verdicts it accepts. Input-shape diagnostics retain JSON
pointers, including array indices, while rule diagnostics name the failed contract.
