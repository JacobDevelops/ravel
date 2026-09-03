package dev.ravel.jvm;

import java.io.File;
import java.net.URL;
import java.net.URLClassLoader;
import java.util.ArrayList;
import java.util.List;

/**
 * Loads every screen class the way a launch loads it, and proves why that way.
 *
 * <p>Two claims, and the second is the one the first real 1.21.1 launch bought.
 *
 * <p><b>Every name this version records is loadable by that name.</b> The agent
 * jar opens Ravel's screens reflectively, and under a version whose game sits in
 * the unnamed package those names have no package at all. Nothing had ever
 * loaded one that way: the harness only ever ran the version whose screens are
 * in {@code dev.ravel.jvm.screen}, so a bare name was a shape the build emitted
 * and no JVM had resolved.
 *
 * <p><b>They are loaded by a loader of Ravel's own.</b> This calls
 * {@link Screens#loader} rather than building a loader that looks like it, so
 * what runs here is the code a launch runs. The screen jar is deliberately not
 * on this program's class path: if it were, the parent would answer first and
 * the screens would be defined where a launch must never define them.
 *
 * <p>The signer half reproduces the failure that forced the change. Mojang's
 * client jar is signed and an obfuscated version puts its classes in the unnamed
 * package, which is where the generated screens go too, so defining a screen
 * into the loader that already holds the game fails {@code checkCerts} with a
 * {@code SecurityException}. Both halves run here: through one loader it throws,
 * and through a child of that loader it does not.
 *
 * <p>What this still cannot prove is a launch. The game's classes here are the
 * generated stubs, and an access a different runtime package forbids is thrown
 * when the member is resolved rather than when the class is defined, so a screen
 * that loads here can still fail on the first click there.
 */
public final class LoadScreens {

    private LoadScreens() {}

    /** A class in the signed jar, and one in the unsigned jar beside it. */
    private static final String SIGNED = "RavelSignerProbeSigned";
    private static final String UNSIGNED = "RavelSignerProbeUnsigned";

    /**
     * @param arguments the screen jar, the signed probe jar, the unsigned probe
     *     jar, then every screen name to load
     */
    public static void main(String[] arguments) throws Exception {
        if (arguments.length < 4) {
            throw new IllegalArgumentException(
                    "expected the screen jar, both probe jars and at least one screen name, "
                            + "so this would have proved nothing");
        }
        String screenJar = arguments[0];
        signerCheck(arguments[1], arguments[2]);

        ClassLoader system = LoadScreens.class.getClassLoader();
        ClassLoader loader = Screens.loader(screenJar, system);
        List<String> declared =
                List.of(arguments).subList(3, arguments.length);
        List<String> loaded = new ArrayList<>();
        for (int at = 3; at < arguments.length; at++) {
            String name = arguments[at];
            // Not initialized: running a screen's static initializer would need
            // the game. Defining it still resolves its superclass, which is the
            // stub, and that is the linkage this is about.
            Class<?> screen = Class.forName(name, false, loader);
            if (!screen.getName().equals(name)) {
                throw new IllegalStateException(
                        "asked for " + name + " and got " + screen.getName());
            }
            if (screen.getClassLoader() != loader) {
                throw new IllegalStateException(name
                        + " was defined by " + screen.getClassLoader()
                        + " rather than by Ravel's own loader, so it lands in the runtime "
                        + "package a signed game jar has already claimed");
            }
            // A screen may extend another class the screen jar carries — the
            // keyboard half of a scrolling screen is one, because the versions
            // declare the key event differently and only a per-version source can
            // answer that. What the jar must never carry is a copy of a game
            // class, so the chain is walked to the first class this loader did
            // not define, and every Ravel-owned class on the way has to be one
            // this version's names record.
            Class<?> supertype = screen.getSuperclass();
            while (supertype != null && supertype.getClassLoader() == loader) {
                if (!declared.contains(supertype.getName())) {
                    throw new IllegalStateException(name + " resolved its supertype "
                            + supertype.getName() + " to a class inside the screen jar that this "
                            + "version's names do not record, so it is a second copy of a game "
                            + "class rather than one of Ravel's own");
                }
                supertype = supertype.getSuperclass();
            }
            loaded.add(name);
        }
        entryPoints(loader, Content.CONTENT, Content.SIBLING_ENTRY_POINTS);
        entryPoints(loader, Persistence.WORLD_DATA, Persistence.SIBLING_ENTRY_POINTS);
        System.out.println("loaded " + loaded.size() + " screens by name: "
                + String.join(", ", loaded));
    }

    /**
     * Requires this version's sibling to declare what the agent jar asks it for.
     *
     * <p>The sibling is generated per version and a build makes one version at a
     * time, so a version whose override renamed or dropped an entry point
     * compiles and ships and fails at the first mod or the first world. The
     * agent jar names those entry points once, beside the calls that use them,
     * and this reads that declaration rather than restating it.
     *
     * @param loader the loader the screen jar was opened under
     * @param owner the sibling class, as this version's names spell it
     * @param entries each entry point, as its name, a slash, and its arity
     */
    private static void entryPoints(ClassLoader loader, String owner, String[] entries)
            throws Exception {
        Class<?> sibling = Class.forName(owner, false, loader);
        for (String entry : entries) {
            try {
                Members.sibling(sibling, entry);
            } catch (ReflectiveOperationException absent) {
                throw new IllegalStateException(owner
                        + " declares no " + entry + ", so the agent jar would reach for it at "
                        + "runtime and find nothing", absent);
            }
        }
    }

    /**
     * Proves the certificate check both ways, over jars this build just signed.
     *
     * @param signed a signed jar holding {@link #SIGNED}
     * @param unsigned an unsigned jar holding {@link #UNSIGNED}, in the same
     *     package as the class above, which for the unnamed package means no
     *     package at all
     */
    private static void signerCheck(String signed, String unsigned) throws Exception {
        // The old mechanism, modelled rather than called: one loader holding the
        // signed jar and the unsigned one, which is what appending the screen jar
        // to the loader the game is already on amounts to.
        ClassLoader together =
                new URLClassLoader(new URL[] {url(signed), url(unsigned)}, null);
        together.loadClass(SIGNED);
        try {
            together.loadClass(UNSIGNED);
            throw new IllegalStateException(
                    "defining an unsigned class beside a signed one raised no SecurityException, "
                            + "so this check proves nothing about the failure it was written for");
        } catch (SecurityException expected) {
            // What the launch hit on every click of the mods button.
        }

        // The mechanism now: the game's loader holds only the signed jar, and
        // Ravel's own loader is a child of it whose runtime packages carry no
        // certificates to disagree with.
        ClassLoader game = new URLClassLoader(new URL[] {url(signed)}, null);
        if (game.loadClass(SIGNED).getClassLoader() != game) {
            throw new IllegalStateException("the signed probe was not defined where it was put");
        }
        Class<?> intruder = Screens.loader(unsigned, game).loadClass(UNSIGNED);
        if (intruder.getClassLoader() == game) {
            throw new IllegalStateException(
                    "the unsigned probe was defined by the parent, so the child loader is not "
                            + "what admitted it");
        }
    }

    /** One jar path as the URL a loader reads it through. */
    private static URL url(String path) throws Exception {
        return new File(path).toURI().toURL();
    }
}
