# Evidence admission

1. Validate the milestone manifest against
   `schemas/evidence.schema.json`.
2. Confirm every input, command, summary, and review record is complete and
   hashed where required.
3. Scan for credentials, launcher tokens, identifying absolute paths,
   restricted filenames, official content, private-world data, and
   rights-sensitive research.
4. Verify that referenced raw artifacts remain outside tracked public paths.
5. Confirm deviations and risks are classified as blocking or non-blocking.
6. Confirm nontrivial work has an independent review result and any required
   manual attestation is explicit.
7. Add only the sanitized manifest and small decision-relevant summaries.

Any failed schema, redaction, rights, integrity, or review check blocks
admission.
