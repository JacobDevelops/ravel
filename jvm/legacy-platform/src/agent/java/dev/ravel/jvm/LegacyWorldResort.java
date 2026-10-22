package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;

final class LegacyWorldResort {
    private static final boolean SELECTED = System.getProperty("ravel.renderer", "vanilla").equals("resident-world");
    private static final Slot[] SLOTS = new Slot[64];
    private static final Field[] OFFSETS = new Field[3];
    private static final long[] METRICS = new long[3];
    private static final int[] ORIGIN = new int[3];
    private static final float[] CAMERA = new float[3];
    private static Method buffer;
    private static Method destination;
    private static int allocated;

    private LegacyWorldResort() { }

    static long[] metrics() { synchronized (LegacyWorldDraw.class) { return METRICS.clone(); } }

    static void clear() {
        java.util.Arrays.fill(SLOTS, null);
        allocated = 0;
    }

    static void sort(Object builder, float x, float y, float z) {
        if (!SELECTED) return;
        synchronized (LegacyWorldDraw.class) {
            try { sortCurrent(builder, x, y, z); }
            catch (ReflectiveOperationException failure) {
                throw new IllegalStateException("Resident transparency sort failed", failure);
            }
        }
    }

    private static void initialize() throws ReflectiveOperationException {
        if (buffer != null) return;
        buffer = LegacyAccess.method("LEGACY_BUFFER_BUILDER", "getBuffer");
        destination = LegacyAccess.method("LEGACY_RENDER_CHUNK", "getBuffer");
        String[] names = {"offsetX", "offsetY", "offsetZ"};
        for (int axis = 0; axis < 3; axis++)
            OFFSETS[axis] = Members.field(LegacyAccess.type("LEGACY_BUFFER_BUILDER"),
                    LegacyNames.member("LEGACY_BUFFER_BUILDER", names[axis]));
    }

    private static void sortCurrent(Object builder, float x, float y, float z)
            throws ReflectiveOperationException {
        initialize();
        Object bytes = buffer.invoke(builder);
        for (Slot slot : SLOTS)
            if (slot != null && slot.builder == builder && !slot.uploaded) slot.clear();
        CAMERA[0] = x; CAMERA[1] = y; CAMERA[2] = z;
        for (int axis = 0; axis < 3; axis++) {
            double translation = OFFSETS[axis].getDouble(builder);
            if (!Double.isFinite(translation) || translation != Math.rint(translation)) return;
            ORIGIN[axis] = Math.toIntExact(-(long) translation);
            CAMERA[axis] = (float) (CAMERA[axis] + translation);
        }
        LegacyWorldDraw.Entry entry = LegacyWorldDraw.sorting(ORIGIN);
        if (entry == null || entry.layers()[3] == 0) return;
        Slot slot = reserve(4 + entry.layers()[3] * 28);
        if (slot == null) { refuse(entry.key()); return; }
        System.arraycopy(entry.layers(), 0, slot.output, 0, 4);
        if (Hook.ravelRendererBakedResort(entry.generation(), entry.key(), CAMERA, slot.output) != 1) {
            slot.bytes = null; refuse(entry.key()); return;
        }
        slot.entry = entry;
        slot.camera[0] = x; slot.camera[1] = y; slot.camera[2] = z;
        slot.task = LegacyWorldDraw.sortTask(entry);
        slot.builder = builder;
        slot.destination = destination.invoke(entry.chunk(), 3);
        slot.bytes = bytes;
        METRICS[0]++;
        entry.sorts()[0]++;
    }

    private static Slot reserve(int words) {
        Slot available = null;
        for (int index = 0; index < SLOTS.length; index++) {
            if (SLOTS[index] == null) SLOTS[index] = new Slot();
            Slot slot = SLOTS[index];
            if (slot.entry != null && LegacyWorldDraw.resident(slot.entry.key()) == null) slot.clear();
            if (slot.entry == null) {
                if (slot.output != null && slot.output.length == words) return slot;
                if (available == null) available = slot;
            }
        }
        if (available == null) return null;
        int previous = available.output == null ? 0 : available.output.length * 4;
        int next = allocated - previous + words * 4;
        if (next > 16 * 1024 * 1024) return null;
        available.output = new int[words]; allocated = next;
        return available;
    }

    static void upload(Object destination, Object bytes) {
        if (!SELECTED) return;
        synchronized (LegacyWorldDraw.class) {
            try {
                for (Slot slot : SLOTS) {
                    if (slot == null || slot.bytes != bytes) continue;
                    LegacyWorldDraw.Entry entry = LegacyWorldDraw.resident(slot.entry.key());
                    if (entry == null) { slot.clear(); return; }
                    if (slot.destination != destination) { slot.clear(); return; }
                    if (!LegacyWorldDraw.sorted(entry.key(), slot.task)) {
                        refuse(entry.key()); slot.clear(); return;
                    }
                    slot.bytes = null; slot.uploaded = true;
                    if (entry.ready()) apply(slot);
                    else for (Slot previous : SLOTS)
                        if (previous != null && previous != slot && previous.uploaded
                                && previous.entry.key() == entry.key()) previous.clear();
                    return;
                }
            } catch (ReflectiveOperationException failure) {
                throw new IllegalStateException("Resident transparency upload failed", failure);
            }
        }
    }

    static boolean acknowledge(long key) {
        for (Slot slot : SLOTS)
            if (slot != null && slot.uploaded && slot.entry.key() == key) return apply(slot);
        return true;
    }

    private static boolean apply(Slot slot) {
        LegacyWorldDraw.Entry entry = slot.entry;
        boolean applied = LegacyRenderer.accepts(entry.generation())
                && LegacyGeometryProof.resortWorld(slot.output, entry.generation(), entry.key());
        if (applied) {
            METRICS[1]++; entry.sorts()[1]++;
            System.arraycopy(slot.camera, 0, entry.sortCamera(), 0, 3);
        }
        else refuse(entry.key());
        slot.clear();
        return applied;
    }

    private static void refuse(long key) {
        METRICS[2]++;
        LegacyWorldDraw.Entry entry = LegacyWorldDraw.resident(key);
        if (entry != null) entry.sorts()[2]++;
        LegacyWorldDraw.refuse(key);
    }

    private static final class Slot {
        final float[] camera = new float[3];
        int[] output;
        Object bytes;
        Object builder;
        Object task;
        boolean uploaded;
        Object destination;
        LegacyWorldDraw.Entry entry;
        void clear() {
            bytes = null; builder = null; task = null; destination = null; entry = null; uploaded = false;
        }
    }
}
