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
            : Boolean.getBoolean("ravel.test.transform.outward")
                    ? new HookTarget[] {
                        // The seam a queued action is performed at, with the
                        // internal name, member and descriptor the pinned
                        // inventory carries for `client.tick`. Carrying an
                        // action needs no new target, which is the whole reason
                        // this one was chosen.
                        new HookTarget(
                                "net/minecraft/client/Minecraft",
                                "tick",
                                "()V",
                                false,
                                "clientTick",
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
        // The seam a world's own data pack is written at, which the game reaches
        // before it has a repository to read that world's packs with. It is in
        // both sets below, because the case the save seam is missing from is
        // about the save marker and says nothing about the pack: dropping this
        // one there would make that case silently stop covering it.
        HookTarget packRepository = new HookTarget(
                "net/minecraft/server/packs/repository/ServerPacksSource",
                "createPackRepository",
                "(Lnet/minecraft/world/level/storage/LevelStorageSource$LevelStorageAccess;)"
                        + "Lnet/minecraft/server/packs/repository/PackRepository;",
                true,
                "worldCreatePackRepository",
                "(Ljava/lang/Object;)V",
                false);
        // The seam a world being created has its data staged at. In both sets
        // for the same reason the pack seam is: the missing-save case is about
        // the save marker and says nothing about staging.
        HookTarget createWorld = new HookTarget(
                "net/minecraft/client/gui/screens/worldselection/CreateWorldScreen",
                "init",
                "()V",
                false,
                "screenCreateWorldInit",
                "()V",
                false);
        if (Boolean.getBoolean("ravel.test.persistence.saveSeamMissing")) {
            return new HookTarget[] {open, packRepository, createWorld};
        }
        return new HookTarget[] {
            open,
            packRepository,
            createWorld,
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
        } else if (Boolean.getBoolean("ravel.test.transform.outward")) {
            className = "net.minecraft.client.Minecraft";
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

    public static void clientTick() {
        invocations++;
    }

    /**
     * Answers the outward crossing the way the property asks.
     *
     * <p>{@code empty} is a tick nobody asked for anything on and {@code null}
     * is what the JVM receives when the native failed. Anything else is handed
     * through verbatim, which is how the harness drives this side with the exact
     * payload {@code crates/ravel-bootstrap/src/outward.rs} pins — the two ends
     * of this crossing are two languages, so a helper that built the payload
     * here would only prove Java agrees with itself.
     */
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

    public static boolean worldOpenPersistence(
            Object access, Object packs, Object stem, Object gameRules, boolean newWorld) {
        invocations++;
        return false;
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

    /** How many times the staging native was asked, which the guard bounds. */
    private static int stagedWrites;

    /** Nothing is staged: no mod supplied data, or the write was refused. */
    private static final int UNSTAGED = 0;

    /** The staged pack holds what it already held. */
    private static final int STAGED_UNCHANGED = 1;

    /** The staged pack's files are not the ones that were there. */
    private static final int STAGED_ANEW = 2;

    /** What a pack this writes holds, which is not what a replaced one held. */
    private static final String STAGED_CONTENTS = "new";

    /** The one file a staged pack holds, which is what a load of it reads. */
    public static final String PACK_FILE = "pack.txt";

    /** Where the pack was staged, or empty when staging never happened. */
    private static String stagedDirectory = "";

    public static String stagedDirectory() {
        return stagedDirectory;
    }

    public static int stagedWrites() {
        return stagedWrites;
    }

    /**
     * Answers the staging crossing and writes the pack where the screen loads.
     *
     * <p>One directory level shallower than the world crossing below, because the
     * game copies this directory's contents into the world it makes. Writing a
     * {@code datapacks} directory here would put the pack at
     * {@code datapacks/datapacks/ravel} in the created world.
     *
     * <p>Which of the answers Rust would have given is a property, like the two
     * world verdicts: what a run is about is what the caller does with an answer,
     * not how the pack writer arrived at one. {@code undiscovered} is the
     * exception that writes rather than answers — it puts the pack a level too
     * deep, which is a pack on disk that no scan of this directory finds under
     * the id the crossing names.
     */
    public static int ravelWriteStagedDatapack(String directory) {
        invocations++;
        stagedWrites++;
        stagedDirectory = directory;
        String staging = System.getProperty("ravel.test.persistence.staging", "written");
        if (staging.equals("nothing")) {
            // The launch whose mods supply no data half. Nothing is staged and
            // the caller must not ask the screen to reload for it.
            return UNSTAGED;
        }
        java.nio.file.Path pack = staging.equals("undiscovered")
                ? java.nio.file.Path.of(directory, "deeper", "ravel")
                : java.nio.file.Path.of(directory, "ravel");
        try {
            java.nio.file.Files.createDirectories(pack);
            if (staging.equals("unchanged")) {
                // The answer that says the tree on disk is the one the mods
                // supply, so writing a file here would contradict it.
                return STAGED_UNCHANGED;
            }
            java.nio.file.Files.writeString(pack.resolve(PACK_FILE), STAGED_CONTENTS);
        } catch (java.io.IOException unwritten) {
            stagedDirectory = "unwritable";
            return UNSTAGED;
        }
        return STAGED_ANEW;
    }


    /**
     * Answers the id crossing, which Rust composes from the directory it writes.
     *
     * <p>Spelled here because this class stands in for the native library, which
     * is the side that owns it. Nothing on the game's side of the seam and
     * nothing in the agent jar spells it at all.
     */
    public static String ravelPackId() {
        invocations++;
        return "file/ravel";
    }

    /** The world the pack crossing was given, or empty when it never happened. */
    private static String datapackWorld = "";

    /** Where the pack was asked for, which says the crossing carried a directory. */
    public static String datapackWorld() {
        return datapackWorld;
    }

    /**
     * Answers the pack crossing, records the world it named, and writes the pack.
     *
     * <p>It writes a real directory because that is what the native side does,
     * and because the filesystem is the only thing this class and the stand-in
     * game share: this class is compiled without the game on its class path and
     * the game is compiled without this one, exactly as the two sides are in a
     * launch. The seam's whole claim is that the pack is on disk before the
     * world's repository is built over that directory, and a listing taken there
     * is what settles it.
     */
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
