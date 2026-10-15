package net.minecraft.client.main;

import dev.ravel.jvm.harness.Entry;

/** Loading this distribution entry point selects the client side. */
public final class Main {

    private Main() {
    }

    public static void main(String[] arguments) throws Throwable {
        Entry.run(arguments);
    }
}
