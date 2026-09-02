package dev.ravel.jvm;

/**
 * Puts the mods' data pack in a world before the game reads that world's packs.
 *
 * <p><b>Why there is a second world seam at all.</b> The crossing that admits a
 * world is handed the world stem, and the stem is what the game builds by
 * scanning the world's {@code datapacks} directory. So a pack written from there
 * is on disk in time for the world's <em>next</em> open, and a world opened for
 * the first time runs its whole first session without the mods' tags, recipes
 * and loot tables. The twelfth real launch is where that stopped being a
 * paragraph in {@code packs.rs} and started being a missing recipe.
 *
 * <p>This runs at {@code world.open-level-data}, which the game reaches with the
 * storage access made and two calls before it builds the pack repository. That
 * is the whole window, and the only thing that has to cross it is the world's
 * own directory.
 *
 * <p><b>Nothing here decides whether a world opens.</b> The pack is a
 * convenience the game would have started without, so a failure is reported by
 * the native side and this returns. That is the opposite of {@link Persistence},
 * deliberately: losing a mod's recipes for a session is not losing a player's
 * saved work.
 */
final class Datapack {

    private Datapack() {
    }

    /**
     * Writes the pack for the world this access is holding.
     *
     * @param access the selected {@code LevelStorageAccess}, held opaquely
     */
    static void write(Object access) {
        try {
            Hook.ravelWriteWorldDatapack(Persistence.worldPath(access));
        } catch (Throwable unwritten) {
            // Deliberately everything. This runs inside the game's own world-open
            // call, and an escape here would cost the player the world over a
            // file the world does not need to open.
            Agent.diagnose("the mods' data pack could not be written into this world ("
                    + unwritten.getClass().getName() + ")");
        }
    }
}
