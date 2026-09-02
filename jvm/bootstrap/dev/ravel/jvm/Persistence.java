package dev.ravel.jvm;

import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * ADG-7's world persistence adapter, on the Java side of the two crossings.
 *
 * <p>Everything here is reflective, because this class ships in the always
 * present agent jar and ADG-1 forbids that jar <em>declaring</em> a game type:
 * the jar is compiled with no client jar on the class path, so a source declaring
 * one would not compile. The class names below are strings this class resolves at
 * runtime, which the rule permits and which is not free — they are a version
 * coupling no lock covers, recorded as such in {@code evidence/m3/manifest.json}
 * and pinned as a set by {@code verify_persistence_reflection}. The one half that
 * cannot be reflective — a concrete {@code SavedData} subclass and its codec — is
 * compiled in the sibling artifact, and this class reaches it by name too, for
 * exactly the reason {@link Screens} does.
 *
 * <p><b>The sibling is required here, where a screen is optional.</b> An absent
 * screen jar loses the settings UI and leaves a working game, so
 * {@link Screens#append} treats it as a diagnostic. An absent sibling loses all
 * persistence, and a world opened without it would run, accept a player's work,
 * and save none of it. So this path refuses the world load instead. That
 * asymmetry is deliberate and is the opposite of the policy next door.
 *
 * <p><b>Rust decides, Java cleans up and presents.</b> Nothing here inspects the
 * aggregate, measures it, or bounds its length: a length check on this side
 * would make Java an admission authority, and ADG-7 reserves every admission
 * decision for {@code ravel-save}. The bytes are opaque in both directions.
 */
final class Persistence {

    /**
     * The generated hook the save seam is rewritten to call.
     *
     * <p>Read as ADG-7's installed-prologue marker. The open crossing proves its
     * own seam by firing at all — a prologue that never ran cannot ask — but the
     * save seam has no such self-proof at open time, so it is asked for by name.
     */
    private static final String SAVE_HOOK = "worldSavePersistence";

    // Generated from the version's reviewed reflective surface, because this
    // source is shared across versions and the names in it are not: the storage
    // one version calls SavedDataStorage another calls DimensionDataStorage, and
    // an obfuscated version calls it neither.

    /** The class whose transformation the marker above is about. */
    private static final String SERVER = GameNames.SERVER;

    /** The world's aggregate store, whatever this version names it. */
    private static final String STORAGE = GameNames.STORAGE;

    private static final String LEVEL_RESOURCE = GameNames.LEVEL_RESOURCE;
    private static final String DATA_FIXERS = GameNames.DATA_FIXERS;
    private static final String WORLD_DATA = "dev.ravel.jvm.screen.RavelWorldData";

    /** Tag byte at element zero of a save answer, payload in the remainder. */
    private static final byte UNCHANGED = 0x00;
    private static final byte REPLACEMENT = 0x01;
    private static final byte FAILED = 0x02;
    private static final byte IDENTITY_MISMATCH = 0x03;

    private Persistence() {
    }

    /**
     * Admits or refuses one world load, before the game's own body mutates
     * anything.
     *
     * <p>Every failure lands on refusal. A world Ravel cannot read is a world
     * whose next save would either lose a player's mod state or overwrite it
     * with state nothing checked, and neither is worth a running game.
     *
     * @param access the selected {@code LevelStorageAccess}, held opaquely
     * @param stem the selected {@code WorldStem}, held opaquely
     * @param freshWorld the game's advisory new-world marker; ADG-7 says absence
     *     rather than this marker is what initializes fresh state
     * @return whether to return from the host method without running its body
     */
    static boolean open(Object access, Object stem, boolean freshWorld) {
        String refusal;
        try {
            refusal = decide(access, stem, freshWorld);
        } catch (InvocationTargetException failed) {
            Throwable cause = failed.getCause() == null ? failed : failed.getCause();
            refusal = "Ravel could not read this world's saved data ("
                    + cause.getClass().getName() + ")";
        } catch (Throwable unusable) {
            // Deliberately everything. This runs inside the game's own call and
            // ADG-7 says a refusal must not leak a raw exception; an escape here
            // would also leave the world lock held by the access below.
            refusal = "Ravel could not read this world's saved data ("
                    + unusable.getClass().getName() + ")";
        }
        if (refusal == null) {
            return false;
        }
        refuse(access, stem, refusal);
        return true;
    }

    /**
     * Reads the stored aggregate and asks Rust about it.
     *
     * @return {@code null} when the world is admitted, or the player-facing
     *     diagnostic when it is refused
     */
    private static String decide(Object access, Object stem, boolean freshWorld)
            throws ReflectiveOperationException {
        if (!saveSeamInstalled()) {
            return "Ravel's world-save hook was not installed, so this world's "
                    + "mod data could not be saved again.";
        }
        Class<?> data = sibling();
        if (data == null) {
            return "Ravel's world persistence component is missing from this "
                    + "installation.";
        }

        String identity = path(access, "getLevelPath", "ROOT");
        byte[] stored = read(access, stem, data);

        String verdict = Hook.ravelOpenWorldPersistence(stored, identity, freshWorld);
        // Null is not an answer. It is what the JVM receives when the native
        // failed, so it refuses; an empty string is what says admitted. That way
        // round is why a panic refuses an unreadable world rather than admitting
        // one nothing checked.
        if (verdict == null) {
            return "Ravel's world persistence did not answer for this world.";
        }
        return verdict.isEmpty() ? null : verdict;
    }

    /**
     * Whether the save seam was actually rewritten.
     *
     * <p>{@code Class.forName} first, because the marker only holds what the
     * transformer has already seen and the server class may not be loaded yet at
     * world open. Loading it runs the transformer for it, so the answer after
     * this line is about a transformation that has happened rather than one that
     * might. A class that is present but whose transformation matched zero
     * methods answers false here, which is the hole ADG-7's rejected alternative
     * leaves open.
     */
    private static boolean saveSeamInstalled() {
        try {
            Class.forName(SERVER, true, ClassLoader.getSystemClassLoader());
        } catch (ClassNotFoundException | LinkageError absent) {
            return false;
        }
        return Agent.isInstalled(SAVE_HOOK);
    }

    /**
     * Reads Ravel's stored object out of the world being opened.
     *
     * <p>The world has no server yet at this seam, so the storage is opened over
     * the same directory {@code MinecraftServer} will use and closed again. It
     * is opened to read: nothing is set on it, so the {@code saveAndJoin} its
     * {@code close} performs has nothing to write.
     *
     * @return the aggregate, or {@code null} when this world holds no Ravel
     *     object — which ADG-7 initializes fresh state for, in any world
     */
    private static byte[] read(Object access, Object stem, Class<?> data)
            throws ReflectiveOperationException {
        Object directory = call(access, "getLevelPath", constant(LEVEL_RESOURCE, "DATA"));
        Object fixer = Class.forName(DATA_FIXERS, true, ClassLoader.getSystemClassLoader())
                .getMethod("getDataFixer")
                .invoke(null);
        Object registries = call(call(stem, "registries"), "compositeAccess");

        Class<?> storage = Class.forName(STORAGE, true, ClassLoader.getSystemClassLoader());
        Object opened = only(storage.getConstructors(), 3)
                .newInstance(directory, fixer, registries);
        Object stored;
        try {
            stored = method(opened.getClass(), "get", 1).invoke(opened, type(data));
        } finally {
            method(opened.getClass(), "close", 0).invoke(opened);
        }
        return (byte[]) method(data, "bytes", 1).invoke(null, stored);
    }

    /**
     * Cleans up the refused world and shows the player why.
     *
     * <p>The order matters: the access holds the world's lock, so it is closed
     * whatever the stem does, and neither close is allowed to take the refusal
     * screen away from the player.
     */
    private static void refuse(Object access, Object stem, String diagnostic) {
        close(stem);
        close(access);
        Agent.diagnose("refused a world load: " + diagnostic);
        Object screen = Screens.openWorldRefused(diagnostic);
        if (screen == null) {
            // No sibling artifact is itself a reason this path runs, and it is
            // also what leaves nothing to draw with. The world is still refused;
            // the player is left on the screen they came from.
            return;
        }
        try {
            Class<?> client = Class.forName(
                    GameNames.MINECRAFT, true, ClassLoader.getSystemClassLoader());
            Object instance = client.getMethod("getInstance").invoke(null);
            method(client, "setScreen", 1).invoke(instance, screen);
        } catch (ReflectiveOperationException | LinkageError | RuntimeException undrawable) {
            Agent.diagnose("the world refusal screen could not be shown ("
                    + undrawable.getClass().getName() + ")");
        }
    }

    /**
     * Offers Ravel's aggregate for the world that is saving.
     *
     * <p>The opposite failure priority to {@link #open}. This is called from
     * inside the game's own save, so it catches everything and writes nothing on
     * doubt: losing one Ravel checkpoint is acceptable and losing the player's
     * vanilla save is not. Only tag {@code 0x01} reaches {@code set}, and only
     * once, because the pinned implementation already marks the replacement
     * dirty exactly once.
     *
     * @param server the saving {@code MinecraftServer}, held opaquely
     */
    static void save(Object server) {
        try {
            String identity = path(server, "getWorldPath", "ROOT");
            byte[] answer = Hook.ravelSaveWorldPersistence(identity);
            if (answer == null || answer.length == 0) {
                // Null is what a failed native leaves behind, so it is read as a
                // failure that writes nothing rather than as an empty answer.
                Agent.diagnose("world persistence produced no answer for this save");
                return;
            }
            switch (answer[0]) {
                case REPLACEMENT ->
                        // Nothing here bounds the remainder. Java never inspects
                        // or interprets the aggregate; a length check on this
                        // side would make it an admission authority.
                        write(server, Arrays.copyOfRange(answer, 1, answer.length));
                case FAILED, IDENTITY_MISMATCH -> Agent.diagnose(
                        "world persistence skipped this save: " + diagnostic(answer));
                case UNCHANGED -> {
                    // Nothing to write, which is what keeps an idle autosave from
                    // rewriting the Ravel object.
                }
                default -> Agent.diagnose("world persistence answered with an unknown tag");
            }
        } catch (Throwable unusable) {
            // Deliberately everything, for the reason above: this must never
            // abort the vanilla player or chunk saving it runs inside.
            Agent.diagnose("world persistence failed during a save ("
                    + unusable.getClass().getName() + ")");
        }
    }

    /** Hands the replacement to the storage, exactly once. */
    private static void write(Object server, byte[] aggregate) throws ReflectiveOperationException {
        Class<?> data = sibling();
        if (data == null) {
            Agent.diagnose("world persistence has no component to store the replacement with");
            return;
        }
        Object storage = call(server, "getDataStorage");
        Object held = method(data, "of", 1).invoke(null, (Object) aggregate);
        method(storage.getClass(), "set", 2).invoke(storage, type(data), held);
    }

    /** The remainder of a tagged answer, as the diagnostic it carries. */
    private static String diagnostic(byte[] answer) {
        return new String(answer, 1, answer.length - 1, StandardCharsets.UTF_8);
    }

    /** Ravel's stored-object class from the sibling artifact, or {@code null}. */
    private static Class<?> sibling() {
        if (!Screens.isAppended()) {
            return null;
        }
        try {
            return Class.forName(WORLD_DATA, true, ClassLoader.getSystemClassLoader());
        } catch (ClassNotFoundException | LinkageError absent) {
            Agent.diagnose("the world persistence component could not be loaded ("
                    + absent.getClass().getName() + ")");
            return null;
        }
    }

    /** The registration Minecraft reads and writes Ravel's object through. */
    private static Object type(Class<?> data) throws ReflectiveOperationException {
        return method(data, "type", 0).invoke(null);
    }

    /** One world identity, as the seam that produced it spells it. */
    private static String path(Object receiver, String getter, String resource)
            throws ReflectiveOperationException {
        return String.valueOf(call(receiver, getter, constant(LEVEL_RESOURCE, resource)));
    }

    /** Reads one static constant by name. */
    private static Object constant(String owner, String name) throws ReflectiveOperationException {
        return Class.forName(owner, true, ClassLoader.getSystemClassLoader())
                .getField(name)
                .get(null);
    }

    /** Invokes one method by name and argument count, on the receiver's own class. */
    private static Object call(Object receiver, String name, Object... arguments)
            throws ReflectiveOperationException {
        return method(receiver.getClass(), name, arguments.length).invoke(receiver, arguments);
    }

    /**
     * Finds one member by name and shape rather than by declared parameter types.
     *
     * <p>Naming the parameter types would mean either declaring them here, which
     * the agent jar cannot do because it compiles with no client jar on the class
     * path, or resolving every one of them by name as well — which widens the set
     * of game names this jar carries and buys nothing. Name and arity identify
     * each member used here uniquely in the pinned classes.
     *
     * <p>What that costs is worth stating where the lookup is. These member names,
     * the two {@code LevelResource} field constants, and the three-argument
     * {@code SavedDataStorage} constructor appear in no pinned file, so ADG-3's
     * drift gate does not reach any of them and a rename in the next pinned
     * version lands as a runtime refusal rather than as a generation failure. The
     * class names are pinned as a set by {@code verify_persistence_reflection},
     * which cannot see a rename either.
     */
    private static Method method(Class<?> owner, String name, int arity)
            throws ReflectiveOperationException {
        for (Class<?> current = owner; current != null; current = current.getSuperclass()) {
            for (Method candidate : current.getDeclaredMethods()) {
                if (candidate.getName().equals(name)
                        && candidate.getParameterCount() == arity) {
                    candidate.setAccessible(true);
                    return candidate;
                }
            }
        }
        throw new NoSuchMethodException(owner.getName() + "." + name + "/" + arity);
    }

    /** The one constructor of the given arity. */
    private static Constructor<?> only(Constructor<?>[] candidates, int arity)
            throws ReflectiveOperationException {
        for (Constructor<?> candidate : candidates) {
            if (candidate.getParameterCount() == arity) {
                return candidate;
            }
        }
        throw new NoSuchMethodException("no constructor of " + arity + " arguments");
    }

    /** Closes one {@code AutoCloseable} game object without letting it throw. */
    private static void close(Object closeable) {
        if (closeable == null) {
            return;
        }
        try {
            method(closeable.getClass(), "close", 0).invoke(closeable);
        } catch (ReflectiveOperationException | LinkageError | RuntimeException unclosed) {
            Agent.diagnose("a refused world could not be closed cleanly ("
                    + unclosed.getClass().getName() + ")");
        }
    }
}
