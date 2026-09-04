package net.minecraft.client.main;

import dev.ravel.jvm.harness.Entry;

/**
 * Stands in for the client distribution's entry point.
 *
 * <p>Ravel names the running side from the first distribution entry point the
 * transformer is shown, and this is the client's. Loading it is therefore the
 * whole of what makes a harness case a client, which is why every case that
 * expects a mod set enters here and names its driver as an argument.
 *
 * <p>The name is what matters and nothing is called on it, so a case that leaves
 * it out of its class path is what proves the side is read rather than assumed.
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
