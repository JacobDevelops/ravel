package dev.ravel.jvm;

import java.util.ArrayList;
import java.util.List;

/**
 * Performs the actions Ravel asked the game for.
 *
 * <p>Mod actions cross outward as data and this side makes the call.
 * Unlike {@link Content}'s single registration verb, this adapter dispatches
 * verbs through one switch with a shared payload, reader and failure handling.
 * See {@code docs/architecture/overview.md#mechanisms}.
 *
 * <p>An action states how many arguments it carries, which is the one thing this
 * payload has that {@link Content}'s does not. It is what lets a verb this
 * version does not know be stepped over: the reader skips exactly that many
 * fields and reads the next action, instead of losing the rest of the payload
 * behind a width it cannot compute.
 *
 * <p>Everything is reflective for the reason {@link Content} gives: this class
 * ships in the always-present agent jar, which is compiled with no client jar on
 * the class path, so a source declaring a game type would not compile. Every
 * name it resolves — the classes through {@link GameNames}, the members through
 * {@link GameMembers} — is generated from the version's reviewed reflective
 * surface, so this one source is right about every tracked version. What the
 * surface says about the sound path differs between them and this source does
 * not have to know: {@code SoundManager.play} answers a
 * {@code SoundEngine$PlayResult} under one version and nothing under another,
 * and either way this side discards it.
 *
 * <p><b>Rust decides, Java performs.</b> Nothing here defaults a value, filters
 * an action or judges whether a mod may ask for one. A pitch the mod did not
 * give is stated in the payload, so there is one place that decides it and this
 * side has no second opinion to disagree with.
 */
final class Outward {

    /** The wire contract the payload is read under. */
    private static final String VERSION = "1";

    // Generated from the version's reviewed reflective surface, because this
    // source is shared across versions and the names in it are not.
    private static final String MINECRAFT = GameNames.MINECRAFT;
    private static final String IDENTIFIER = GameNames.IDENTIFIER;
    private static final String SOUND_EVENT = GameNames.SOUND_EVENT;
    private static final String SIMPLE_SOUND_INSTANCE = GameNames.SIMPLE_SOUND_INSTANCE;
    private static final String SOUND_MANAGER = GameNames.SOUND_MANAGER;

    /** Plays one sound a mod declared: its owner, its entry, and the pitch. */
    private static final String PLAY = "audio.play";

    private Outward() {
    }

    /**
     * Drains everything Ravel queued and performs it.
     *
     * <p>Called from the client tick prologue. A tick that has nothing queued
     * costs one crossing answering an empty string.
     */
    static void perform() {
        String payload;
        try {
            payload = Hook.ravelOutwardActions();
        } catch (Throwable unreachable) {
            // The native failing must not take a client tick down with it.
            Agent.diagnose("outward actions could not be read ("
                    + unreachable.getClass().getName() + ")");
            return;
        }
        if (payload == null) {
            // What the JVM receives when the native failed. Nothing is performed
            // on it, rather than a payload nothing checked.
            Agent.diagnose("outward actions were not answered for this tick");
            return;
        }
        performAll(payload);
    }

    /**
     * Reads one payload and performs every action in it.
     *
     * <p>Separate from {@link #perform} so the JVM harness can drive this side
     * with the payload {@code crates/ravel-bootstrap/src/outward.rs} pins,
     * without a queue to fill first.
     *
     * @param payload the encoded actions, empty when nothing was queued
     * @return how many actions were performed
     */
    static int performAll(String payload) {
        if (payload.isEmpty()) {
            return 0;
        }
        int performed = 0;
        try {
            List<String> fields = fields(payload);
            if (fields.size() < 2 || !fields.get(0).equals(VERSION)) {
                Agent.diagnose("an outward payload was offered that cannot be read");
                return 0;
            }
            int actions = Integer.parseInt(fields.get(1));
            int at = 2;
            for (int action = 0; action < actions; action++) {
                if (at + 1 >= fields.size()) {
                    Agent.diagnose("an outward payload declares more actions than it carries");
                    break;
                }
                String verb = fields.get(at);
                int arity = Integer.parseInt(fields.get(at + 1));
                if (at + 2 + arity > fields.size()) {
                    Agent.diagnose("an outward action is missing its arguments");
                    break;
                }
                if (one(verb, fields.subList(at + 2, at + 2 + arity))) {
                    performed++;
                }
                at += 2 + arity;
            }
        } catch (Throwable unusable) {
            // Deliberately everything, for the reason Content gives: this runs
            // inside the game's own tick and an escape here would take the
            // client down over a mod's sound.
            Agent.diagnose("an outward payload could not be performed ("
                    + unusable.getClass().getName() + ")");
        }
        return performed;
    }

    /**
     * Performs one action, and reports rather than throws when it cannot.
     *
     * @return whether the game did it
     */
    private static boolean one(String verb, List<String> arguments) {
        try {
            if (verb.equals(PLAY) && arguments.size() == 3) {
                play(arguments.get(0), arguments.get(1), Float.parseFloat(arguments.get(2)));
                return true;
            }
            // A verb this version does not know, or one whose arity moved under
            // it. Both are the same thing to this side — an action it must not
            // guess at — and the payload said how wide it is, so the actions
            // after it are still read.
            Agent.diagnose("an outward action was not performed: " + verb
                    + "/" + arguments.size());
            return false;
        } catch (Throwable unperformed) {
            Agent.diagnose("could not perform " + verb + " (" + unusableName(unperformed) + ")");
            return false;
        }
    }

    /** Plays one declared sound through the running client's sound manager. */
    private static void play(String owner, String entry, float pitch)
            throws ReflectiveOperationException {
        Class<?> minecraft = Members.game(MINECRAFT);
        Object client = Members.method(minecraft, GameMembers.MINECRAFT_GET_INSTANCE)
                .invoke(null);
        if (client == null) {
            throw new IllegalStateException("no client is running");
        }
        Object identifier = Members.method(
                Members.game(IDENTIFIER), GameMembers.IDENTIFIER_FROM_NAMESPACE_AND_PATH)
                .invoke(null, owner, entry);
        Object event = Members.method(
                Members.game(SOUND_EVENT), GameMembers.SOUND_EVENT_CREATE_VARIABLE_RANGE_EVENT)
                .invoke(null, identifier);
        Object sound = Members.method(
                Members.game(SIMPLE_SOUND_INSTANCE), GameMembers.SIMPLE_SOUND_INSTANCE_FOR_UI)
                .invoke(null, event, pitch);
        Object manager = Members.method(minecraft, GameMembers.MINECRAFT_GET_SOUND_MANAGER)
                .invoke(client);
        Members.method(Members.game(SOUND_MANAGER), GameMembers.SOUND_MANAGER_PLAY)
                .invoke(manager, sound);
    }

    /** The type that stopped an action, unwrapped past the reflection layer. */
    private static String unusableName(Throwable thrown) {
        Throwable cause = thrown.getCause() == null ? thrown : thrown.getCause();
        return cause.getClass().getName();
    }

    /**
     * Reads a length-prefixed payload back into its fields.
     *
     * <p>Every field is {@code <length>:<that many UTF-16 code units>}, which is
     * the framing {@code crates/ravel-bootstrap/src/wire.rs} states and both
     * {@link Content} and the settings screen already read. A length says where a
     * field ends, so a mod identifier carrying a colon cannot be misread as a
     * separator.
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
