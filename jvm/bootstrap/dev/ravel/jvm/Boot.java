package dev.ravel.jvm;

import java.io.File;
import java.io.IOException;
import java.lang.instrument.Instrumentation;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.URISyntaxException;
import java.security.CodeSource;
import java.util.jar.JarFile;

/**
 * The JVM premain entry; modern bundled launches define only this Ravel class in the system loader.
 *
 * <p>Ravel's classes have to be visible from every loader in the process, and
 * the {@code -javaagent} jar is on the system class path, which is not. The
 * published server jar builds the loader the game runs under as
 * {@code new URLClassLoader(urls, getClass().getClassLoader().getParent())} —
 * the parent is the platform loader, so the system class path is invisible to
 * every game class, and the {@code invokestatic dev/ravel/jvm/Agent.entered}
 * the transformer prepends to the entry point threw {@code NoClassDefFoundError}
 * and killed the first real dedicated server this was attached to.
 *
 * <p>So the jar is appended to the bootstrap loader's search path here, before
 * any other Ravel class is named. Bootstrap is every loader's furthest parent,
 * including one parented to the platform loader, so after the append there is
 * exactly one copy of every Ravel class and every loader delegates to it. This
 * class is the exception and is why it is separate and empty: the JVM defines
 * the {@code Premain-Class} through the system loader before the append can
 * happen, so anything it touched before then would be defined twice — once by
 * the system loader, once by bootstrap — and the two copies would hold separate
 * static state. It touches nothing, and reaches {@link Agent} by asking the
 * bootstrap loader for it by name.
 *
 * <p>The jar is passed on rather than found again from there. A class the
 * bootstrap loader defined has no {@code CodeSource}, so nothing after this
 * point can say where the agent jar is, and {@code ravel-screen.jar} is found
 * beside it.
 *
 * <p>The legacy platform instead resolves this jar as the automatic module
 * {@code dev.ravel.agent} and deliberately delegates Ravel to its system parent.
 * A named Boot therefore starts Agent through its own loader, without appending
 * the jar. Hook stays in that named module for a narrow native-access grant;
 * unnamed game libraries, including unsupported narrator/JNA, remain ungranted.
 */
public final class Boot {

    /**
     * The appended jar, held open for the life of the process.
     *
     * <p>{@code appendToBootstrapClassLoaderSearch} takes a jar the caller keeps
     * open: the loader reads entries out of it whenever a class it holds is
     * first named, which is at any point in a launch.
     */
    private static volatile JarFile appended;

    private Boot() {
    }

    /**
     * Starts Ravel in its named module, or appends the unnamed jar to bootstrap first.
     *
     * @param arguments must be empty; the agent is configured by what it ships with
     * @param instrumentation the JVM's own instrumentation service
     * @throws IOException if the agent jar cannot be read
     * @throws URISyntaxException if the agent was not loaded from a file
     * @throws ReflectiveOperationException if the appended jar holds no agent
     */
    public static void premain(String arguments, Instrumentation instrumentation)
            throws IOException, URISyntaxException, ReflectiveOperationException {
        if (arguments != null && !arguments.isEmpty()) {
            throw new IllegalArgumentException("the Ravel agent takes no arguments");
        }
        CodeSource source = Boot.class.getProtectionDomain().getCodeSource();
        if (source == null) {
            throw new IllegalStateException(
                    "the Ravel agent was not loaded from a jar, so nothing can be appended"
                            + " to the bootstrap class loader search");
        }
        File self = new File(source.getLocation().toURI());
        ClassLoader loader = null;
        if (Boot.class.getModule().isNamed()) {
            // The legacy platform delegates Ravel to this resolved automatic module.
            // Keeping Hook here permits a module-scoped JNI grant, never ALL-UNNAMED.
            loader = Boot.class.getClassLoader();
        } else {
            JarFile jar = new JarFile(self);
            appended = jar;
            instrumentation.appendToBootstrapClassLoaderSearch(jar);
        }
        Class<?> agent = Class.forName("dev.ravel.jvm.Agent", true, loader);
        Method start = agent.getMethod("start", String.class, Instrumentation.class);
        try {
            start.invoke(null, self.getAbsolutePath(), instrumentation);
        } catch (InvocationTargetException failed) {
            Throwable cause = failed.getCause();
            if (cause instanceof RuntimeException runtime) {
                throw runtime;
            }
            if (cause instanceof Error error) {
                throw error;
            }
            throw new IllegalStateException(cause);
        }
    }
}
