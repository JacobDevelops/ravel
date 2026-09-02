package net.minecraft.core.registries;

import net.minecraft.core.DefaultedRegistry;

/**
 * The built-in registries, with the internal name, member and descriptor the
 * pinned inventory carries for {@code registry.built-in-freeze}.
 */
public final class BuiltInRegistries {
    public static final DefaultedRegistry BLOCK = new DefaultedRegistry("block");
    public static final DefaultedRegistry ITEM = new DefaultedRegistry("item");

    private BuiltInRegistries() {
    }

    /** The last point new content can be added, which is why the prologue is here. */
    public static void freeze() {
        BLOCK.freeze();
        ITEM.freeze();
    }
}
