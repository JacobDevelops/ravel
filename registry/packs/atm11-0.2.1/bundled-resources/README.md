# Pack-bundled resources (ATM11 0.2.1)

These are third-party assets the pinned pack *bundles and redistributes* — not
mods and not standalone registry entries. Per
`ravel/docs/architecture/official-registry-design.md` §5, a pack ships its own
authored/bundled data (config, scripts, quests, branding, and resource packs
such as shaderpacks); those assets belong to the pack composition, not to the mod
line under `mods/`.

## shaderpacks/

The six shaderpacks the pack ships. A shaderpack is GLSL shader assets with no
code and no Ravel `Mod`; it needs no SDK and does not load through the mod loader.
It is consumed by a shader-loader mod (the Iris counterpart, tracked as a normal
mod under `mods/`), which is the only thing here that needs render-pipeline
SDK surface. Each directory keeps the asset's rights/provenance record only —
every one is `blocked:unresolved` (third-party assets, no declared licence), so
the pack's composition provenance must clear redistribution before any pack ships.

The shader *bytes* are never tracked here; they live only in the gitignored
`tests/fixtures/local/` working tree, like every other original source.
