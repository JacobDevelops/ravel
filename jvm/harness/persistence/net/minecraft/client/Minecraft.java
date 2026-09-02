package net.minecraft.client;

import java.util.Optional;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.server.WorldStem;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.world.level.storage.LevelStorageSource;

/**
 * Stands in for the client, carrying only {@code doWorldLoad} with the exact
 * descriptor the hook inventory pins.
 *
 * <p>The body counts itself. That count is the whole proof: a refused world load
 * is one where this never ran, and an admitted one is where it did.
 */
public final class Minecraft {

    private static final Minecraft INSTANCE = new Minecraft();

    private static int loads;
    private static String shown = "";

    public static Minecraft getInstance() {
        return INSTANCE;
    }

    /** Records the screen a refusal put the player on. */
    public void setScreen(Screen screen) {
        shown = String.valueOf(screen);
    }

    /** What the player was last shown, or empty when nothing was. */
    public static String shown() {
        return shown;
    }

    /** How many times the world-load body ran. */
    public static int loads() {
        return loads;
    }

    public void doWorldLoad(
            LevelStorageSource.LevelStorageAccess access,
            PackRepository packs,
            WorldStem stem,
            Optional<Object> gameRules,
            boolean newWorld) {
        loads++;
    }
}
