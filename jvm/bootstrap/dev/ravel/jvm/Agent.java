package dev.ravel.jvm;

import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.instrument.Instrumentation;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/** Installs the generated hooks and native runtime once Boot exposes the agent jar. */
public final class Agent {

    /** Distribution entry points use generated, version-specific names. */
    private static final String CLIENT_ENTRY_POINT =
            GameNames.CLIENT_ENTRY_POINT.replace('.', '/');

    private static final String SERVER_ENTRY_POINT =
            GameNames.SERVER_ENTRY_POINT.replace('.', '/');

    private static final int CLIENT = 0;

    private static final int DEDICATED_SERVER = 1;

    static final int UNNAMED = -1;

    private static volatile Transformer transformer;

    private static volatile File agentJar;

    // Dedicated-server game classes live in the bundler's child loader, not the system loader.
    private static volatile ClassLoader game;

    /** Distinguishes an unseen game entry point from one whose prologue never ran. */
    private static final AtomicReference<String> entryPoint = new AtomicReference<>();

    private static final CountDownLatch installed = new CountDownLatch(1);

    /** Named once by the first entry-point invocation. */
    private static final AtomicInteger side = new AtomicInteger(UNNAMED);

    private Agent() {
    }

    /** Installs the transformer; composition waits until a game entry point names the side. */
    public static void start(String jar, Instrumentation instrumentation) {
        agentJar = new File(jar);

        // Hook must load the native library in the loader that declares its native methods.
        Hook.initialize();

        boolean reportTargets = Boolean.getBoolean("ravel.report.targets");
        Transformer installedTransformer = new Transformer(Hook.TARGETS, reportTargets);
        transformer = installedTransformer;
        instrumentation.addTransformer(installedTransformer, false);

        // Read profiling once so the disabled crossing path needs no atomic lookup.
        Hook.ravelEnableProfiling(System.getProperty("ravel.profiling"));
        Runtime.getRuntime().addShutdownHook(new Thread(Agent::report, "ravel-report"));
    }

    /** Names the distribution from its entry point, not the launcher's class path. */
    static int sideOf(String internalName) {
        if (CLIENT_ENTRY_POINT.equals(internalName)) {
            return CLIENT;
        }
        if (SERVER_ENTRY_POINT.equals(internalName)) {
            return DEDICATED_SERVER;
        }
        return UNNAMED;
    }

    /**
     * Composes the mod set in the entry-point prologue, before game code runs.
     * Classes loaded inside a transformer bypass transformation, so initialization must wait.
     */
    public static void entered(int named, Class<?> entry) {
        if (!side.compareAndSet(UNNAMED, named)) {
            // Do not let another entrant run game code against a half-installed set.
            await();
            return;
        }
        game = entry.getClassLoader();
        try {
            Hook.ravelInitialize(named);
            Screens.install();
        } finally {
            installed.countDown();
        }
    }

    private static void await() {
        boolean interrupted = false;
        while (true) {
            try {
                installed.await();
                break;
            } catch (InterruptedException interrupt) {
                // Finish waiting before restoring the caller's interrupt status.
                interrupted = true;
            }
        }
        if (interrupted) {
            Thread.currentThread().interrupt();
        }
    }

    /** Uses the system loader until the game entry point supplies its defining loader. */
    static ClassLoader gameLoader() {
        ClassLoader named = game;
        return named == null ? ClassLoader.getSystemClassLoader() : named;
    }

    static File jar() {
        return agentJar;
    }

    static void sawEntryPoint(String internalName) {
        entryPoint.compareAndSet(null, internalName.replace('/', '.'));
    }

    public static int transformedClasses() {
        Transformer installed = transformer;
        return installed == null ? 0 : installed.transformedClasses();
    }

    /** Whether the target's hook prologue was installed; false without a transformer. */
    public static boolean isInstalled(String hookMethod) {
        Transformer installed = transformer;
        return installed != null && installed.isInstalled(hookMethod);
    }

    public static int rewrittenMethods() {
        Transformer installed = transformer;
        return installed == null ? 0 : installed.rewrittenMethods();
    }

    /** Loader evidence is opt-in because identity hashes differ between runs. */
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

    /** Writes directly to stdout's descriptor because the game's logger may have stopped. */
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
        String entry = entryPoint.get();
        if (entry == null) {
            line = line + "[ravel] no distribution entry point loaded, so no side was named"
                    + " and no mod set was installed\n";
        } else if (side.get() == UNNAMED) {
            line = line + "[ravel] the distribution entry point " + entry
                    + " loaded and Ravel's prologue did not run, so no side was named"
                    + " and no mod set was installed\n";
        }
        line = line + transformationFailures();
        write(FileDescriptor.out, line);
    }

    static void diagnose(String message) {
        write(FileDescriptor.err, "[ravel] " + message + "\n");
    }

    private static void write(FileDescriptor descriptor, String line) {
        // Closing this stream would close the process's descriptor.
        OutputStream out = new FileOutputStream(descriptor);
        try {
            out.write(line.getBytes(StandardCharsets.UTF_8));
            out.flush();
        } catch (IOException unwritable) {
            // A failed diagnostic write has nowhere more reliable to report to.
        }
    }
}
