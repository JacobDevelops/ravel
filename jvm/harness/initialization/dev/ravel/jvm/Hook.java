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
                        // The seam content is registered at. The prologue runs
                        // before the body that closes the registries.
                        linkedTarget("registryBuiltInFreeze"),
                    }
            : Boolean.getBoolean("ravel.test.transform.outward")
                    ? new HookTarget[] {
                        // The seam a queued action is performed at. Carrying an
                        // action needs no new target, which is the whole reason
                        // this one was chosen.
                        linkedTarget("clientTick"),
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
     * ADG-7's four seams, read out of the generated hook table.
     *
     * <p>{@code ravel.test.persistence.saveSeamMissing} keeps the open seam and
     * drops the save seam, which is the one arrangement that makes
     * {@code Persistence.saveSeamInstalled} answer false: the server class is
     * present and loads, its transformation matches no method, and the
     * installed-prologue marker stays empty. That is the hole ADG-7's rejected
     * alternative leaves open, and without this case its false branch never runs.
     *
     * <p>The pack and staging seams are in both sets below, because the case the
     * save seam is missing from is about the save marker and says nothing about
     * either: dropping them there would make that case silently stop covering
     * them.
     */
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

    /**
     * One entry of the generated hook table, as this version links it.
     *
     * <p>Every section that uses it runs against a stand-in game generated per
     * version, so the class, the member and the prologue's own descriptor are
     * this version's rather than the reviewed ones. This class stands in for the
     * native library and not for the inventory, so the harness reads the whole
     * entry out of the generated table and hands it in here rather than letting a
     * stand-in spell one version's names.
     *
     * <p>The prologue is Ravel's own name on Ravel's own class, so it is the same
     * word under every version and is what an entry is asked for by.
     */
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

    /** How many fields the generated table writes for one entry. */
    private static final int ENTRY_FIELDS = 7;

    /** The table entry this run named for one prologue. */
    private static String seam(String prologue) {
        String named = System.getProperty("ravel.test.target." + prologue);
        if (named == null) {
            throw new IllegalStateException(
                    prologue + " transforms a seam and this run named none");
        }
        return named;
    }

    /** The binary name of the class one seam is declared on. */
    private static String linkedTargetClass(String prologue) {
        return seam(prologue).split(":")[0].replace('/', '.');
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

    /**
     * Refuses the open seam whose argument count is not the one this version links.
     *
     * <p>Both overloads are declared because one source has to compile at every
     * version, and that is what a rewritten call site of the wrong width would
     * otherwise resolve against: the section would cross into Java, count the
     * crossing and never notice it passed a vector the game does not have. The
     * count is read from the target member's own descriptor rather than from the
     * prologue's, because the prologue's is what a wrong entry gets wrong and the
     * transformer pushes the target's parameters whatever the prologue says.
     */
    private static void requireOpenArity(int arity) {
        int declared = arity(seam("worldOpenPersistence").split(":")[2]);
        if (declared != arity) {
            throw new IllegalStateException(
                    "this version's world-open seam declares " + declared
                            + " parameters and the rewritten call site passed " + arity);
        }
    }

    /** How many parameters one method descriptor declares. */
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
