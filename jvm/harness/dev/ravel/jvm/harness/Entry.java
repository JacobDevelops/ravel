package dev.ravel.jvm.harness;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

/** Invokes unnamed-package drivers reflectively from a distribution entry point. */
public final class Entry {

    private Entry() {
    }

    /**
     * Runs {@code arguments[0]}'s {@code main} with the arguments after it.
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
