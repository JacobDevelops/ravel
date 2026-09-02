package net.minecraft.client.gui.screens.worldselection;

import net.minecraft.world.level.WorldDataConfiguration;

/** Stands in for the settings a create-world screen holds. */
public final class WorldCreationContext {

    private final WorldDataConfiguration dataConfiguration;

    public WorldCreationContext(WorldDataConfiguration dataConfiguration) {
        this.dataConfiguration = dataConfiguration;
    }

    public WorldDataConfiguration dataConfiguration() {
        return dataConfiguration;
    }
}
