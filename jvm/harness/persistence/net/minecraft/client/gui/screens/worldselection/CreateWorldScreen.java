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

    private PackRepository stagingRepository;
    private int loads;

    public CreateWorldScreen(Path staging, List<String> enabled) {
        this.staging = staging;
        uiState = new WorldCreationUiState(
                new WorldCreationContext(new WorldDataConfiguration(enabled)));
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
        if (stagingRepository == null) {
            stagingRepository = new PackRepository(staging);
        }
        stagingRepository.setSelected(configuration.enabled());
        return Pair.of(staging, stagingRepository);
    }

    /**
     * Reloads this screen's data from a repository.
     *
     * <p>The listing is taken here rather than at construction because that is
     * where the game takes it: the reload rescans the directory and adds every
     * pack it finds whose source asks to be added automatically. A pack staged
     * after this ran is a pack this screen never loaded.
     */
    private void tryApplyNewDataPacks(
            PackRepository repository, boolean confirmExperimental, Consumer<Object> onRefused)
            throws IOException {
        loads++;
        loaded.clear();
        loaded.addAll(repository.selected());
        try (Stream<Path> entries = Files.list(repository.directory())) {
            entries.map(entry -> entry.getFileName().toString())
                    .filter(name -> !loaded.contains(name))
                    .sorted()
                    .forEach(loaded::add);
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
