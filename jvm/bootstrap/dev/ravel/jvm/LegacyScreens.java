package dev.ravel.jvm;

import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.net.URL;
import java.net.URLClassLoader;
import java.util.jar.JarFile;

/** Legacy siblings share one child loader, avoiding signed unnamed-package collisions. */
final class LegacyScreens {
    static final String SCREEN_JAR = "ravel-screen.jar";
    private static volatile ClassLoader screens;

    private LegacyScreens() {}

    static synchronized void install() {
        if (screens != null) return;
        File agent = Agent.jar();
        if (agent == null || agent.getParentFile() == null) {
            Agent.diagnose("could not locate the legacy screen jar beside the agent");
            return;
        }
        File jar = new File(agent.getParentFile(), SCREEN_JAR);
        try (JarFile opened = new JarFile(jar)) {
            screens = loader(opened.getName(), Agent.gameLoader());
        } catch (java.io.IOException failure) {
            Agent.diagnose("legacy screen jar is missing or unreadable (" + failure.getClass().getName() + ")");
        }
    }

    static ClassLoader loader(String path, ClassLoader parent) throws java.net.MalformedURLException {
        return new URLClassLoader(new URL[] {new File(path).toURI().toURL()}, parent);
    }
    static boolean isInstalled() { return screens != null; }
    static Class<?> load(String name) throws ClassNotFoundException {
        if (screens == null) throw new ClassNotFoundException("legacy screen jar is not installed: " + name);
        return Class.forName(name, true, screens);
    }
    static Object openSettings(String owner, Object parent) {
        return build(ScreenNames.RAVEL_SETTINGS_SCREEN, new Class<?>[] {String.class, Object.class}, owner, parent);
    }
    static Object openMods() { return build(ScreenNames.RAVEL_MODS_SCREEN, new Class<?>[0]); }
    static Object openWorldRefused(String diagnostic) {
        return build(ScreenNames.RAVEL_WORLD_REFUSED_SCREEN, new Class<?>[] {String.class}, diagnostic);
    }
    static int addTitleMenu(Object screen, int bottom, int spacing) {
        if (!isInstalled()) return bottom;
        invoke(ScreenNames.RAVEL_MENU, "add", new Class<?>[] {Object.class, int.class, int.class}, screen, bottom, spacing);
        return bottom + spacing;
    }
    static void onOpened(Object screen) {
        if (screen != null && isInstalled())
            invoke(ScreenNames.RAVEL_MENU, "onOpened", new Class<?>[] {Object.class}, screen);
    }
    static boolean exercise(Object game) {
        return Boolean.TRUE.equals(invoke(ScreenNames.RAVEL_SCREEN_EXERCISE, "exercise", new Class<?>[] {Object.class}, game));
    }
    private static Object build(String name, Class<?>[] types, Object... arguments) {
        if (!isInstalled()) return null;
        return invoke(name, "open", types, arguments);
    }
    private static Object invoke(String name, String method, Class<?>[] types, Object... arguments) {
        try {
            return load(name).getMethod(method, types).invoke(null, arguments);
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause() == null ? failure : failure.getCause();
            throw new IllegalStateException("legacy screen operation failed: " + name + "." + method, cause);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("legacy screen operation unavailable: " + name + "." + method, failure);
        }
    }
}
