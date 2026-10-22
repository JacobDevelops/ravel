package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;

final class LegacyBakedResort {
    private static Slot[] slots;
    private static int[] uploaded;
    private static final Field[] offsets = new Field[3];
    private static final float[] camera = new float[3];
    private static final int[] origin = new int[3];
    private static final long[] METRICS = new long[3];
    private static Method buffer;
    private static long generation;
    private static long key;
    private static volatile Object target;
    private static boolean acknowledged;
    private static boolean hasUpload;

    private LegacyBakedResort() { }

    static synchronized long[] metrics() { return METRICS.clone(); }

    static synchronized void clear() {
        generation = 0; key = 0; target = null;
        acknowledged = false; hasUpload = false;
        if (slots != null) for (Slot slot : slots) if (slot != null) slot.bytes = null;
    }

    static synchronized void offer(long epoch, long lease, int[] section, int[] emitted, Object chunk, Object bytes)
            throws ReflectiveOperationException {
        clear();
        if (slots == null) {
            slots = new Slot[205];
            buffer = LegacyAccess.method("LEGACY_BUFFER_BUILDER", "getBuffer");
            String[] names = {"offsetX", "offsetY", "offsetZ"};
            for (int axis = 0; axis < 3; axis++)
                offsets[axis] = Members.field(LegacyAccess.type("LEGACY_BUFFER_BUILDER"),
                        LegacyNames.member("LEGACY_BUFFER_BUILDER", names[axis]));
        }
        generation = epoch; key = lease;
        int words = 4 + emitted[3] * 28;
        if (uploaded == null || uploaded.length != words) uploaded = new int[words];
        System.arraycopy(section, 0, origin, 0, 3);
        System.arraycopy(emitted, 0, uploaded, 0, 4);
        target = LegacyAccess.method("LEGACY_RENDER_CHUNK", "getBuffer").invoke(chunk, 3);
        Slot initial = slot(bytes);
        System.arraycopy(emitted, 0, initial.output, 0, 4);
        int start = 4 + (emitted[0] + emitted[1] + emitted[2]) * 28;
        System.arraycopy(emitted, start, initial.output, 4, words - 4);
        initial.bytes = bytes;
    }

    static synchronized boolean acknowledge(long epoch, long lease) {
        if (target == null || generation != epoch || key != lease) return false;
        acknowledged = true;
        if (hasUpload) LegacyGeometryProof.resortBaked(uploaded, generation, key);
        return true;
    }

    static synchronized void sort(Object builder, float x, float y, float z) {
        if (target == null || !LegacyRenderer.accepts(generation)) return;
        try {
            Object bytes = buffer.invoke(builder);
            camera[0] = x; camera[1] = y; camera[2] = z;
            for (int axis = 0; axis < 3; axis++) {
                double translation = offsets[axis].getDouble(builder);
                if (translation != -origin[axis]) {
                    for (Slot slot : slots) if (slot != null && slot.bytes == bytes) slot.bytes = null;
                    return;
                }
                camera[axis] = (float) (camera[axis] + translation);
            }
            Slot slot = slot(bytes);
            if (slot == null) { refuse(); return; }
            System.arraycopy(uploaded, 0, slot.output, 0, 4);
            if (Hook.ravelRendererBakedResort(generation, key, camera, slot.output) != 1) {
                if (LegacyRenderer.accepts(generation)) refuse();
                else clear();
                return;
            }
            slot.bytes = bytes;
            METRICS[0]++;
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Transparency sort capture failed", failure);
        }
    }

    private static Slot slot(Object bytes) {
        for (Slot slot : slots) if (slot != null && slot.bytes == bytes) return slot;
        for (int i = 0; i < slots.length; i++) {
            if (slots[i] == null) slots[i] = new Slot();
            Slot slot = slots[i];
            if (slot.bytes != null) continue;
            if (slot.output == null || slot.output.length != uploaded.length) slot.output = new int[uploaded.length];
            return slot;
        }
        return null;
    }

    static void upload(Object destination, Object bytes) {
        if (destination != target) return;
        synchronized (LegacyBakedResort.class) { uploadCurrent(destination, bytes); }
    }

    private static void uploadCurrent(Object destination, Object bytes) {
        if (destination != target || !LegacyRenderer.accepts(generation)) return;
        for (Slot slot : slots) {
            if (slot == null || slot.bytes != bytes) continue;
            if (acknowledged) LegacyGeometryProof.resortBaked(slot.output, generation, key);
            else {
                System.arraycopy(slot.output, 4, uploaded, 4, uploaded.length - 4);
                hasUpload = true;
            }
            slot.bytes = null;
            METRICS[1]++;
            return;
        }
        if (acknowledged && uploaded[3] != 0) refuse();
    }

    private static void refuse() {
        METRICS[2]++;
        LegacyBakedCapture.invalidateResident();
        clear();
    }

    private static final class Slot {
        int[] output;
        Object bytes;
    }
}
