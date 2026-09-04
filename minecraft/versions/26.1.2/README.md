# Minecraft 26.1.2 version data

M1 populates the pinned mapping provenance, hook inventory, and adapter input
hashes consumed by `ravel-minecraft-adapter` after ADG-3. Placeholder mapping or
hook data is intentionally absent.

This version carries 39 reviewed hooks and records 0 non-goals. Both counts are
checked against `hooks.toml` by the scaffold contract.

`harness/` holds the seven JVM-harness templates only this version renders.
`harness/game/Registries.java.in` and `harness/game/ResourceKey.java.in` are the
root key and the key type this version's properties builders demand before a
block or an item exists; `harness/game/SOUND_PLAY_RESULT.java.in` is what a play
request is answered with here, where the other version answers nothing at all;
`harness/game/SavedDataType.java.in` is the single typed key a saved datum is
addressed by here, where the other version names one with a factory and a string;
`harness/game/KeyEvent.java.in` is the record a keystroke reaches a screen as
here, where the other version passes the same three numbers as three arguments.
Each of those five stands in for a class the other tracked version's game does not
have.

`harness/game/MINECRAFT.java.in` is a shape difference rather than a missing
class: this version's world-load seam takes the optional rule overrides a world
is created with and the other's takes none, and a class is declared once or not
at all.

`harness/game/chat.filter-text.java.in` is a shape difference of the same kind:
this version's hook table pins a fourth target on the chat text class, the
character filter, which the other version's does not, so the settings stand-in
here carries four members where the shared template carries three.

Each of the seven has to be reached by something this version renders, or the
generator refuses it as an override that outlived the template it shadowed.
