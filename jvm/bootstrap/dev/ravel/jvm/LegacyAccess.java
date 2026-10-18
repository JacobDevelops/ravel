package dev.ravel.jvm;

import java.lang.reflect.Method;

/** Exact descriptor-bound access to generated legacy roles. */
final class LegacyAccess {
    private LegacyAccess() {
    }

    static Class<?> type(String role) throws ReflectiveOperationException {
        return Members.game(LegacyNames.className(role));
    }

    static Object call(String role, String member, Object receiver, Object... arguments)
            throws ReflectiveOperationException {
        return method(role, member).invoke(receiver, arguments);
    }

    static Method method(String role, String member) throws ReflectiveOperationException {
        return Members.method(type(role), LegacyNames.member(role, member));
    }

    static Object field(String role, String member, Object receiver)
            throws ReflectiveOperationException {
        return Members.field(type(role), LegacyNames.member(role, member)).get(receiver);
    }

    static Object create(String role, Object... arguments) throws ReflectiveOperationException {
        return Members.constructor(type(role), LegacyNames.member(role, "<init>"))
                .newInstance(arguments);
    }
}
