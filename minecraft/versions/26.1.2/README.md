# Minecraft 26.1.2 version data

M1 populates the pinned mapping provenance, hook inventory, and adapter input
hashes consumed by `ravel-minecraft-adapter` after ADG-3. Placeholder mapping or
hook data is intentionally absent.

This version carries 39 reviewed hooks and records 0 non-goals. Both counts are
checked against `hooks.toml` by the scaffold contract.

`harness/` holds the five JVM-harness templates only this version renders.
`harness/game/Registries.java.in` and `harness/game/ResourceKey.java.in` are the
root key and the key type this version's properties builders demand before a
block or an item exists; `harness/game/SOUND_PLAY_RESULT.java.in` is what a play
request is answered with here, where the other version answers nothing at all;
`harness/game/SavedDataType.java.in` is the single typed key a saved datum is
addressed by here, where the other version names one with a factory and a string.
Each of those four stands in for a class the other tracked version's game does not
have.

`harness/game/SERVER.java.in` is here for a different reason, and the reason is
the Java language rather than the game. Both versions have this class. This one
leaves its whole game in packages, so a stand-in for the saving server can name
the level and the resource constant its two reflected members are typed by; the
other splits its game between `net.minecraft` and the unnamed package, and
nothing inside a package can name a class outside every package. That version
renders no stand-in for it, and its `adapter.toml` records the persistence section
as one it does not run.

Each of the five has to be reached by something this version renders, or the
generator refuses it as an override that outlived the template it shadowed.
