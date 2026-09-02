package net.minecraft.world.level.block.state;

import net.minecraft.resources.ResourceKey;

/** The behaviour a block's properties are built from. */
public class BlockBehaviour {

    /**
     * What carries a state's cache step.
     *
     * <p>Declared here rather than on {@code BlockState} because that is where
     * the pinned jars declare it, and the reviewed surface names the class that
     * carries a member rather than one that inherits it.
     */
    public static class BlockStateBase {

        private boolean cached;

        public void initCache() {
            cached = true;
        }

        /** Whether the walk initialised this state, as the harness reads it back. */
        public boolean isCached() {
            return cached;
        }
    }

    /**
     * A block's properties, which refuse to describe a block that has no id.
     *
     * <p>Deliberately hostile: the pinned 26.1.2 constructor reads the id back
     * out, so a properties object that never had one fails at construction. An
     * adapter that forgot {@code setId} would pass a stand-in that let it.
     *
     * <p>The applied values are kept so the harness can read back what the
     * adapter did rather than only that it did something. A stand-in that
     * accepted every setter and remembered none would pass an adapter that
     * applied a mod's description to the wrong property.
     */
    public static final class Properties {
        private ResourceKey id;
        private float hardness = -1.0f;
        private float explosionResistance = -1.0f;
        private boolean requiresCorrectTool;
        private boolean randomTicks;

        private Properties() {
        }

        public static Properties of() {
            return new Properties();
        }

        public Properties setId(ResourceKey id) {
            this.id = id;
            return this;
        }

        public Properties strength(float hardness, float explosionResistance) {
            this.hardness = hardness;
            this.explosionResistance = explosionResistance;
            return this;
        }

        public Properties requiresCorrectToolForDrops() {
            this.requiresCorrectTool = true;
            return this;
        }

        public Properties randomTicks() {
            this.randomTicks = true;
            return this;
        }

        public ResourceKey id() {
            if (id == null) {
                throw new IllegalStateException("Block id not set");
            }
            return id;
        }

        /** What was applied, as the harness reads it back. */
        public String applied() {
            return hardness + "/" + explosionResistance + "/" + requiresCorrectTool + "/"
                    + randomTicks;
        }
    }
}
