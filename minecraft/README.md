# Minecraft adapters

Version-specific adapter inputs live under `versions/`. General loader and SDK
crates must not depend on their concrete details. The shared
`ravel-minecraft-adapter` crate consumes this data; a version gets its own crate
only if ADG-3 demonstrates a separately compiled boundary is necessary.
