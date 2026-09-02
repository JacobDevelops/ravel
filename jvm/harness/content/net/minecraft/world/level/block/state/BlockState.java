package net.minecraft.world.level.block.state;

/**
 * Stands in for one block state, with the cache step vanilla's own bootstrap
 * runs on every state it registers.
 */
public class BlockState extends BlockBehaviour.BlockStateBase {

    private final String name;

    public BlockState(String name) {
        this.name = name;
    }

    @Override
    public String toString() {
        return name;
    }
}
