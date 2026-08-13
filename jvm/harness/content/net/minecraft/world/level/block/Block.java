package net.minecraft.world.level.block;

import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.block.state.BlockBehaviour;

/** One block, which exists only once its properties name it. */
public class Block {
    private final ResourceKey id;
    private final String applied;

    public Block(BlockBehaviour.Properties properties) {
        this.id = properties.id();
        this.applied = properties.applied();
    }

    public ResourceKey id() {
        return id;
    }

    /** How this block behaves, as the properties it was built from describe it. */
    public String applied() {
        return applied;
    }
}
