import java.util.Optional;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.worldselection.WorldOpenFlows;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.WorldStem;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.SavedDataStorage;

/**
 * Drives ADG-7's two world persistence seams against a stand-in game.
 *
 * <p>What this proves: that a refused open never runs the body it precedes and
 * releases the world lock and the stem it was handed, that only a replacement
 * answer reaches the storage and reaches it once, and that vanilla saving runs
 * whatever Ravel answers. What it does not prove: anything about the real
 * client, which needs a launch.
 */
public final class PersistenceMain {

    public static void main(String[] args) {
        SavedDataStorage.seed(
                System.getProperty("ravel.test.persistence.stored", "").isEmpty()
                        ? null
                        : System.getProperty("ravel.test.persistence.stored").getBytes());

        LevelStorageSource.LevelStorageAccess access =
                new LevelStorageSource.LevelStorageAccess("world");
        WorldStem stem = new WorldStem();

        // The pack step first, as the game runs it: the access exists, and the
        // pack list has not been built. What the harness reads back is that the
        // crossing named this world and that no world load had run when it did.
        new WorldOpenFlows().openWorldLoadLevelData(access, () -> {});
        System.out.println("datapack=" + dev.ravel.jvm.Hook.datapackWorld()
                + " loadsAtWrite=" + Minecraft.loads());

        new Minecraft().doWorldLoad(access, new PackRepository(), stem, Optional.empty(), false);
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
}
