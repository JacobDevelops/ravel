package net.minecraft.core.registries;

import net.minecraft.resources.ResourceKey;

/** The root keys the game names its registries by. */
public final class Registries {
    public static final ResourceKey BLOCK = ResourceKey.createRegistryKey("block");
    public static final ResourceKey ITEM = ResourceKey.createRegistryKey("item");

    private Registries() {
    }
}
