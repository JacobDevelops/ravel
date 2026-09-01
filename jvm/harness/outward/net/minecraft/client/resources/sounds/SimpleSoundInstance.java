package net.minecraft.client.resources.sounds;

import net.minecraft.sounds.SoundEvent;

/**
 * The instance the adapter builds through {@code forUI}.
 *
 * <p>It keeps the event and the pitch it was given so the harness can print
 * them: a payload whose arguments were decoded in the wrong order reaches this
 * class and is visible here rather than being lost in a void method.
 */
public final class SimpleSoundInstance implements SoundInstance {
    private final SoundEvent sound;
    private final float pitch;

    private SimpleSoundInstance(SoundEvent sound, float pitch) {
        this.sound = sound;
        this.pitch = pitch;
    }

    public static SimpleSoundInstance forUI(SoundEvent sound, float pitch) {
        return new SimpleSoundInstance(sound, pitch);
    }

    @Override
    public String toString() {
        return sound.location() + "@" + pitch;
    }
}
