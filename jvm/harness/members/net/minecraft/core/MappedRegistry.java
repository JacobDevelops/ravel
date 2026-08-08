package net.minecraft.core;

/**
 * Stands in for the one writable registry, carrying both members the inventory
 * pins.
 *
 * <p>{@code freeze} is the rewritten method and {@code frozen} is the field
 * beside it, which is the registry's own record of whether it still accepts
 * entries.
 */
public final class MappedRegistry implements Registry {

    private boolean frozen;

    public Registry freeze() {
        frozen = true;
        return this;
    }
}
