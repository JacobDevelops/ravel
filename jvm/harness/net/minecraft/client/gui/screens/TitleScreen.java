package net.minecraft.client.gui.screens;

import net.minecraft.network.chat.Component;

public final class TitleScreen extends Screen {
    public TitleScreen() {
        super(Component.literal("title"));
    }

    private int createNormalMenuOptions(int top, int spacing) {
        return top + 2 * spacing;
    }
}
