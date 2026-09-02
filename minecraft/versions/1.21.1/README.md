# Minecraft 1.21.1 version data

The version All the Mods 10: To the Sky 2.0.2 runs on, and the first obfuscated
one this repository targets: names are read from Mojang's published client
mapping rather than from the class files, which is what `mappingSource` records.

This version carries 29 reviewed hooks and records 8 non-goals. Twenty-seven of
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

The persistence adapter no longer spells 26.1.2's member names: every member it
reaches is declared in `reflection.toml`, checked against this version's jar, and
generated into `GameMembers` as the name and descriptor the JVM links. The content
and outward adapters still spell literals, which is 26.1.2's spelling and the
obfuscated name of nothing here.
