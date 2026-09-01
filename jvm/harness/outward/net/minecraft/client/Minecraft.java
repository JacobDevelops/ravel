package net.minecraft.client;

import net.minecraft.client.sounds.SoundManager;

/**
 * The running client, as much of it as the outward adapter reaches.
 *
 * <p>{@code tick} is the seam the reviewed inventory pins as {@code client.tick}
 * and carries the descriptor it pins, so the transformer's prologue lands on the
 * same method here as in a launch. Its body does nothing, which is what makes an
 * action that was performed visible: only the prologue can have caused it.
 */
public final class Minecraft {
    private static final Minecraft INSTANCE = new Minecraft();
    private final SoundManager sounds = new SoundManager();

    private Minecraft() {
    }

    public static Minecraft getInstance() {
        return INSTANCE;
    }

    public SoundManager getSoundManager() {
        return sounds;
    }

    public void tick() {
    }
}
