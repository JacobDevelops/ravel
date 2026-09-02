import java.util.ArrayList;
import java.util.List;

/**
 * Loads every screen class by the name the generated {@code ScreenNames} records.
 *
 * <p>The agent jar opens Ravel's screens reflectively and by name, and under a
 * version whose game sits in the unnamed package those names have no package at
 * all. Nothing had ever loaded one that way: the harness only ever ran the
 * version whose screens are in {@code dev.ravel.jvm.screen}, so a bare name was
 * a shape the build emitted and no JVM had resolved.
 *
 * <p>What this proves is that a name that version records is loadable by that
 * name from the screen jar. What it cannot prove is the launch: there the jar is
 * appended with {@code Instrumentation.appendToSystemClassLoaderSearch} and the
 * game is already loaded, and here the class path is ordinary and the game's
 * classes are the generated stubs. Same name, different loader, and only a
 * launch exercises the second.
 */
public final class LoadScreens {

    private LoadScreens() {}

    public static void main(String[] arguments) throws ClassNotFoundException {
        if (arguments.length == 0) {
            throw new IllegalArgumentException(
                    "no screen names were given, so this would have proved nothing");
        }
        List<String> loaded = new ArrayList<>();
        for (String name : arguments) {
            // Not initialized: running a screen's static initializer would need
            // the game. Defining it still resolves its superclass, which is the
            // stub, and that is the linkage this is about.
            Class<?> screen = Class.forName(name, false, LoadScreens.class.getClassLoader());
            if (!screen.getName().equals(name)) {
                throw new IllegalStateException(
                        "asked for " + name + " and got " + screen.getName());
            }
            loaded.add(name);
        }
        System.out.println("loaded " + loaded.size() + " screens by name: "
                + String.join(", ", loaded));
    }
}
