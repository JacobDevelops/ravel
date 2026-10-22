package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Arrays;

final class LegacyWorldDraw {
    private static final boolean SELECTED = System.getProperty("ravel.renderer", "vanilla").equals("resident-world");
    private static final String[] AXES = {"getX", "getY", "getZ"};
    private static Object chunk;
    private static Object task;
    private static int[] origin;
    private static long generation;
    private static long key;
    private static boolean ready;
    private static final int[] ids = new int[4];

    private LegacyWorldDraw() { }

    static boolean selected() { return SELECTED; }

    static synchronized void clear() {
        chunk = null; task = null; origin = null; ready = false;
    }

    static synchronized void offer(Object section, Object work, int[] position, long epoch, long lease) {
        if (!selected()) return;
        chunk = section; task = work; origin = position.clone();
        generation = epoch; key = lease; ready = false;
    }

    static synchronized void acknowledge(long epoch, long lease) {
        ready = chunk != null && generation == epoch && key == lease;
    }

    static synchronized boolean ready(int x, int y, int z) {
        if (!ready || origin[0] != x || origin[1] != y || origin[2] != z
                || !LegacyRenderer.accepts(generation)) return false;
        try { return current(); }
        catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Resident world admission lookup failed", failure);
        }
    }

    static synchronized void layer(Object value) {
        if (!selected()) return;
        int layer = value == null || LegacyCompositeCapture.replaying() ? -1 : ((Enum<?>) value).ordinal();
        Arrays.fill(ids, 0);
        try {
            if (layer >= 0 && ready && LegacyRenderer.accepts(generation) && current()) {
                Method buffer = LegacyAccess.method("LEGACY_RENDER_CHUNK", "getBuffer");
                Field id = Members.field(LegacyAccess.type("LEGACY_VERTEX_BUFFER"),
                        LegacyNames.member("LEGACY_VERTEX_BUFFER", "id"));
                Object source = buffer.invoke(chunk, layer);
                ids[layer] = source == null ? 0 : id.getInt(source);
            }
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Resident world section identity lookup failed", failure);
        }
        LegacyGeometryProof.worldLayer(layer, ids, generation, key);
    }

    private static boolean current() throws ReflectiveOperationException {
        Object position = Members.field(LegacyAccess.type("LEGACY_RENDER_CHUNK"),
                LegacyNames.member("LEGACY_RENDER_CHUNK", "origin")).get(chunk);
        for (int axis = 0; axis < 3; axis++)
            if ((int) LegacyAccess.method("LEGACY_VEC3I", AXES[axis]).invoke(position) != origin[axis]) return false;
        Object compiled = LegacyAccess.method("LEGACY_COMPILE_TASK", "getCompiledChunk").invoke(task);
        if (compiled != LegacyAccess.method("LEGACY_RENDER_CHUNK", "getCompiledChunk").invoke(chunk)) return false;
        Object pending = Members.field(LegacyAccess.type("LEGACY_RENDER_CHUNK"),
                LegacyNames.member("LEGACY_RENDER_CHUNK", "pendingTask")).get(chunk);
        return pending == null || pending == task;
    }
}
