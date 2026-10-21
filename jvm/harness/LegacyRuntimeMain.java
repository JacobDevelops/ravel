package dev.ravel.jvm;

import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.util.List;

/** Exercises policy helpers and the actual pinned vanilla members through premain. */
public final class LegacyRuntimeMain {
    private LegacyRuntimeMain() {
    }

    public static void main(String[] arguments) throws ReflectiveOperationException {
        if (arguments.length == 0 || arguments[0].equals("unit")) {
            helpers();
            System.out.println("legacy-policy=passed");
            return;
        }
        Agent.entered(0, LegacyAccess.type("LEGACY_CLIENT"));
        LegacyAccess.call("LEGACY_BOOTSTRAP", "init", null);
        switch (arguments[0]) {
            case "content" -> {
                content();
                recipes();
                palette();
                lifecycle();
            }
            case "members" -> members();
            case "outward" -> outwardQueue();
            default -> throw new IllegalArgumentException("unknown legacy fixture section");
        }
        System.out.println("section=" + arguments[0]);
    }

    public static void exercise(Object actualClient) throws ReflectiveOperationException {
        content();
        members();
        recipes();
        palette();
        require(LegacyRuntime.completedResourceGeneration() > 0
                && LegacyRuntime.completedResourceGeneration() == LegacyRuntime.resourceGeneration(),
                "live recipe lookup did not follow a successfully completed actual resource reload");
        fireResistance(actualClient);
        require(LegacyOutward.performed() > 0,
                "actual native tick queue has not reached the legacy sound manager");
        require(actualClient == LegacyAccess.call("LEGACY_CLIENT", "getInstance", null),
                "probe did not run on the actual client");
        System.out.println("legacy-live=content,members,outward-manager-accepted");
        System.out.println("legacy-resource-recipes=actual-lookup,idempotent-reload,duplicate-preserved");
    }

    private static void content() throws ReflectiveOperationException {
        require(LegacyRuntime.registrationComplete(), "vanilla bootstrap did not complete Ravel barrier");
        List<LegacyContent.Row> rows = LegacyContent.decode(Hook.ravelAdmittedContent());
        require(!rows.isEmpty(), "content fixture must admit real block/item declarations");
        int previousBlock = -1;
        for (LegacyContent.Row row : rows) {
            boolean block = row.kind().equals("block");
            LegacyRegistry registry = block ? LegacyRuntime.BLOCKS : LegacyRuntime.ITEMS;
            Object value = registry.value(row.key());
            require(value != null, "admitted declaration absent from vanilla: " + row.key());
            int id = (Integer) LegacyAccess.call(block ? "LEGACY_BLOCK" : "LEGACY_ITEM", "getId", null, value);
            require(id == registry.number(row.key()), "vanilla numeric ID differs from barrier record");
            if (block) {
                require(id > previousBlock && id <= 4095, "block IDs are not deterministic and bounded");
                previousBlock = id;
            }
        }
        LegacyContent.Row first = rows.getFirst();
        LegacyRegistry registry = first.kind().equals("block") ? LegacyRuntime.BLOCKS : LegacyRuntime.ITEMS;
        Object key = LegacyAccess.create("LEGACY_IDENTIFIER", first.namespace(), first.path());
        try {
            LegacyAccess.call(first.kind().equals("block") ? "LEGACY_BLOCK" : "LEGACY_ITEM", "register", null,
                    registry.number(first.key()), key, registry.value(first.key()));
            throw new AssertionError("actual transformed registration bypassed the barrier");
        } catch (InvocationTargetException refused) {
            require(refused.getCause() instanceof IllegalStateException, "wrong late-registration refusal");
        }
    }

    private static void members() throws ReflectiveOperationException {
        synchronized (LegacyRuntime.class) {
            metadataRoundTrips();
        }
    }

    private static void metadataRoundTrips() throws ReflectiveOperationException {
        long decoded = LegacyRuntime.hookCount("legacy.block-state-from-metadata");
        long encoded = LegacyRuntime.hookCount("legacy.block-metadata-from-state");
        int block = LegacyRuntime.BLOCKS.number("minecraft:wool");
        int packed = block | 11 << 12;
        Object state = LegacyAccess.call("LEGACY_BLOCK", "deserialize", null, packed);
        LegacyRuntime.Metadata observed = LegacyRuntime.lastMetadata();
        require(observed.identity() == state && observed.blockId() == block
                && observed.blockMetadata() == 11, "decode callback lost actual argument/result identity");
        int roundTrip = (Integer) LegacyAccess.call("LEGACY_BLOCK", "serialize", null, state);
        require(roundTrip == packed, "actual vanilla block metadata round trip changed bits");
        require(LegacyRuntime.hookCount("legacy.block-state-from-metadata") > decoded
                && LegacyRuntime.hookCount("legacy.block-metadata-from-state") > encoded,
                "metadata members did not cross transformed hooks");
        Object item = LegacyRuntime.ITEMS.value("minecraft:paper");
        require(item != null, "official item registry missing paper");
        Object stack = LegacyAccess.create("LEGACY_ITEM_STACK", item, 1, 32767);
        long itemCalls = LegacyRuntime.hookCount("legacy.item-metadata");
        int metadata = (Integer) LegacyAccess.call("LEGACY_ITEM_STACK", "getMetadata", stack);
        require(metadata == 32767 && LegacyRuntime.lastMetadata().itemMetadata() == 32767,
                "item variant was truncated to block metadata width");
        require(LegacyRuntime.lastMetadata().identity() == stack
                && LegacyRuntime.hookCount("legacy.item-metadata") > itemCalls,
                "actual item receiver/result did not cross transformed hook");
    }

    private static void fireResistance(Object client) throws ReflectiveOperationException {
        Object world = LegacyAccess.field("LEGACY_CLIENT", "world", client);
        require(world != null, "fire policy requires the actual joined client world");
        Object fire = LegacyAccess.field("LEGACY_DAMAGE_SOURCE", "FIRE", null);
        Object generic = LegacyAccess.field("LEGACY_DAMAGE_SOURCE", "GENERIC", null);
        Object resistant = null;
        for (LegacyContent.Row row : LegacyContent.decode(Hook.ravelAdmittedContent())) {
            if (row.kind().equals("item") && row.behaviour().get(1).equals("true")) {
                resistant = LegacyRuntime.ITEMS.value(row.key());
                break;
            }
        }
        require(resistant != null, "live fixture must declare a fire-resistant item");
        Object stack = LegacyAccess.create("LEGACY_ITEM_STACK", resistant, 1, 7);
        Object entity = LegacyAccess.create("LEGACY_ITEM_ENTITY", world, 0D, 64D, 0D, stack);
        require(LegacyContent.refuseFire(entity, fire), "owned fire-resistant item not selected");
        require(!LegacyContent.refuseFire(entity, generic), "non-fire damage incorrectly selected");
        long calls = LegacyRuntime.hookCount("legacy.item-fire-damage");
        Object accepted = LegacyAccess.call("LEGACY_ENTITY", "takeDamage", entity, fire, 20F);
        require(Boolean.FALSE.equals(accepted)
                && LegacyRuntime.hookCount("legacy.item-fire-damage") > calls,
                "actual transformed fire damage did not cross its hook and return false");
        Object vanilla = LegacyAccess.create("LEGACY_ITEM_STACK",
                LegacyRuntime.ITEMS.value("minecraft:paper"), 1, 7);
        Object untouched = LegacyAccess.create("LEGACY_ITEM_ENTITY", world, 0D, 64D, 0D, vanilla);
        require(!LegacyContent.refuseFire(untouched, fire), "vanilla item selected by owned policy");
        System.out.println("legacy-fire=owned-fire-refused,nonfire-and-vanilla-unselected");
    }

    private static void outwardQueue() throws ReflectiveOperationException {
        Hook.ravelLegacyEvent("client-tick");
        String payload = Hook.ravelOutwardActions();
        require(payload != null && !payload.isEmpty(), "fixture must queue an admitted native SDK sound");
        List<String> fields = Agent.fields(payload);
        require(fields.size() >= 7 && fields.get(0).equals("1") && fields.get(2).equals("audio.play")
                && fields.get(3).equals("3"), "native outward encoding differs from SDK action");
        Object key = LegacyAccess.create("LEGACY_IDENTIFIER", fields.get(4), fields.get(5));
        Object event = LegacyAccess.create("LEGACY_SOUND_EVENT", key);
        Object sound = LegacyAccess.call("LEGACY_SIMPLE_SOUND", "of", null, event,
                Float.parseFloat(fields.get(6)), 0.25F);
        require(key.equals(LegacyAccess.call("LEGACY_SOUND_INSTANCE", "getLocation", sound)),
                "actual vanilla sound factory changed admitted source identity");
        require(Hook.ravelOutwardActions().isEmpty(), "native outward drain delivered the action twice");
        require(LegacyOutward.performAll(wire("1", "1", "unsupported", "0")) == 0,
                "unsupported outward verb was reported performed");
        require(LegacyOutward.performAll(wire("1", "1", "audio.play", "3", "test", "sound", "NaN")) == 0,
                "nonfinite audio pitch was not refused");
        require(LegacyOutward.performAll(wire("1", "1", "audio.play", "-1")) == 0,
                "negative argument count was not refused");
        System.out.println("legacy-outward=actual-native-queue-and-vanilla-source-not-playback");
    }

    private static void lifecycle() {
        Object receiver = new Object();
        require(LegacyRuntime.PROOF, "lifecycle fixture requires ravel.legacy.proof=true");
        long server = LegacyRuntime.semanticCount("server-tick");
        LegacyRuntime.enter("legacyIntegratedServerTick", receiver, null, null, 0);
        LegacyRuntime.enter("legacyServerWorldTick", receiver, null, null, 0);
        require(LegacyRuntime.semanticCount("server-tick") == server,
                "paused integrated/world observations became simulation events");
        LegacyRuntime.enter("legacyServerTick", receiver, null, null, 0);
        LegacyRuntime.enter("legacyServerWorldTick", receiver, null, null, 0);
        require(LegacyRuntime.semanticCount("server-tick") == server + 1,
                "one base simulation tick was duplicated by its world hook");
        long events = LegacyRuntime.semanticCount("resources-reloaded");
        long completed = LegacyRuntime.completedResourceGeneration();
        LegacyRuntime.beginGeneration(receiver);
        long failed = LegacyRuntime.resourceGeneration();
        LegacyRuntime.exit("legacyAtlasReload", receiver, null, null, 0, null, 0);
        require(LegacyRuntime.atlasResourceGeneration() == failed,
                "atlas did not join the active generation");
        require(LegacyRuntime.completedResourceGeneration() == completed
                && LegacyRuntime.semanticCount("resources-reloaded") == events,
                "incomplete/throwing reload published successful completion");
        LegacyRuntime.beginGeneration(receiver);
        long succeeded = LegacyRuntime.resourceGeneration();
        LegacyRuntime.exit("legacyAtlasReload", receiver, null, null, 0, null, 0);
        LegacyRuntime.exit("legacyResourceReload", receiver, null, null, 0, null, 0);
        require(succeeded == failed + 1 && LegacyRuntime.completedResourceGeneration() == succeeded
                && LegacyRuntime.atlasResourceGeneration() == succeeded
                && LegacyRuntime.semanticCount("resources-reloaded") == events + 1,
                "successful reload/atlas did not publish exactly one matching generation");
        LegacyRuntime.exit("legacyAtlasReload", receiver, null, null, 0, null, 0);
        require(LegacyRuntime.atlasResourceGeneration() == succeeded,
                "registration-time atlas changed the completed resource generation");
        LegacyRuntime.beginGeneration(receiver);
        LegacyRuntime.exit("legacyResourceReload", receiver, null, null, 0, null, 0);
        long registered = LegacyRuntime.completedResourceGeneration();
        require(registered == succeeded + 1 && LegacyRuntime.atlasResourceGeneration() == succeeded,
                "reload without a stitch moved the installed atlas generation");
        LegacyRuntime.exit("legacyAtlasReload", receiver, null, null, 0, null, 0);
        require(LegacyRuntime.atlasResourceGeneration() == registered,
                "registration-time atlas did not adopt its completed resource generation");
        Object first = new Object();
        LegacyRuntime.enter("legacyRecipeRegister", receiver, "fixture:lifecycle", first, 0);
        LegacyRuntime.exit("legacyRecipeRegister", receiver, "fixture:lifecycle", first, 0, null, 0);
        int recipes = LegacyRuntime.recipeCount();
        refused(() -> LegacyRuntime.enter("legacyRecipeRegister", receiver,
                "fixture:lifecycle", new Object(), 0));
        require(LegacyRuntime.recipeCount() == recipes, "recipe conflict overwrote the admitted set");
        int blocks = LegacyRuntime.BLOCKS.size();
        int items = LegacyRuntime.ITEMS.size();
        LegacyRuntime.exit("legacyBootstrap", receiver, null, null, 0, null, 0);
        require(LegacyRuntime.BLOCKS.size() == blocks && LegacyRuntime.ITEMS.size() == items,
                "second bootstrap installed declarations twice");
    }

    private static void recipes() throws ReflectiveOperationException {
        java.util.Map<String, Object> installed = LegacyRecipes.installed();
        require(!installed.isEmpty(), "fixture must admit authored legacy recipe JSON");
        LegacyRecipes.install();
        for (java.util.Map.Entry<String, Object> entry : installed.entrySet()) {
            String key = entry.getKey();
            int colon = key.indexOf(':');
            Object identifier = LegacyAccess.create("LEGACY_IDENTIFIER",
                    key.substring(0, colon), key.substring(colon + 1));
            require(LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "getRecipe", null, identifier)
                    == entry.getValue(), "actual recipe lookup lost admitted identity");
            Object different = LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "getRecipe", null,
                    LegacyAccess.create("LEGACY_IDENTIFIER", "minecraft", "stick"));
            require(different != null && different != entry.getValue(), "missing vanilla recipe conflict fixture");
            try {
                LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "register", null, identifier, different);
                throw new AssertionError("duplicate recipe key overwrote the first identity");
            } catch (InvocationTargetException refused) {
                require(refused.getCause() instanceof IllegalStateException, "wrong recipe conflict refusal");
            }
            require(LegacyAccess.call("LEGACY_CRAFTING_MANAGER", "getRecipe", null, identifier)
                    == entry.getValue(), "duplicate refusal changed the actual vanilla recipe");
        }
    }

    private static void palette() throws ReflectiveOperationException {
        LegacyContent.Row row = LegacyContent.decode(Hook.ravelAdmittedContent()).stream()
                .filter(entry -> entry.kind().equals("block")).reduce((left, right) -> right).orElseThrow();
        Object block = LegacyRuntime.BLOCKS.value(row.key());
        int blockId = (Integer) LegacyAccess.call("LEGACY_BLOCK", "getId", null, block);
        require(blockId == LegacyRuntime.BLOCKS.number(row.key()) && blockId >= 512,
                "palette fixture must admit a real block beyond vanilla thirteen-bit state storage");
        Object definition = LegacyAccess.call("LEGACY_BLOCK", "stateDefinition", block);
        Object state = ((Iterable<?>) LegacyAccess.call("LEGACY_STATE_DEFINITION", "all", definition))
                .iterator().next();
        int metadata = (Integer) LegacyAccess.call("LEGACY_BLOCK", "getMetadataFromState", block, state);
        require((blockId << 4 | metadata) >= 8192, "palette state does not exceed vanilla bit width");
        Object stateIds = LegacyAccess.field("LEGACY_BLOCK", "STATE_REGISTRY", null);
        int vanillaSize = (Integer) LegacyAccess.call("LEGACY_STATE_IDS", "size", stateIds);
        int capacity = LegacyRuntime.BLOCKS.capacity() << 4;
        require(LegacyRuntime.paletteSize(vanillaSize) == Math.max(vanillaSize, capacity),
                "production palette sizing lost the actual reflected block registry capacity");
        int expectedBits = Integer.SIZE - Integer.numberOfLeadingZeros(capacity - 1);
        require(expectedBits > 13, "palette fixture does not require widening vanilla storage");
        Object container = LegacyAccess.create("LEGACY_PALETTED_CONTAINER");
        LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "setBits", container, 9);
        List<java.lang.reflect.Field> widths = java.util.Arrays.stream(container.getClass().getDeclaredFields())
                .filter(field -> field.getType() == int.class
                        && !java.lang.reflect.Modifier.isStatic(field.getModifiers())).toList();
        require(widths.size() == 1, "pinned container no longer has exactly one instance bit-width field");
        java.lang.reflect.Field width = widths.getFirst();
        width.setAccessible(true);
        require(width.getInt(container) == expectedBits,
                "actual global container did not apply the targeted palette width fix");
        LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "set", container, 0, 0, 0, state);
        require(LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "get", container, 0, 0, 0) == state,
                "global palette truncated admitted high state ID");
        byte[] base = new byte[4096];
        Object low = LegacyAccess.create("LEGACY_NIBBLE_STORAGE");
        Object high = LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "write", container, base, low);
        require(high != null, "high block ID was omitted from split nibble storage");
        Object decoded = LegacyAccess.create("LEGACY_PALETTED_CONTAINER");
        LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "setBits", decoded, 9);
        LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "read", decoded, base, low, high);
        require(LegacyAccess.call("LEGACY_PALETTED_CONTAINER", "get", decoded, 0, 0, 0) == state,
                "split nibble storage lost admitted high block identity");
        require(width.getInt(decoded) == expectedBits,
                "split nibble decode did not retain the widened actual global palette");
        System.out.println("legacy-palette=global-high-state,split-nibble-roundtrip");
    }

    private static void helpers() {
        require(LegacyRuntime.PROOF, "legacy helper fixture requires -Dravel.legacy.proof=true");
        require("fixture:nested/test".equals(
                LegacyRecipes.key("data/fixture/recipe/nested/test.json")), "legacy recipe path mapping");
        require("fixture:test".equals(LegacyRecipes.key("assets/fixture/recipes/test.json")),
                "native legacy assets recipe path mapping");
        require(LegacyRecipes.key("assets/fixture/textures/test.json") == null,
                "unrelated client asset misclassified as recipe");
        require(LegacyRecipes.key("data/fixture/recipes/test.json") == null,
                "noncanonical data recipe directory accepted");
        LegacyRegistry registry = new LegacyRegistry(4095);
        Object first = new Object();
        registry.record(4095, "fixture:first", first);
        registry.record(4095, "fixture:first", first);
        require(registry.capacity() == 4096, "full twelve-bit registry capacity truncated");
        require(registry.value("fixture:first") == first && registry.size() == 1,
                "idempotent identity registration changed identity");
        refused(() -> registry.record(4095, "fixture:second", new Object()));
        refused(() -> registry.record(1, "fixture:first", new Object()));
        refused(() -> registry.record(1, "fixture:second", first));
        refused(registry::nextId);
        require(registry.value("fixture:first") == first && registry.size() == 1,
                "conflicting numeric/key/identity registration overwrote the first value");
        require(LegacyRuntime.paletteSize(5365) == 5365, "empty registry narrowed vanilla palette");
        LegacyRuntime.BLOCKS.record(4095, "fixture:palette-capacity", new Object());
        require(LegacyRuntime.BLOCKS.capacity() == 4096
                && LegacyRuntime.paletteSize(5365) == 65536,
                "production palette helper did not cover the full twelve-bit block registry");
        require(LegacyRuntime.paletteSize(65537) == 65537,
                "production palette helper narrowed a larger vanilla state count");
        Object state = new Object();
        LegacyRuntime.observeMetadata(4095 | 15 << 12, state, 0, false);
        require(LegacyRuntime.lastMetadata().blockId() == 4095
                && LegacyRuntime.lastMetadata().blockMetadata() == 15, "packed metadata bits changed");
        LegacyRuntime.observeMetadata(0, state, 65535, true);
        require(LegacyRuntime.lastMetadata().itemMetadata() == 65535, "item metadata narrowed");
        require(LegacyPayload.validChannel("fixture:updates"), "owned channel syntax refused");
        require(!LegacyPayload.validChannel("MC|Brand") && !LegacyPayload.validChannel("fixture:a:b")
                && !LegacyPayload.validChannel("fixture:" + "x".repeat(20)), "invalid channel admitted");
        List<LegacyContent.Row> rows = LegacyContent.decode(wire("1", "2", "item", "fixture", "z", "fixture",
                "16", "true", "block", "fixture", "a", "fixture", "1", "2", "true", "false"));
        require(rows.getFirst().key().equals("fixture:a"), "admitted ordering is not stable");
        refused(() -> LegacyContent.decode(wire("1", "1", "item", "fixture", "x", "fixture", "65", "false")));
        refused(() -> LegacyContent.decode(wire("1", "0", "extra")));
        LegacyRuntime.recipe("fixture:recipe", state);
        refused(() -> LegacyRuntime.recipe("fixture:recipe", new Object()));
    }

    private static String wire(String... fields) {
        StringBuilder output = new StringBuilder();
        for (String field : fields) {
            output.append(field.getBytes(StandardCharsets.UTF_8).length).append(':').append(field);
        }
        return output.toString();
    }

    private static void refused(Runnable operation) {
        try {
            operation.run();
        } catch (IllegalArgumentException | IllegalStateException expected) {
            return;
        }
        throw new AssertionError("conflicting or malformed input was accepted");
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
