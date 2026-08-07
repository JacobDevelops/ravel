package net.minecraft.client;

import net.minecraft.client.gui.screens.Screen;

public final class Minecraft {
    private static final Minecraft INSTANCE = new Minecraft();

    private Screen screen;

    private Minecraft() {
    }

    public static Minecraft getInstance() {
        return INSTANCE;
    }

    public void setScreen(Screen screen) {
        this.screen = screen;
    }

    public Screen screen() {
        return screen;
    }
}
