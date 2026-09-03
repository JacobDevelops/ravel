package net.minecraft.client.gui.components;

import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Component;

/**
 * A stand-in for the game's read-only text label.
 *
 * <p>It takes no input, so unlike the {@link EditBox} the rows used to be drawn
 * in there is no caret, no focus border and no click target. It has no border
 * either, which is why the text it measures against is its whole width.
 *
 * <p>It centres its text, which is what the pinned version that offers an
 * alignment member does by default. The other draws at the left edge and offers
 * no alignment member at all, so a screen has no call it can make on both and
 * has to size the box to the text instead. Modelling the centring version is
 * what makes that a testable claim: a row whose box is wider than its text draws
 * its text somewhere other than the box's left edge, and {@link #textX} is where.
 */
public final class StringWidget extends AbstractWidget {

    private final Font font;
    private final int x;
    private final int y;
    private final int width;
    private final int height;

    public StringWidget(int x, int y, int width, int height, Component message, Font font) {
        super(message);
        this.font = font;
        this.x = x;
        this.y = y;
        this.width = width;
        this.height = height;
    }

    /** The text this label draws. */
    public String text() {
        return message().text();
    }

    /** Whether that text would render whole rather than clipped. */
    public boolean fits() {
        return font.width(text()) <= width;
    }

    /** Where the text starts, which is the box's left edge only when it is aligned there. */
    public int textX() {
        return x + Math.round(0.5F * (width - font.width(text())));
    }

    public int x() {
        return x;
    }

    public int y() {
        return y;
    }

    public int height() {
        return height;
    }
}
