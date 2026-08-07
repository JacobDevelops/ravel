package dev.ravel.jvm;

public final class Hook {
    public static final HookTarget[] TARGETS = {
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "isAllowedChatCharacter",
                "(I)Z",
                true,
                "chatAllowedCharacter",
                "(I)V"),
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "filterText",
                "(Ljava/lang/String;Z)Ljava/lang/String;",
                true,
                "chatFilterText",
                "(Ljava/lang/String;Z)V"),
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "filterText",
                "(Ljava/lang/String;)Ljava/lang/String;",
                true,
                "chatFilterTextSimple",
                "(Ljava/lang/String;)V"),
        new HookTarget(
                "net/minecraft/util/StringUtil",
                "isValidPlayerName",
                "(Ljava/lang/String;)Z",
                true,
                "playerNameValid",
                "(Ljava/lang/String;)V"),
    };

    private static long invocations;

    private Hook() {
    }

    public static void initialize() {
    }

    public static void ravelEnableProfiling(String value) {
    }

    public static void ravelInitialize() {
        try {
            Class.forName(
                    "net.minecraft.util.StringUtil",
                    true,
                    ClassLoader.getSystemClassLoader());
        } catch (ClassNotFoundException absent) {
            throw new IllegalStateException(absent);
        }
    }

    public static long nativeInvocations() {
        return invocations;
    }

    public static void chatAllowedCharacter(int codepoint) {
        invocations++;
    }

    public static void chatFilterText(String text, boolean keepNewlines) {
        invocations++;
    }

    public static void chatFilterTextSimple(String text) {
        invocations++;
    }

    public static void playerNameValid(String name) {
        invocations++;
    }
}
