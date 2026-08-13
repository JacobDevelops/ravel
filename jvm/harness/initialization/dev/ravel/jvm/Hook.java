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
                    ? persistenceTargets()
            : Boolean.getBoolean("ravel.test.transform.content")
                    ? new HookTarget[] {
                        // The seam content is registered at, with the internal
                        // name, member and descriptor the pinned inventory
                        // carries for `registry.built-in-freeze`. The prologue
                        // runs before the body that closes the registries.
                        new HookTarget(
                                "net/minecraft/core/registries/BuiltInRegistries",
                                "freeze",
                                "()V",
                                true,
                                "registryBuiltInFreeze",
                                "()V",
                                false),
                    }
            : Boolean.getBoolean("ravel.test.transform.members")
                    ? new HookTarget[] {
                        // One method target in each of the four classes that
                        // also carry a pinned field, with the exact internal
                        // names, members and descriptors the inventory holds. A
                        // field is not among them because a field is not a call
                        // target: the generator emits no export and no prologue
                        // for one, so what a field entry claims is that the
                        // class the transformer rewrote still declares it under
                        // the pinned descriptor.
                        new HookTarget(
                                "net/minecraft/util/SmoothDouble",
                                "getNewDeltaValue",
                                "(DD)D",
                                false,
                                "inputSmoothDelta",
                                "(DD)V",
                                false),
                        new HookTarget(
                                "net/minecraft/core/MappedRegistry",
                                "freeze",
                                "()Lnet/minecraft/core/Registry;",
                                false,
                                "registryFreeze",
                                "()V",
                                false),
                        new HookTarget(
                                "net/minecraft/commands/Commands",
                                "performPrefixedCommand",
                                "(Lnet/minecraft/commands/CommandSourceStack;Ljava/lang/String;)V",
                                false,
                                "commandPerformPrefixed",
                                "(Ljava/lang/Object;Ljava/lang/String;)V",
                                false),
                        new HookTarget(
                                "net/minecraft/server/MinecraftServer",
                                "tickServer",
                                "(Ljava/util/function/BooleanSupplier;)V",
                                false,
                                "serverTick",
                                "(Ljava/lang/Object;)V",
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

    /**
     * ADG-7's two seams, with the exact internal names, members and descriptors
     * the pinned inventory carries. The adapter is injected by matching those, so
     * a stand-in that differed anywhere would not be hooked.
     *
     * <p>{@code ravel.test.persistence.saveSeamMissing} keeps the open seam and
     * drops the save seam, which is the one arrangement that makes
     * {@code Persistence.saveSeamInstalled} answer false: the server class is
     * present and loads, its transformation matches no method, and the
     * installed-prologue marker stays empty. That is the hole ADG-7's rejected
     * alternative leaves open, and without this case its false branch never runs.
     */
    private static HookTarget[] persistenceTargets() {
        HookTarget open = new HookTarget(
                "net/minecraft/client/Minecraft",
                "doWorldLoad",
                "(Lnet/minecraft/world/level/storage/LevelStorageSource$LevelStorageAccess;"
                        + "Lnet/minecraft/server/packs/repository/PackRepository;"
                        + "Lnet/minecraft/server/WorldStem;Ljava/util/Optional;Z)V",
                false,
                "worldOpenPersistence",
                "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;"
                        + "Ljava/lang/Object;Z)Z",
                true);
        if (Boolean.getBoolean("ravel.test.persistence.saveSeamMissing")) {
            return new HookTarget[] {open};
        }
        return new HookTarget[] {
            open,
            new HookTarget(
                    "net/minecraft/server/MinecraftServer",
                    "saveAllChunks",
                    "(ZZZ)Z",
                    false,
                    "worldSavePersistence",
                    "(ZZZ)V",
                    false),
        };
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
        } else if (Boolean.getBoolean("ravel.test.transform.content")) {
            className = "net.minecraft.core.registries.BuiltInRegistries";
        } else if (Boolean.getBoolean("ravel.test.transform.members")) {
            className = "net.minecraft.util.SmoothDouble";
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

    public static void inputSmoothDelta(double target, double scale) {
        invocations++;
    }

    public static void registryFreeze() {
        invocations++;
    }

    public static void registryBuiltInFreeze() {
        invocations++;
    }

    /**
     * Answers the content crossing the way the property asks.
     *
     * <p>The property names the claims a set admitted, comma separated and
     * spelled the way the barrier spells them, and this encodes them onto the
     * wire the adapter reads. {@code null} is what the JVM receives when the
     * native failed and {@code empty} is a launch with no set installed;
     * {@code malformed} is a payload built under a contract this adapter never
     * agreed to, which it must refuse whole rather than read half of.
     */
    public static String ravelAdmittedContent() {
        invocations++;
        String answer = System.getProperty("ravel.test.content.admitted", "empty");
        if (answer.equals("null")) {
            return null;
        }
        if (answer.equals("empty")) {
            return "";
        }
        if (answer.equals("malformed")) {
            return field("2") + field("1");
        }
        String[] claims = answer.split(",");
        StringBuilder payload = new StringBuilder(field("1")).append(field(
                String.valueOf(claims.length)));
        for (String claim : claims) {
            // kind:namespace:path followed by that kind's behaviour fields, which
            // is the shape the real payload has and the reason a row's width is
            // not fixed.
            String[] parts = claim.split(":");
            payload.append(field(parts[0]))
                    .append(field(parts[1]))
                    .append(field(parts[2]))
                    .append(field(parts[1]));
            for (int at = 3; at < parts.length; at++) {
                payload.append(field(parts[at]));
            }
        }
        return payload.toString();
    }

    /** One length-prefixed field, counted in the units a Java string counts in. */
    private static String field(String value) {
        return value.length() + ":" + value;
    }

    public static void commandPerformPrefixed(Object source, String command) {
        invocations++;
    }

    public static void serverTick(Object hasTimeLeft) {
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
