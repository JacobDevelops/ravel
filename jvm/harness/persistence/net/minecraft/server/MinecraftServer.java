package net.minecraft.server;

import java.nio.file.Path;
import net.minecraft.world.level.storage.LevelResource;
import net.minecraft.world.level.storage.SavedDataStorage;

/** Stands in for the saving server, and reports whether its own body still ran. */
public final class MinecraftServer {

    private static int saves;

    private final SavedDataStorage storage = new SavedDataStorage(Path.of("world", "data"), null, null);

    /** How many times the vanilla save body ran, which a Ravel failure must never reduce. */
    public static int saves() {
        return saves;
    }

    public Path getWorldPath(LevelResource resource) {
        return Path.of("world", resource.id());
    }

    public SavedDataStorage getDataStorage() {
        return storage;
    }

    public boolean saveAllChunks(boolean quiet, boolean flush, boolean forced) {
        saves++;
        return true;
    }
}
