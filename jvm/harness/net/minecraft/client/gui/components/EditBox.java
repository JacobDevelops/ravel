package net.minecraft.client.gui.components;

import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Component;

public final class EditBox extends AbstractWidget {
    private final Font font;
    private final int y;
    private final int width;
    private final int height;
    private String value = "";

    public EditBox(Font font, int x, int y, int width, int height, Component message) {
        super(message);
        this.font = font;
        this.y = y;
        this.width = width;
        this.height = height;
    }

    public String getValue() {
        return value;
    }

    public void setValue(String value) {
        this.value = value;
    }

    public void setEditable(boolean editable) {
    }

    public void setMaxLength(int maximum) {
    }

    /**
     * How much of this box's width its text has, which is its width less its
     * border. The pinned class computes it the same way for a bordered box, and
     * every box this harness builds is one.
     */
    public int getInnerWidth() {
        return width - 8;
    }

    /** Whether the value would render whole rather than scrolled to its tail. */
    public boolean fits() {
        return font.width(value) <= getInnerWidth();
    }

    public int y() {
        return y;
    }

    public int height() {
        return height;
    }
}
