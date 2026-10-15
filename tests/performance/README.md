# Performance tests

The single-pair performance protocol is split into:

- `profiles/initial.toml` — the versioned workload and pass calculations;
- `fixtures/collector-dry-run.json` — platform-neutral normalized collector data;
- `fixtures/result-dry-run.json` — platform-neutral aggregate output.

The dry-run fixtures contain synthetic values only. They validate shape and
wiring and are not admissible M9 acceptance evidence.

Run the cross-platform dry-run entry point with:

```console
cargo make ravel:baseline
```

The local task deliberately accepts only the `initial` profile in `dry-run`
mode. The future native Linux pair remains the operator-run procedure in
`docs/runbooks/baseline-capture.md`; M0 uses Linux launch proof instead.

Validate the static contract with:

```console
taplo format --check tests/performance/profiles/initial.toml
check-jsonschema --check-metaschema schemas/performance-collector.schema.json
check-jsonschema --check-metaschema schemas/performance-result.schema.json
check-jsonschema --schemafile schemas/performance-collector.schema.json \
  tests/performance/fixtures/collector-dry-run.json
check-jsonschema --schemafile schemas/performance-result.schema.json \
  tests/performance/fixtures/result-dry-run.json
```
