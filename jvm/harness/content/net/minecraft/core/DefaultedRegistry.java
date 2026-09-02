package net.minecraft.core;

/**
 * The registry type the built-in block and item holders are declared as.
 *
 * <p>Separate from {@link Registry} because the pinned inventory says so: the
 * fields are declared as this and the register call takes the supertype, so a
 * stand-in that collapsed the two would let a field lookup by descriptor pass
 * against a type the game does not declare.
 */
public final class DefaultedRegistry extends Registry {

    public DefaultedRegistry(String name) {
        super(name);
    }
}
