package net.minecraft.world.level;

import java.util.List;

/**
 * Stands in for the pack configuration, carrying only the enabled list.
 *
 * <p>The list is what the selection call selects, and the adapter reads it off
 * the screen rather than passing a default. A run that lost it would be a world
 * being recreated with a player's own choices thrown away.
 */
public final class WorldDataConfiguration {

    private final List<String> enabled;

    public WorldDataConfiguration(List<String> enabled) {
        this.enabled = List.copyOf(enabled);
    }

    public List<String> enabled() {
        return enabled;
    }
}
