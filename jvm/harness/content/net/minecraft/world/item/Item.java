package net.minecraft.world.item;

import net.minecraft.resources.ResourceKey;

/** One item, with the same id rule its block counterpart has. */
public class Item {

    /** An item's properties, which refuse to describe an item that has no id. */
    public static final class Properties {
        private ResourceKey id;
        private int stackSize = -1;
        private boolean fireResistant;

        public Properties() {
        }

        public Properties setId(ResourceKey id) {
            this.id = id;
            return this;
        }

        public Properties stacksTo(int stackSize) {
            this.stackSize = stackSize;
            return this;
        }

        public Properties fireResistant() {
            this.fireResistant = true;
            return this;
        }

        public ResourceKey id() {
            if (id == null) {
                throw new IllegalStateException("Item id not set");
            }
            return id;
        }

        /** What was applied, as the harness reads it back. */
        public String applied() {
            return stackSize + "/" + fireResistant;
        }
    }

    private final ResourceKey id;
    private final String applied;

    public Item(Properties properties) {
        this.id = properties.id();
        this.applied = properties.applied();
    }

    public ResourceKey id() {
        return id;
    }

    /** How this item behaves, as the properties it was built from describe it. */
    public String applied() {
        return applied;
    }
}
