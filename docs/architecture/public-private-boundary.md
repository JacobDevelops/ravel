# Registry and local-material boundary

Ravel is one repository and one Cargo workspace. `registry/` contains counterpart
sources, pack declarations, inventories, behaviour specifications, provenance,
and checks alongside the loader and SDK. `registry/mods/*` belongs to the root
workspace and shares its `Cargo.lock` and target directory; registry checks use
this checkout rather than a sibling repository or an external workspace path.

Cargo path dependencies must resolve inside this checkout. Counterparts consume
the public SDK, not loader internals. General SDK gaps become capability
proposals with original test-mod acceptance cases; colocating the sources does
not relax those dependency rules.

Authorized official fixtures and compatibility worlds remain ignored local
material. `tests/fixtures/local/` stays untracked and is not a source or package
dependency. Registry scratch output,
extracted inputs, and generated build artifacts remain ignored.

Credentials and launcher tokens are never tracked. Credential scanning and
diagnostic redaction protect authentication data. Tracked evidence contains
sanitized summaries and hashes, not secrets, raw captures, or fixture contents.

The in-tree registry is not a deployed public package registry. Publication,
publisher identity, signing, moderation, and distribution remain deferred M13
work under `SPEC.md` §12.3. The publication-service layout in
[`official-registry-design.md`](official-registry-design.md) is a proposal, not
a description of the current source tree or an authorization to publish.
