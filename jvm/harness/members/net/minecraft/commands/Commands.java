package net.minecraft.commands;

import com.mojang.brigadier.CommandDispatcher;

/**
 * Stands in for the command table, carrying both members the inventory pins.
 *
 * <p>Vanilla builds the tree in this class's constructor, so the stand-in does
 * too: {@code dispatcher} is already populated by the time anything reads it,
 * which is the state the field hook exists to reach.
 */
public final class Commands {

    private static int performed;

    private final CommandDispatcher dispatcher = new CommandDispatcher();

    public Commands() {
        dispatcher.register("ravel");
    }

    /** How many times the vanilla body ran. */
    public static int performed() {
        return performed;
    }

    public void performPrefixedCommand(CommandSourceStack source, String command) {
        performed++;
    }
}
