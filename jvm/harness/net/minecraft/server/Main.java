package net.minecraft.server;

import dev.ravel.jvm.harness.Entry;

/**
 * Stands in for the dedicated server distribution's entry point.
 *
 * <p>The other half of the side rule. The published server jar is a bundler, so
 * this class is not on any loader at premain and arrives only once the bundler
 * has extracted the server it ships; loading it is what tells Ravel this process
 * is a dedicated server, and nothing about the class path could have.
 */
public final class Main {

    private Main() {
    }

    /**
     * Runs the driver this case named.
     *
     * @param arguments the driver's binary name, then the driver's own arguments
     * @throws Throwable whatever the driver threw
     */
    public static void main(String[] arguments) throws Throwable {
        Entry.run(arguments);
    }
}
