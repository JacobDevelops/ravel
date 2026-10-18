package dev.ravel.jvm;

import java.io.FileDescriptor;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/** Policy adapter for the legacy hooks; native entry observations are not events. */
public final class LegacyRuntime {
    static final boolean PROOF = Boolean.getBoolean("ravel.legacy.proof");
    static final LegacyRegistry BLOCKS = new LegacyRegistry(4095);
    static final LegacyRegistry ITEMS = new LegacyRegistry(31999);
    private static final Map<String, AtomicLong> COUNTS = PROOF ? new ConcurrentHashMap<>() : Map.of();
    private static final Map<String, Object> RECIPES = new LinkedHashMap<>();
    private static final AtomicLong CLIENT_EVENTS = PROOF ? new AtomicLong() : null;
    private static final AtomicLong SERVER_EVENTS = PROOF ? new AtomicLong() : null;
    private static final AtomicLong RESOURCE_EVENTS = PROOF ? new AtomicLong() : null;
    private static boolean barrier;
    private static boolean installing;
    private static long generation;
    private static long completedGeneration;
    private static long atlasGeneration;
    private static Object reloading;
    private static int metadataPacked;
    private static int metadataItem;
    private static boolean metadataIsItem;
    private static Object metadataIdentity;

    static {
        if (PROOF) {
            Runtime.getRuntime().addShutdownHook(new Thread(LegacyRuntime::report, "ravel-legacy-report"));
        }
    }

    private LegacyRuntime() {
    }

    public static void observe(String id) {
        if (PROOF) {
            COUNTS.computeIfAbsent(id, key -> new AtomicLong()).incrementAndGet();
        }
    }

    public static void enter(String id, Object receiver, Object first, Object second, int number) {
        try {
            switch (id) {
                case "legacyBlockRegister" -> registration(BLOCKS, first, second, number, false);
                case "legacyItemRegister" -> registration(ITEMS, first, second, number, false);
                case "legacyClientTick" -> {
                    Hook.ravelLegacyEvent("client-tick");
                    if (PROOF) {
                        CLIENT_EVENTS.incrementAndGet();
                    }
                    LegacyOutward.perform();
                    LegacyProbe.run(receiver, false);
                }
                // Only the base method dispatches simulation: a paused override never reaches it.
                case "legacyServerTick" -> {
                    Hook.ravelLegacyEvent("server-tick");
                    if (PROOF) {
                        SERVER_EVENTS.incrementAndGet();
                    }
                    LegacyProbe.run(receiver, true);
                }
                case "legacyPayloadClient" -> LegacyPayload.receive(receiver, first, false);
                case "legacyPayloadServer" -> LegacyPayload.receive(receiver, first, true);
                case "legacyResourceReload" -> {
                    LegacyResources.install(receiver);
                    beginGeneration(receiver);
                }
                case "legacyRecipeRegister" -> checkRecipe(first.toString(), second);
                default -> { }
            }
        } catch (ReflectiveOperationException | java.io.IOException failure) {
            throw new IllegalStateException("legacy adapter failed at " + id, failure);
        }
    }

    public static void exit(String id, Object receiver, Object first, Object second,
            int number, Object result, int resultNumber) {
        try {
            switch (id) {
                case "legacyBootstrap" -> install();
                case "legacyBlockRegister" -> registration(BLOCKS, first, second, number, true);
                case "legacyItemRegister" -> registration(ITEMS, first, second, number, true);
                case "legacyBlockStateFromMetadata" -> observeMetadata(number, result, 0, false);
                case "legacyBlockMetadataFromState" -> observeMetadata(resultNumber, first, 0, false);
                case "legacyItemMetadata" -> observeMetadata(0, receiver, resultNumber, true);
                case "legacyResourceReload" -> completeReload(receiver);
                case "legacyAtlasReload" -> atlas();
                case "legacyRecipeRegister" -> recipe(first.toString(), second);
                default -> { }
            }
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("legacy adapter failed at " + id, failure);
        }
    }
    public static boolean refuse(String id, Object receiver, Object source) {
        if (!id.equals("legacyItemFireDamage")) {
            return false;
        }
        try {
            return LegacyContent.refuseFire(receiver, source);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("legacy item fire policy failed", failure);
        }
    }

    public static int paletteSize(int vanillaCount) {
        return Math.max(vanillaCount, BLOCKS.capacity() << 4);
    }


    private static synchronized void registration(
            LegacyRegistry registry, Object identifier, Object value, int number, boolean succeeded) {
        String key = identifier.toString();
        if (barrier && !installing) {
            throw new IllegalStateException("legacy registration after Ravel barrier: " + key);
        }
        if (succeeded) {
            registry.record(number, key, value);
        } else {
            registry.check(number, key, value);
        }
    }

    private static synchronized void install() throws ReflectiveOperationException {
        if (barrier) {
            return;
        }
        installing = true;
        try {
            LegacyContent.register();
            LegacyRecipes.install();
            barrier = true;
        } finally {
            installing = false;
        }
    }

    static void observeMetadata(int packed, Object value, int itemMetadata, boolean isItem) {
        if (!PROOF) {
            return;
        }
        snapshotMetadata(packed, value, itemMetadata, isItem);
    }

    private static synchronized void snapshotMetadata(
            int packed, Object value, int itemMetadata, boolean isItem) {
        if (value == null) {
            throw new IllegalStateException("legacy metadata returned no identity");
        }
        // Packed state IDs use a twelve-bit block number and four-bit metadata.
        metadataPacked = packed;
        metadataItem = itemMetadata;
        metadataIsItem = isItem;
        metadataIdentity = value;
    }

    static synchronized void beginGeneration(Object receiver) {
        generation++;
        reloading = receiver;
    }

    private static synchronized void atlas() {
        if (reloading != null) {
            atlasGeneration = generation;
        }
    }

    private static synchronized void completeReload(Object receiver) throws ReflectiveOperationException {
        if (reloading != receiver || receiver == null) {
            throw new IllegalStateException("legacy resource generation has no matching entry");
        }
        LegacyRecipes.install();
        completedGeneration = generation;
        reloading = null;
        Hook.ravelLegacyEvent("resource-reloaded");
        if (PROOF) {
            RESOURCE_EVENTS.incrementAndGet();
        }
    }

    private static synchronized void checkRecipe(String key, Object value) {
        Object old = RECIPES.get(key);
        if (old != null && old != value) {
            throw new IllegalStateException("conflicting legacy recipe identity: " + key);
        }
    }

    static synchronized void recipe(String key, Object value) {
        checkRecipe(key, value);
        RECIPES.put(key, value);
    }

    public static long hookCount(String id) {
        for (Map.Entry<String, AtomicLong> entry : COUNTS.entrySet()) {
            if (reviewedId(entry.getKey()).equals(id)) {
                return entry.getValue().get();
            }
        }
        return 0;
    }

    public static synchronized long resourceGeneration() {
        return generation;
    }

    public static synchronized long completedResourceGeneration() {
        return completedGeneration;
    }

    public static synchronized long atlasResourceGeneration() {
        return atlasGeneration;
    }

    public static synchronized boolean registrationComplete() {
        return barrier;
    }

    static synchronized Metadata lastMetadata() {
        return new Metadata(metadataPacked & 4095, metadataPacked >>> 12 & 15,
                metadataItem, metadataIsItem, metadataIdentity);
    }

    public static synchronized int recipeCount() {
        return RECIPES.size();
    }

    static long semanticCount(String kind) {
        if (!PROOF) {
            return 0;
        }
        return switch (kind) {
            case "client-tick" -> CLIENT_EVENTS.get();
            case "server-tick" -> SERVER_EVENTS.get();
            case "resource-reloaded" -> RESOURCE_EVENTS.get();
            default -> throw new IllegalArgumentException("unknown legacy semantic event");
        };
    }

    private static String reviewedId(String id) {
        StringBuilder key = new StringBuilder();
        for (int at = 0; at < id.length(); at++) {
            char value = id.charAt(at);
            if (Character.isUpperCase(value)) {
                key.append('-').append(Character.toLowerCase(value));
            } else {
                key.append(value);
            }
        }
        return "legacy." + key.substring("legacy-".length());
    }

    /** Writes to stderr's descriptor: the game's redirected streams log through a stopped logger. */
    private static void report() {
        StringBuilder text = new StringBuilder();
        COUNTS.entrySet().stream().sorted(Map.Entry.comparingByKey()).forEach(entry -> text
                .append("[ravel-legacy] hook ").append(reviewedId(entry.getKey())).append('=')
                .append(entry.getValue()).append('\n'));
        text.append("[ravel-legacy] state barrier=").append(registrationComplete())
                .append(" blocks=").append(BLOCKS.size()).append(" items=").append(ITEMS.size())
                .append(" recipes=").append(recipeCount())
                .append(" generation=").append(completedResourceGeneration()).append('\n');
        Agent.write(FileDescriptor.err, text.toString());
    }

    record Metadata(int blockId, int blockMetadata, int itemMetadata, boolean itemVariant, Object identity) {
    }
}
