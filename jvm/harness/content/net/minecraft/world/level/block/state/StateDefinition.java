package net.minecraft.world.level.block.state;

import com.google.common.collect.ImmutableList;

/**
 * Stands in for a block's state set.
 *
 * <p>Its accessor returns the Guava list the pinned jar declares, which is why
 * the adapter walks it reflectively: neither tracked jar carries Guava, so a
 * stub declaring this return type could not compile and the compiled half of
 * the content seam cannot do this walk.
 */
public final class StateDefinition {

    private final ImmutableList states;

    public StateDefinition(ImmutableList states) {
        this.states = states;
    }

    public ImmutableList getPossibleStates() {
        return states;
    }
}
