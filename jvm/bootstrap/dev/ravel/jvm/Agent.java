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

/**
 * Ravel's agent, started from {@link Boot} once its jar is on every loader.
 *
 * <p>This is a Ravel artefact, not a mod loader. It installs one transformer,
 * loads one native library, and prints one line at shutdown. It takes no
 * arguments: what to rewrite comes from the generated {@link Hook} class in this
 * same jar, which is the only thing that knows what the pinned game version
 * resolved to.
 */
public final class Agent {

    /**
     * The client distribution's own entry point, in internal form.
     *
     * <p>Named through {@link GameNames} rather than spelled here: it is
     * unobfuscated under both tracked versions, and a literal would be one
     * version's spelling asserted over every other.
     */
    private static final String CLIENT_ENTRY_POINT =
            GameNames.CLIENT_ENTRY_POINT.replace('.', '/');

    /** The dedicated server distribution's own entry point, in internal form. */
    private static final String SERVER_ENTRY_POINT =
            GameNames.SERVER_ENTRY_POINT.replace('.', '/');

    /** The code {@code ravel-bootstrap}'s {@code side} reads as the client. */
    private static final int CLIENT = 0;

    /** The code it reads as the dedicated server. */
    private static final int DEDICATED_SERVER = 1;

    /** No entry point has been seen, so no side has been named. */
    static final int UNNAMED = -1;

    private static volatile Transformer transformer;

    /** Where the agent jar sits, so the screen jar can be found beside it. */
    private static volatile File agentJar;

    /**
     * The loader that defined the entry point this launch was started at.
     *
     * <p>Every game class is resolved through this one rather than through the
     * system loader. On the client the two are the same object, which
     * {@code evidence/m3/class-loader-identity.json} measured. On the published
     * dedicated server they are not: that jar is a bundler and the game it
     * extracts is loaded by a child of the platform loader, where a lookup made
     * through the system loader answers {@code ClassNotFoundException}.
     */
    private static volatile ClassLoader game;

    /**
     * The distribution entry point the transformer saw, or {@code null}.
     *
     * <p>Held apart from {@link #side} so the shutdown report can tell a process
     * that was never the game from one that was and whose prologue did not run.
     */
    private static final AtomicReference<String> entryPoint = new AtomicReference<>();

    /** Released once the entrant that won {@link #side} has finished installing. */
    private static final CountDownLatch installed = new CountDownLatch(1);

    /**
     * The side this launch was named, or {@link #UNNAMED}.
     *
     * <p>Written exactly once, by whichever thread enters the first entry point,
     * and read at shutdown to say whether this process was the game at all.
     */
    private static final AtomicInteger side = new AtomicInteger(UNNAMED);

    private Agent() {
    }

    /**
     * Installs Ravel's transformer before the game's main class is loaded.
     *
     * <p>It installs nothing else. Which distribution this process is cannot be
     * known here, and every rule about a mod set is a rule about a side, so the
     * set is composed from {@link #entered} instead.
     *
     * @param jar the agent jar {@link Boot} was loaded from, which a class this
     *     loader defined can no longer ask for itself
     * @param instrumentation the JVM's own instrumentation service
     */
    public static void start(String jar, Instrumentation instrumentation) {
        agentJar = new File(jar);

        // Load the library before installing the transformer, so a missing or
        // unloadable library fails at startup rather than at the first hook. It
        // is loaded from Hook's own initializer, which is what binds it to the
        // loader that declares the native methods.
        Hook.initialize();

        boolean reportTargets = Boolean.getBoolean("ravel.report.targets");
        Transformer installedTransformer = new Transformer(Hook.TARGETS, reportTargets);
        transformer = installedTransformer;
        instrumentation.addTransformer(installedTransformer, false);

        // Opt-in profiling, off unless this launch asked for it. Handed across
        // once, here, because a system property is Java's to read and the
        // collectors are Rust's to hold; reading it once is what lets the off
        // path stay a single branch rather than an atomic on every crossing.
        Hook.ravelEnableProfiling(System.getProperty("ravel.profiling"));
        Runtime.getRuntime().addShutdownHook(new Thread(Agent::report, "ravel-report"));
    }

    /**
     * The side a class the transformer is handed would name, or {@link #UNNAMED}.
     *
     * <p>Nothing at premain says which distribution this process is. The command
     * line carries no program arguments, the only property the game sets is
     * {@code java.awt.headless}, and asking what the JVM can load answers wrongly
     * on the published server jar, which is a bundler: at premain the only class
     * on any loader is the bundler's own, and the server's real jars arrive later
     * under a loader of its making.
     *
     * <p>What does say it is the entry point the process was started at. The
     * transformer is shown every class in every loader before it initialises, so
     * this is asked of every one of them and answers for exactly two.
     *
     * @param internalName the class the transformer was handed
     */
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
     * Composes and installs the mod set, from the entry point's own {@code main}.
     *
     * <p>Called by the prologue the transformer prepends to that method rather
     * than from inside the transformation itself, and the difference is not
     * stylistic: a class loaded from within a transformer never reaches the
     * transformer, so initialising there would leave every class it touched
     * unrewritten. It runs before the entry point's first statement, which is
     * before any game code.
     *
     * <p>A JVM that enters neither entry point — a launcher, a bundler that has
     * not extracted yet, this repository's own harness — never calls this. That
     * is not a refusal: nothing has gone wrong in a process that is not the game.
     * Every crossing into the runtime answers empty until a set is installed, and
     * {@link #report()} says so at shutdown.
     *
     * @param named the code this launch's entry point named the side by
     * @param entry the entry point itself, whose defining loader is the loader
     *     every game class is resolved through
     */
    public static void entered(int named, Class<?> entry) {
        // Class loading is concurrent and an entry point may be entered twice by
        // a launcher that reflects into it, so installing once is a
        // compare-and-set rather than an assumption about who got here first.
        if (!side.compareAndSet(UNNAMED, named)) {
            // The loser waits rather than returning, because returning would let
            // the game's first statement run against a half-installed set.
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

    /** Blocks until the entrant that won the install has finished it. */
    private static void await() {
        boolean interrupted = false;
        while (true) {
            try {
                installed.await();
                break;
            } catch (InterruptedException interrupt) {
                // An interrupt is not a reason to return early; the flag is
                // restored once the install is complete so whoever sent it is
                // still owed the news.
                interrupted = true;
            }
        }
        if (interrupted) {
            Thread.currentThread().interrupt();
        }
    }

    /**
     * The loader every game class is resolved through.
     *
     * <p>Before a side is named there is no game loader and nothing resolves a
     * game class, so the system loader is what this answers: it is where the
     * client's game is, and it is what a lookup made outside a launch means.
     */
    static ClassLoader gameLoader() {
        ClassLoader named = game;
        return named == null ? ClassLoader.getSystemClassLoader() : named;
    }

    /** The agent jar this launch loaded, so a sibling can be found beside it. */
    static File jar() {
        return agentJar;
    }

    /** Records the distribution entry point the transformer was handed. */
    static void sawEntryPoint(String internalName) {
        entryPoint.compareAndSet(null, internalName.replace('/', '.'));
    }

    /** Classes this agent rewrote. */
    public static int transformedClasses() {
        Transformer installed = transformer;
        return installed == null ? 0 : installed.transformedClasses();
    }

    /**
     * Whether the target calling {@code hookMethod} was actually rewritten.
     *
     * <p>The installed-prologue marker is read through the agent, which holds
     * the transformer instance. An agent that installed no
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
     * <p>The shutdown report records whether transformations actually happened,
     * rather than merely whether the JVM accepted the launch arguments.
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
        String entry = entryPoint.get();
        if (entry == null) {
            // The one launch this line is for is the one where nothing went
            // wrong and nothing happened either: no entry point loaded, so no
            // side was named and no set was composed. Without it that process is
            // indistinguishable from one whose mods all failed silently.
            line = line + "[ravel] no distribution entry point loaded, so no side was named"
                    + " and no mod set was installed\n";
        } else if (side.get() == UNNAMED) {
            // And this one is the launch that went wrong in the way nothing else
            // reports: the entry point loaded and was rewritten, and the
            // prologue never reached this class — a prologue that failed to link
            // or threw before the compare-and-set. Ravel is installed and inert,
            // and without this line that is what a working launch looks like.
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
