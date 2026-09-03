package net.minecraft.server.packs.repository;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.stream.Stream;

/**
 * Stands in for the pack repository.
 *
 * <p><b>Discovering and selecting are two steps.</b> A pack that appears in the
 * directory after a scan is not available until another scan, and a pack that is
 * available is not selected until something selects it. Neither happens on its
 * own, and a stand-in that let either happen implicitly would pass a caller that
 * takes neither step.
 */
public final class PackRepository {

    /**
     * The pack every repository has, which no directory holds.
     *
     * <p>The real one comes from a built-in source beside the directory source.
     * It is here because selecting drops ids the last scan did not find, and a
     * screen whose configuration names this one is the ordinary case.
     */
    private static final String BUILT_IN = "vanilla";

    /**
     * How a pack found in a directory is identified, which is the game's rule.
     *
     * <p>Spelled here rather than asked for, unlike everywhere on Ravel's side of
     * the seam: this stands in for the game's own folder source, which is what
     * owns the rule.
     */
    private static final String DISCOVERED = "file/";

    private final Path directory;
    private final List<String> available = new ArrayList<>();
    private final List<String> selected = new ArrayList<>();

    public PackRepository() {
        this(Path.of("."));
    }

    public PackRepository(Path directory) {
        this.directory = directory;
    }

    /**
     * Rescans the directory, keeping whatever of the selection survives it.
     *
     * <p>This is the step a pack written after the repository was built needs:
     * until it runs, the pack is on disk and this repository has never heard of
     * it.
     */
    public void reload() {
        List<String> found = new ArrayList<>();
        found.add(BUILT_IN);
        if (Files.isDirectory(directory)) {
            try (Stream<Path> entries = Files.list(directory)) {
                entries.map(entry -> DISCOVERED + entry.getFileName()).sorted().forEach(found::add);
            } catch (IOException unreadable) {
                throw new UncheckedIOException(unreadable);
            }
        }
        List<String> keep = List.copyOf(selected);
        available.clear();
        available.addAll(found);
        setSelected(keep);
    }

    /**
     * What a selected pack's files hold, which is what loading it reads.
     *
     * <p>Here because a load that only restated the ids it was given would pass a
     * caller that never re-read anything: the ids are identical across a
     * replacement and the files are not.
     */
    public String contents(String id) {
        if (!id.startsWith(DISCOVERED)) {
            return "";
        }
        Path pack = directory.resolve(id.substring(DISCOVERED.length()));
        if (!Files.isDirectory(pack)) {
            return "";
        }
        try (Stream<Path> tree = Files.walk(pack)) {
            List<String> read = new ArrayList<>();
            for (Path entry : tree.filter(Files::isRegularFile).sorted().toList()) {
                read.add(Files.readString(entry).strip());
            }
            return String.join("+", read);
        } catch (IOException unreadable) {
            throw new UncheckedIOException(unreadable);
        }
    }

    public Collection<String> getSelectedIds() {
        return List.copyOf(selected);
    }

    /** Replaces the selection with the ids given, dropping what no scan found. */
    public void setSelected(Collection<String> ids) {
        List<String> kept = ids.stream().filter(available::contains).toList();
        selected.clear();
        selected.addAll(kept);
    }
}
