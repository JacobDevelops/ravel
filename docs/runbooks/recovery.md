# Recovery

Ravel fails closed when required mods, integrity checks, sides, registries,
dependency graphs, or saved-data versions are invalid.

1. Stop before world access when a pre-world gate fails.
2. Preserve the original world and diagnostic bundle without opening or saving
   it through a reduced required-mod graph.
3. Record the mod identity, dependency chain, lifecycle phase, Minecraft and
   Ravel versions, platform, and failing invariant.
4. Redact tokens, private paths, and restricted content before sharing a
   diagnostic.
5. Restore the exact compatible package set or run a tested, declared migration
   against a copy.
6. Revalidate the complete graph and saved-data envelope before reopening the
   world.

There is no recovery mode that silently disables a required mod and saves
partial state.
