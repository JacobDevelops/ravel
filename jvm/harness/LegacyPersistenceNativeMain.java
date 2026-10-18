package dev.ravel.jvm;

import java.io.DataOutputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.zip.GZIPOutputStream;

/** Real JNI composition and official file-backed MapStorage; no game stand-ins. */
public final class LegacyPersistenceNativeMain {
    private LegacyPersistenceNativeMain() {}

    public static void main(String[] arguments) throws Exception {
        Path base = Path.of(arguments[0]).toAbsolutePath();
        Files.createDirectories(base);
        Class<?> server = Members.game(LegacyNames.className("LEGACY_SERVER"));
        boolean integrated = arguments[1].equals("integrated");
        Agent.entered(integrated ? 0 : 1, server);
        if (integrated) {
            Hook.ravelInitializeIntegratedServer();
        }
        var screens = LegacyScreens.class.getDeclaredField("screens");
        screens.setAccessible(true);
        screens.set(null, LegacyScreens.loader(arguments[2], Agent.gameLoader()));
        Class<?> data = LegacyScreens.load(ScreenNames.RAVEL_WORLD_DATA);
        File root = base.resolve("world").toFile();
        byte[] old = aggregate(1);
        seed(root.toPath(), old);
        Object owner = new Object();
        LegacyPersistence.openAt(owner, root);
        require(Files.isRegularFile(root.toPath().resolve("datapacks/ravel/pack.mcmeta")),
                "legacy open did not write the pack through its dedicated crossing");
        Object storage = storage(base.toFile(), "world");
        LegacyPersistence.bind(owner, root, storage);
        Object live = load(storage, data);
        require(Arrays.equals(bytes(data, live), old), "admission rewrote live data");
        LegacyPersistence.save(owner);
        require(load(storage, data) == live, "checkpoint replaced live SavedData identity");
        byte[] migrated = bytes(data, live);
        String json = new String(migrated, StandardCharsets.UTF_8);
        require(json.contains("\"dataVersion\":3") && json.contains("\"spawn\""),
                "native migration did not reach schema 3 with spawn waypoint");
        flush(storage);
        Path file = root.toPath().resolve("data/ravel_mods.dat");
        byte[] checkpoint = Files.readAllBytes(file);
        failedSave(owner, storage, data, live, checkpoint, file);
        Object reopened = storage(base.toFile(), "world");
        LegacyPersistence.openAt(owner, root);
        LegacyPersistence.bind(owner, root, reopened);
        require(Arrays.equals(bytes(data, load(reopened, data)), migrated), "reopen lost payload");
        Object dimension = storage(base.toFile(), "dimension");
        require(load(dimension, data) == null, "dimension inherited global aggregate");
        flush(dimension);
        require(!Files.exists(base.resolve("dimension/data/ravel_mods.dat")),
                "dimension observation duplicated global checkpoint");
        refusedAdmission(base, owner, data);
        System.out.println("legacy-native-persistence=" + arguments[1]
                + " migrated=3 reopened=true identity=true failurePreserved=true dimensionIsolated=true");
    }

    private static void failedSave(Object owner, Object storage, Class<?> data, Object live,
            byte[] checkpoint, Path file) throws Exception {
        byte[] failed = Hook.ravelSaveWorldPersistence(file + "/wrong-world");
        require(failed != null && failed.length > 0 && failed[0] == 3,
                "native save did not reject wrong identity");
        byte[] before = bytes(data, live).clone();
        String switched = Hook.ravelOpenWorldPersistence(before, file + "/other-world", false);
        require(switched != null && switched.isEmpty(), "native alternate identity did not admit");
        LegacyPersistence.save(owner);
        require(load(storage, data) == live && Arrays.equals(bytes(data, live), before),
                "failed save changed live identity or bytes");
        flush(storage);
        require(Arrays.equals(Files.readAllBytes(file), checkpoint), "failed save rewrote NBT");
        String restored = Hook.ravelOpenWorldPersistence(before, file.getParent().getParent()
                .toFile().getCanonicalPath(), false);
        require(restored != null && restored.isEmpty(), "native original identity did not reopen");
        LegacyPersistence.save(owner);
    }

    private static void refusedAdmission(Path base, Object owner, Class<?> data) throws Exception {
        File root = base.resolve("future").toFile();
        seed(root.toPath(), aggregate(99));
        Path file = root.toPath().resolve("data/ravel_mods.dat");
        byte[] before = Files.readAllBytes(file);
        Object storage = storage(base.toFile(), "future");
        Object held = load(storage, data);
        try {
            LegacyPersistence.openAt(owner, root);
            throw new AssertionError("future schema was admitted");
        } catch (IllegalStateException refused) {
            require(refused.getMessage() != null, "admission refusal lost diagnostic");
        }
        require(load(storage, data) == held, "failed admission installed another live object");
        flush(storage);
        require(Arrays.equals(Files.readAllBytes(file), before), "failed admission dirtied NBT");
        require(!Files.exists(root.toPath().resolve("datapacks/ravel"))
                && !Files.exists(root.toPath().resolve("ravel")),
                "failed admission wrote a world pack");
    }

    private static Object storage(File base, String name) throws ReflectiveOperationException {
        Class<?> fixer = Members.game(LegacyNames.className("LEGACY_DATA_FIXER"));
        Object dataFixer = fixer.getConstructor(int.class).newInstance(1343);
        Class<?> world = Members.game(LegacyNames.className("LEGACY_ALPHA_WORLD_STORAGE"));
        Object backing = world.getConstructor(File.class, String.class, boolean.class, fixer)
                .newInstance(base, name, false, dataFixer);
        Class<?> contract = Members.game(LegacyNames.className("LEGACY_WORLD_STORAGE"));
        return Members.game(LegacyNames.className("LEGACY_MAP_STORAGE"))
                .getConstructor(contract).newInstance(backing);
    }

    private static Object load(Object storage, Class<?> data) throws ReflectiveOperationException {
        Object[] lookup = (Object[]) Members.sibling(data, "lookup/0").invoke(null);
        return Members.method(storage.getClass(), LegacyNames.member("LEGACY_MAP_STORAGE", "load"))
                .invoke(storage, lookup);
    }

    private static void flush(Object storage) throws ReflectiveOperationException {
        Members.method(storage.getClass(), LegacyNames.member("LEGACY_MAP_STORAGE", "save"))
                .invoke(storage);
    }

    private static byte[] bytes(Class<?> data, Object held) throws ReflectiveOperationException {
        return (byte[]) Members.sibling(data, "bytes/1").invoke(null, held);
    }

    private static byte[] aggregate(int version) {
        return ("{\"formatVersion\":1,\"owners\":[{\"schemaVersion\":1,"
                + "\"owner\":\"saved_data\",\"dataVersion\":" + version
                + ",\"requiredPackages\":[],\"payload\":{\"visits\":4}}]}")
                .getBytes(StandardCharsets.UTF_8);
    }

    private static void seed(Path root, byte[] aggregate) throws java.io.IOException {
        Files.createDirectories(root.resolve("data"));
        try (var output = new DataOutputStream(new GZIPOutputStream(
                Files.newOutputStream(root.resolve("data/ravel_mods.dat"))))) {
            output.writeByte(10);
            output.writeUTF("");
            output.writeByte(10);
            output.writeUTF("data");
            output.writeByte(7);
            output.writeUTF("aggregate");
            output.writeInt(aggregate.length);
            output.write(aggregate);
            output.writeByte(0);
            output.writeByte(0);
        }
    }

    private static void require(boolean value, String message) {
        if (!value) {
            throw new AssertionError(message);
        }
    }
}
