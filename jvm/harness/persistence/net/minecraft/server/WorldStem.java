package net.minecraft.server;

import net.minecraft.core.LayeredRegistryAccess;

/** Stands in for the pinned record, carrying the two members the adapter uses. */
public final class WorldStem implements AutoCloseable {

    private final LayeredRegistryAccess registries = new LayeredRegistryAccess();
    private boolean closed;

    public LayeredRegistryAccess registries() {
        return registries;
    }

    /** Whether a refusal closed the stem it was handed. */
    public boolean isClosed() {
        return closed;
    }

    @Override
    public void close() {
        closed = true;
    }
}
