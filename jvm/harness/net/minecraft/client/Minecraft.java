package net.minecraft.client;

import net.minecraft.client.gui.screens.Screen;

public final class Minecraft {
    private static final Minecraft INSTANCE = new Minecraft();

    private Screen screen;

    private int ticks;

    private Minecraft() {
    }

    public static Minecraft getInstance() {
        return INSTANCE;
    }

    // The pinned client tick. It is the one seam here that reaches the mod set
    // rather than a screen: the loader turns this hook into the game event a
    // client-side mod is shown, which is what lets a harness produce a report
    // with real per-mod figures in it. The body counts, so a run that never
    // reached the original would say so.
    public void tick() {
        ticks++;
    }

    public int ticks() {
        return ticks;
    }

    public void setScreen(Screen screen) {
        this.screen = screen;
    }

    public Screen screen() {
        return screen;
    }
}
