package net.minecraft.server;

import dev.ravel.jvm.harness.Entry;

/** Loading this distribution entry point selects the dedicated-server side. */
public final class Main {

    private Main() {
    }

    public static void main(String[] arguments) throws Throwable {
        Entry.run(arguments);
    }
}
