package dev.ravel.jvm;

import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.util.Map;
import java.time.Duration;
import java.util.concurrent.atomic.AtomicReference;

/** Run once on an official server's owning thread, with its actual server receiver. */
public final class LegacyCommandLive {
    private static final String COMMAND = "commands:warp";
    private static final String TYPED = COMMAND + " set home";

    private LegacyCommandLive() {
    }

    /** Exercises real vanilla permission policy through vanilla's permission-capping wrapper. */
    public static void exercise(Object server)
            throws ReflectiveOperationException, InterruptedException {
        Class<?> serverClass = type("LEGACY_SERVER");
        require(serverClass.isInstance(server), "scenario needs the actual vanilla server");
        Object handler = method("LEGACY_SERVER", "getCommandHandler").invoke(server);
        LegacyCommand.installServer(server);
        Map<?, ?> commands = (Map<?, ?>) method("LEGACY_COMMAND_HANDLER", "getCommands")
                .invoke(handler);
        Object command = commands.get(COMMAND);
        require(type("LEGACY_COMMAND").isInstance(command), "native command is a real ICommand");
        Constructor<?> stack = Members.constructor(type("LEGACY_COMMAND_SOURCE_STACK"),
                LegacyNames.member("LEGACY_COMMAND_SOURCE_STACK", "<init>"));
        Object denied = stack.newInstance(server, null, null, Integer.valueOf(0), null, Boolean.TRUE);
        Object authorized = stack.newInstance(server, null, null, Integer.valueOf(4), null, Boolean.TRUE);
        Method permission = method("LEGACY_COMMAND_SOURCE", "canUseCommand");
        require(Boolean.FALSE.equals(permission.invoke(denied, 2, COMMAND)),
                "vanilla sender cap must deny level two");
        require(Boolean.TRUE.equals(permission.invoke(authorized, 2, COMMAND)),
                "actual server must authorize this command");
        Method canUse = method("LEGACY_COMMAND", "canUse");
        require(Boolean.FALSE.equals(canUse.invoke(command, server, denied)),
                "registered ICommand must retain vanilla denial");
        require(Boolean.TRUE.equals(canUse.invoke(command, server, authorized)),
                "registered ICommand must retain vanilla authorization");
        Method execute = method("LEGACY_COMMAND_HANDLER", "run");
        wrongThread(execute, handler, authorized);
        require(Integer.valueOf(0).equals(execute.invoke(handler, denied, TYPED)),
                "denied command must not reach the native callback");
        require(Integer.valueOf(1).equals(execute.invoke(handler, authorized, TYPED)),
                "authorized command must reach the real native callback successfully");
        System.out.println("[ravel-test] legacy-command-live permission-denied=0 authorized=1"
                + " wrong-thread=0");
    }

    private static void wrongThread(Method execute, Object handler, Object sender)
            throws InterruptedException {
        AtomicReference<Object> result = new AtomicReference<>();
        AtomicReference<Throwable> failure = new AtomicReference<>();
        Thread worker = Thread.ofPlatform().name("ravel-command-off-thread-test").start(() -> {
            try {
                result.set(execute.invoke(handler, sender, TYPED));
            } catch (Throwable thrown) {
                failure.set(thrown);
            }
        });
        require(worker.join(Duration.ofSeconds(5)), "off-thread refusal must not wait on server work");
        if (failure.get() != null) {
            throw new AssertionError("off-thread command must return an explicit refusal", failure.get());
        }
        require(Integer.valueOf(0).equals(result.get()),
                "off-thread command must be rejected before native callback");
    }

    private static Class<?> type(String role) throws ReflectiveOperationException {
        return Members.game(LegacyNames.className(role));
    }

    private static Method method(String role, String name) throws ReflectiveOperationException {
        return Members.method(type(role), LegacyNames.member(role, name));
    }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
