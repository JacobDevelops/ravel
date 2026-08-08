package net.minecraft.client;

import java.util.Optional;
import net.minecraft.server.WorldStem;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.world.level.storage.LevelStorageSource;

/**
 * Stands in for the client, carrying only {@code doWorldLoad} with the exact
 * descriptor the hook inventory pins.
 *
 * <p>The body counts itself. That count is the whole proof: a refused world load
 * is one where this never ran, and an admitted one is where it did.
 */
public final class Minecraft {

    private static int loads;

    /** How many times the world-load body ran. */
    public static int loads() {
        return loads;
    }

    public void doWorldLoad(
            LevelStorageSource.LevelStorageAccess access,
            PackRepository packs,
            WorldStem stem,
            Optional<Object> gameRules,
            boolean newWorld) {
        loads++;
    }
}
