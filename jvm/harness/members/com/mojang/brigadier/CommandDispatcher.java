package com.mojang.brigadier;

import java.util.ArrayList;
import java.util.List;

/**
 * Stands in for the command tree {@code Commands.dispatcher} holds.
 *
 * <p>Declared without a type parameter on purpose. The pinned descriptor is
 * {@code Lcom/mojang/brigadier/CommandDispatcher;}, which is the erased form the
 * JVM links on, so a stand-in that resolves that descriptor needs nothing else.
 */
public final class CommandDispatcher {

    private final List<String> registered = new ArrayList<>();

    public void register(String name) {
        registered.add(name);
    }

    /** How many commands the built tree holds, which is what reaching it is for. */
    public int size() {
        return registered.size();
    }
}
