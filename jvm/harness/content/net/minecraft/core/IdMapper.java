package net.minecraft.core;

import java.util.ArrayList;
import java.util.List;

/**
 * Stands in for the mapper the network protocol writes a block update through.
 *
 * <p>What the harness has to prove about it is membership: a block whose states
 * never reached this is a block the game can place and cannot encode, which is
 * the disconnect the seventh real launch hit.
 */
public final class IdMapper {

    private final List<Object> entries = new ArrayList<>();

    public void add(Object entry) {
        entries.add(entry);
    }

    /** How many states this holds, as the harness reads it back. */
    public int size() {
        return entries.size();
    }
}
