package net.minecraft.client.gui.screens.worldselection;

import net.minecraft.world.level.storage.LevelStorageSource;

/**
 * Stands in for the flow that opens a world, carrying only the step the data
 * pack has to be written at.
 *
 * <p>The real one reaches this step with the storage access made and two calls
 * before it builds the pack repository over the world's {@code datapacks}
 * directory. That ordering is the whole reason the seam exists, so the body
 * counts itself: a pack written after this ran is a pack the game did not read.
 */
public final class WorldOpenFlows {

    private static int steps;

    /** How many times the step's own body ran. */
    public static int steps() {
        return steps;
    }

    public void openWorldLoadLevelData(
            LevelStorageSource.LevelStorageAccess access, Runnable onFail) {
        steps++;
    }
}
