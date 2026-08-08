package net.minecraft.client.gui.components;

import net.minecraft.network.chat.Component;

public final class Button$Builder {
    private final Component message;
    private final Button$OnPress onPress;
    private int y;
    private int height;

    Button$Builder(Component message, Button$OnPress onPress) {
        this.message = message;
        this.onPress = onPress;
    }

    public Button$Builder bounds(int x, int y, int width, int height) {
        this.y = y;
        this.height = height;
        return this;
    }

    public Button build() {
        return new Button(message, onPress, y, height);
    }
}
