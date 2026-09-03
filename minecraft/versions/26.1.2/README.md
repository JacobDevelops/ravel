# Minecraft 26.1.2 version data

M1 populates the pinned mapping provenance, hook inventory, and adapter input
hashes consumed by `ravel-minecraft-adapter` after ADG-3. Placeholder mapping or
hook data is intentionally absent.

This version carries 39 reviewed hooks and records 0 non-goals. Both counts are
checked against `hooks.toml` by the scaffold contract.

`harness/` holds the three JVM-harness templates only this version renders, and
each stands in for a class the other tracked version's game does not have.
`harness/game/Registries.java.in` and `harness/game/ResourceKey.java.in` are the
root key and the key type this version's properties builders demand before a
block or an item exists; `harness/game/SOUND_PLAY_RESULT.java.in` is what a play
request is answered with here, where the other version answers nothing at all.
Each has to be reached by something this version renders, or the generator refuses
it as an override that outlived the template it shadowed.
