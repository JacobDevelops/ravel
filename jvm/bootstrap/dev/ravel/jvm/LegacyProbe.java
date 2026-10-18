package dev.ravel.jvm;

import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Set;

/** Explicit acceptance-only probes execute on the same thread as the actual game callback. */
final class LegacyProbe {
    private static final boolean ENABLED = Boolean.getBoolean("ravel.legacy.acceptance");
    private static final Set<Object> CLIENTS = Collections.newSetFromMap(new IdentityHashMap<>());
    private static final Set<Object> SERVERS = Collections.newSetFromMap(new IdentityHashMap<>());

    private LegacyProbe() {
    }

    static void run(Object receiver, boolean server) throws ReflectiveOperationException {
        if (!ENABLED) {
            return;
        }
        String name = System.getProperty(server ? "ravel.legacy.serverProbe" : "ravel.legacy.clientProbe");
        if (name == null || name.isEmpty()) {
            return;
        }
        Set<Object> completed = server ? SERVERS : CLIENTS;
        synchronized (completed) {
            if (completed.contains(receiver)) {
                return;
            }
        }
        Class<?> probe = Class.forName(name, true, LegacyRuntime.class.getClassLoader());
        Object result = probe.getMethod("exercise", Object.class).invoke(null, receiver);
        if (!Boolean.FALSE.equals(result)) {
            synchronized (completed) {
                completed.add(receiver);
            }
        }
    }
}
