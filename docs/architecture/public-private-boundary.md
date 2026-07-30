# Public and private boundary

This public repository contains reusable loader and SDK infrastructure,
unrestricted original test mods, synthetic fixtures, public schemas, and
sanitized evidence.

The separate private compatibility workspace contains ATM11-equivalent
counterparts, official pack fixtures, private manifests and integrations,
rights-sensitive behavioural research, permission status, private worlds, and
unsanitized reports. It may consume released public Ravel artifacts. The public
workspace must not depend on private paths, packages, fixture contents, or
source-derived notes.

Official Minecraft files, ATM-owned scripts, quests, menus and assets,
third-party mod JARs, credentials, and launcher tokens are never tracked here.
Authorized reconstruction may write pinned material only to
`tests/fixtures/local/`, which is ignored.

Public evidence contains sanitized summaries and hashes, not restricted inputs
or raw captures. Every publication candidate needs a resolved provenance record
covering code, behaviour, assets, names, branding, permissions, attribution,
and licence obligations. Reimplementation in Rust does not itself clear those
rights.
