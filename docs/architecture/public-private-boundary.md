# Public and private boundary

This repository contains reusable loader and SDK infrastructure, original test
mods, synthetic fixtures, public schemas, and sanitized evidence.

Private compatibility workspaces may consume released public Ravel artifacts.
This workspace does not depend on their paths, packages, or fixture contents.

The reference runs one way. The private workspace resolves this checkout from
an environment variable and reads the ignored fixture in place; nothing is
copied back, and this repository gains no path, dependency, or configuration
pointing the other way.

The scaffold gate rejects tracked references to the private workspace's name and
Cargo path dependencies that escape this checkout. The private workspace stays
unnamed here so tracked files cannot acquire that dependency by reference.

Credentials and launcher tokens are never tracked here. Credential scanning
and diagnostic redaction protect authentication data. Local fixtures live in
`tests/fixtures/local/`, which is ignored; evidence contains sanitized summaries
and hashes rather than secrets or raw captures.
