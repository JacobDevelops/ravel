package net.minecraft.world.level.block;

import com.google.common.collect.ImmutableList;
import net.minecraft.core.IdMapper;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;

/** One block, which exists only once its properties name it. */
public class Block {

    /**
     * Where every registered block's states have to end up.
     *
     * <p>Static and shared, because the game's is: a block update is written as
     * an index into this one mapper, and a state that never reached it cannot be
     * encoded at all.
     */
    public static final IdMapper BLOCK_STATE_REGISTRY = new IdMapper();

    private final ResourceKey id;
    private final String applied;
    private final StateDefinition stateDefinition;

    public Block(BlockBehaviour.Properties properties) {
        this.id = properties.id();
        this.applied = properties.applied();
        // Two states rather than one, so a walk that adds the first and stops
        // reads differently from one that adds them all.
        this.stateDefinition = new StateDefinition(
                ImmutableList.of(new BlockState("default"), new BlockState("alternate")));
    }

    public StateDefinition getStateDefinition() {
        return stateDefinition;
    }

    public ResourceKey id() {
        return id;
    }

    /** How this block behaves, as the properties it was built from describe it. */
    public String applied() {
        return applied;
    }
}
