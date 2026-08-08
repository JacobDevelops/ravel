package net.minecraft.world.level.storage;

import java.nio.file.Path;

/**
 * Stands in for the pinned class only so its nested access type has the binary
 * name {@code LevelStorageSource$LevelStorageAccess} the hook descriptor pins.
 */
public final class LevelStorageSource {

    private LevelStorageSource() {
    }

    /** The selected world's directory handle, and the thing that holds its lock. */
    public static final class LevelStorageAccess implements AutoCloseable {

        private final String world;
        private boolean closed;

        public LevelStorageAccess(String world) {
            this.world = world;
        }

        public Path getLevelPath(LevelResource resource) {
            return Path.of(world, resource.id());
        }

        /** Whether a refusal released the world lock, which is what closing means here. */
        public boolean isClosed() {
            return closed;
        }

        @Override
        public void close() {
            closed = true;
        }
    }
}
