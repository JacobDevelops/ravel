package net.minecraft.server.level;

import net.minecraft.world.level.storage.SavedDataStorage;

/** Stands in for the level a running server hangs its saved-data storage off. */
public final class ServerLevel {

    private final SavedDataStorage storage;

    public ServerLevel(SavedDataStorage storage) {
        this.storage = storage;
    }

    public SavedDataStorage getDataStorage() {
        return storage;
    }
}
