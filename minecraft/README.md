# Minecraft adapters

Version-specific adapter inputs live under `versions/`. General loader and SDK
crates must not depend on their concrete details. The shared
`ravel-minecraft-adapter` crate generates bindings, hooks and screen sources from
the selected version's pinned inputs.
