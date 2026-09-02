package dev.ravel.jvm;

import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.net.MalformedURLException;
import java.net.URISyntaxException;
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
 * <p>ADG-1's amendment named two ways to get a screen class into the game
 * process and chose the agent carrying a second jar, rather than a launcher
 * putting it on the class path. That keeps ADG-5's property that Ravel adds JVM
 * arguments to a stock instance and nothing else, because a class path cannot be
 * appended through a JVM argument on a launcher that builds its own and takes
 * the last one.
 *
 * <p>The jar is found beside the agent's own jar rather than named in an
 * argument, so {@link Agent#premain} keeps rejecting arguments and there is
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
 * <p>The parent is the system loader, so a screen still resolves its game
 * supertype to the class the game itself loaded rather than to a second copy.
 * That the game's classes are on that loader is not a guess:
 * {@code evidence/m3/class-loader-identity.json} measured the loader of a
 * rewritten {@code net.minecraft} class to be the same object as
 * {@code ClassLoader.getSystemClassLoader()}.
 *
 * <p>Constructing the loader is not defining. This runs at {@code premain} and
 * loads no class, so no game class is forced to load before the client
 * bootstraps. Defining a screen is lazy and belongs to whatever hook first needs
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
            screens = loader(opened.getName(), ClassLoader.getSystemClassLoader());
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
     * system loader, which is where the game is; the check hands it a loader
     * holding a signed jar, which is the condition that made the launch fail and
     * which nothing on a build machine has otherwise.
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
     * Builds the settings screen for one mod, loading its class on first use.
     *
     * <p>This is the lazy half, and it is lazy on purpose. The screen extends
     * {@code net.minecraft.client.gui.screens.Screen}, so defining it loads
     * {@code Screen}; doing that from {@code premain} would pull the game's
     * screen stack in before the client bootstraps. Calling this from a hook
     * means {@code Screen} is already loaded by the time it matters.
     *
     * <p>Reflection rather than a direct reference, because this class is in the
     * agent jar and ADG-1 requires that jar to declare no game type — and
     * {@code RavelSettingsScreen}'s supertype is one. The screen is loaded
     * through Ravel's own loader, whose parent is the system class loader, so
     * the superclass resolves against the real {@code Screen} rather than a
     * second copy while the screen itself lands in a runtime package the game's
     * signed jar has no claim on.
     *
     * @param owner the canonical mod id whose settings to show
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be
     *     used
     */
    static Object open(String owner) {
        return build(ScreenNames.RAVEL_SETTINGS_SCREEN, owner);
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
        return build(ScreenNames.RAVEL_MODS_SCREEN, null);
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
        return build(ScreenNames.RAVEL_PROFILING_SCREEN, null);
    }

    /**
     * Builds the screen a refused world load leaves the player on.
     *
     * <p>ADG-6's mechanism a fourth time, and the one screen whose absence is
     * not the end of the story: {@link Persistence} has already refused the
     * world by the time it asks for this, so a {@code null} here costs the
     * player the explanation and not the protection.
     *
     * @param diagnostic Rust's refusal text, which Java presents and never
     *     composes
     * @return the screen as an {@code Object} for the caller to hand to the
     *     game, or {@code null} if there is no screen jar or it could not be used
     */
    static Object openWorldRefused(String diagnostic) {
        return build(ScreenNames.RAVEL_WORLD_REFUSED_SCREEN, diagnostic);
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
     * <p>Reflection rather than a direct reference, because this class is in the
     * agent jar and ADG-1 requires that jar to declare no game type — and every
     * screen's supertype is one.
     *
     * @param name the screen class to load
     * @param owner the mod id to open it for, or {@code null} for a screen that
     *     takes none
     */
    private static Object build(String name, String owner) {
        if (!isInstalled()) {
            return null;
        }
        try {
            Class<?> screen = load(name);
            if (owner == null) {
                return screen.getMethod("open").invoke(null);
            }
            return screen.getMethod("open", String.class).invoke(null, owner);
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
