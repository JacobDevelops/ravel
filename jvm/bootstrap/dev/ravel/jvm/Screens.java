package dev.ravel.jvm;

import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.net.MalformedURLException;
import java.net.URL;
import java.net.URLClassLoader;
import java.util.jar.JarFile;

/**
 * Puts the generated settings screen where the game's own loader can find it.
 *
 * <p>Every screen class is named through {@link ScreenNames} rather than by a
 * literal here. The screens are generated per version, and a version that
 * obfuscates the game into the unnamed package generates them there too, because
 * a Java source in a package cannot name a supertype that has none.
 *
 * <p>The agent carries a second jar so Ravel can add JVM arguments to a stock
 * instance without changing its class path. A launcher that builds its own
 * class path and takes the last one cannot have it appended through a JVM
 * argument.
 *
 * <p>The jar is found beside the agent's own jar rather than named in an
 * argument, so {@link Boot#premain} keeps rejecting arguments and there is
 * still nothing to configure.
 *
 * <p><b>A child loader rather than the system one.</b> The jar was appended with
 * {@code Instrumentation.appendToSystemClassLoaderSearch}, and the first real
 * 1.21.1 launch showed why that cannot stand: Mojang's client jar is signed and
 * an obfuscated version puts its classes in the unnamed package, which is where
 * the generated screens go too. Defining an unsigned {@code RavelMenu} into a
 * runtime package the system loader had already recorded signers for fails
 * {@code ClassLoader.checkCerts} with a {@code SecurityException}, and the mods
 * button threw on every click. A runtime package is a defining loader and a
 * package name together, so a loader of Ravel's own puts the screens in a
 * package of their own and the certificate check has nothing to compare.
 *
 * <p>The parent is {@link Agent#gameLoader()}, the loader that defined the entry
 * point, so a screen still resolves its game supertype to the class the game
 * itself loaded rather than to a second copy. On the client that loader is the
 * system loader, which {@code evidence/m3/class-loader-identity.json} measured;
 * on the published dedicated server it is a child of the platform loader and
 * the system loader holds no game class at all, so parenting to the system
 * loader there fails every game supertype with {@code ClassNotFoundException}.
 *
 * <p>Constructing the loader is not defining. This runs the moment the
 * transformer sees the distribution entry point, which is where the running side
 * first exists, and it loads no class — so no game class is forced to load
 * before the client bootstraps. Defining a screen is lazy and belongs to whatever hook first needs
 * it, because a {@code Screen} subclass cannot be defined without loading
 * {@code Screen} itself.
 */
final class Screens {

    /** The jar the generated screen ships in, beside the agent's own. */
    static final String SCREEN_JAR = "ravel-screen.jar";

    /**
     * The loader every Ravel class in the screen jar is defined by.
     *
     * <p>One instance for the process, because the screens and the sibling
     * saved-data class have to be one another's nestmates in the plain sense: a
     * second loader would define a second {@code RavelWorldData}, and the
     * {@code instanceof} that reads a stored aggregate back would answer false.
     */
    private static volatile ClassLoader screens;

    private Screens() {
    }

    /**
     * Opens the screen jar under a loader of Ravel's own.
     *
     * <p>A missing jar is not a failure. The screen is an optional artefact and
     * a launch without it is a launch with no settings screen, which is a
     * working game; refusing to start would turn a missing convenience into an
     * outage. {@link #isInstalled} is what a caller asks before it tries to
     * define anything.
     *
     * <p>The jar is opened before the loader is built, so a truncated or
     * non-jar file is reported here rather than as a class that cannot be found
     * much later.
     */
    static void install() {
        if (screens != null) {
            // Called once, from `Agent.entered`. A second call would replace a loader
            // that holds an open jar, and nothing would ever close the first: the
            // classes it defined stay reachable and so does the file handle. It
            // is a diagnostic rather than a close, because a loader whose classes
            // are already loaded cannot be closed safely and a second install is
            // a caller defect rather than a state to recover from.
            diagnose(SCREEN_JAR + " is already open; ignoring a second attempt to open it");
            return;
        }
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
            screens = loader(opened.getName(), Agent.gameLoader());
        } catch (java.io.IOException unreadable) {
            screens = null;
            diagnose(SCREEN_JAR + " is unreadable or invalid ("
                    + unreadable.getClass().getName() + ")");
        }
    }

    /**
     * The loader the screen jar is read through.
     *
     * <p>Package-private so the per-version Java compile can build the same
     * loader over the same jar rather than a second one written to look like it.
     * A check that models the mechanism proves nothing about the mechanism.
     *
     * <p>The parent is a parameter for the same reason. A launch hands it the
     * entry point's defining loader, which is where the game is; the check hands
     * it a loader holding a signed jar, which is the condition that made the
     * launch fail and which nothing on a build machine has otherwise.
     *
     * @param path the screen jar to read
     * @param parent where a class this loader does not hold is looked for
     */
    static ClassLoader loader(String path, ClassLoader parent) throws MalformedURLException {
        return new URLClassLoader(new URL[] {new File(path).toURI().toURL()}, parent);
    }

    /** Whether the screen jar was opened and may be defined from. */
    static boolean isInstalled() {
        return screens != null;
    }

    /**
     * One class out of the screen jar, defined by Ravel's own loader.
     *
     * @param name the class to load, as this version's names spell it
     */
    static Class<?> load(String name) throws ClassNotFoundException {
        return Class.forName(name, true, screens);
    }

    /**
     * Builds the settings screen for one mod.
     *
     * <p>The screen loads lazily from the optional jar through reflection.
     * The agent jar is compiled without game classes, including the screen's
     * supertype. The parent is the screen to close back to, typed {@code Object}
     * for the same reason.
     *
     * @param owner the opaque settings route supplied by the mod-list wire document
     * @param parent the screen to return to on close
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openSettings(String owner, Object parent) {
        return build(ScreenNames.RAVEL_SETTINGS_SCREEN,
                new Class<?>[] {String.class, Object.class}, owner, parent);
    }

    /**
     * Builds the opt-in profiling report screen.
     *
     * @param parent the screen to return to on close
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openProfiling(Object parent) {
        return build(ScreenNames.RAVEL_PROFILING_SCREEN, new Class<?>[] {Object.class}, parent);
    }

    /**
     * Builds the mod list and dependency diagnostics screen.
     *
     * <p>Loads reflectively from the optional screen jar.
     *
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openMods() {
        return build(ScreenNames.RAVEL_MODS_SCREEN, new Class<?>[0]);
    }

    /**
     * Builds the screen a refused world load leaves the player on.
     *
     * <p>{@link Persistence} has already refused the world before requesting
     * this screen, so a {@code null} here loses the explanation, not the
     * protection.
     *
     * @param diagnostic Rust's refusal text, which Java presents and never
     *     composes
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openWorldRefused(String diagnostic) {
        return build(ScreenNames.RAVEL_WORLD_REFUSED_SCREEN, new Class<?>[] {String.class}, diagnostic);
    }

    /** Adds Ravel's entry to the title screen through the optional screen jar. */
    static int addTitleMenu(Object titleScreen, int bottom, int spacing) {
        if (!isInstalled()) {
            return bottom;
        }
        String name = ScreenNames.RAVEL_MENU;
        try {
            Class<?> bridge = load(name);
            bridge.getMethod("add", Object.class, int.class, int.class)
                    .invoke(null, titleScreen, bottom, spacing);
            return bottom + spacing;
        } catch (InvocationTargetException failed) {
            diagnoseFailure(name, failed.getCause() == null ? failed : failed.getCause());
        } catch (ReflectiveOperationException | LinkageError | SecurityException unusable) {
            // SecurityException is what a refused define looks like. A screen
            // class the JVM will not admit costs the player a button; letting it
            // out of here costs them the click handler it was thrown inside.
            diagnoseFailure(name, unusable);
        }
        return bottom;
    }

    /**
     * Loads one screen class and calls its {@code open}.
     *
     * <p>Reflection avoids a direct reference to a screen whose supertype is a
     * game class: the agent jar compiles without game classes.
     *
     * @param name the screen class to load
     * @param types the parameter types of the {@code open} to call
     * @param arguments what to call it with
     */
    private static Object build(String name, Class<?>[] types, Object... arguments) {
        if (!isInstalled()) {
            return null;
        }
        try {
            return load(name).getMethod("open", types).invoke(null, arguments);
        } catch (InvocationTargetException failed) {
            diagnoseFailure(name, failed.getCause() == null ? failed : failed.getCause());
            return null;
        } catch (ReflectiveOperationException | LinkageError | SecurityException unusable) {
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
     * Resolves {@code name} beside the agent jar.
     *
     * <p>Asked of {@link Agent} rather than of this class's own code source: the
     * agent jar is on the bootstrap loader's search path, and a class the
     * bootstrap loader defined has no code source to ask.
     *
     * @return the file, or {@code null} if nothing recorded where the agent jar is
     */
    private static File beside(String name) {
        File self = Agent.jar();
        if (self == null) {
            return null;
        }
        File parent = self.getParentFile();
        return parent == null ? null : new File(parent, name);
    }
}
