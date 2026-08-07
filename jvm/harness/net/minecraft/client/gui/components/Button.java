package net.minecraft.client.gui.components;

import net.minecraft.network.chat.Component;

public final class Button extends AbstractWidget {
    private final Button$OnPress onPress;
    private final int y;

    Button(Component message, Button$OnPress onPress, int y) {
        super(message);
        this.onPress = onPress;
        this.y = y;
    }

    public static Button$Builder builder(Component message, Button$OnPress onPress) {
        return new Button$Builder(message, onPress);
    }

    public int y() {
        return y;
    }

    public void press() {
        onPress.onPress(this);
    }
}
