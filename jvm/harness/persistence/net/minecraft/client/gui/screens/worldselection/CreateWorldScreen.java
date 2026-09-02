package net.minecraft.client.gui.screens.worldselection;

import com.mojang.datafixers.util.Pair;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.stream.Stream;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.world.level.WorldDataConfiguration;

/**
 * Stands in for the screen a world is created from, carrying the three steps
 * that decide what the created world knows.
 *
 * <p>The real one loads its data once when it opens and hands the created world
 * those same resources, which is why a pack that reaches only the world's own
 * directory is listed there and read by nothing. So the shape that matters is
 * the one modelled here: a staging directory, a load that reads it, and a copy
 * of that directory's contents into the world.
 *
 * <p>Nothing here knows Ravel exists. The two sides meet on the filesystem and
 * through the members the reviewed reflective surface pins, exactly as they do
 * in the game.
 */
public final class CreateWorldScreen {

    /** What the screen's own data load found, which is what the world inherits. */
    private final List<String> loaded = new ArrayList<>();

    private final Path staging;
    private final WorldCreationUiState uiState;

    private final PackRepository stagingRepository;
    private int loads;

    public CreateWorldScreen(Path staging, List<String> enabled) {
        this.staging = staging;
        WorldDataConfiguration configuration = new WorldDataConfiguration(enabled);
        uiState = new WorldCreationUiState(new WorldCreationContext(configuration));
        // The load the real screen runs when it opens, before Ravel has staged
        // anything. Without it nothing here holds data a later load can replace,
        // and a case about re-reading a pack would have nothing to re-read.
        stagingRepository = new PackRepository(staging);
        stagingRepository.reload();
        stagingRepository.setSelected(enabled);
        load(stagingRepository, configuration);
    }

    /** The resources this screen loaded, which the created world is handed. */
    public List<String> loaded() {
        return List.copyOf(loaded);
    }

    /** How many data loads this screen ran, which the staging guard bounds. */
    public int loads() {
        return loads;
    }

    public WorldCreationUiState getUiState() {
        return uiState;
    }

    /**
     * Builds widgets, which is the seam Ravel stages from.
     *
     * <p>The body does nothing, deliberately. Everything this case is about
     * happens in the prologue, and the game runs this again every time the screen
     * is shown — including after the load staging asks for — so a second call
     * that staged again would be the loop the guard exists to stop.
     */
    public void init() {
    }

    /**
     * Forces the staging directory and its repository, and selects what it is given.
     *
     * <p>Private, as the game's is. Reaching it is the whole reason the reviewed
     * surface names it.
     */
    private Pair<Path, PackRepository> getDataPackSelectionSettings(
            WorldDataConfiguration configuration) throws IOException {
        Files.createDirectories(staging);
        stagingRepository.reload();
        stagingRepository.setSelected(configuration.enabled());
        return Pair.of(staging, stagingRepository);
    }

    /**
     * Reloads this screen's data from a repository it is handed.
     *
     * <p>It reads a selection and makes none. Discovering the pack and selecting
     * it are the caller's two steps, and this is where skipping either one costs
     * the world its data.
     *
     * <p>The game's own version returns without loading when the selection it
     * reads is the one this screen already holds, which is what makes a replaced
     * pack invisible to it: the same ids over different files are the same ids.
     * That comparison is not modelled here, because Ravel reaches this call only
     * where it has just put an id into a selection the screen did not hold, so no
     * run could take the branch — a stand-in carrying it would be an untested
     * model of the game. The case it would have guarded is the one Ravel reaches
     * {@link #applyNewPackConfig} directly for.
     */
    private void tryApplyNewDataPacks(
            PackRepository repository, boolean confirmExperimental, Consumer<Object> onRefused) {
        List<String> enabled = List.copyOf(repository.getSelectedIds());
        applyNewPackConfig(repository, new WorldDataConfiguration(enabled), onRefused);
    }

    /**
     * Loads the data a configuration names, which is the reload itself.
     *
     * <p>Split from the call above as the game splits it, because the two are
     * reached separately: that one decides whether to load and this one loads.
     * What the screen holds afterwards is what the created world is handed.
     */
    private void applyNewPackConfig(
            PackRepository repository,
            WorldDataConfiguration configuration,
            Consumer<Object> onRefused) {
        load(repository, configuration);
    }

    /**
     * Reads what the selected packs hold, which is what the screen then holds.
     *
     * <p>Reading the files rather than restating the ids is what makes a
     * replacement visible at all: the same ids over different files are the same
     * ids, so a load that copied its configuration would look identical whether
     * it ran over the pack that is there or the one that was.
     */
    private void load(PackRepository repository, WorldDataConfiguration configuration) {
        uiState.setSettings(new WorldCreationContext(configuration));
        loads++;
        loaded.clear();
        for (String id : configuration.enabled()) {
            String contents = repository.contents(id);
            loaded.add(contents.isEmpty() ? id : id + "=" + contents);
        }
    }

    /**
     * Copies the staging directory's contents into a world, as creating one does.
     *
     * <p>Its contents rather than the directory, which is what decides where a
     * staged pack has to sit: the game copies each entry below the staging
     * directory into the world's {@code datapacks} directory, so a pack directory
     * staged one level too deep arrives one level too deep.
     */
    public void copyInto(Path worldPacks) throws IOException {
        // The game makes its staging directory only when something stages a pack,
        // and creates no world packs directory when nothing did.
        if (!Files.isDirectory(staging)) {
            return;
        }
        Files.createDirectories(worldPacks);
        try (Stream<Path> tree = Files.walk(staging)) {
            for (Path entry : tree.filter(path -> !path.equals(staging)).sorted().toList()) {
                Path destination = worldPacks.resolve(staging.relativize(entry).toString());
                if (Files.isDirectory(entry)) {
                    Files.createDirectories(destination);
                } else {
                    Files.copy(entry, destination);
                }
            }
        }
    }
}
