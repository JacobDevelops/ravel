# Release

Status: local release-candidate scaffold. Publishing packages or creating a
registry is post-initial scope.

1. Run the complete `ravel:verify` gate on Linux through M10. After the
   Apple Silicon phase begins, run it on every then-supported target.
2. Validate the relevant milestone evidence and independent review.
3. Confirm package identity, compatibility, portable source-file integrity, licence,
   repository, and provenance metadata.
4. Audit dependency licences and advisories.
5. Scan history and artifacts for credentials and secrets.
6. Reproduce candidate packages from pinned inputs and compare hashes.
7. Record rollback inputs and retain the last compatible release.

Publisher signing, publisher identity, moderation, distribution, and remote publication
require a separately approved plan.
