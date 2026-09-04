package dev.ravel.jvm.harness;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

/**
 * Runs the driver a harness case names, from the entry point that named the side.
 *
 * <p>Ravel names the running side from the first distribution entry point the
 * transformer sees, so a case that expects a mod set has to enter at one, exactly
 * as a launch of the game does. The entry point stand-ins are in a package and
 * every driver is in the unnamed one, which no source in a package can name, so
 * the driver is named as an argument and reached reflectively.
 */
public final class Entry {

    private Entry() {
    }

    /**
     * Runs {@code arguments[0]}'s {@code main} with the arguments after it.
     *
     * <p>What the driver threw is rethrown as it stands rather than wrapped, so a
     * case reading the JVM's own abort reads the same text it read before the
     * driver moved behind this.
     *
     * @param arguments the driver's binary name, then the driver's own arguments
     * @throws Throwable whatever the driver threw
     */
    public static void run(String[] arguments) throws Throwable {
        if (arguments.length == 0) {
            throw new IllegalArgumentException("no harness driver was named");
        }
        Method main = Class.forName(arguments[0]).getMethod("main", String[].class);
        String[] rest = new String[arguments.length - 1];
        System.arraycopy(arguments, 1, rest, 0, rest.length);
        try {
            main.invoke(null, (Object) rest);
        } catch (InvocationTargetException thrown) {
            throw thrown.getCause();
        }
    }
}
