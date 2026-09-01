package net.minecraft.sounds;

import net.minecraft.resources.Identifier;

/** The pinned 26.1.2 sound event, built the way the adapter builds one. */
public final class SoundEvent {
    private final Identifier location;

    private SoundEvent(Identifier location) {
        this.location = location;
    }

    public static SoundEvent createVariableRangeEvent(Identifier location) {
        return new SoundEvent(location);
    }

    public Identifier location() {
        return location;
    }
}
