package dev.ravel.jvm;

import java.lang.reflect.Method;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** One admitted legacy command; permission levels are policy, never sender identity. */
record LegacyCommandEntry(String literal, int permissionLevel, String usage) {

    static Map<String, LegacyCommandEntry> decode(String payload) {
        List<String> fields = Agent.fields(payload);
        if (fields.size() < 2 || !fields.get(0).equals("1")) {
            throw new IllegalArgumentException("unsupported legacy command payload");
        }
        int count = Integer.parseInt(fields.get(1));
        if (count < 0 || count > (fields.size() - 2) / 3 || fields.size() != 2 + count * 3) {
            throw new IllegalArgumentException("legacy command row count differs");
        }
        Map<String, LegacyCommandEntry> entries = new LinkedHashMap<>();
        for (int at = 2; at < fields.size(); at += 3) {
            String literal = fields.get(at);
            int level = Integer.parseInt(fields.get(at + 1));
            if (!literal.matches("[a-z0-9_.-]+:[a-z0-9_./-]+") || level < 0 || level > 4) {
                throw new IllegalArgumentException("invalid legacy command declaration");
            }
            LegacyCommandEntry entry = new LegacyCommandEntry(literal, level, fields.get(at + 2));
            if (entries.putIfAbsent(literal, entry) != null) {
                throw new IllegalArgumentException("duplicate legacy command " + literal);
            }
        }
        return entries;
    }

    /** Calls the sender's actual permission member, including its command-specific policy. */
    boolean allowed(Object sender, Method permission) throws ReflectiveOperationException {
        return Boolean.TRUE.equals(permission.invoke(sender, permissionLevel, literal));
    }

    static String literal(String typed) {
        String command = typed.trim();
        if (command.startsWith("/")) {
            command = command.substring(1);
        }
        int space = command.indexOf(' ');
        return space < 0 ? command : command.substring(0, space);
    }
}
