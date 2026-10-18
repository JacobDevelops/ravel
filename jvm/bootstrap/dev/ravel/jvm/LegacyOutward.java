package dev.ravel.jvm;

import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

/** Drains the native SDK audio queue into the actual legacy client sound manager. */
final class LegacyOutward {
    private static final AtomicLong PERFORMED = LegacyRuntime.PROOF ? new AtomicLong() : null;

    private LegacyOutward() {
    }

    static void perform() {
        String payload = Hook.ravelOutwardActions();
        if (payload == null) {
            Agent.diagnose("legacy outward actions were not answered for this tick");
            return;
        }
        performAll(payload);
    }

    static int performAll(String payload) {
        if (payload.isEmpty()) {
            return 0;
        }
        try {
            List<String> fields = Agent.fields(payload);
            if (fields.size() < 2 || !fields.get(0).equals("1")) {
                throw new IllegalArgumentException("unsupported outward document");
            }
            int count = Integer.parseInt(fields.get(1));
            if (count < 0) {
                throw new IllegalArgumentException("negative outward action count");
            }
            int at = 2;
            int performed = 0;
            for (int index = 0; index < count; index++) {
                if (at + 2 > fields.size()) {
                    throw new IllegalArgumentException("truncated outward action");
                }
                String verb = fields.get(at);
                int arity = Integer.parseInt(fields.get(at + 1));
                if (arity < 0 || arity > fields.size() - at - 2) {
                    throw new IllegalArgumentException("truncated outward arguments");
                }
                if (one(verb, fields.subList(at + 2, at + 2 + arity))) {
                    performed++;
                }
                at += 2 + arity;
            }
            if (at != fields.size()) {
                throw new IllegalArgumentException("trailing outward fields");
            }
            return performed;
        } catch (IllegalArgumentException failure) {
            Agent.diagnose("legacy outward payload refused: " + failure.getMessage());
            return 0;
        }
    }

    private static boolean one(String verb, List<String> arguments) {
        if (!verb.equals("audio.play") || arguments.size() != 3) {
            Agent.diagnose("legacy outward action refused: " + verb + "/" + arguments.size());
            return false;
        }
        try {
            float pitch = Float.parseFloat(arguments.get(2));
            if (!Float.isFinite(pitch) || pitch <= 0) {
                throw new IllegalArgumentException("invalid audio pitch");
            }
            play(arguments.get(0), arguments.get(1), pitch);
            if (LegacyRuntime.PROOF) {
                PERFORMED.incrementAndGet();
            }
            return true;
        } catch (ReflectiveOperationException | IllegalArgumentException | IllegalStateException failure) {
            Agent.diagnose("legacy audio action refused: " + Agent.unusableName(failure));
            return false;
        }
    }

    private static void play(String namespace, String path, float pitch)
            throws ReflectiveOperationException {
        Object client = LegacyAccess.call("LEGACY_CLIENT", "getInstance", null);
        if (client == null) {
            throw new IllegalStateException("no legacy client is running");
        }
        Object key = LegacyAccess.create("LEGACY_IDENTIFIER", namespace, path);
        Object manager = LegacyAccess.call("LEGACY_CLIENT", "getSoundManager", client);
        if (LegacyAccess.call("LEGACY_SOUND_MANAGER", "get", manager, key) == null) {
            throw new IllegalStateException("unresolved legacy sound " + namespace + ":" + path);
        }
        Object event = LegacyAccess.create("LEGACY_SOUND_EVENT", key);
        Object sound = LegacyAccess.call("LEGACY_SIMPLE_SOUND", "of", null, event, pitch, 0.25F);
        LegacyAccess.call("LEGACY_SOUND_MANAGER", "play", manager, sound);
    }

    static long performed() {
        return LegacyRuntime.PROOF ? PERFORMED.get() : 0;
    }
}
