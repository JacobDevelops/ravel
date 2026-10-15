package net.minecraft.util;

// A stand-in carrying the internal name, member names and exact descriptors
// from `minecraft/versions/<version>/hooks.toml`. Invented bodies return values
// the harness can check.
public final class StringUtil {
    public static boolean isAllowedChatCharacter(int codepoint) {
        return codepoint >= 32;
    }

    public static String filterText(String text, boolean keepNewlines) {
        return keepNewlines ? text : text.replace("\n", "");
    }

    // The overload that makes descriptor matching testable: same name, different
    // descriptor, and it delegates, so a transformer that matched on name alone
    // would rewrite the wrong one and the invocation count would give it away.
    public static String filterText(String text) {
        return filterText(text, false);
    }

    public static boolean isValidPlayerName(String name) {
        return !name.isEmpty();
    }
}
