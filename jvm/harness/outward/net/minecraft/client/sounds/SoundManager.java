package net.minecraft.client.sounds;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.resources.sounds.SoundInstance;

/**
 * The stand-in sound manager, which records rather than plays.
 *
 * <p>Nothing here reaches sound hardware; what the harness asserts is that the
 * adapter asked for the sound the payload named, with the pitch the payload
 * stated, through the member the pinned jar declares.
 */
public final class SoundManager {
    private final List<String> played = new ArrayList<>();

    public void play(SoundInstance sound) {
        played.add(sound.toString());
    }

    public String contents() {
        return String.join(",", played);
    }
}
