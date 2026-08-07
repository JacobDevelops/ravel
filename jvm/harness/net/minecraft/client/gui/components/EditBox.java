package net.minecraft.client.gui.components;

import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Component;

public final class EditBox extends AbstractWidget {
    private String value = "";

    public EditBox(Font font, int x, int y, int width, int height, Component message) {
        super(message);
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
}
