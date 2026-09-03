package net.minecraft.client;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.network.chat.FormattedText;
import net.minecraft.network.chat.Style;

/**
 * A stand-in for the game's splitter, measuring one width for every glyph.
 *
 * <p>The game's glyphs are not one width and this one's are, which is the whole
 * of what is stood in for. What the harness proves is the screen's half of the
 * contract: that it asks the splitter to split, that it asks for the width it
 * will actually draw at, and that it lays out one row per line it was answered.
 * A screen that counted characters itself would pass against a splitter like
 * this one and fail against the game's, so the assertions are written against
 * what this splitter reports rather than against any character count.
 */
public final class StringSplitter {

    /** What one character advances, in the units widths are given in. */
    public static final int ADVANCE = 6;

    /** What the given text measures. */
    public int width(String text) {
        return text.length() * ADVANCE;
    }

    /**
     * Splits text into lines no wider than {@code maxWidth}, breaking on words.
     *
     * <p>A word too long to fit on a line of its own is broken mid-word, as the
     * pinned splitter does, so every line this answers fits the width asked for.
     */
    public List<FormattedText> splitLines(String text, int maxWidth, Style style) {
        List<FormattedText> lines = new ArrayList<>();
        StringBuilder line = new StringBuilder();
        for (String word : text.split(" ", -1)) {
            String rest = word;
            while (width(rest) > maxWidth) {
                if (line.length() > 0) {
                    lines.add(FormattedText.of(line.toString()));
                    line.setLength(0);
                }
                int fits = Math.max(1, maxWidth / ADVANCE);
                lines.add(FormattedText.of(rest.substring(0, fits)));
                rest = rest.substring(fits);
            }
            String candidate = line.length() == 0 ? rest : line + " " + rest;
            if (width(candidate) <= maxWidth) {
                line.setLength(0);
                line.append(candidate);
            } else {
                lines.add(FormattedText.of(line.toString()));
                line.setLength(0);
                line.append(rest);
            }
        }
        if (line.length() > 0 || lines.isEmpty()) {
            lines.add(FormattedText.of(line.toString()));
        }
        return lines;
    }
}
