import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.stream.Stream;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.worldselection.CreateWorldScreen;
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
 * that used to go wrong. The screen builds its widgets and loads its data; the
 * screen's staging directory is copied into the new world; the world's pack
 * repository is built over that directory; and only then is the world loaded. No
 * world-open step appears here, because a fresh level reaches none of them, and
 * no second data load appears either, because creating a world runs none.
 *
 * <p>What this proves: that a refused open never runs the body it precedes and
 * releases the world lock and the stem it was handed, that only a replacement
 * answer reaches the storage and reaches it once, that vanilla saving runs
 * whatever Ravel answers, that the mods' pack is in the load the created world
 * inherits, and that it is on disk in time for that world's repository. What it
 * does not prove: anything about the real client, which needs a launch.
 */
public final class PersistenceMain {

    public static void main(String[] args) throws IOException {
        SavedDataStorage.seed(
                System.getProperty("ravel.test.persistence.stored", "").isEmpty()
                        ? null
                        : System.getProperty("ravel.test.persistence.stored").getBytes());

        // A world and a staging directory nothing has written into yet, so what
        // the steps below find is what this run put there. Runs share a working
        // directory, and one that inherited the previous run's pack would pass
        // without proving anything.
        emptyTree(Path.of("world"));
        emptyTree(STAGING);

        // The screen, twice. Building widgets is what the game runs again every
        // time the screen is shown, including after the load staging asks for, so
        // the second call is the loop guard's own case: one staged write and one
        // data load however many times this runs.
        CreateWorldScreen screen = new CreateWorldScreen(STAGING, List.of("vanilla"));
        screen.init();
        screen.init();
        System.out.println("staged=" + where(dev.ravel.jvm.Hook.stagedDirectory())
                + " stagedWrites=" + dev.ravel.jvm.Hook.stagedWrites()
                + " screenLoads=" + screen.loads()
                + " loaded=" + screen.loaded());

        LevelStorageSource.LevelStorageAccess access =
                new LevelStorageSource.LevelStorageAccess("world");
        WorldStem stem = new WorldStem();

        // Creating the world copies the staging directory's contents into it,
        // which is where a staged pack has to end up for the world to keep it.
        screen.copyInto(Path.of("world", "datapacks"));

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

    /**
     * Where the stand-in screen stages packs.
     *
     * <p>Absolute because the game's own is: it makes a temporary directory, and
     * the native side is handed whatever the screen spells. A relative one would
     * also be a path the production write refuses, which is a rule this harness
     * does not reach and should not quietly contradict.
     */
    private static final Path STAGING = Path.of("staging").toAbsolutePath();

    /**
     * Which directory the staging crossing named.
     *
     * <p>Reported as a place rather than as a path because the path is this
     * machine's. What the case is about is whether the pack went where the
     * screen's own load would read it, and that is a comparison rather than a
     * string.
     */
    private static String where(String directory) {
        if (directory.isEmpty()) {
            return "none";
        }
        return STAGING.equals(Path.of(directory)) ? "screen" : "elsewhere";
    }

    /** Removes whatever a previous run left in one tree. */
    private static void emptyTree(Path root) throws IOException {
        if (!Files.exists(root)) {
            return;
        }
        try (Stream<Path> tree = Files.walk(root)) {
            for (Path entry : tree.sorted(Comparator.reverseOrder()).toList()) {
                Files.delete(entry);
            }
        }
    }
}
