package dev.ravel.jvm;

import static java.nio.charset.StandardCharsets.UTF_8;

public final class Hook {
    public static final HookTarget[] TARGETS = Boolean.getBoolean("ravel.test.transform.hierarchy")
            ? new HookTarget[] {
                new HookTarget(
                        "dev/ravel/jvm/hierarchy/HierarchySource",
                        "choose",
                        "(Z)Ldev/ravel/jvm/hierarchy/HierarchyBase;",
                        true,
                        "hierarchySource",
                        "(Z)V",
                        false),
                new HookTarget(
                        "dev/ravel/jvm/hierarchy/HierarchyTarget",
                        "value",
                        "()Ljava/lang/String;",
                        false,
                        "hierarchyTarget",
                        "()V",
                        false),
            }
            : Boolean.getBoolean("ravel.test.transform.refusal")
                    ? new HookTarget[] {
                        new HookTarget(
                                "dev/ravel/jvm/refusal/RefusalTarget",
                                "record",
                                "()V",
                                true,
                                "refusalRecord",
                                "()Z",
                                true),
                    }
            : Boolean.getBoolean("ravel.test.transform.persistence")
                    ? new HookTarget[] {
                        // ADG-7's two seams, with the exact internal names,
                        // members and descriptors the pinned inventory carries.
                        // The adapter is injected by matching those, so a
                        // stand-in that differed anywhere would not be hooked.
                        new HookTarget(
                                "net/minecraft/client/Minecraft",
                                "doWorldLoad",
                                "(Lnet/minecraft/world/level/storage/LevelStorageSource$LevelStorageAccess;"
                                        + "Lnet/minecraft/server/packs/repository/PackRepository;"
                                        + "Lnet/minecraft/server/WorldStem;Ljava/util/Optional;Z)V",
                                false,
                                "worldOpenPersistence",
                                "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;"
                                        + "Ljava/lang/Object;Z)Z",
                                true),
                        new HookTarget(
                                "net/minecraft/server/MinecraftServer",
                                "saveAllChunks",
                                "(ZZZ)Z",
                                false,
                                "worldSavePersistence",
                                "(ZZZ)V",
                                false),
                    }
            : Boolean.getBoolean("ravel.test.transform.failure")
                    ? new HookTarget[] {
                        new HookTarget(
                                "net/minecraft/util/StringUtil",
                                "isAllowedChatCharacter",
                                "(I)Z",
                                true,
                                "chatAllowedCharacter",
                                "(I)V",
                                false),
                        new HookTarget(
                                "net/minecraft/util/StringUtil",
                                "broken",
                                "not-a-descriptor",
                                true,
                                "broken",
                                "()V",
                                false),
                    }
                    : new HookTarget[] {
                new HookTarget(
                        "net/minecraft/util/StringUtil",
                        "isAllowedChatCharacter",
                        "(I)Z",
                        true,
                        "chatAllowedCharacter",
                        "(I)V",
                        false),
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "filterText",
                "(Ljava/lang/String;Z)Ljava/lang/String;",
                true,
                "chatFilterText",
                "(Ljava/lang/String;Z)V",
                false),
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "filterText",
                "(Ljava/lang/String;)Ljava/lang/String;",
                true,
                "chatFilterTextSimple",
                "(Ljava/lang/String;)V",
                false),
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "isValidPlayerName",
                "(Ljava/lang/String;)Z",
                true,
                "playerNameValid",
                "(Ljava/lang/String;)V",
                false),
    };

    private static long invocations;
    private static long openCrossings;
    private static String openAggregate = "absent";

    private Hook() {
    }

    public static void initialize() {
    }

    public static void ravelEnableProfiling(String value) {
    }

    public static void ravelInitialize() {
        String className;
        if (Boolean.getBoolean("ravel.test.transform.hierarchy")) {
            className = "dev.ravel.jvm.hierarchy.HierarchySource";
        } else if (Boolean.getBoolean("ravel.test.transform.refusal")) {
            className = "dev.ravel.jvm.refusal.RefusalTarget";
        } else if (Boolean.getBoolean("ravel.test.transform.persistence")) {
            className = "net.minecraft.client.Minecraft";
        } else {
            className = "net.minecraft.util.StringUtil";
        }
        try {
            Class.forName(className, true, ClassLoader.getSystemClassLoader());
        } catch (ClassNotFoundException absent) {
            throw new IllegalStateException(absent);
        }
    }

    public static long nativeInvocations() {
        return invocations;
    }

    public static void chatAllowedCharacter(int codepoint) {
        invocations++;
    }

    public static void chatFilterText(String text, boolean keepNewlines) {
        invocations++;
    }

    public static void chatFilterTextSimple(String text) {
        invocations++;
    }

    public static void playerNameValid(String name) {
        invocations++;
    }

    public static void hierarchySource(boolean target) {
        invocations++;
    }

    public static void hierarchyTarget() {
        invocations++;
    }

    public static boolean refusalRecord() {
        invocations++;
        return Boolean.getBoolean("ravel.test.refusal.verdict");
    }

    public static boolean worldOpenPersistence(
            Object access, Object packs, Object stem, Object gameRules, boolean newWorld) {
        invocations++;
        return false;
    }

    public static void worldSavePersistence(boolean quiet, boolean flush, boolean forced) {
        invocations++;
    }

    /** How many times the open crossing was reached, which a refusal before it never does. */
    public static long openCrossings() {
        return openCrossings;
    }

    /** What the open crossing was handed, which says the stored bytes were read. */
    public static String openAggregate() {
        return openAggregate;
    }

    /**
     * Answers the open crossing the way the property asks.
     *
     * <p>The three answers are the three the real native can give: an empty
     * string admits, a non-empty one refuses and carries the diagnostic, and
     * null is what the JVM receives when the native failed.
     */
    public static String ravelOpenWorldPersistence(
            byte[] aggregate, String identity, boolean newWorld) {
        invocations++;
        openCrossings++;
        openAggregate = aggregate == null ? "absent" : new String(aggregate, UTF_8);
        String answer = System.getProperty("ravel.test.persistence.open", "empty");
        if (answer.equals("null")) {
            return null;
        }
        return answer.equals("empty") ? "" : answer;
    }

    /** Answers the save crossing with one tagged array, or with the null a failed native leaves. */
    public static byte[] ravelSaveWorldPersistence(String identity) {
        invocations++;
        String answer = System.getProperty("ravel.test.persistence.save", "unchanged");
        return switch (answer) {
            case "null" -> null;
            case "unchanged" -> new byte[] {0x00};
            case "replacement" -> tagged((byte) 0x01, "replaced");
            case "failed" -> tagged((byte) 0x02, "production failed");
            case "mismatch" -> tagged((byte) 0x03, "another world is saving");
            default -> tagged((byte) 0x7f, "unknown");
        };
    }

    private static byte[] tagged(byte tag, String payload) {
        byte[] text = payload.getBytes(UTF_8);
        byte[] answer = new byte[text.length + 1];
        answer[0] = tag;
        System.arraycopy(text, 0, answer, 1, text.length);
        return answer;
    }
}
