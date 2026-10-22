package dev.ravel.legacy.opengl;

import java.util.Iterator;
import java.util.LinkedHashMap;

final class BakedWorldTable {
    private static final LinkedHashMap<Long, Entry> ENTRIES = new LinkedHashMap<>();
    private static long budget = 16L * 1024 * 1024;
    private static long bytes;
    private static long peak;
    private static long evictions;
    private static final java.util.Set<Long> EVICTED = bounded();
    private static final java.util.Set<Long> OVER_BUDGET = bounded();
    private static final long SCRATCH = 3L * GeometryProof.MAX_BAKED_QUADS * 112;

    record Entry(long generation, long key, long resource, long atlas, int x, int y, int z,
            int buffer, int[] layers, int bytes) { }

    private BakedWorldTable() { }
    private static java.util.Set<Long> bounded() {
        return java.util.Collections.newSetFromMap(new LinkedHashMap<Long, Boolean>() {
            @Override protected boolean removeEldestEntry(java.util.Map.Entry<Long, Boolean> eldest) { return size() > 4096; }
        });
    }

    static boolean configure(int distance) {
        if (distance < 1 || distance > 32 || !ENTRIES.isEmpty()) return false;
        long diameter = distance * 2L + 1;
        long next = Math.clamp(diameter * diameter * 262144L, 16L * 1024 * 1024, 256L * 1024 * 1024);
        if (budget != next) peak = 0;
        budget = next;
        return true;
    }

    static boolean publish(int[] emitted, long[] identity) {
        int[] layers = new int[4];
        int size = 0;
        for (int layer = 0; layer < 4; layer++) {
            layers[layer] = emitted[layer] * 4;
            size += layers[layer] * 28;
        }
        if (size == 0) return false;
        if (size + SCRATCH > budget) { OVER_BUDGET.add(identity[1]); return false; }
        Iterator<Entry> iterator = ENTRIES.values().iterator();
        while (iterator.hasNext()) {
            Entry entry = iterator.next();
            if (entry.generation == identity[0] && entry.x == identity[4]
                    && entry.y == identity[5] && entry.z == identity[6]) {
                retire(entry); iterator.remove();
            }
        }
        while (bytes + size + SCRATCH > budget || ENTRIES.size() >= 4096) evict(identity);
        int buffer = GeometryProof.retainWorld(emitted, size);
        if (buffer == 0) return false;
        ENTRIES.put(identity[1], new Entry(identity[0], identity[1], identity[2], identity[3],
                Math.toIntExact(identity[4]), Math.toIntExact(identity[5]), Math.toIntExact(identity[6]),
                buffer, layers, size));
        bytes += size;
        peak = Math.max(peak, bytes + SCRATCH);
        return true;
    }

    static Entry get(long generation, long key, long resource, long atlas) {
        Entry entry = ENTRIES.get(key);
        return entry != null && entry.generation == generation && entry.resource == resource
                && entry.atlas == atlas ? entry : null;
    }
    static int missing(long key) {
        return EVICTED.contains(key) ? ResidentWorldDraw.GL_EVICTED
                : OVER_BUDGET.contains(key) ? ResidentWorldDraw.GL_OVER_BUDGET : ResidentWorldDraw.GL_STALE_KEY;
    }
    static long[] identity(int x, int y, int z) {
        for (Entry entry : ENTRIES.values())
            if (entry.x == x && entry.y == y && entry.z == z)
                return new long[]{entry.generation, entry.key, entry.resource, entry.atlas,
                        entry.x, entry.y, entry.z};
        return new long[0];
    }

    private static void evict(long[] identity) {
        Entry farthest = null;
        long distance = -1;
        for (Entry entry : ENTRIES.values()) {
            long next = Math.abs(entry.x - identity[4]) + Math.abs(entry.y - identity[5])
                    + Math.abs(entry.z - identity[6]);
            if (next > distance) { farthest = entry; distance = next; }
        }
        if (farthest == null) throw new IllegalStateException("World residency accounting drift");
        retire(farthest);
        ENTRIES.remove(farthest.key);
        EVICTED.add(farthest.key);
        evictions++;
    }

    private static void retire(Entry entry) {
        GeometryProof.delete(entry.buffer);
        bytes -= entry.bytes;
    }

    static void clear() {
        for (Entry entry : ENTRIES.values()) retire(entry);
        ENTRIES.clear();
        EVICTED.clear();
        OVER_BUDGET.clear();
    }

    static long[] metrics() { return new long[]{budget, bytes + (ENTRIES.isEmpty() ? 0 : SCRATCH), peak,
            ENTRIES.size(), evictions}; }
}
