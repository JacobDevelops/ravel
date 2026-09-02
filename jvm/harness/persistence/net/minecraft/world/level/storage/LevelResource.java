package net.minecraft.world.level.storage;

/**
 * Stands in for the pinned record of the same name, carrying the one constant
 * ADG-7's adapter names and nothing else.
 */
public final class LevelResource {

    /** The world's own directory, which is the shape both identities come from. */
    public static final LevelResource ROOT = new LevelResource(".");

    private final String id;

    public LevelResource(String id) {
        this.id = id;
    }

    public String id() {
        return id;
    }

    @Override
    public String toString() {
        return id;
    }
}
