package net.minecraft.world.level.storage;

import com.mojang.datafixers.DataFixer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import net.minecraft.core.HolderLookup;
import net.minecraft.world.level.saveddata.SavedData;
import net.minecraft.world.level.saveddata.SavedDataType;

/**
 * Stands in for the pinned storage, and counts what the adapter does to it.
 *
 * <p>The counters are static because ADG-7's two crossings reach two different
 * storages — the open path opens one over the world directory and closes it
 * again, and the save path asks the server for its own — and what the harness
 * has to prove is about the pair: that a replacement is written exactly once and
 * that every other answer writes nothing at all.
 */
public final class SavedDataStorage implements AutoCloseable {

    /** What a read finds, seeded by the harness for a world that holds data. */
    private static byte[] stored;

    private static int writes;
    private static byte[] written;

    private boolean closed;

    public SavedDataStorage(Path directory, DataFixer fixer, HolderLookup.Provider registries) {
    }

    /** Seeds the aggregate a world already holds, or {@code null} for a world that holds none. */
    public static void seed(byte[] aggregate) {
        stored = aggregate;
        writes = 0;
        written = null;
    }

    /** How many times the adapter wrote a replacement. */
    public static int writes() {
        return writes;
    }

    /** The replacement bytes as text, or empty when nothing was written. */
    public static String written() {
        return written == null ? "" : new String(written, StandardCharsets.UTF_8);
    }

    public SavedData get(SavedDataType type) {
        return stored == null ? null : new SavedData(stored);
    }

    public void set(SavedDataType type, SavedData data) {
        writes++;
        written = data.aggregate();
    }

    @Override
    public void close() {
        closed = true;
    }
}
