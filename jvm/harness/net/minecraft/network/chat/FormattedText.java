package net.minecraft.network.chat;

/**
 * A stand-in for the text the game splits and measures.
 *
 * <p>The pinned type is an interface with a default {@code getString} and a
 * static {@code of}, and those two are what the profiling screen links on. This
 * carries the same names and descriptors and nothing else.
 */
public interface FormattedText {

    static FormattedText of(String text) {
        return () -> text;
    }

    String getString();
}
