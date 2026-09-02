package dev.ravel.jvm;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Registers the content Ravel's barrier admitted, on the Java side of the
 * crossing.
 *
 * <p>The loader already decides <em>that</em> a mod owns
 * {@code block:example_mod:copper}: two mods cannot claim one identifier, and a
 * claim outside the registration window is refused. What no part of Rust can do
 * is make the block exist, because ADG-4 admits a hook that observes or vetoes
 * and nothing else — a hook cannot call back into the JVM.
 * {@code docs/decisions/carrying-an-action-outward.md} settles that the way
 * ADG-7 settled persistence: the hook fires, and a hand-written adapter performs
 * the action with its own crossing. This is that adapter.
 *
 * <p>Everything here is reflective for the reason {@link Persistence} gives: this
 * class ships in the always-present agent jar, which is compiled with no client
 * jar on the class path, so a source declaring a game type would not compile.
 *
 * <p><b>What this class no longer does.</b> It built the block and the item
 * itself, member by member, and that could only ever be one version's shape.
 * 26.1.2's properties builders read a registry key back out at construction and
 * refuse a value that was never given one; 1.21.1 declares no such setter and
 * has no key to stamp. A member one version does not have cannot be declared for
 * it, so the first real 1.21.1 launch registered nothing and said so twice.
 * Construction and registration moved to {@code RavelContent} in the sibling
 * artifact, which is generated per version and compiled against that version's
 * stubs — the same answer {@code RavelWorldData} already was for persistence.
 *
 * <p>What is left here is the half that is the same under every version: read
 * the payload Rust admitted, and fail one row without failing the rest. The
 * sibling is reached by name and argument count, which is exact because those
 * are Ravel's own names on Ravel's own class.
 *
 * <p><b>Rust decides, Java performs.</b> Which claims are registrable is an
 * admission question and is answered on the other side: the payload carries only
 * the kinds a registry can take, so nothing here filters, skips, or judges a
 * claim. What it does own is failing one row without failing the rest.
 */
final class Content {

    /** The wire contract the payload is read under. */
    private static final String VERSION = "1";

    /** How many behaviour fields a block row carries, and an item row. */
    private static final int BLOCK_FIELDS = 4;
    private static final int ITEM_FIELDS = 2;

    /** The width of a kind this version has none for, which is not a width. */
    private static final int UNKNOWN_KIND = -1;

    /** The generated registration class, wherever this version's names put it. */
    static final String CONTENT = ScreenNames.RAVEL_CONTENT;

    /** The class carrying the state mapper the network protocol writes through. */
    private static final String BLOCK = GameNames.BLOCK;

    private static final String REGISTER_BLOCK = "registerBlock/6";
    private static final String REGISTER_ITEM = "registerItem/4";
    private static final String REGISTER_BLOCK_ITEM = "registerBlockItem/5";

    /**
     * Everything this adapter asks the sibling for, so a check can ask too.
     *
     * <p>Declared rather than only called, because the sibling is generated per
     * version and a build makes one version at a time. A version whose override
     * dropped an entry point compiles and ships; this is what lets the
     * per-version Java compile refuse it instead.
     */
    static final String[] SIBLING_ENTRY_POINTS = {
        REGISTER_BLOCK, REGISTER_ITEM, REGISTER_BLOCK_ITEM
    };

    private Content() {
    }

    /**
     * Registers everything admitted, at the last point the built-in registries
     * still accept entries.
     *
     * <p>Called from the {@code registry.built-in-freeze} prologue, which runs
     * before the body that closes them. Every failure is caught: a block that
     * cannot be registered is a missing block, and a missing block is visible to
     * the player in a way that a half-initialized game is not.
     */
    static void register() {
        int admitted = 0;
        int registered = 0;
        try {
            String payload = Hook.ravelAdmittedContent();
            if (payload == null) {
                // What the JVM receives when the native failed. Nothing is
                // registered on it, rather than a set nothing checked.
                Agent.diagnose("content registration was not answered for this launch");
                return;
            }
            if (payload.isEmpty()) {
                // No set is installed yet, which is a launch that has not got far
                // enough to have claimed anything rather than a failure.
                return;
            }
            List<String> fields = fields(payload);
            if (fields.size() < 2 || !fields.get(0).equals(VERSION)) {
                Agent.diagnose("content registration was offered a payload it cannot read");
                return;
            }
            admitted = Integer.parseInt(fields.get(1));
            List<Row> rows = new ArrayList<>();
            int at = 2;
            for (int row = 0; row < admitted; row++) {
                // A row is its three identity fields, its owner, and then as many
                // behaviour fields as its kind declares. The kind is the field
                // before them, so the width is known by the time it is needed and
                // nothing has to be tagged.
                if (at + 3 >= fields.size()) {
                    Agent.diagnose("content registration was offered fewer rows than it declares");
                    break;
                }
                String kind = fields.get(at);
                // A kind this version does not know cannot be stepped over: a
                // row's width is what its kind says it is, so guessing one reads
                // the next row's fields as this row's behaviour and every row
                // after it silently. Reading on is refused, the way a payload
                // written under another contract is refused whole.
                int width = switch (kind) {
                    case "block" -> BLOCK_FIELDS;
                    case "item" -> ITEM_FIELDS;
                    default -> UNKNOWN_KIND;
                };
                if (width == UNKNOWN_KIND) {
                    Agent.diagnose("content registration was offered the kind " + kind
                            + ", whose width it does not know, so it read no row after it");
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

            // Blocks first, and this is the whole reason the payload is read into
            // rows before any of it is registered. An item whose identifier is a
            // block's identifier is that block's placing item, and it is built
            // from the block, so the block has to exist by the time the item is
            // reached. The payload's own order is the mod author's and says
            // nothing about that.
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
            // Deliberately everything. This runs inside the game's own
            // initialization, and an escape here would take the launch down over
            // a mod's block.
            Agent.diagnose("content registration failed ("
                    + unusable.getClass().getName() + ")");
        }
        if (admitted > 0) {
            // Said out loud on every launch that had anything to register, not
            // only on a partial one. A launch is the only place this surface can
            // be observed at all, and a line that appears only on failure leaves
            // success and "the seam never fired" looking identical in the log.
            Agent.diagnose("registered " + registered + " of " + admitted
                    + " admitted content entries");
        }
    }

    /**
     * Registers one entry, and reports rather than throws when it cannot.
     *
     * @return whether the entry reached its registry
     */
    private static boolean registerBlock(Row row, Map<String, Object> blocks) {
        try {
            Class<?> content = sibling();
            if (content == null) {
                return false;
            }
            List<String> behaviour = row.behaviour();
            Object block = Members.sibling(content, REGISTER_BLOCK)
                    .invoke(
                            null,
                            row.namespace(),
                            row.path(),
                            Float.parseFloat(behaviour.get(0)),
                            Float.parseFloat(behaviour.get(1)),
                            Boolean.parseBoolean(behaviour.get(2)),
                            Boolean.parseBoolean(behaviour.get(3)));
            mapStates(block);
            blocks.put(row.identifier(), block);
            return true;
        } catch (Throwable unregistered) {
            return refuse(row, unregistered);
        }
    }

    /**
     * Registers one item, as the placing item of a block of its name if there is
     * one.
     *
     * <p>Vanilla's placing item is not a plain item that happens to share a
     * block's name; it is built from the block, and that is what gives it a
     * use-on. An item registered plainly under a block's identifier looks right
     * in the inventory and does nothing when it is right clicked, which is what
     * the eighth real launch found.
     *
     * <p>Two mods cannot claim one identifier — the loader refuses that before
     * any of this — so an identifier that matches a block Ravel just registered
     * matches a block of the same mod, and matching the name is matching the
     * owner too.
     *
     * @param blocks what the block pass registered, by identifier
     */
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

    /** Reports one row that could not be registered, and registers no other. */
    private static boolean refuse(Row row, Throwable unregistered) {
        Agent.diagnose("could not register " + row.kind() + " " + row.identifier()
                + " (" + unusableName(unregistered) + ")");
        return false;
    }

    /**
     * One decoded row of the payload, before anything is registered.
     *
     * <p>The payload is read whole before any of it is registered, because an
     * item may be a block's placing item and the block has to exist first. The
     * order the rows arrive in is the mod author's.
     */
    private record Row(String kind, String namespace, String path, List<String> behaviour) {

        /** What names this row's content, which is what an item matches a block by. */
        String identifier() {
            return namespace + ":" + path;
        }
    }

    /**
     * Adds one block's states where the game writes block updates through.
     *
     * <p>Registering a block puts it in the registry and does nothing else. What
     * makes it usable is a second walk vanilla runs once, after its own blocks
     * are registered: every state of every registered block is added to the id
     * mapper the network protocol writes a block update as an index into, and
     * each state's cache is initialised. Content registered after that walk has
     * run is in the registry and in no mapper, so the game resolves the block by
     * name, places it, and then disconnects the session encoding the update —
     * which is what the seventh real launch did on the first {@code setblock}.
     *
     * <p>So Ravel runs the same walk for the blocks it registers, in the order
     * vanilla runs it. It is reflective rather than compiled beside the
     * registration because the state list is a Guava type and neither tracked
     * jar carries Guava, so a stub declaring it could not compile. Reflection
     * names no type, which is the one thing it is better at.
     *
     * <p>Vanilla also asks each block for its loot table here and drops the
     * answer, which only warms a lazy field. 26.1.2 stopped doing it and nothing
     * else changed, so Ravel does not do it either.
     */
    private static void mapStates(Object block) throws ReflectiveOperationException {
        Class<?> blocks = game(BLOCK);
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

    /** One game class, resolved through the loader the game itself is on. */
    private static Class<?> game(String name) throws ReflectiveOperationException {
        return Class.forName(name, true, ClassLoader.getSystemClassLoader());
    }

    /**
     * Ravel's registration class from the sibling artifact, or {@code null}.
     *
     * <p>Absent means no content, and that is a diagnostic rather than a refused
     * launch: a missing block is visible to the player in a way a game that
     * refused to start is not. The world persistence adapter next door makes the
     * opposite call, deliberately, because what it loses is a player's saved
     * work rather than a block.
     */
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

    /** The type that stopped a registration, unwrapped past the reflection layer. */
    private static String unusableName(Throwable thrown) {
        Throwable cause = thrown.getCause() == null ? thrown : thrown.getCause();
        return cause.getClass().getName();
    }

    /**
     * Reads a length-prefixed payload back into its fields.
     *
     * <p>Every field is {@code <length>:<that many UTF-16 code units>}, which is
     * the framing {@code crates/ravel-bootstrap/src/wire.rs} states and the
     * settings screen already reads. A length says where a field ends, so a mod
     * identifier carrying a colon cannot be misread as a separator.
     */
    private static List<String> fields(String payload) {
        List<String> read = new ArrayList<>();
        int at = 0;
        while (at < payload.length()) {
            int separator = payload.indexOf(':', at);
            if (separator < 0) {
                throw new IllegalArgumentException("a field states no length");
            }
            int length = Integer.parseInt(payload.substring(at, separator));
            int start = separator + 1;
            read.add(payload.substring(start, start + length));
            at = start + length;
        }
        return read;
    }
}
