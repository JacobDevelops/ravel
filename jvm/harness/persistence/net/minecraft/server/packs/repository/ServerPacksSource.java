package net.minecraft.server.packs.repository;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;
import net.minecraft.world.level.storage.LevelResource;
import net.minecraft.world.level.storage.LevelStorageSource;

/**
 * Stands in for the source that builds a world's pack repository.
 *
 * <p>The real one is handed the world's storage access and returns a repository
 * over that world's {@code datapacks} directory, and the game configures the
 * repository from what the directory holds. So this body lists the directory,
 * and the listing is the whole proof: a pack the seam writes after this ran is
 * a pack the session never enables. Nothing here knows Ravel exists, which is
 * the point — the two sides meet on the filesystem, exactly as they do in the
 * game.
 */
public final class ServerPacksSource {

    private static final List<String> AVAILABLE = new ArrayList<>();

    private ServerPacksSource() {
    }

    /** The pack directories the last repository was built over. */
    public static List<String> available() {
        return List.copyOf(AVAILABLE);
    }

    public static PackRepository createPackRepository(
            LevelStorageSource.LevelStorageAccess access) {
        AVAILABLE.clear();
        Path packs = access.getLevelPath(LevelResource.ROOT).resolve("datapacks");
        try (Stream<Path> entries = Files.list(packs)) {
            entries.map(entry -> entry.getFileName().toString()).sorted().forEach(AVAILABLE::add);
        } catch (IOException absent) {
            // A world with no packs directory has no packs, which is what the
            // game makes of it too.
        }
        return new PackRepository();
    }
}
