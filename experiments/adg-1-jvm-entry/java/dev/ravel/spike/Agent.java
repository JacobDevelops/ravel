package dev.ravel.spike;

import java.lang.instrument.Instrumentation;

/**
 * Minimal instrumentation entry point.
 *
 * <p>Arguments are {@code <target class>:<target method>:<callback method>}. The
 * agent adds one transformer and nothing else, so its cost is the cost of the
 * entry mechanism rather than of a framework.
 */
public final class Agent {

    private static Transformer transformer;

    private Agent() {
    }

    public static void premain(String arguments, Instrumentation instrumentation) {
        String[] parts = arguments.split(":");
        if (parts.length != 3) {
            throw new IllegalArgumentException("expected <class>:<method>:<callback>");
        }
        transformer = new Transformer(parts[0], parts[1], parts[2]);
        instrumentation.addTransformer(transformer, false);
    }

    /** True once the pinned target class was rewritten by this agent. */
    public static boolean transformed() {
        return transformer != null && transformer.transformed();
    }
}
