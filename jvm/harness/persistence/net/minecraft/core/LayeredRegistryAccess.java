package net.minecraft.core;

/** Stands in for the layered registries a storage is constructed against. */
public final class LayeredRegistryAccess {

    private final RegistryAccess.Frozen composite = new RegistryAccess.Frozen();

    public RegistryAccess.Frozen compositeAccess() {
        return composite;
    }
}
