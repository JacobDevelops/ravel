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

The private workspace is a separate repository beside this one, not an ignored
directory inside it, so the boundary is a repository boundary rather than a
pattern somebody has to keep correct. It is not named here, and no tracked file
in this repository names it: `ravel:verify-scaffold` sweeps every tracked file
for that name and for any manifest whose path dependency resolves outside this
checkout, and reports either as a violation. A document that started naming the
private workspace would be the first sign the split had stopped being a split,
so the check is what keeps this paragraph true rather than the paragraph being
the claim.

The reference runs one way. The private workspace resolves this checkout from
an environment variable and reads the ignored fixture in place; nothing is
copied back, and this repository gains no path, dependency, or configuration
pointing the other way.

Official Minecraft files, ATM-owned scripts, quests, menus and assets,
third-party mod JARs, credentials, and launcher tokens are never tracked here.
Authorized reconstruction may write pinned material only to
`tests/fixtures/local/`, which is ignored.

Public evidence contains sanitized summaries and hashes, not restricted inputs
or raw captures. Every publication candidate needs a resolved provenance record
covering code, behaviour, assets, names, branding, permissions, attribution,
and licence obligations. Reimplementation in Rust does not itself clear those
rights.
