package com.google.common.collect;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

/**
 * Stands in for the Guava list the game's state accessor returns.
 *
 * <p>Neither tracked jar carries Guava and this harness has no Guava either, so
 * the name is stood in for exactly as a game class is. It is iterable and
 * nothing else, because iterating it is all the adapter does.
 */
public final class ImmutableList implements Iterable<Object> {

    private final List<Object> entries = new ArrayList<>();

    public static ImmutableList of(Object... members) {
        ImmutableList held = new ImmutableList();
        for (Object member : members) {
            held.entries.add(member);
        }
        return held;
    }

    @Override
    public Iterator<Object> iterator() {
        return entries.iterator();
    }
}
