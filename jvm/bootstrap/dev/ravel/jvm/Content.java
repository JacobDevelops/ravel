package dev.ravel.jvm;

import java.lang.reflect.Method;
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
 * The class and member names below are strings resolved at runtime, and that is
 * a version coupling no lock covers — a rename in the next pinned version lands
 * as a diagnostic at registration time rather than as a generation failure. They
 * were read out of the pinned 26.1.2 client jar rather than guessed: the
 * identifier type is {@code Identifier} and not {@code ResourceLocation}, and
 * both properties objects require {@code setId} before their value is
 * constructed.
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

    // Generated from the version's reviewed reflective surface, because this
    // source is shared across versions and the names in it are not.
    private static final String REGISTRY = GameNames.REGISTRY;
    private static final String BUILT_IN = GameNames.BUILT_IN;
    private static final String REGISTRIES = GameNames.REGISTRIES;
    private static final String IDENTIFIER = GameNames.IDENTIFIER;
    private static final String RESOURCE_KEY = GameNames.RESOURCE_KEY;
    private static final String BLOCK = GameNames.BLOCK;
    private static final String BLOCK_PROPERTIES = GameNames.BLOCK_PROPERTIES;
    private static final String ITEM = GameNames.ITEM;
    private static final String ITEM_PROPERTIES = GameNames.ITEM_PROPERTIES;

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
            Object identifier = Class.forName(IDENTIFIER, true, loader())
                    .getMethod("fromNamespaceAndPath", String.class, String.class)
                    .invoke(null, namespace, path);
            Class<?> resourceKey = Class.forName(RESOURCE_KEY, true, loader());
            Class<?> identifierType = Class.forName(IDENTIFIER, true, loader());

            String registryField = kind.equals("block") ? "BLOCK" : "ITEM";
            Object registry = constant(BUILT_IN, registryField);
            Object registryKey = resourceKey
                    .getMethod("create", resourceKey, identifierType)
                    .invoke(null, constant(REGISTRIES, registryField), identifier);

            Object value = kind.equals("block")
                    ? block(registryKey, behaviour)
                    : item(registryKey, behaviour);

            Class<?> registryClass = Class.forName(REGISTRY, true, loader());
            registryClass
                    .getMethod("register", registryClass, identifierType, Object.class)
                    .invoke(null, registry, identifier, value);
            return true;
        } catch (Throwable unregistered) {
            Agent.diagnose("could not register " + kind + " " + namespace + ":" + path
                    + " (" + unusableName(unregistered) + ")");
            return false;
        }
    }

    /**
     * One block, behaving the way the mod that claimed it described.
     *
     * <p>Every value crossed as a primitive and is applied through the setter the
     * pinned 26.1.2 properties declare for it. Nothing is invented here and
     * nothing is defaulted here either: a mod that described nothing was given
     * its kind's defaults on the other side, where admission lives, so this
     * applies four values in every case and has no second path to disagree with
     * the first.
     */
    private static Object block(Object registryKey, List<String> behaviour)
            throws ReflectiveOperationException {
        Class<?> properties = Class.forName(BLOCK_PROPERTIES, true, loader());
        Object built = properties.getMethod("of").invoke(null);
        built = withId(properties, built, registryKey);
        built = properties
                .getMethod("strength", float.class, float.class)
                .invoke(
                        built,
                        Float.parseFloat(behaviour.get(0)),
                        Float.parseFloat(behaviour.get(1)));
        if (Boolean.parseBoolean(behaviour.get(2))) {
            built = properties.getMethod("requiresCorrectToolForDrops").invoke(built);
        }
        if (Boolean.parseBoolean(behaviour.get(3))) {
            built = properties.getMethod("randomTicks").invoke(built);
        }
        return Class.forName(BLOCK, true, loader())
                .getConstructor(properties)
                .newInstance(built);
    }

    /** One item, described the same way and for the same reason. */
    private static Object item(Object registryKey, List<String> behaviour)
            throws ReflectiveOperationException {
        Class<?> properties = Class.forName(ITEM_PROPERTIES, true, loader());
        Object built = properties.getConstructor().newInstance();
        built = withId(properties, built, registryKey);
        built = properties
                .getMethod("stacksTo", int.class)
                .invoke(built, Integer.parseInt(behaviour.get(0)));
        if (Boolean.parseBoolean(behaviour.get(1))) {
            built = properties.getMethod("fireResistant").invoke(built);
        }
        return Class.forName(ITEM, true, loader())
                .getConstructor(properties)
                .newInstance(built);
    }

    /**
     * Stamps a properties object with the key its value will be registered under.
     *
     * <p>Not optional in 26.1.2: both constructors read the id back out, and a
     * properties object without one fails at construction rather than at
     * registration. The returned object is used rather than the argument because
     * the setter answers a builder.
     */
    private static Object withId(Class<?> properties, Object built, Object registryKey)
            throws ReflectiveOperationException {
        Method setId = properties.getMethod(
                "setId", Class.forName(RESOURCE_KEY, true, loader()));
        return setId.invoke(built, registryKey);
    }

    /** Reads one static constant by name. */
    private static Object constant(String owner, String name) throws ReflectiveOperationException {
        return Class.forName(owner, true, loader()).getField(name).get(null);
    }

    private static ClassLoader loader() {
        return ClassLoader.getSystemClassLoader();
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
