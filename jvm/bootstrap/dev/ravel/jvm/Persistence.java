package dev.ravel.jvm;

import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * ADG-7's world persistence adapter, on the Java side of the two crossings.
 *
 * <p>Everything here is reflective, because this class ships in the always
 * present agent jar and ADG-1 forbids that jar <em>declaring</em> a game type:
 * the jar is compiled with no client jar on the class path, so a source declaring
 * one would not compile. Every game name it resolves at runtime — the classes
 * through {@link GameNames}, the members through {@link GameMembers} — is
 * generated from the version's reviewed reflective surface, so this one source
 * is right about every tracked version. The one half that cannot be reflective —
 * a concrete {@code SavedData} subclass and its codec — is compiled in the
 * sibling artifact, and this class reaches it by name too, for exactly the
 * reason {@link Screens} does.
 *
 * <p><b>What the sibling decides and this does not.</b> Three things about the
 * storage are version shape rather than version naming: what its constructor
 * wants the directory as, what it is keyed by, and what it is handed. Those are
 * the sibling's {@code folder}, {@code lookup} and {@code replacement}, because
 * the sibling already exists per version and a role binding answers a rename
 * rather than a different argument list.
 *
 * <p><b>The sibling is required here, where a screen is optional.</b> An absent
 * screen jar loses the settings UI and leaves a working game, so
 * {@link Screens#install} treats it as a diagnostic. An absent sibling loses all
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

    /**
     * The saved-data directory, below the world root.
     *
     * <p>Written here rather than read off a game constant because only one of
     * the tracked versions has one to read: 26.1.2 names it as a
     * {@code LevelResource} and 1.21.1 resolves the same word against the
     * dimension directory itself.
     */
    private static final String DATA_DIRECTORY = "data";
    /**
     * The generated saved-data class, wherever this version's names put it.
     *
     * <p>It is generated for the same reason it is reached by name: it extends
     * {@code SavedData}, so what it compiles against is what the JVM links, and a
     * version that obfuscates the game into the unnamed package takes this class
     * there with it.
     */
    static final String WORLD_DATA = ScreenNames.RAVEL_WORLD_DATA;

    private static final String FOLDER = "folder/1";
    private static final String LOOKUP = "lookup/0";
    private static final String REPLACEMENT_ENTRY = "replacement/1";
    private static final String BYTES = "bytes/1";

    /**
     * Everything this adapter asks the sibling for, so a check can ask too.
     *
     * <p>Declared rather than only called, for the reason {@link Content} gives:
     * the sibling is generated per version and a build makes one version at a
     * time, so a version whose override dropped an entry point would compile and
     * ship and fail at the first world.
     */
    static final String[] SIBLING_ENTRY_POINTS = {FOLDER, LOOKUP, REPLACEMENT_ENTRY, BYTES};

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
     * <p>A refusal the player cannot be shown is thrown rather than returned.
     * Returning leaves the game on the loading screen it was already on with
     * nothing coming, which the first real 1.21.1 launch sat on: the world was
     * correctly refused, the screen jar was absent so there was nothing to draw,
     * and the client held the session lock at "Preparing for world creation"
     * until it was killed. A throw reaches the game's own crash path, which
     * reports the reason and shuts the client down, and the world's lock is
     * already released by then because the access is closed first.
     *
     * @param access the selected {@code LevelStorageAccess}, held opaquely
     * @param stem the selected {@code WorldStem}, held opaquely
     * @param freshWorld the game's advisory new-world marker; ADG-7 says absence
     *     rather than this marker is what initializes fresh state
     * @return whether to return from the host method without running its body
     * @throws IllegalStateException when the world is refused and no screen can
     *     say so, which is a refusal the player would otherwise never see
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
        if (!refuse(access, stem, refusal)) {
            throw new IllegalStateException("Ravel refused this world load and could not show "
                    + "the reason on screen: " + refusal);
        }
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

        String identity = path(access, GameMembers.LEVEL_STORAGE_ACCESS_GET_LEVEL_PATH);
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
            game(SERVER);
        } catch (ReflectiveOperationException | LinkageError absent) {
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
        Object root = Members.method(
                access.getClass(), GameMembers.LEVEL_STORAGE_ACCESS_GET_LEVEL_PATH)
                .invoke(access, constant(LEVEL_RESOURCE, GameMembers.LEVEL_RESOURCE_ROOT));
        Object fixer = Members.method(game(DATA_FIXERS), GameMembers.DATA_FIXERS_GET_DATA_FIXER)
                .invoke(null);
        Object layers = Members.method(stem.getClass(), GameMembers.WORLD_STEM_REGISTRIES)
                .invoke(stem);
        Object registries = Members.method(
                layers.getClass(), GameMembers.REGISTRY_LAYERS_COMPOSITE_ACCESS).invoke(layers);

        Class<?> storage = game(STORAGE);
        Object opened = Members.constructor(storage, GameMembers.STORAGE_NEW)
                .newInstance(folder(data, root), fixer, registries);
        Object stored;
        try {
            stored = Members.method(opened.getClass(), GameMembers.STORAGE_GET)
                    .invoke(opened, (Object[]) Members.sibling(data, LOOKUP).invoke(null));
        } finally {
            close(opened);
        }
        return (byte[]) Members.sibling(data, BYTES).invoke(null, stored);
    }

    /**
     * The directory this version's storage is opened over.
     *
     * <p>The sibling decides it, because the two versions disagree about the
     * type and not only the place: one storage takes a {@code Path} and the
     * other a {@code File}, and a shared source cannot hold both.
     */
    private static Object folder(Class<?> data, Object root) throws ReflectiveOperationException {
        return Members.sibling(data, FOLDER)
                .invoke(null, ((java.nio.file.Path) root).resolve(DATA_DIRECTORY));
    }

    /**
     * Cleans up the refused world and shows the player why.
     *
     * <p>The order matters: the access holds the world's lock, so it is closed
     * whatever the stem does, and neither close is allowed to take the refusal
     * screen away from the player.
     *
     * @return whether the player was actually shown the reason, which is what
     *     decides between a refused load the player can act on and one the caller
     *     has to turn into a crash
     */
    private static boolean refuse(Object access, Object stem, String diagnostic) {
        close(stem);
        close(access);
        Agent.diagnose("refused a world load: " + diagnostic);
        Object screen = Screens.openWorldRefused(diagnostic);
        if (screen == null) {
            // An absent sibling is itself a reason this path runs, and it is also
            // what leaves nothing to draw with. The caller throws rather than
            // returning the player to a loading screen with nothing coming.
            return false;
        }
        try {
            Class<?> client = game(GameNames.MINECRAFT);
            Object instance = Members.method(client, GameMembers.MINECRAFT_GET_INSTANCE)
                    .invoke(null);
            Members.method(client, GameMembers.MINECRAFT_SET_SCREEN).invoke(instance, screen);
            return true;
        } catch (ReflectiveOperationException | LinkageError | RuntimeException undrawable) {
            Agent.diagnose("the world refusal screen could not be shown ("
                    + undrawable.getClass().getName() + ")");
            return false;
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
            String identity = path(server, GameMembers.SERVER_GET_WORLD_PATH);
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
        // Through the overworld rather than off the server, because only one of
        // the two tracked versions declares a storage getter on the server and
        // both hang one off a level.
        Object level = Members.method(server.getClass(), GameMembers.SERVER_OVERWORLD)
                .invoke(server);
        Object storage = Members.method(
                level.getClass(), GameMembers.SERVER_LEVEL_GET_DATA_STORAGE).invoke(level);
        Members.method(storage.getClass(), GameMembers.STORAGE_SET)
                .invoke(storage,
                        (Object[]) Members.sibling(data, REPLACEMENT_ENTRY)
                                .invoke(null, (Object) aggregate));
    }

    /** The remainder of a tagged answer, as the diagnostic it carries. */
    private static String diagnostic(byte[] answer) {
        return new String(answer, 1, answer.length - 1, StandardCharsets.UTF_8);
    }

    /** Ravel's stored-object class from the sibling artifact, or {@code null}. */
    private static Class<?> sibling() {
        if (!Screens.isInstalled()) {
            return null;
        }
        try {
            return Screens.load(WORLD_DATA);
        } catch (ReflectiveOperationException | LinkageError absent) {
            Agent.diagnose("the world persistence component could not be loaded ("
                    + absent.getClass().getName() + ")");
            return null;
        }
    }

    /** One world identity, as the seam that produced it spells it. */
    private static String path(Object receiver, String getter)
            throws ReflectiveOperationException {
        return String.valueOf(Members.method(receiver.getClass(), getter)
                .invoke(receiver, constant(LEVEL_RESOURCE, GameMembers.LEVEL_RESOURCE_ROOT)));
    }

    /** Reads one static constant the reviewed surface names. */
    private static Object constant(String owner, String member) throws ReflectiveOperationException {
        return Members.field(game(owner), member).get(null);
    }

    /** One game class, resolved through the loader the game itself is on. */
    private static Class<?> game(String name) throws ReflectiveOperationException {
        return Class.forName(name, true, ClassLoader.getSystemClassLoader());
    }

    /**
     * Closes one game object that has a close to perform.
     *
     * <p>Asked of the JVM rather than of the reviewed surface, because whether a
     * thing closes is what {@code AutoCloseable} already answers and the answer
     * differs between the versions: 26.1.2's storage flushes on close and
     * 1.21.1's is not closeable at all. A read that set nothing has nothing to
     * flush either way, so the version that does close writes nothing.
     */
    private static void close(Object closeable) {
        if (!(closeable instanceof AutoCloseable shut)) {
            return;
        }
        try {
            shut.close();
        } catch (Exception | LinkageError unclosed) {
            Agent.diagnose("a refused world could not be closed cleanly ("
                    + unclosed.getClass().getName() + ")");
        }
    }
}
