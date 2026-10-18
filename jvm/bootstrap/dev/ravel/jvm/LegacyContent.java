package dev.ravel.jvm;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Installs the native admitted set into vanilla's numeric registries. */
final class LegacyContent {
    private static final Set<Object> FIRE_RESISTANT =
            java.util.Collections.newSetFromMap(new IdentityHashMap<>());

    private LegacyContent() {
    }

    static void register() throws ReflectiveOperationException {
        String payload = Hook.ravelAdmittedContent();
        if (payload == null) {
            throw new IllegalStateException("legacy registration has no admitted native set");
        }
        List<Row> rows = decode(payload);
        for (Row row : rows) {
            if (row.kind().equals("block")) {
                registerBlock(row, LegacyRuntime.BLOCKS.nextId());
            }
        }
        for (Row row : rows) {
            if (row.kind().equals("item")) {
                registerItem(row, LegacyRuntime.ITEMS.nextId());
            }
        }
    }

    private static void registerBlock(Row row, int id) throws ReflectiveOperationException {
        Class<?> implementation = LegacyScreens.load(ScreenNames.RAVEL_LEGACY_BLOCK);
        Object block = implementation.getConstructor(float.class, float.class,
                boolean.class, boolean.class).newInstance(Float.parseFloat(row.behaviour().get(0)),
                        Float.parseFloat(row.behaviour().get(1)), flag(row.behaviour().get(2)),
                        flag(row.behaviour().get(3)));
        Object key = LegacyAccess.create("LEGACY_IDENTIFIER", row.namespace(), row.path());
        LegacyRuntime.BLOCKS.check(id, row.key(), block);
        LegacyAccess.call("LEGACY_BLOCK", "register", null, id, key, block);
        LegacyRuntime.BLOCKS.record(id, row.key(), block);
        mapStates(block, id);
    }

    private static void mapStates(Object block, int id) throws ReflectiveOperationException {
        Object definition = LegacyAccess.call("LEGACY_BLOCK", "stateDefinition", block);
        Object states = LegacyAccess.call("LEGACY_STATE_DEFINITION", "all", definition);
        Object mapper = LegacyAccess.field("LEGACY_BLOCK", "STATE_REGISTRY", null);
        for (Object state : (Iterable<?>) states) {
            int metadata = (Integer) LegacyAccess.call("LEGACY_BLOCK", "getMetadataFromState", block, state);
            if (metadata < 0 || metadata > 15) {
                throw new IllegalStateException("legacy block metadata is not four-bit");
            }
            // Chunk/network state IDs use block << 4 | metadata, unlike serialize's packing.
            LegacyAccess.call("LEGACY_STATE_IDS", "put", mapper, state, id << 4 | metadata);
        }
    }

    private static void registerItem(Row row, int id) throws ReflectiveOperationException {
        Object block = LegacyRuntime.BLOCKS.value(row.key());
        Object item = block == null ? LegacyAccess.create("LEGACY_ITEM")
                : LegacyAccess.create("LEGACY_BLOCK_ITEM", block);
        LegacyAccess.call("LEGACY_ITEM", "setMaxStackSize", item,
                Integer.parseInt(row.behaviour().get(0)));
        Object key = LegacyAccess.create("LEGACY_IDENTIFIER", row.namespace(), row.path());
        LegacyRuntime.ITEMS.check(id, row.key(), item);
        LegacyAccess.call("LEGACY_ITEM", "register", null, id, key, item);
        LegacyRuntime.ITEMS.record(id, row.key(), item);
        if (block != null) {
            Object blockItems = LegacyAccess.field("LEGACY_ITEM", "BLOCK_ITEMS", null);
            Map.class.getMethod("put", Object.class, Object.class).invoke(blockItems, block, item);
        }
        if (flag(row.behaviour().get(1))) {
            synchronized (FIRE_RESISTANT) {
                FIRE_RESISTANT.add(item);
            }
        }
    }

    static boolean refuseFire(Object entity, Object source) throws ReflectiveOperationException {
        if (!Boolean.TRUE.equals(LegacyAccess.call("LEGACY_DAMAGE_SOURCE", "isFire", source))) {
            return false;
        }
        Object stack = LegacyAccess.call("LEGACY_ITEM_ENTITY", "getItem", entity);
        Object item = LegacyAccess.call("LEGACY_ITEM_STACK", "getItem", stack);
        synchronized (FIRE_RESISTANT) {
            return FIRE_RESISTANT.contains(item);
        }
    }

    static List<Row> decode(String payload) {
        if (payload.isEmpty()) {
            return List.of();
        }
        List<String> fields = Agent.fields(payload);
        if (fields.size() < 2 || !fields.get(0).equals("1")) {
            throw new IllegalArgumentException("unsupported legacy content document");
        }
        int count = Integer.parseInt(fields.get(1));
        if (count < 0 || count > 32768) {
            throw new IllegalArgumentException("invalid legacy content count");
        }
        List<Row> rows = new ArrayList<>();
        Set<String> keys = new HashSet<>();
        int at = 2;
        for (int index = 0; index < count; index++) {
            if (at + 4 > fields.size()) {
                throw new IllegalArgumentException("truncated legacy content identity");
            }
            String kind = fields.get(at);
            int width = switch (kind) {
                case "block" -> 4;
                case "item" -> 2;
                default -> throw new IllegalArgumentException("unknown legacy content kind");
            };
            if (at + 4 + width > fields.size()) {
                throw new IllegalArgumentException("truncated legacy content behavior");
            }
            Row row = new Row(kind, fields.get(at + 1), fields.get(at + 2),
                    List.copyOf(fields.subList(at + 4, at + 4 + width)));
            validate(row);
            if (!keys.add(kind + "/" + row.key())) {
                throw new IllegalArgumentException("duplicate legacy content identity");
            }
            rows.add(row);
            at += 4 + width;
        }
        if (at != fields.size()) {
            throw new IllegalArgumentException("trailing legacy content fields");
        }
        rows.sort(Comparator.comparing(Row::key).thenComparing(Row::kind));
        return rows;
    }

    private static void validate(Row row) {
        if (!row.namespace().matches("[a-z0-9_.-]+") || !row.path().matches("[a-z0-9_./-]+")) {
            throw new IllegalArgumentException("invalid legacy content key");
        }
        List<String> behaviour = row.behaviour();
        if (row.kind().equals("block")) {
            float hardness = Float.parseFloat(behaviour.get(0));
            float resistance = Float.parseFloat(behaviour.get(1));
            if (!Float.isFinite(hardness) || !Float.isFinite(resistance)) {
                throw new IllegalArgumentException("nonfinite legacy block behavior");
            }
            flag(behaviour.get(2));
            flag(behaviour.get(3));
        } else {
            int size = Integer.parseInt(behaviour.get(0));
            if (size < 1 || size > 64) {
                throw new IllegalArgumentException("invalid legacy stack size");
            }
            flag(behaviour.get(1));
        }
    }

    private static boolean flag(String value) {
        return switch (value) {
            case "true" -> true;
            case "false" -> false;
            default -> throw new IllegalArgumentException("invalid legacy content flag");
        };
    }

    record Row(String kind, String namespace, String path, List<String> behaviour) {
        String key() {
            return namespace + ":" + path;
        }
    }
}
