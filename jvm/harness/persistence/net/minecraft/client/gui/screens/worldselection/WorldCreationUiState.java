package net.minecraft.client.gui.screens.worldselection;

/** Stands in for the screen's own state, which is where its settings live. */
public final class WorldCreationUiState {

    private final WorldCreationContext settings;

    public WorldCreationUiState(WorldCreationContext settings) {
        this.settings = settings;
    }

    public WorldCreationContext getSettings() {
        return settings;
    }
}
