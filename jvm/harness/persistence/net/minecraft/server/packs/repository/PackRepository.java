package net.minecraft.server.packs.repository;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Stands in for the pack repository.
 *
 * <p>It was a parameter and nothing else until a world being created turned out
 * to read its packs through one. It now carries the directory it was built over
 * and what is selected in it, which is the pair the create-world screen's own
 * load reads.
 */
public final class PackRepository {

    private final Path directory;
    private final List<String> selected = new ArrayList<>();

    public PackRepository() {
        this(Path.of("."));
    }

    public PackRepository(Path directory) {
        this.directory = directory;
    }

    public Path directory() {
        return directory;
    }

    public List<String> selected() {
        return List.copyOf(selected);
    }

    public void setSelected(List<String> ids) {
        selected.clear();
        selected.addAll(ids);
    }
}
