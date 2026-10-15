package net.minecraft.util;

// Fixture names and descriptors match the version's hook inventory.
public final class StringUtil {
    public static boolean isAllowedChatCharacter(int codepoint) {
        return codepoint >= 32;
    }

    public static String filterText(String text, boolean keepNewlines) {
        return keepNewlines ? text : text.replace("\n", "");
    }

    // Delegation makes a name-only match observable in the invocation count.
    public static String filterText(String text) {
        return filterText(text, false);
    }

    public static boolean isValidPlayerName(String name) {
        return !name.isEmpty();
    }
}
