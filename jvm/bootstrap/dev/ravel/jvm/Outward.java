package dev.ravel.jvm;

import java.util.List;

/**
 * Performs admitted actions through the version-specific reflective surface.
 * Each action carries its arity so unknown verbs can be skipped without losing later actions.
 */
final class Outward {

    private static final String VERSION = "1";

    private static final String MINECRAFT = GameNames.MINECRAFT;
    private static final String IDENTIFIER = GameNames.IDENTIFIER;
    private static final String SOUND_EVENT = GameNames.SOUND_EVENT;
    private static final String SIMPLE_SOUND_INSTANCE = GameNames.SIMPLE_SOUND_INSTANCE;
    private static final String SOUND_MANAGER = GameNames.SOUND_MANAGER;

    private static final String PLAY = "audio.play";

    private Outward() {
    }

    /** Drains the native action queue once per client tick. */
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
            Agent.diagnose("outward actions were not answered for this tick");
            return;
        }
        performAll(payload);
    }

    /** Performs an encoded batch and returns the number of successful actions. */
    static int performAll(String payload) {
        if (payload.isEmpty()) {
            return 0;
        }
        int performed = 0;
        try {
            List<String> fields = Agent.fields(payload);
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
            // A mod's action failure must not abort the client tick.
            Agent.diagnose("an outward payload could not be performed ("
                    + unusable.getClass().getName() + ")");
        }
        return performed;
    }

    /** Performs one action, reporting failures without aborting later actions. */
    private static boolean one(String verb, List<String> arguments) {
        try {
            if (verb.equals(PLAY) && arguments.size() == 3) {
                play(arguments.get(0), arguments.get(1), Float.parseFloat(arguments.get(2)));
                return true;
            }
            // The declared arity lets the next action survive an unsupported verb.
            Agent.diagnose("an outward action was not performed: " + verb
                    + "/" + arguments.size());
            return false;
        } catch (Throwable unperformed) {
            Agent.diagnose("could not perform " + verb + " (" + Agent.unusableName(unperformed) + ")");
            return false;
        }
    }

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
}
