package dev.ravel.jvm;

import java.lang.reflect.Field;
import java.lang.reflect.Method;

final class LegacyCompositeCapture {
    private static Object chunk;
    private static Object task;
    private static int[] origin;
    private static int scene;
    private static int designated;
    private static boolean ready;
    private static boolean vanillaFallback;
    private static boolean replaying;

    private LegacyCompositeCapture() { }

    static boolean pending() { return chunk != null; }
    static int designated() { return chunk == null ? -1 : designated; }
    static boolean replaying() { return replaying; }
    static void clear() { chunk = null; task = null; origin = null; ready = false; }
    static void ready(boolean fallback) { vanillaFallback = fallback; ready = true; }

    static void offer(Object value, Object work, int row, int section, int[] sectionOrigin) {
        if (chunk != null) throw new IllegalStateException("Composited section already pending");
        origin = sectionOrigin.clone();
        chunk = value;
        task = work;
        scene = row;
        designated = section;
    }

    static byte[] capture(Runnable render, int width, int height, boolean finish) {
        if (chunk == null || !ready) return null;
        try {
            Object currentOrigin = Members.field(LegacyAccess.type("LEGACY_RENDER_CHUNK"),
                    LegacyNames.member("LEGACY_RENDER_CHUNK", "origin")).get(chunk);
            if (!sameOrigin(currentOrigin)) {
                clear();
                return null;
            }
            Object compiled = LegacyAccess.method("LEGACY_COMPILE_TASK", "getCompiledChunk").invoke(task);
            if (!vanillaFallback
                    && compiled != LegacyAccess.method("LEGACY_RENDER_CHUNK", "getCompiledChunk").invoke(chunk)) {
                if ((boolean) LegacyAccess.method("LEGACY_COMPILE_TASK", "isCancelled").invoke(task)) {
                    clear();
                    return null;
                }
                Field pending = Members.field(LegacyAccess.type("LEGACY_RENDER_CHUNK"),
                        LegacyNames.member("LEGACY_RENDER_CHUNK", "pendingTask"));
                if (pending.get(chunk) != task) clear();
                return null;
            }
            Method buffer = LegacyAccess.method("LEGACY_RENDER_CHUNK", "getBuffer");
            Field id = Members.field(LegacyAccess.type("LEGACY_VERTEX_BUFFER"),
                    LegacyNames.member("LEGACY_VERTEX_BUFFER", "id"));
            int[] ids = new int[4];
            for (int layer = 0; layer < 4; layer++) ids[layer] = id.getInt(buffer.invoke(chunk, layer));
            replaying = true;
            byte[] raw = LegacyGeometryProof.compositeFrame(render, ids, scene, designated, width, height);
            if (!vanillaFallback
                    && java.nio.ByteBuffer.wrap(raw).order(java.nio.ByteOrder.LITTLE_ENDIAN).getInt(16) == 0)
                return null;
            if (finish) {
                LegacyBakedCapture.composited(designated);
                clear();
            }
            return raw;
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Composited section VBO lookup failed", failure);
        } finally {
            replaying = false;
        }
    }

    private static boolean sameOrigin(Object position) throws ReflectiveOperationException {
        String[] coordinates = {"getX", "getY", "getZ"};
        for (int axis = 0; axis < coordinates.length; axis++)
            if ((int) LegacyAccess.method("LEGACY_VEC3I", coordinates[axis]).invoke(position) != origin[axis])
                return false;
        return true;
    }
}
