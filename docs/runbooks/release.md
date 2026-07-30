# Release

Status: local release-candidate scaffold. Publishing packages or creating a
registry is post-initial scope.

1. Run the complete `ravel:verify` gate on supported Linux and macOS targets.
2. Validate the relevant milestone evidence and independent review.
3. Confirm package identity, compatibility, targets, integrity, licence, source,
   and provenance metadata.
4. Audit dependencies, advisories, notices, assets, names, branding,
   permissions, and attribution.
5. Verify public/private separation and scan history and artifacts for
   restricted content and secrets.
6. Reproduce candidate packages from pinned inputs and compare hashes.
7. Record rollback inputs and retain the last compatible release.

Signing, publisher identity, moderation, distribution, and remote publication
require a separately approved plan.
