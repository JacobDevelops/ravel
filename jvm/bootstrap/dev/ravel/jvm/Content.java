package dev.ravel.jvm;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Registers admitted content through the version-specific sibling artifact. */
final class Content {

    private static final String VERSION = "1";

    private static final int BLOCK_FIELDS = 4;
    private static final int ITEM_FIELDS = 2;

    private static final int UNKNOWN_KIND = -1;

    static final String CONTENT = ScreenNames.RAVEL_CONTENT;

    private static final String BLOCK = GameNames.BLOCK;

    private static final String REGISTER_BLOCK = "registerBlock/6";
    private static final String REGISTER_ITEM = "registerItem/4";
    private static final String REGISTER_BLOCK_ITEM = "registerBlockItem/5";

    /** Entry points required by the per-version sibling compile. */
    static final String[] SIBLING_ENTRY_POINTS = {
        REGISTER_BLOCK, REGISTER_ITEM, REGISTER_BLOCK_ITEM
    };

    private Content() {
    }

    /** Registers blocks before their placing items, before the registries freeze. */
    static void register() {
        int admitted = 0;
        int registered = 0;
        try {
            String payload = Hook.ravelAdmittedContent();
            if (payload == null) {
                Agent.diagnose("content registration was not answered for this launch");
                return;
            }
            if (payload.isEmpty()) {
                return;
            }
            List<String> fields = Agent.fields(payload);
            if (fields.size() < 2 || !fields.get(0).equals(VERSION)) {
                Agent.diagnose("content registration was offered a payload it cannot read");
                return;
            }
            admitted = Integer.parseInt(fields.get(1));
            List<Row> rows = new ArrayList<>();
            int at = 2;
            for (int row = 0; row < admitted; row++) {
                if (at + 3 >= fields.size()) {
                    Agent.diagnose("content registration was offered fewer rows than it declares");
                    break;
                }
                String kind = fields.get(at);
                // An unknown kind has no row width, so the entire set is unreadable.
                int width = switch (kind) {
                    case "block" -> BLOCK_FIELDS;
                    case "item" -> ITEM_FIELDS;
                    default -> UNKNOWN_KIND;
                };
                if (width == UNKNOWN_KIND) {
                    Agent.diagnose("content registration was offered the kind " + kind
                            + ", whose width it does not know, so it registered none of the set");
                    rows.clear();
                    break;
                }
                if (at + 4 + width > fields.size()) {
                    Agent.diagnose("content registration was offered a row missing its behaviour");
                    break;
                }
                rows.add(new Row(
                        kind,
                        fields.get(at + 1),
                        fields.get(at + 2),
                        fields.subList(at + 4, at + 4 + width)));
                at += 4 + width;
            }

            Map<String, Object> blocks = new LinkedHashMap<>();
            for (Row row : rows) {
                if (row.kind().equals("block") && registerBlock(row, blocks)) {
                    registered++;
                }
            }
            for (Row row : rows) {
                if (row.kind().equals("item") && registerItem(row, blocks)) {
                    registered++;
                }
            }
        } catch (Throwable unusable) {
            // A mod's registration failure must not abort game initialization.
            Agent.diagnose("content registration failed ("
                    + unusable.getClass().getName() + ")");
        }
        if (admitted > 0) {
            Agent.diagnose("registered " + registered + " of " + admitted
                    + " admitted content entries");
        }
    }

    /** Registers a block and reports failures without aborting the remaining rows. */
    private static boolean registerBlock(Row row, Map<String, Object> blocks) {
        Object block;
        try {
            Class<?> content = sibling();
            if (content == null) {
                return false;
            }
            List<String> behaviour = row.behaviour();
            block = Members.sibling(content, REGISTER_BLOCK)
                    .invoke(
                            null,
                            row.namespace(),
                            row.path(),
                            Float.parseFloat(behaviour.get(0)),
                            Float.parseFloat(behaviour.get(1)),
                            Boolean.parseBoolean(behaviour.get(2)),
                            Boolean.parseBoolean(behaviour.get(3)));
        } catch (Throwable unregistered) {
            return refuse(row, unregistered);
        }

        // State mapping can fail after registration; retain the block for its placing item.
        blocks.put(row.identifier(), block);
        try {
            mapStates(block);
        } catch (Throwable unmapped) {
            Agent.diagnose("registered " + row.identifier()
                    + " but its states are not mapped, so placing it will disconnect the session ("
                    + Agent.unusableName(unmapped) + ")");
        }
        return true;
    }

    /** Registers an item, using its matching block when one was registered. */
    private static boolean registerItem(Row row, Map<String, Object> blocks) {
        try {
            Class<?> content = sibling();
            if (content == null) {
                return false;
            }
            List<String> behaviour = row.behaviour();
            Object block = blocks.get(row.identifier());
            if (block == null) {
                Members.sibling(content, REGISTER_ITEM)
                        .invoke(
                                null,
                                row.namespace(),
                                row.path(),
                                Integer.parseInt(behaviour.get(0)),
                                Boolean.parseBoolean(behaviour.get(1)));
            } else {
                Members.sibling(content, REGISTER_BLOCK_ITEM)
                        .invoke(
                                null,
                                row.namespace(),
                                row.path(),
                                Integer.parseInt(behaviour.get(0)),
                                Boolean.parseBoolean(behaviour.get(1)),
                                block);
            }
            return true;
        } catch (Throwable unregistered) {
            return refuse(row, unregistered);
        }
    }

    private static boolean refuse(Row row, Throwable unregistered) {
        Agent.diagnose("could not register " + row.kind() + " " + row.identifier()
                + " (" + Agent.unusableName(unregistered) + ")");
        return false;
    }

    /** Decoded content, collected before registration so blocks can be registered first. */
    private record Row(String kind, String namespace, String path, List<String> behaviour) {

        String identifier() {
            return namespace + ":" + path;
        }
    }

    /** Adds registered states to the network id mapper and initializes their caches. */
    private static void mapStates(Object block) throws ReflectiveOperationException {
        Class<?> blocks = Members.game(BLOCK);
        Object definition = Members.method(blocks, GameMembers.BLOCK_GET_STATE_DEFINITION)
                .invoke(block);
        Object mapper = Members.field(blocks, GameMembers.BLOCK_BLOCK_STATE_REGISTRY).get(null);
        Object states = Members.method(
                        definition.getClass(), GameMembers.STATE_DEFINITION_GET_POSSIBLE_STATES)
                .invoke(definition);
        for (Object state : (Iterable<?>) states) {
            Members.method(mapper.getClass(), GameMembers.ID_MAPPER_ADD).invoke(mapper, state);
            Members.method(state.getClass(), GameMembers.BLOCK_STATE_BASE_INIT_CACHE)
                    .invoke(state);
        }
    }

    /** Loads the registration sibling, reporting its absence without refusing launch. */
    private static Class<?> sibling() {
        if (!Screens.isInstalled()) {
            Agent.diagnose("content registration has no component to register through; "
                    + Screens.SCREEN_JAR + " is not beside the agent jar");
            return null;
        }
        try {
            return Screens.load(CONTENT);
        } catch (ClassNotFoundException | LinkageError | SecurityException absent) {
            Agent.diagnose("the content registration component could not be loaded ("
                    + absent.getClass().getName() + ")");
            return null;
        }
    }
}
