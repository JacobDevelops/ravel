package dev.ravel.jvm;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.Map;

/** Supplementary boundary tests; a live vanilla sender remains the authorization acceptance gate. */
public final class LegacyCommandTest {
    private LegacyCommandTest() {
    }

    public static void main(String[] args) throws ReflectiveOperationException {
        declarations();
        permissions();
        generatedSenderReply();
        require(LegacyCommandEntry.literal(" /example:home first second ").equals("example:home"),
                "only the command token selects its owner");
        require(LegacyCommandEntry.literal("minecraft:give x").equals("minecraft:give"),
                "vanilla literals are not rewritten");
        System.out.println("legacy command boundary tests passed");
    }

    private static void declarations() {
        Map<String, LegacyCommandEntry> entries = LegacyCommandEntry.decode(
                wire("1", "1", "example:home", "2", "example:home [name]"));
        require(entries.get("example:home").permissionLevel() == 2, "retain declared permission");
        require(entries.get("example:home").usage().equals("example:home [name]"), "retain usage");
        require(LegacyCommandEntry.decode(wire("1", "0")).isEmpty(), "accept empty declarations");
        for (String malformed : new String[] {
            "", wire("2", "0"), wire("1", "-1"), wire("1", "2"),
            wire("1", "1", "example:home", "-1", "usage"),
            wire("1", "1", "example:home", "5", "usage"),
            wire("1", "1", "home", "2", "usage"),
            wire("1", "1", "example:home extra", "2", "usage"),
            wire("1", "0", "extra"),
            wire("1", "2", "example:home", "2", "a", "example:home", "2", "b")
        }) {
            boolean refused = false;
            try {
                LegacyCommandEntry.decode(malformed);
            } catch (IllegalArgumentException expected) {
                refused = true;
            }
            require(refused, "reject malformed command declarations: " + malformed);
        }
    }

    private static void permissions() throws ReflectiveOperationException {
        LegacyCommandEntry entry = new LegacyCommandEntry("example:home", 2, "example:home");
        Method permission = Sender.class.getDeclaredMethod("canUseCommand", int.class, String.class);
        Sender sender = new Sender();
        require(!entry.allowed(sender, permission), "a denied sender must remain denied");
        require(sender.level == 2 && sender.literal.equals("example:home"),
                "permission sees the actual declared level and complete namespaced literal");
        sender.authorized = true;
        require(entry.allowed(sender, permission), "the sender decides authorization");
        sender.throwing = true;
        boolean refused = false;
        try {
            entry.allowed(sender, permission);
        } catch (InvocationTargetException expected) {
            refused = expected.getCause() instanceof IllegalStateException;
        }
        require(refused, "permission lookup failure cannot authorize a command");
    }

    private static void generatedSenderReply() throws ReflectiveOperationException {
        Class<?> source = Members.game(LegacyNames.className("LEGACY_COMMAND_SOURCE"));
        Method permission = Members.method(source,
                LegacyNames.member("LEGACY_COMMAND_SOURCE", "canUseCommand"));
        Method send = Members.method(source,
                LegacyNames.member("LEGACY_COMMAND_SOURCE", "sendMessage"));
        Object[] delivered = new Object[1];
        Object sender = Proxy.newProxyInstance(source.getClassLoader(), new Class<?>[] { source },
                (proxy, method, arguments) -> {
                    if (method.equals(permission)) {
                        require(arguments[0].equals(2) && arguments[1].equals("example:home"),
                                "generated sender receives the exact permission request");
                        return false;
                    }
                    if (method.equals(send)) {
                        delivered[0] = arguments[0];
                        return null;
                    }
                    throw new UnsupportedOperationException(method.toString());
                });
        LegacyCommandEntry entry = new LegacyCommandEntry("example:home", 2, "usage");
        require(!entry.allowed(sender, permission), "generated sender denial is retained");
        Method reply = LegacyCommand.class.getDeclaredMethod("reply", Object.class, String.class);
        reply.setAccessible(true);
        reply.invoke(null, sender, "permission rejected");
        Class<?> literal = Members.game(LegacyNames.className("LEGACY_LITERAL_TEXT"));
        require(literal.isInstance(delivered[0]), "reply uses the real generated literal text type");
        Object expected = Members.constructor(literal,
                LegacyNames.member("LEGACY_LITERAL_TEXT", "<init>")).newInstance("permission rejected");
        require(expected.equals(delivered[0]), "reply preserves response text");
    }

    private static String wire(String... fields) {
        StringBuilder payload = new StringBuilder();
        for (String field : fields) {
            payload.append(field.length()).append(':').append(field);
        }
        return payload.toString();
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    public static final class Sender {
        boolean authorized;
        boolean throwing;
        int level;
        String literal;

        public boolean canUseCommand(int requestedLevel, String command) {
            level = requestedLevel;
            literal = command;
            if (throwing) {
                throw new IllegalStateException("permission unavailable");
            }
            return authorized;
        }
    }
}
