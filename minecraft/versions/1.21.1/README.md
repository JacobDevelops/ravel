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

No adapter spells 26.1.2's member names any more. Every member the persistence,
content and outward adapters reach is declared in `reflection.toml`, checked
against this version's jar, and generated into `GameMembers` as the name and
descriptor the JVM links.

One member is still written by hand, and it is a shape difference rather than a
rename: `Content.withId` stamps a registry key onto a properties builder, which
26.1.2 requires and this version has no setter for at all. A member a version does
not declare cannot be declared for it, so under 1.21.1 that lookup throws and each
block and item is reported as unregistered. Registering content here wants a
differently shaped call, not the same one under another name, and no work package
owns that yet.
