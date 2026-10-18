package dev.ravel.jvm;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Legacy world admission and same-object MapStorage checkpoints. */
final class LegacyPersistence {
    private static Object admittedServer;
    private static String admittedIdentity;
    private static byte[] admittedBytes;
    private static Object liveStorage;
    private static Object liveData;
    private static Class<?> dataClass;

    private LegacyPersistence() {}

    /** Stage and admit before vanilla creates worlds, chunks, or a session lock. */
    static synchronized void open(Object server, String saveName) {
        try {
            initializeServer(server);
            LegacyCommand.installServer(server);
            Object source = call(server, "LEGACY_SERVER", "getWorldStorageSource");
            File root = (File) call(source, "LEGACY_WORLD_STORAGE_SOURCE", "getFile", saveName, "");
            openAt(server, root);
        } catch (ReflectiveOperationException | java.io.IOException failure) {
            throw new IllegalStateException("Ravel could not admit this legacy world", failure);
        }
    }

    /** Bind only the overworld's global storage; dimension stores never reopen Rust. */
    static synchronized void loaded(Object server) {
        try {
            Object world = call(server, "LEGACY_SERVER", "getWorld", 0);
            Object worldStorage = call(world, "LEGACY_WORLD", "getStorage");
            File root = (File) call(worldStorage, "LEGACY_WORLD_STORAGE", "getDirectory");
            bind(server, root, call(world, "LEGACY_WORLD", "getSavedDataStorage"));
        } catch (ReflectiveOperationException | java.io.IOException failure) {
            throw new IllegalStateException("Ravel could not bind legacy world storage", failure);
        }
    }

    static synchronized void openAt(Object server, File root)
            throws ReflectiveOperationException, java.io.IOException {
        if (!Agent.isInstalled("legacyWorldSave")) {
            throw new IllegalStateException("legacy world-save seam is not installed");
        }
        Class<?> sibling = LegacyScreens.load(ScreenNames.RAVEL_WORLD_DATA);
        String identity = identity(root);
        byte[] bytes = (byte[]) Members.sibling(sibling, "read/1").invoke(null, root);
        String verdict = Hook.ravelOpenWorldPersistence(bytes, identity, bytes == null);
        if (verdict == null || !verdict.isEmpty()) {
            throw new IllegalStateException(verdict == null
                    ? "Ravel persistence did not answer" : verdict);
        }
        Hook.ravelWriteWorldDatapack(identity);
        admittedServer = server;
        admittedIdentity = identity;
        admittedBytes = bytes;
        dataClass = sibling;
        liveStorage = null;
        liveData = null;
    }

    static synchronized void bind(Object server, File root, Object storage)
            throws ReflectiveOperationException, java.io.IOException {
        requireServer(server);
        if (!admittedIdentity.equals(identity(root))) {
            throw new IllegalStateException("legacy staged/live world identity differs");
        }
        Object[] lookup = (Object[]) Members.sibling(dataClass, "lookup/0").invoke(null);
        Object stored = call(storage, "LEGACY_MAP_STORAGE", "load", lookup);
        byte[] bytes = (byte[]) Members.sibling(dataClass, "bytes/1").invoke(null, stored);
        if (!Arrays.equals(admittedBytes, bytes)) {
            throw new IllegalStateException("legacy staged/live aggregate differs");
        }
        liveStorage = storage;
        liveData = stored;
        admittedBytes = null;
    }

    private static void initializeServer(Object server) {
        String integrated = LegacyNames.className("LEGACY_INTEGRATED_SERVER");
        for (Class<?> type = server.getClass(); type != null; type = type.getSuperclass()) {
            if (type.getName().equals(integrated)) {
                Hook.ravelInitializeIntegratedServer();
                return;
            }
        }
    }

    /** A failed checkpoint never prevents vanilla chunk/player saving. */
    static synchronized void save(Object server) {
        try {
            requireServer(server);
            if (liveStorage == null) {
                throw new IllegalStateException("legacy live storage is not bound");
            }
            byte[] answer = Hook.ravelSaveWorldPersistence(admittedIdentity);
            if (answer == null || answer.length == 0) {
                throw new IllegalStateException("legacy persistence produced no save answer");
            }
            switch (answer[0]) {
                case 0 -> { }
                case 1 -> replace(Arrays.copyOfRange(answer, 1, answer.length));
                case 2, 3 -> Agent.diagnose("legacy persistence skipped save: "
                        + new String(answer, 1, answer.length - 1, StandardCharsets.UTF_8));
                default -> throw new IllegalStateException("unknown legacy persistence answer");
            }
        } catch (Throwable failure) {
            Agent.diagnose("legacy persistence preserved prior checkpoint ("
                    + failure.getClass().getName() + ")");
        }
    }

    private static void replace(byte[] bytes) throws ReflectiveOperationException {
        Object[] replacement = (Object[]) Members.sibling(dataClass, "replacement/1")
                .invoke(null, (Object) bytes);
        if (liveData == null) {
            // Validate and dirty the staged object before publishing it in MapStorage.
            Members.sibling(dataClass, "commit/2").invoke(null, replacement[1], replacement[1]);
            call(liveStorage, "LEGACY_MAP_STORAGE", "set", replacement);
            liveData = replacement[1];
        } else {
            Members.sibling(dataClass, "commit/2").invoke(null, liveData, replacement[1]);
        }
    }

    private static void requireServer(Object server) {
        if (server != admittedServer || admittedIdentity == null) {
            throw new IllegalStateException("legacy persistence server identity differs");
        }
    }

    private static String identity(File root) throws java.io.IOException {
        return root.getCanonicalFile().toPath().toString();
    }

    private static Object call(Object receiver, String role, String member, Object... arguments)
            throws ReflectiveOperationException {
        return Members.method(receiver.getClass(), LegacyNames.member(role, member))
                .invoke(receiver, arguments);
    }
}
