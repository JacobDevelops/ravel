// Uses the server bundler's platform-parent loader, which cannot see the system class path.

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
