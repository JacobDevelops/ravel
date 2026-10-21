package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.locks.ReentrantLock;

/**
 * Off-default geometry oracle. Only published, same-task game VBOs are authoritative.
 * Association proves section/task/origin identity, not temporal world equivalence: the native
 * snapshot reads the task's mutable WorldRegion after vanilla compilation. Pixel discrepancies
 * therefore remain explicit; this position-only comparison cannot establish texture fidelity.
 * Only sections whose vanilla geometry is exclusively in SOLID are eligible; nonempty cutout
 * or translucent layers refuse the entire comparison rather than silently omitting geometry.
 */
final class LegacyVanillaGeometry {
    private static final Associations ASSOCIATIONS = new Associations();
    private static Access access;
    private static long selectionStatus, selectionGeneration, selectionKey;

    private LegacyVanillaGeometry() { }

    static void associate(long key, long generation, Object chunk, Object task, Object compiled,
                          int x, int y, int z) {
        if (!LegacyRenderer.geometryProofSelected() || !LegacyRenderer.accepts(generation)) return;
        ASSOCIATIONS.admit(key, generation, chunk, task, compiled, x, y, z);
    }

    static void discard(long key, long generation) { ASSOCIATIONS.remove(key, generation); }

    static void clear(long generation) {
        ASSOCIATIONS.retire(generation);
        selectionStatus = selectionGeneration = selectionKey = 0;
    }

    /** Scope refusal is a completed disposition, never an upload of a partial vanilla section. */
    static long eligibility(long generation, long key) throws ReflectiveOperationException {
        Entry entry = ASSOCIATIONS.get(key, generation);
        long[] status = entry == null ? result(-5, generation, key) : inspect(entry, false);
        selectionStatus = status[0];
        selectionGeneration = generation;
        selectionKey = key;
        return status[0];
    }

    static void retainResident(long generation, long key) {
        ASSOCIATIONS.pin(key, generation);
    }

    /** status,generation,key,vertices,layers,rgbaMismatch,coverageMismatch,depthMismatch,pixels. */
    static long[] compareResident() throws ReflectiveOperationException {
        if (!LegacyRenderer.geometryProofSelected()) return result(0, 0, 0);
        long[] resident = LegacyRenderer.geometryProofIdentity();
        if (resident.length != 2 || resident[0] <= 0 || resident[1] <= 0)
            return result(withoutResident(selectionStatus), selectionGeneration, selectionKey);
        long generation = resident[0], key = resident[1];
        Entry entry = ASSOCIATIONS.get(key, generation);
        return entry == null ? result(-5, generation, key) : inspect(entry, true);
    }

    static long withoutResident(long status) { return status == 1 ? 0 : status; }

    static boolean retryEligibility(long status) { return status == -2; }

    private static long[] inspect(Entry entry, boolean compare) throws ReflectiveOperationException {
        long generation = entry.generation, key = entry.key;
        if (!LegacyRenderer.accepts(generation)) return result(-1, generation, key);
        Access a = access;
        if (a == null) access = a = new Access();
        ReentrantLock taskLock = (ReentrantLock) a.taskLock.get(entry.chunk);
        if (!taskLock.tryLock()) return result(-2, generation, key);
        try {
            ReentrantLock workLock = (ReentrantLock) a.workLock.invoke(entry.task);
            if (!workLock.tryLock()) return result(-2, generation, key);
            try {
                ReentrantLock compileLock = (ReentrantLock) a.compileLock.get(entry.chunk);
                if (!compileLock.tryLock()) return result(-2, generation, key);
                try {
                    return compareLocked(a, entry, compare);
                } finally { compileLock.unlock(); }
            } finally { workLock.unlock(); }
        } finally { taskLock.unlock(); }
    }

    // Caller retains chunk taskLock -> task workLock -> chunk compileLock throughout.
    private static long[] compareLocked(Access a, Entry entry, boolean compare) throws ReflectiveOperationException {
        long generation = entry.generation, key = entry.key;
        if (!current(a, entry)) return result(-1, generation, key);
        Object[] layers = a.layerType.getEnumConstants();
        if (layers == null || layers.length != 4) return result(-3, generation, key);
        Object solid = a.solid.get(null);
        int solidIndex = solid instanceof Enum<?> value ? value.ordinal() : -1;
        int layerMask = 0;
        for (int layer = 0; layer < layers.length; layer++)
            // Feather hasBlock returns true for EMPTY in official bxo.b.
            if (!(boolean) a.empty.invoke(entry.compiled, layers[layer])) layerMask |= 1 << layer;
        if (!solidLayerMask(layerMask, solidIndex)) return result(-4, generation, key);
        if (layers[solidIndex] != solid) return result(-3, generation, key);
        if (!compare) return result(1, generation, key);
        if (!fresh(a, entry)) return result(-1, generation, key);
        Object buffer = a.buffer.invoke(entry.chunk, solidIndex);
        if (buffer == null) return result(-3, generation, key);
        Object format = a.format.get(buffer);
        int count = a.vertexCount.getInt(buffer), id = a.id.getInt(buffer);
        if (id <= 0 || count <= 0 || count > 1_048_576 || count % 4 != 0
                || !a.blockFormat.get(null).equals(format) || (int) a.stride.invoke(format) != 28)
            return result(-3, generation, key);
        long[] comparison = LegacyRenderer.geometryProofCompareVanilla(() -> draw(a, id, count));
        if (!fresh(a, entry) || comparison.length != 4) return result(-1, generation, key);
        if (comparison[3] != 4096) return result(-3, generation, key);
        for (int metric = 0; metric < 3; metric++)
            if (comparison[metric] < 0 || comparison[metric] > comparison[3])
                return result(-3, generation, key);
        long[] after = LegacyRenderer.geometryProofIdentity();
        if (after.length != 2 || after[0] != generation || after[1] != key)
            return result(-1, generation, key);
        return new long[]{1, generation, key, count, 1,
                comparison[0], comparison[1], comparison[2], comparison[3]};
    }

    private static void draw(Access a, int buffer, int count) {
        try {
            LegacyRenderer.geometryProofBindVanillaBuffer(buffer);
            a.pointer.invoke(null, 3, 5126, 28, 0L); // float3, BLOCK position
            a.draw.invoke(null, 7, 0, count); // actual game GL_QUADS
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("vanilla VBO draw failed", failure);
        }
    }

    private static boolean fresh(Access a, Entry e) throws ReflectiveOperationException {
        return current(a, e) && a.published.invoke(e.chunk) == e.compiled;
    }

    private static boolean current(Access a, Entry e) throws ReflectiveOperationException {
        if (!LegacyRenderer.accepts(e.generation) || ASSOCIATIONS.get(e.key, e.generation) != e
                || a.pending.get(e.chunk) != e.task || (boolean) a.cancelled.invoke(e.task)
                || a.taskCompiled.invoke(e.task) != e.compiled) return false;
        Object origin = a.origin.get(e.chunk);
        return (int) a.x.invoke(origin) == e.x && (int) a.y.invoke(origin) == e.y
                && (int) a.z.invoke(origin) == e.z;
    }

    private static long[] result(long status, long generation, long key) {
        return new long[]{status, generation, key, 0, 0, 0, 0, 0, 0};
    }

    static boolean solidLayerMask(int mask, int solidIndex) {
        return solidIndex >= 0 && solidIndex < 4 && mask == (1 << solidIndex);
    }

    static final class Associations {
        private final Entry[] entries = new Entry[LegacyMeshWork.SLOTS];
        private Entry resident;
        private long retired;
        synchronized void admit(long key, long generation, Object chunk, Object task, Object compiled,
                                int x, int y, int z) {
            if (retired(generation)) return;
            if (!put(key, generation, chunk, task, compiled, x, y, z))
                throw new IllegalStateException("Vanilla oracle association table is full or invalid");
        }


        synchronized boolean put(long key, long generation, Object chunk, Object task, Object compiled,
                                 int x, int y, int z) {
            if (key <= 0 || generation <= retired || chunk == null || task == null || compiled == null
                    || ((x | y | z) & 15) != 0) return false;
            for (int i = 0; i < entries.length; i++) {
                if (entries[i] != null) continue;
                entries[i] = new Entry(key, generation, chunk, task, compiled, x, y, z);
                return true;
            }
            return false;
        }

        synchronized Entry get(long key, long generation) {
            if (generation <= retired) return null;
            if (resident != null && resident.key == key && resident.generation == generation) return resident;
            for (Entry entry : entries)
                if (entry != null && entry.key == key && entry.generation == generation) return entry;
            return null;
        }

        synchronized void pin(long key, long generation) {
            Entry entry = get(key, generation);
            if (entry == null) throw new IllegalStateException("Resident vanilla association is absent");
            resident = entry;
        }

        synchronized void remove(long key, long generation) {
            for (int i = 0; i < entries.length; i++)
                if (entries[i] != null && entries[i].key == key && entries[i].generation == generation)
                    entries[i] = null;
        }

        synchronized int size() {
            int count = 0;
            for (Entry entry : entries) if (entry != null) count++;
            return count;
        }

        synchronized void retire(long generation) {
            retired = Math.max(retired, generation);
            if (resident != null && resident.generation <= retired) resident = null;
            for (int i = 0; i < entries.length; i++)
                if (entries[i] != null && entries[i].generation <= retired) entries[i] = null;
        }

        synchronized boolean retired(long generation) { return generation <= retired; }
    }

    private record Entry(long key, long generation, Object chunk, Object task, Object compiled,
                         int x, int y, int z) { }

    private static final class Access {
        final Field taskLock = field("LEGACY_RENDER_CHUNK", "taskLock");
        final Field compileLock = field("LEGACY_RENDER_CHUNK", "compileLock");
        final Field pending = field("LEGACY_RENDER_CHUNK", "pendingTask");
        final Field origin = field("LEGACY_RENDER_CHUNK", "origin");
        final Method published = method("LEGACY_RENDER_CHUNK", "getCompiledChunk");
        final Method buffer = method("LEGACY_RENDER_CHUNK", "getBuffer");
        final Method cancelled = method("LEGACY_COMPILE_TASK", "isCancelled");
        final Method taskCompiled = method("LEGACY_COMPILE_TASK", "getCompiledChunk");
        final Method workLock = method("LEGACY_COMPILE_TASK", "getLock");
        final Class<?> layerType = LegacyAccess.type("LEGACY_BLOCK_LAYER");
        final Field solid = field("LEGACY_BLOCK_LAYER", "SOLID");
        final Method empty = method("LEGACY_COMPILED_CHUNK", "hasBlock");
        final Field id = field("LEGACY_VERTEX_BUFFER", "id");
        final Field vertexCount = field("LEGACY_VERTEX_BUFFER", "vertexCount");
        final Field format = field("LEGACY_VERTEX_BUFFER", "format");
        final Field blockFormat = field("LEGACY_VERTEX_FORMATS", "BLOCK");
        final Method stride = method("LEGACY_VERTEX_FORMAT", "getVertexSize");
        final Method x = method("LEGACY_VEC3I", "getX");
        final Method y = method("LEGACY_VEC3I", "getY");
        final Method z = method("LEGACY_VEC3I", "getZ");
        final Class<?> gl = Class.forName("org.lwjgl.opengl.GL11", true, ClassLoader.getSystemClassLoader());
        final Method pointer = gl.getMethod("glVertexPointer", int.class, int.class, int.class, long.class);
        final Method draw = gl.getMethod("glDrawArrays", int.class, int.class, int.class);

        Access() throws ReflectiveOperationException { }
    }

    private static Field field(String role, String name) throws ReflectiveOperationException {
        return Members.field(LegacyAccess.type(role), LegacyNames.member(role, name));
    }

    private static Method method(String role, String name) throws ReflectiveOperationException {
        return LegacyAccess.method(role, name);
    }
}
