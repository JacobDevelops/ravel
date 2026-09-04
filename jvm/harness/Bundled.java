// Loads a distribution entry point the way the published server jar loads one.
//
// Mojang's server jar is a bundler. It builds the loader the game runs under as
// `new URLClassLoader(urls, getClass().getClassLoader().getParent())`, so the
// parent is the platform loader and the system class path — which is where
// `-javaagent` puts the agent jar — is invisible to every class it defines.
// The first real dedicated server Ravel was attached to died on that: the
// `invokestatic dev/ravel/jvm/Agent.entered` the transformer prepends to the
// entry point threw `NoClassDefFoundError`.
//
// So a case run through this reaches Ravel only if the agent jar is on the
// bootstrap loader's search path, and resolves a game class only if the adapters
// look the name up through the loader that defined the entry point. Neither is
// reachable from a case that puts the game on the class path, which is what
// every other case here does.

import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;

public final class Bundled {

    private Bundled() {
    }

    public static void main(String[] arguments) throws Throwable {
        if (arguments.length < 2) {
            throw new IllegalArgumentException(
                    "usage: Bundled <directory> <entry point> [driver arguments]");
        }
        URL[] bundled = {new File(arguments[0]).toURI().toURL()};
        ClassLoader platform = Bundled.class.getClassLoader().getParent();
        ClassLoader game = new URLClassLoader(bundled, platform);
        Class<?> entry = Class.forName(arguments[1], false, game);
        if (entry.getClassLoader() != game) {
            // The whole shape of this case. An entry point the system loader
            // could also see would be defined there instead, and the case would
            // prove the thing every other case already proves.
            throw new IllegalStateException(
                    arguments[1] + " was defined by " + entry.getClassLoader()
                            + " rather than by the bundled loader, so this case is not"
                            + " the bundler's shape");
        }
        Method main = entry.getMethod("main", String[].class);
        String[] rest = new String[arguments.length - 2];
        System.arraycopy(arguments, 2, rest, 0, rest.length);
        try {
            main.invoke(null, (Object) rest);
        } catch (InvocationTargetException thrown) {
            throw thrown.getCause();
        }
    }
}
