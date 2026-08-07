package dev.ravel.jvm;

import java.io.File;
import java.lang.instrument.Instrumentation;
import java.lang.reflect.InvocationTargetException;
import java.net.URISyntaxException;
import java.util.jar.JarFile;

/**
 * Puts the generated settings screen where the game's own loader can find it.
 *
 * <p>ADG-1's amendment named two ways to get a screen class into the game
 * process and chose this one: the agent appends a second jar to the class path
 * the game already loads from, rather than a launcher putting it there. That
 * keeps ADG-5's property that Ravel adds JVM arguments to a stock instance and
 * nothing else, because a class path cannot be appended through a JVM argument
 * on a launcher that builds its own and takes the last one.
 *
 * <p>The jar is found beside the agent's own jar rather than named in an
 * argument, so {@link Agent#premain} keeps rejecting arguments and there is
 * still nothing to configure.
 *
 * <p>Appending is not defining. This runs at {@code premain} and only widens
 * where the loader may look; it loads no class, so no game class is forced to
 * load before the client bootstraps. Defining the screen is lazy and belongs to
 * whatever hook first needs it, because a {@code Screen} subclass cannot be
 * defined without loading {@code Screen} itself.
 *
 * <p>That the appended entry is then found is not a guess:
 * {@code evidence/m3/class-loader-identity.json} measured the loader that loads
 * a rewritten {@code net.minecraft} class to be the same object as
 * {@code ClassLoader.getSystemClassLoader()}, which is the loader this method
 * widens.
 */
final class Screens {

    /** The jar the generated screen ships in, beside the agent's own. */
    static final String SCREEN_JAR = "ravel-screen.jar";

    private static volatile boolean appended;

    private Screens() {
    }

    /**
     * Appends the screen jar to the system class loader's search path.
     *
     * <p>A missing jar is not a failure. The screen is an optional artefact and
     * a launch without it is a launch with no settings screen, which is a
     * working game; refusing to start would turn a missing convenience into an
     * outage. {@link #isAppended} is what a caller asks before it tries to
     * define anything.
     *
     * @param instrumentation the JVM's own instrumentation service
     */
    static void append(Instrumentation instrumentation) {
        File jar = beside(SCREEN_JAR);
        if (jar == null) {
            diagnose("could not locate " + SCREEN_JAR + " beside the agent jar");
            return;
        }
        if (!jar.isFile()) {
            diagnose(SCREEN_JAR + " is missing or not a regular file beside the agent jar");
            return;
        }
        try (JarFile opened = new JarFile(jar)) {
            instrumentation.appendToSystemClassLoaderSearch(opened);
            appended = true;
        } catch (java.io.IOException unreadable) {
            appended = false;
            diagnose(SCREEN_JAR + " is unreadable or invalid ("
                    + unreadable.getClass().getName() + ")");
        }
    }

    /** Whether the screen jar was appended and may be defined from. */
    static boolean isAppended() {
        return appended;
    }

    /**
     * Builds the settings screen for one mod, loading its class on first use.
     *
     * <p>This is the lazy half, and it is lazy on purpose. The screen extends
     * {@code net.minecraft.client.gui.screens.Screen}, so defining it loads
     * {@code Screen}; doing that from {@code premain} would pull the game's
     * screen stack in before the client bootstraps. Calling this from a hook
     * means {@code Screen} is already loaded by the time it matters.
     *
     * <p>Reflection rather than a direct reference, because this class is in the
     * agent jar and ADG-1 requires that jar to name no game type — and
     * {@code RavelSettingsScreen}'s supertype is one. The screen is loaded
     * through the system class loader, which
     * {@code evidence/m3/class-loader-identity.json} measured to be the same
     * object as the loader that loads the game's own classes, so the superclass
     * resolves against the real {@code Screen} rather than a second copy.
     *
     * @param owner the canonical mod id whose settings to show
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be
     *     used
     */
    static Object open(String owner) {
        return build("dev.ravel.jvm.screen.RavelSettingsScreen", owner);
    }

    /**
     * Builds the mod list and dependency diagnostics screen.
     *
     * <p>M3-W5's screen is ADG-6's mechanism rather than a second one, and that
     * holds here too: it loads the same lazy way, out of the same jar, through
     * the same reflection, for the same ADG-1 reason.
     *
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openMods() {
        return build("dev.ravel.jvm.screen.RavelModsScreen", null);
    }

    /**
     * Builds the opt-in profiling report screen.
     *
     * <p>M3-W6's report is ADG-6's mechanism a third time, so it loads the same
     * lazy way, out of the same jar, through the same reflection, for the same
     * ADG-1 reason.
     *
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openProfiling() {
        return build("dev.ravel.jvm.screen.RavelProfilingScreen", null);
    }

    /** Adds Ravel's entry to the title screen through the optional screen jar. */
    static int addTitleMenu(Object titleScreen, int bottom, int spacing) {
        if (!appended) {
            return bottom;
        }
        String name = "dev.ravel.jvm.screen.RavelMenu";
        try {
            Class<?> bridge = Class.forName(name, true, ClassLoader.getSystemClassLoader());
            bridge.getMethod("add", Object.class, int.class, int.class)
                    .invoke(null, titleScreen, bottom, spacing);
            return bottom + spacing;
        } catch (InvocationTargetException failed) {
            diagnoseFailure(name, failed.getCause() == null ? failed : failed.getCause());
        } catch (ReflectiveOperationException | LinkageError unusable) {
            diagnoseFailure(name, unusable);
        }
        return bottom;
    }

    /**
     * Loads one screen class and calls its {@code open}.
     *
     * <p>Reflection rather than a direct reference, because this class is in the
     * agent jar and ADG-1 requires that jar to name no game type — and every
     * screen's supertype is one.
     *
     * @param name the screen class to load
     * @param owner the mod id to open it for, or {@code null} for a screen that
     *     takes none
     */
    private static Object build(String name, String owner) {
        if (!appended) {
            return null;
        }
        try {
            Class<?> screen = Class.forName(name, true, ClassLoader.getSystemClassLoader());
            if (owner == null) {
                return screen.getMethod("open").invoke(null);
            }
            return screen.getMethod("open", String.class).invoke(null, owner);
        } catch (InvocationTargetException failed) {
            diagnoseFailure(name, failed.getCause() == null ? failed : failed.getCause());
            return null;
        } catch (ReflectiveOperationException | LinkageError unusable) {
            diagnoseFailure(name, unusable);
            return null;
        }
    }

    private static void diagnoseFailure(String name, Throwable failure) {
        diagnose("screen class could not be opened: " + name + " ("
                + failure.getClass().getName() + ")");
    }

    private static void diagnose(String message) {
        Agent.diagnose(message);
    }

    /**
     * Resolves {@code name} beside the jar this class was loaded from.
     *
     * @return the file, or {@code null} if this class did not come from a jar on
     *     disk, which is what happens when the agent runs from a directory
     */
    private static File beside(String name) {
        try {
            var source = Screens.class.getProtectionDomain().getCodeSource();
            if (source == null) {
                return null;
            }
            File self = new File(source.getLocation().toURI());
            File parent = self.getParentFile();
            return parent == null ? null : new File(parent, name);
        } catch (URISyntaxException | IllegalArgumentException notAFile) {
            return null;
        }
    }
}
