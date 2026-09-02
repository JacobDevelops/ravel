import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Optional;
import java.util.stream.Stream;
import net.minecraft.client.Minecraft;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.WorldStem;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.SavedDataStorage;

/**
 * Drives ADG-7's two world persistence seams against a stand-in game.
 *
 * <p>The sequence is the one a world being created runs, which is the sequence
 * that used to go wrong: the storage access is made, the world's pack
 * repository is built over its {@code datapacks} directory, and only then is the
 * world loaded. No world-open step appears here, because a fresh level reaches
 * none of them.
 *
 * <p>What this proves: that a refused open never runs the body it precedes and
 * releases the world lock and the stem it was handed, that only a replacement
 * answer reaches the storage and reaches it once, that vanilla saving runs
 * whatever Ravel answers, and that the mods' pack is on disk in time for the
 * repository this session is given. What it does not prove: anything about the
 * real client, which needs a launch.
 */
public final class PersistenceMain {

    public static void main(String[] args) throws IOException {
        SavedDataStorage.seed(
                System.getProperty("ravel.test.persistence.stored", "").isEmpty()
                        ? null
                        : System.getProperty("ravel.test.persistence.stored").getBytes());

        // A world nothing has written into yet, so what the repository below
        // finds is what this run put there. Runs share a directory, and one that
        // inherited the previous run's pack would pass without proving anything.
        emptyWorld();

        LevelStorageSource.LevelStorageAccess access =
                new LevelStorageSource.LevelStorageAccess("world");
        WorldStem stem = new WorldStem();

        // The repository step, as a world being created runs it: the access
        // exists, no world has loaded, and the packs the session gets are the
        // ones this call finds on disk.
        PackRepository packs = ServerPacksSource.createPackRepository(access);
        System.out.println("datapack=" + dev.ravel.jvm.Hook.datapackWorld()
                + " loadsAtWrite=" + Minecraft.loads()
                + " available=" + ServerPacksSource.available());

        new Minecraft().doWorldLoad(access, packs, stem, Optional.empty(), false);
        System.out.println("loads=" + Minecraft.loads()
                + " accessClosed=" + access.isClosed()
                + " stemClosed=" + stem.isClosed()
                + " crossed=" + dev.ravel.jvm.Hook.openCrossings()
                + " read=" + dev.ravel.jvm.Hook.openAggregate());

        new MinecraftServer().saveAllChunks(false, false, false);
        System.out.println("saves=" + MinecraftServer.saves()
                + " writes=" + SavedDataStorage.writes()
                + " written=" + SavedDataStorage.written());
    }

    /** Removes whatever a previous run left in the stand-in world. */
    private static void emptyWorld() throws IOException {
        Path world = Path.of("world");
        if (!Files.exists(world)) {
            return;
        }
        try (Stream<Path> tree = Files.walk(world)) {
            for (Path entry : tree.sorted(Comparator.reverseOrder()).toList()) {
                Files.delete(entry);
            }
        }
    }
}
