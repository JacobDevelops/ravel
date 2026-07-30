# Test fixtures

`public/` contains original, unrestricted inputs that may be committed and
redistributed with Ravel. Every fixture should state its purpose, author, and
licence.

`local/` is ignored except for its policy README. It is the only location in
this repository tree for reconstructed official Minecraft, ATM11, third-party
mod, private-world, or other restricted compatibility inputs. Tools may write
there only from authorized sources and must verify pinned hashes. Tests must
refer to those inputs through explicit environment or command-line paths and
must skip collection, not silently substitute content, when they are absent.

Never commit credentials, launcher tokens, official mod JARs, ATM-owned assets,
rights-sensitive notes, or derived restricted content.
