package net.minecraft.core;

/**
 * Stands in for the lookup only so its nested provider type has the binary name
 * the storage constructor's declared descriptor carries.
 */
public final class HolderLookup {

    private HolderLookup() {
    }

    /** What a storage decodes its stored objects against. */
    public interface Provider {
    }
}
