package net.minecraft.resources;

/** A key naming one entry inside one registry. */
public final class ResourceKey {
    private final String registry;
    private final Identifier identifier;

    private ResourceKey(String registry, Identifier identifier) {
        this.registry = registry;
        this.identifier = identifier;
    }

    /** The root key each registry is named by, which nothing outside this class builds. */
    public static ResourceKey createRegistryKey(String registry) {
        return new ResourceKey(registry, null);
    }

    public static ResourceKey create(ResourceKey registry, Identifier identifier) {
        return new ResourceKey(registry.registry, identifier);
    }

    public Identifier identifier() {
        return identifier;
    }

    @Override
    public String toString() {
        return registry + "/" + identifier;
    }
}
