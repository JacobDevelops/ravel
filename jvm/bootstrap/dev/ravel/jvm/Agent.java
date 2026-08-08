package dev.ravel.jvm;

import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.instrument.Instrumentation;
import java.nio.charset.StandardCharsets;

/**
 * Ravel's premain entry point.
 *
 * <p>This is a Ravel artefact, not a mod loader. It installs one transformer,
 * loads one native library, and prints one line at shutdown. It takes no
 * arguments: what to rewrite comes from the generated {@link Hook} class in this
 * same jar, which is the only thing that knows what the pinned game version
 * resolved to.
 */
public final class Agent {

    private static volatile Transformer transformer;

    private Agent() {
    }

    /**
     * Installs Ravel's transformer before the game's main class is loaded.
     *
     * @param arguments must be empty; the agent is configured by what it ships with
     * @param instrumentation the JVM's own instrumentation service
     */
    public static void premain(String arguments, Instrumentation instrumentation) {
        if (arguments != null && !arguments.isEmpty()) {
            throw new IllegalArgumentException("the Ravel agent takes no arguments");
        }

        // Load the library before installing the transformer, so a missing or
        // unloadable library fails at startup rather than at the first hook. It
        // is loaded from Hook's own initializer, which is what binds it to the
        // loader that declares the native methods.
        Hook.initialize();

        boolean reportTargets = Boolean.getBoolean("ravel.report.targets");
        Transformer installed = new Transformer(Hook.TARGETS, reportTargets);
        transformer = installed;
        instrumentation.addTransformer(installed, false);

        // Opt-in profiling, off unless this launch asked for it. Handed across
        // once, here, because a system property is Java's to read and the
        // collectors are Rust's to hold; reading it once is what lets the off
        // path stay a single branch rather than an atomic on every crossing.
        Hook.ravelEnableProfiling(System.getProperty("ravel.profiling"));
        Hook.ravelInitialize();
        Screens.append(instrumentation);
        Runtime.getRuntime().addShutdownHook(new Thread(Agent::report, "ravel-report"));
    }

    /** Classes this agent rewrote. */
    public static int transformedClasses() {
        Transformer installed = transformer;
        return installed == null ? 0 : installed.transformedClasses();
    }

    /**
     * Whether the target calling {@code hookMethod} was actually rewritten.
     *
     * <p>ADG-7's installed-prologue marker, read through the agent because the
     * transformer instance is the agent's to hold. An agent that installed no
     * transformer has rewritten nothing, so it answers {@code false} rather than
     * throwing — the reader refuses a world load on a false either way, and a
     * marker that can throw is one the open path would have to guard.
     *
     * @param hookMethod the generated hook name the target is rewritten to call
     */
    public static boolean isInstalled(String hookMethod) {
        Transformer installed = transformer;
        return installed != null && installed.isInstalled(hookMethod);
    }

    /** Methods this agent rewrote. */
    public static int rewrittenMethods() {
        Transformer installed = transformer;
        return installed == null ? 0 : installed.rewrittenMethods();
    }

    /**
     * The loader that loaded the first rewritten class, or {@code null}.
     *
     * <p>Reported only when {@code -Dravel.report.loader=true} is set. It is off
     * by default because an ordinary launch prints exactly the one line
     * {@code evidence/m2/prism-launch.json} recorded, and because the identity
     * hashes in this line differ on every run, which an exact-output contract
     * cannot hold. A measurement run turns it on and reads it once.
     */
    private static String loaderIdentity() {
        Transformer installed = transformer;
        if (installed == null || !Boolean.getBoolean("ravel.report.loader")) {
            return null;
        }
        return installed.loaderIdentity();
    }

    private static String transformationFailures() {
        Transformer installed = transformer;
        return installed == null ? "" : installed.transformationFailures();
    }

    private static String rewrittenTargets() {
        Transformer installed = transformer;
        return installed == null ? null : installed.rewrittenTargets();
    }

    /**
     * Prints what this launch actually did.
     *
     * <p>ADG-5 proved the JVM accepted Ravel's arguments and the client ran, but
     * not that the rewrite happened, because nothing reported it. One line at
     * shutdown is what makes an ordinary launch direct evidence.
     *
     * <p>The line goes to the process's own standard output rather than to
     * {@code System.out}. The game replaces {@code System.out} with its logger
     * and that logger has stopped by the time a shutdown hook runs, so a
     * {@code System.out.println} here reaches nothing in the one place the line
     * exists to be read. Writing the file descriptor is what makes the report
     * survive a real launch; M2's launch is what found that out.
     */
    public static void report() {
        long invocations;
        try {
            invocations = Hook.nativeInvocations();
        } catch (UnsatisfiedLinkError absent) {
            invocations = -1L;
        }
        String line = "[ravel] rewrote " + transformedClasses() + " classes and "
                + rewrittenMethods() + " methods; " + invocations
                + " hook invocations reached native code\n";
        String loaders = loaderIdentity();
        if (loaders != null) {
            line = line + "[ravel] " + loaders + "\n";
        }
        String targets = rewrittenTargets();
        if (targets != null) {
            line = line + "[ravel] rewritten targets: " + targets + "\n";
        }
        line = line + transformationFailures();
        write(FileDescriptor.out, line);
    }

    static void diagnose(String message) {
        write(FileDescriptor.err, "[ravel] " + message + "\n");
    }

    private static void write(FileDescriptor descriptor, String line) {
        // Not closed: the descriptor is the process's, and closing it would take
        // the stream away from whatever else is still using it.
        OutputStream out = new FileOutputStream(descriptor);
        try {
            out.write(line.getBytes(StandardCharsets.UTF_8));
            out.flush();
        } catch (IOException unwritable) {
            // A failed diagnostic write has nowhere more reliable to report to.
        }
    }
}
