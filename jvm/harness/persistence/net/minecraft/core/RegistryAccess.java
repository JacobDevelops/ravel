package net.minecraft.core;

/**
 * Stands in for the registry access, with the supertype the storage's declared
 * constructor needs it to satisfy.
 *
 * <p>The edge is the point: the storage takes a {@code HolderLookup.Provider}
 * and the layered access hands it a frozen registry access, so a stand-in that
 * left the two unrelated would let a constructor call compile here and throw at
 * a launch.
 */
public interface RegistryAccess extends HolderLookup.Provider {

    /** What the layered access composites down to. */
    final class Frozen implements RegistryAccess {
    }
}
