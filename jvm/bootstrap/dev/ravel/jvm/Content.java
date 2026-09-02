package dev.ravel.jvm;

import java.util.ArrayList;
import java.util.List;

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

    /** The generated registration class, wherever this version's names put it. */
    static final String CONTENT = ScreenNames.RAVEL_CONTENT;

    private static final String REGISTER_BLOCK = "registerBlock/6";
    private static final String REGISTER_ITEM = "registerItem/4";

    /**
     * Everything this adapter asks the sibling for, so a check can ask too.
     *
     * <p>Declared rather than only called, because the sibling is generated per
     * version and a build makes one version at a time. A version whose override
     * dropped an entry point compiles and ships; this is what lets the
     * per-version Java compile refuse it instead.
     */
    static final String[] SIBLING_ENTRY_POINTS = {REGISTER_BLOCK, REGISTER_ITEM};

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
                int width = kind.equals("block") ? BLOCK_FIELDS : ITEM_FIELDS;
                if (at + 4 + width > fields.size()) {
                    Agent.diagnose("content registration was offered a row missing its behaviour");
                    break;
                }
                List<String> behaviour = fields.subList(at + 4, at + 4 + width);
                if (one(kind, fields.get(at + 1), fields.get(at + 2), behaviour)) {
                    registered++;
                }
                at += 4 + width;
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
    private static boolean one(
            String kind, String namespace, String path, List<String> behaviour) {
        try {
            Class<?> content = sibling();
            if (content == null) {
                return false;
            }
            if (kind.equals("block")) {
                Members.sibling(content, REGISTER_BLOCK)
                        .invoke(
                                null,
                                namespace,
                                path,
                                Float.parseFloat(behaviour.get(0)),
                                Float.parseFloat(behaviour.get(1)),
                                Boolean.parseBoolean(behaviour.get(2)),
                                Boolean.parseBoolean(behaviour.get(3)));
            } else {
                Members.sibling(content, REGISTER_ITEM)
                        .invoke(
                                null,
                                namespace,
                                path,
                                Integer.parseInt(behaviour.get(0)),
                                Boolean.parseBoolean(behaviour.get(1)));
            }
            return true;
        } catch (Throwable unregistered) {
            Agent.diagnose("could not register " + kind + " " + namespace + ":" + path
                    + " (" + unusableName(unregistered) + ")");
            return false;
        }
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
            Agent.diagnose("content registration has no component to register through");
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
