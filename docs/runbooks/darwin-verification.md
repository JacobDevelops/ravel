# Apple Silicon verification

Tangled Spindle engines are Linux-based, so required macOS evidence must come
from an actual Apple Silicon host. Linux cross-evaluation is useful for catching
output-shape drift but is not a substitute for native execution.

On an `aarch64-darwin` machine with Nix and devenv installed:

```console
devenv test --no-tui
nix flake check --accept-flake-config
devenv shell -- rustc -vV
uname -a
```

Record command status, tool versions, the Ravel revision, dirty state, hardware
profile, and sanitized log hashes in the milestone evidence manifest. A
milestone that requires macOS evidence does not pass until this native run and
its independent review are complete.
