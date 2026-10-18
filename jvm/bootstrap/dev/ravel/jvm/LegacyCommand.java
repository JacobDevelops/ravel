package dev.ravel.jvm;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.List;
import java.util.Map;
import java.util.WeakHashMap;

/** Legacy ICommand registration and dispatch, using only pinned generated members. */
final class LegacyCommand {
    private static final Map<Object, Map<String, LegacyCommandEntry>> INSTALLED = new WeakHashMap<>();

    private LegacyCommand() {
    }

    /** Called on the owning server thread, after its independent native composition is installed. */
    static void installServer(Object server) {
        try {
            if (!onServerThread(server)) {
                throw new IllegalStateException("legacy commands require the owning server thread");
            }
            Object handler = member("LEGACY_SERVER", "getCommandHandler").invoke(server);
            install(handler);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("legacy server command handler is unavailable", failure);
        }
    }

    /** The constructor is only observed; it may run on the client before server initialization. */
    private static synchronized void install(Object handler) {
        if (INSTALLED.containsKey(handler)) {
            return;
        }
        try {
            String payload = Hook.ravelLegacyCommands();
            if (payload == null) {
                throw new IllegalStateException("legacy command declarations were not answered");
            }
            Map<String, LegacyCommandEntry> entries = LegacyCommandEntry.decode(payload);
            Map<?, ?> existing = (Map<?, ?>) member("LEGACY_COMMAND_HANDLER", "getCommands")
                    .invoke(handler);
            for (String literal : entries.keySet()) {
                if (existing.containsKey(literal)) {
                    throw new IllegalStateException("legacy command already registered: " + literal);
                }
            }
            Class<?> command = type("LEGACY_COMMAND");
            Method register = member("LEGACY_COMMAND_REGISTRY", "register");
            List<Object> proxies = new java.util.ArrayList<>();
            for (LegacyCommandEntry entry : entries.values()) {
                proxies.add(Proxy.newProxyInstance(command.getClassLoader(),
                        new Class<?>[] { command }, new CommandProxy(entry)));
            }
            for (Object proxy : proxies) {
                register.invoke(handler, proxy);
            }
            INSTALLED.put(handler, entries);
        } catch (Throwable failure) {
            Agent.diagnose("legacy commands could not be installed ("
                    + Agent.unusableName(failure) + ")");
            throw new IllegalStateException("declared legacy commands were not installed", failure);
        }
    }

    /** Null leaves a vanilla/unknown command completely untouched. */
    static Integer execute(Object handler, Object sender, String typed) {
        Map<String, LegacyCommandEntry> entries;
        synchronized (LegacyCommand.class) {
            entries = INSTALLED.get(handler);
        }
        if (entries == null || typed == null) {
            return null;
        }
        LegacyCommandEntry entry = entries.get(LegacyCommandEntry.literal(typed));
        return entry == null ? null : invoke(entry, sender, typed.trim());
    }

    private static int invoke(LegacyCommandEntry entry, Object sender, String typed) {
        try {
            Object server = member("LEGACY_COMMAND_SOURCE", "getServer").invoke(sender);
            if (server == null || !onServerThread(server)) {
                reply(sender, "Ravel commands must run on the owning server thread");
                return 0;
            }
            if (!entry.allowed(sender, member("LEGACY_COMMAND_SOURCE", "canUseCommand"))) {
                reply(sender, "You do not have permission to use " + entry.literal());
                return 0;
            }
            String command = typed.startsWith("/") ? typed.substring(1) : typed;
            String failure = Hook.ravelLegacyCommand(command);
            if (failure == null) {
                throw new IllegalStateException("legacy command execution was not answered");
            }
            if (!failure.isEmpty()) {
                reply(sender, failure);
                return 0;
            }
            return 1;
        } catch (Throwable failure) {
            Agent.diagnose("legacy command failed: " + entry.literal() + " ("
                    + Agent.unusableName(failure) + ")");
            return 0;
        }
    }

    private static boolean onServerThread(Object server) throws ReflectiveOperationException {
        return Boolean.TRUE.equals(member("LEGACY_EVENT_LOOP", "isOnSameThread").invoke(server));
    }

    private static void reply(Object sender, String message) throws ReflectiveOperationException {
        Object text = Members.constructor(type("LEGACY_LITERAL_TEXT"),
                LegacyNames.member("LEGACY_LITERAL_TEXT", "<init>")).newInstance(message);
        member("LEGACY_COMMAND_SOURCE", "sendMessage").invoke(sender, text);
    }

    private static Class<?> type(String role) throws ReflectiveOperationException {
        return Members.game(LegacyNames.className(role));
    }

    private static Method member(String role, String reviewed) throws ReflectiveOperationException {
        return Members.method(type(role), LegacyNames.member(role, reviewed));
    }

    private static final class CommandProxy implements InvocationHandler {
        private final LegacyCommandEntry entry;
        private final Map<Method, String> methods;
        private final Method permission;
        private final Method getName;

        CommandProxy(LegacyCommandEntry entry) throws ReflectiveOperationException {
            this.entry = entry;
            Map<Method, String> linked = new java.util.HashMap<>();
            for (String name : List.of("getName", "getUsage", "getAliases", "getSuggestions",
                    "hasTargetSelectorAt", "canUse", "run")) {
                linked.put(member("LEGACY_COMMAND", name), name);
            }
            methods = Map.copyOf(linked);
            permission = member("LEGACY_COMMAND_SOURCE", "canUseCommand");
            getName = member("LEGACY_COMMAND", "getName");
        }

        @Override
        public Object invoke(Object proxy, Method method, Object[] args) throws Throwable {
            if (method.getDeclaringClass() == Object.class) {
                return switch (method.getName()) {
                    case "equals" -> proxy == args[0];
                    case "hashCode" -> System.identityHashCode(proxy);
                    case "toString" -> entry.literal();
                    default -> throw new UnsupportedOperationException(method.toString());
                };
            }
            if (method.getName().equals("compareTo") && method.getParameterCount() == 1) {
                return entry.literal().compareTo((String) getName.invoke(args[0]));
            }
            return gameMethod(method, args);
        }

        private Object gameMethod(Method method, Object[] args) throws ReflectiveOperationException {
            String role = methods.get(method);
            if (role == null) {
                throw new UnsupportedOperationException(method.toString());
            }
            return switch (role) {
                case "getName" -> entry.literal();
                case "getUsage" -> entry.usage();
                case "getAliases", "getSuggestions" -> List.of();
                case "hasTargetSelectorAt" -> false;
                case "canUse" -> entry.allowed(args[1], permission);
                case "run" -> {
                    LegacyCommand.invoke(entry, args[1], entry.literal() + " "
                            + String.join(" ", (String[]) args[2]));
                    yield null;
                }
                default -> throw new UnsupportedOperationException(method.toString());
            };
        }
    }
}
