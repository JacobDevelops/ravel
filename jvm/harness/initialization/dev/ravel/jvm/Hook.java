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
                        linkedTarget("registryBuiltInFreeze"),
                    }
            : Boolean.getBoolean("ravel.test.transform.outward")
                    ? new HookTarget[] {
                        linkedTarget("clientTick"),
                    }
            : Boolean.getBoolean("ravel.test.transform.members")
                    ? new HookTarget[] {
                        // Rewrite each field-bearing class without treating fields as call targets.
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

    /** This fixture provides the modern Hook surface. */
    public static final boolean LEGACY_ADAPTER = false;

    private static long invocations;
    private static long openCrossings;
    private static String openAggregate = "absent";

    private Hook() {
    }

    /** Modern inventories do not expand the legacy global palette. */
    public static int legacyPaletteSize(int count) {
        return count;
    }

    /** Modern inventories have no legacy palette-size call. */
    public static boolean legacyPaletteSizeCall(String owner, String name, String descriptor) {
        return false;
    }

    /** Modern inventories have no legacy palette field. */
    public static boolean legacyPaletteField(String owner, String name, String descriptor) {
        return false;
    }

    // Keep pack and staging coverage when only the save seam is intentionally missing.
    private static HookTarget[] persistenceTargets() {
        HookTarget open = linkedTarget("worldOpenPersistence");
        HookTarget packRepository = linkedTarget("worldCreatePackRepository");
        HookTarget createWorld = linkedTarget("screenCreateWorldInit");
        if (Boolean.getBoolean("ravel.test.persistence.saveSeamMissing")) {
            return new HookTarget[] {open, packRepository, createWorld};
        }
        return new HookTarget[] {
            open, packRepository, createWorld, linkedTarget("worldSavePersistence"),
        };
    }

    // Read version-specific names from the generated table by stable prologue id.
    private static HookTarget linkedTarget(String prologue) {
        String[] parts = seam(prologue).split(":");
        if (parts.length != ENTRY_FIELDS) {
            throw new IllegalStateException(
                    "a hook table entry is a class, a member, a descriptor, whether it is static, "
                            + "a prologue, its descriptor and whether it refuses: "
                            + seam(prologue));
        }
        return new HookTarget(
                parts[0],
                parts[1],
                parts[2],
                Boolean.parseBoolean(parts[3]),
                parts[4],
                parts[5],
                Boolean.parseBoolean(parts[6]));
    }

    private static final int ENTRY_FIELDS = 7;

    private static String seam(String prologue) {
        String named = System.getProperty("ravel.test.target." + prologue);
        if (named == null) {
            throw new IllegalStateException(
                    prologue + " transforms a seam and this run named none");
        }
        return named;
    }

    private static String linkedTargetClass(String prologue) {
        return seam(prologue).split(":")[0].replace('/', '.');
    }

    public static void initialize() {
    }

    // Force class loading during initialization; side selection is tested against the real host.
    public static void ravelInitialize(int side, boolean profilingRequested) {
        String className;
        if (Boolean.getBoolean("ravel.test.transform.hierarchy")) {
            className = "dev.ravel.jvm.hierarchy.HierarchySource";
        } else if (Boolean.getBoolean("ravel.test.transform.refusal")) {
            className = "dev.ravel.jvm.refusal.RefusalTarget";
        } else if (Boolean.getBoolean("ravel.test.transform.persistence")) {
            className = linkedTargetClass("worldOpenPersistence");
        } else if (Boolean.getBoolean("ravel.test.transform.content")) {
            className = linkedTargetClass("registryBuiltInFreeze");
        } else if (Boolean.getBoolean("ravel.test.transform.outward")) {
            className = linkedTargetClass("clientTick");
        } else if (Boolean.getBoolean("ravel.test.transform.members")) {
            className = "net.minecraft.util.SmoothDouble";
        } else {
            className = "net.minecraft.util.StringUtil";
        }
        try {
            // Use the agent's game loader rather than the system loader.
            Members.game(className);
        } catch (ReflectiveOperationException absent) {
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

    /** Encodes admitted claims, including absent, failed and malformed crossing cases. */
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
            // kind:namespace:path, then a variable number of kind-specific fields.
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

    public static void clientTick() {
        invocations++;
    }

    // Pass externally authored wire payloads through verbatim.
    public static String ravelOutwardActions() {
        invocations++;
        String answer = System.getProperty("ravel.test.outward.actions", "empty");
        if (answer.equals("null")) {
            return null;
        }
        return answer.equals("empty") ? "" : answer;
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

    /** The seam under a version whose world load carries the selected data pack list. */
    public static boolean worldOpenPersistence(
            Object access, Object packs, Object stem, Object gameRules, boolean newWorld) {
        requireOpenArity(5);
        invocations++;
        return false;
    }

    /** The same seam under a version whose world load takes no rule overrides. */
    public static boolean worldOpenPersistence(
            Object access, Object packs, Object stem, boolean newWorld) {
        requireOpenArity(4);
        invocations++;
        return false;
    }

    // Both version overloads compile here; reject a call using the wrong target arity.
    private static void requireOpenArity(int arity) {
        int declared = arity(seam("worldOpenPersistence").split(":")[2]);
        if (declared != arity) {
            throw new IllegalStateException(
                    "this version's world-open seam declares " + declared
                            + " parameters and the rewritten call site passed " + arity);
        }
    }

    private static int arity(String descriptor) {
        int count = 0;
        int at = descriptor.indexOf('(') + 1;
        while (at < descriptor.length() && descriptor.charAt(at) != ')') {
            while (descriptor.charAt(at) == '[') {
                at++;
            }
            at = descriptor.charAt(at) == 'L' ? descriptor.indexOf(';', at) + 1 : at + 1;
            count++;
        }
        return count;
    }

    public static void worldSavePersistence(boolean quiet, boolean flush, boolean forced) {
        invocations++;
    }

    public static void worldCreatePackRepository(Object access) {
        invocations++;
    }

    public static void screenCreateWorldInit() {
        invocations++;
    }

    private static int stagedWrites;

    /** Nothing is staged: no mod supplied data, or the write was refused. */
    private static final int UNSTAGED = 0;

    private static final int STAGED_UNCHANGED = 1;

    private static final int STAGED_ANEW = 2;

    private static final String STAGED_CONTENTS = "new";

    public static final String PACK_FILE = "pack.txt";

    /** Where the pack was staged, or empty when staging never happened. */
    private static String stagedDirectory = "";

    public static String stagedDirectory() {
        return stagedDirectory;
    }

    public static int stagedWrites() {
        return stagedWrites;
    }

    // The game copies this directory into datapacks; "undiscovered" deliberately nests too deep.
    public static int ravelWriteStagedDatapack(String directory) {
        invocations++;
        stagedWrites++;
        stagedDirectory = directory;
        String staging = System.getProperty("ravel.test.persistence.staging", "written");
        if (staging.equals("nothing")) {
            return UNSTAGED;
        }
        java.nio.file.Path pack = staging.equals("undiscovered")
                ? java.nio.file.Path.of(directory, "deeper", "ravel")
                : java.nio.file.Path.of(directory, "ravel");
        try {
            java.nio.file.Files.createDirectories(pack);
            if (staging.equals("unchanged")) {
                // Do not change the tree when reporting it unchanged.
                return STAGED_UNCHANGED;
            }
            java.nio.file.Files.writeString(pack.resolve(PACK_FILE), STAGED_CONTENTS);
        } catch (java.io.IOException unwritten) {
            stagedDirectory = "unwritable";
            return UNSTAGED;
        }
        return STAGED_ANEW;
    }


    /** Supplies the pack id owned by the native side. */
    public static String ravelPackId() {
        invocations++;
        return "file/ravel";
    }

    /** The world the pack crossing was given, or empty when it never happened. */
    private static String datapackWorld = "";

    public static String datapackWorld() {
        return datapackWorld;
    }

    // The game must find this real directory when it builds the world's pack repository.
    public static void ravelWriteWorldDatapack(String world) {
        invocations++;
        datapackWorld = world;
        try {
            java.nio.file.Files.createDirectories(
                    java.nio.file.Path.of(world, "datapacks", "ravel"));
        } catch (java.io.IOException unwritten) {
            datapackWorld = "unwritable";
        }
    }

    public static long openCrossings() {
        return openCrossings;
    }

    public static String openAggregate() {
        return openAggregate;
    }

    /** Returns empty to admit, a diagnostic to refuse, or null for native failure. */
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
