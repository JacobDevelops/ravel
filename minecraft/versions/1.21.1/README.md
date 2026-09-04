# Minecraft 1.21.1 version data

The version All the Mods 10: To the Sky 2.0.2 runs on, and the first obfuscated
one this repository targets: names are read from Mojang's published client
mapping rather than from the class files, which is what `mappingSource` records.

This version carries 31 reviewed hooks and records 8 non-goals. Twenty-nine of
the hooks resolve against the pinned jar unchanged from 26.1.2's inventory; the
other two, `world.open-persistence` and `screen.title-menu-options`, are carried
at drifted descriptors with re-reviewed capability sentences. Every non-goal is
declared in `hooks.toml` with the reason it is not simply retargeted, as data
rather than prose, so a caller naming one is told it named a decision.

The two counts above are checked against `hooks.toml` by the scaffold contract,
because a README that miscounts the file beside it is worse than one that says
nothing.

What is not proved here is any of it running. Nothing in this repository has
started a 1.21.1 process with Ravel attached, so this version's saved-data path is
generated and compiled but not driven. `ROADMAP.md` carries that as owed work.

No adapter spells 26.1.2's member names any more. Every member the persistence,
content and outward adapters reach is declared in `reflection.toml`, checked
against this version's jar, and generated into `GameMembers` as the name and
descriptor the JVM links.

`harness/` holds the nine JVM-harness templates this version renders instead of
the shared ones, and each is a shape difference rather than a rename. This
version's block and item properties builders carry no id setter in any spelling —
a block is named by the identifier it is registered under and by nothing it
carries — so `harness/game/BlockBehaviour$Properties.java.in` and
`harness/game/Item$Properties.java.in` stamp no registry key and refuse the
absence of none. `harness/game/SOUND_MANAGER.java.in` answers a play request with
nothing, where the other tracked version answers with a type of its own.

The persistence half is four more. `harness/game/STORAGE.java.in` and
`harness/sibling/RavelWorldData.java.in` want the world's data directory as a
`File`, ask for a stored datum with a factory and a string beside it, and hand a
replacement that string and the object, where the other version carries all three
inside one typed key; `harness/game/SavedDataFactory.java.in` is that factory,
which the other version's game does not have at all.
`harness/game/MINECRAFT.java.in` declares a world load that takes no optional rule
overrides, where the other version's takes them, so the two seams differ in their
argument vector.

The screen half is two, and both are the same disagreement seen twice. A
keystroke reaches a screen here as three numbers and reaches the other version's
as one record, so `harness/game/Screen.java.in` writes the whole screen out to
declare its keyboard override with three parameters. And this version's title
menu hands its column builder's caller nothing back where the other hands back
the row below the column, so `harness/game/screen.title-menu-options.java.in`
declares that builder returning void and this version's `hooks.toml` declares
where Ravel's own row goes instead.

A role binding answers a rename; it does not answer a call one version makes and
the other does not have.

This version's name space is split: 26 classes stay in `net.minecraft` because
they carry `@DontObfuscate` and the rest move into the unnamed package, and no
Java source can name a class outside every package from inside one. So the whole
stand-in game is written in the unnamed package and `MinecraftServer`, the save
seam's receiver, is the one class whose written name is not the name this version
links. `jvm/harness/remap/Remap.java` puts it back in the compiled class files
before any section runs against them, which is why the harness runs all twelve
sections at this version and nothing here declares its way out of one.
