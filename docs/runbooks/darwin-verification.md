# Apple Silicon verification

The GitHub Actions gate runs on Linux, so any required macOS evidence must
come from an actual Apple Silicon host. Linux cross-evaluation is useful for
catching output-shape drift but is not a substitute for native execution once
the post-Linux-M10 Mac phase begins.

Do not run this procedure for M0 through M10. macOS work starts only after Linux
M10 has passed with the loader, counterpart library, complete pack, constrained
performance, and progression evidence already accepted.

Install Bash 4+ and GNU findutils/coreutils with the system package manager
(for example, `brew install bash findutils coreutils`). Put their binaries on
PATH ahead of the macOS system tools: the task scripts require GNU `find` and
Bash's `mapfile`, which the system Bash does not provide.

On an `aarch64-darwin` machine with mise, rustup, and native build tools installed:

```console
mise install
mise exec -- cargo make ravel:verify
mise exec -- rustc -vV
uname -a
```

Record command status, tool versions, the Ravel revision, dirty state, hardware
profile, and sanitized log hashes in the milestone evidence manifest.
A milestone that requires macOS evidence does not pass until this native run and
its independent review are complete.
