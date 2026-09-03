package net.minecraft.client.gui;

import net.minecraft.client.StringSplitter;

/**
 * A stand-in for the game's font, measuring one width for every glyph.
 *
 * <p>It holds a splitter and answers it, which is the shape the screens go
 * through: the game's font renames and retypes its own split between the pinned
 * versions and the splitter does neither.
 */
public final class Font {

    private final StringSplitter splitter = new StringSplitter();

    /** What the given text measures. */
    public int width(String text) {
        return splitter.width(text);
    }

    /** What splits text into the lines it draws as. */
    public StringSplitter getSplitter() {
        return splitter;
    }
}
