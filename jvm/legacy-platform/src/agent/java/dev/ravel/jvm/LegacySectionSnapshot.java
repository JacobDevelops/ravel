package dev.ravel.jvm;

import static dev.ravel.jvm.LegacyAccess.method;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.locks.ReentrantLock;

final class LegacySectionSnapshot {
    private static final int CELLS = 18 * 18 * 18;
    private static final AtomicBoolean BUSY = new AtomicBoolean();
    private static final long[] METRICS = new long[23];
    private static final long[] TIMES = new long[4096];
    private static int samples;
    private static Access access;
    private static int[] proofStates;
    private static byte[] proofLight;
    private static long proofGeneration;

    private LegacySectionSnapshot() { }

    static void capture(Object chunk, Object task, long generation, boolean retainProof)
            throws ReflectiveOperationException {
        if (!BUSY.compareAndSet(false, true)) { increment(8); return; }
        long started = System.nanoTime();
        try {
            if (!Hook.ravelRendererAccept(generation)) { increment(7); return; }
            if (access == null) access = new Access();
            Access a = access;
            ReentrantLock lock = (ReentrantLock) a.lock.get(chunk);
            if (!lock.tryLock()) { increment(8); return; }
            Object region;
            int x, y, z;
            try {
                if (a.pending.get(chunk) != task || (boolean) a.cancelled.invoke(task)
                        || a.compiled.invoke(task) == null) { increment(8); return; }
                region = a.region.get(chunk);
                if (region == null) { increment(8); return; }
                Object origin = a.origin.get(chunk);
                x = (int) a.x.invoke(origin);
                y = (int) a.y.invoke(origin);
                z = (int) a.z.invoke(origin);
            } finally { lock.unlock(); }
            int[] states = new int[CELLS];
            byte[] light = new byte[CELLS];
            long nonAir = fill(a, region, x, y, z, states, light);
            long[] candidate = retainProof && !proofReady() ? oracle(a, region, x, y, z, states, light) : null;
            long[] receipt = Hook.ravelRendererSnapshot(generation, states, light);
            if (receipt.length == 0) { increment(7); return; }
            if (receipt.length != 6 || receipt[0] != 1 || receipt[2] != 0 || receipt[3] != CELLS)
                throw new IllegalStateException("Native snapshot did not round-trip every copied region cell");
            record(receipt, System.nanoTime() - started, generation, nonAir, states, light);
            if (candidate != null) synchronized (LegacySectionSnapshot.class) {
                if (proofStates == null) {
                    proofStates = states;
                    proofLight = light;
                    proofGeneration = generation;
                    METRICS[18] = candidate[0];
                    METRICS[19] = candidate[1];
                    METRICS[20] = candidate[2];
                }
            }
        } catch (RuntimeException | Error failure) {
            throw failure;
        } catch (Throwable failure) {
            if (failure instanceof ReflectiveOperationException reflection) throw reflection;
            throw new ReflectiveOperationException("Snapshot game access failed", failure);
        } finally { BUSY.set(false); }
    }

    private static long fill(Access a, Object region, int ox, int oy, int oz, int[] states, byte[] light)
            throws Throwable {
        Object pos = (Object) a.position.invokeExact(0, 0, 0);
        long nonAir = 0;
        for (int y = 0; y < 18; y++) for (int z = 0; z < 18; z++) for (int x = 0; x < 18; x++) {
            a.set.invokeExact(pos, ox + x - 1, oy + y - 1, oz + z - 1);
            Object state = (Object) a.state.invokeExact(region, pos);
            Object block = (Object) a.block.invokeExact(state);
            int id = (int) a.id.invokeExact(block), meta = (int) a.meta.invokeExact(block, state);
            int sky = (int) a.light.invokeExact(region, a.sky, pos);
            int emitted = (int) a.light.invokeExact(region, a.emitted, pos);
            if (id < 0 || id > 1048575 || meta < 0 || meta > 15 || sky < 0 || sky > 15
                    || emitted < 0 || emitted > 15) throw new IllegalStateException("Vanilla snapshot value out of bounds");
            int index = ((y * 18) + z) * 18 + x;
            states[index] = (id << 4) | meta;
            light[index] = (byte) ((sky << 4) | emitted);
            if (id != 0) nonAir++;
        }
        return nonAir;
    }

    // Coordinate oracle only: same exact mappings, independently constructed origin-offset positions.
    private static long[] oracle(Access a, Object region, int ox, int oy, int oz,
            int[] states, byte[] light) throws Throwable {
        int[] offsets = {-1, 0, 7, 15, 16};
        long checks = 0, mismatches = 0, border = 0;
        for (int dx : offsets) for (int dy : offsets) for (int dz : offsets) {
            Object pos = (Object) a.position.invokeExact(ox + dx, oy + dy, oz + dz);
            Object state = (Object) a.state.invokeExact(region, pos);
            Object block = (Object) a.block.invokeExact(state);
            int key = ((int) a.id.invokeExact(block) << 4) | (int) a.meta.invokeExact(block, state);
            int sky = (int) a.light.invokeExact(region, a.sky, pos);
            int emitted = (int) a.light.invokeExact(region, a.emitted, pos);
            int index = (dy + 1) * 324 + (dz + 1) * 18 + dx + 1;
            checks++;
            if (states[index] != key || Byte.toUnsignedInt(light[index]) != (sky * 16 + emitted)) mismatches++;
            if (dx == -1 || dx == 16 || dy == -1 || dy == 16 || dz == -1 || dz == 16) border++;
        }
        synchronized (LegacySectionSnapshot.class) {
            if (mismatches != 0) {
                METRICS[21] += mismatches;
                METRICS[22]++;
                return null; // Mutable chunk/light data may change between copy and reread.
            }
        }
        return new long[] {checks, mismatches, border};
    }

    private static synchronized void increment(int index) { METRICS[index]++; }

    private static synchronized void record(long[] receipt, long nanos, long generation,
            long nonAir, int[] states, byte[] light) {
        METRICS[0]++;
        METRICS[1] += receipt[1];
        METRICS[2] = Math.max(METRICS[2], receipt[1]);
        METRICS[3] += nanos;
        METRICS[4] = Math.max(METRICS[4], nanos);
        METRICS[5] += receipt[2];
        METRICS[6] += receipt[3];
        METRICS[9] += 1736;
        METRICS[10] = Thread.currentThread().threadId();
        METRICS[11] = generation;
        METRICS[12] += nonAir;
        for (int i = 1; i < CELLS; i++) if (states[i] != states[i - 1] || light[i] != light[i - 1]) METRICS[13]++;
        METRICS[14] = Math.max(METRICS[14], receipt[4]);
        METRICS[15] = Math.max(METRICS[15], receipt[5]);
        if (Thread.currentThread().threadId() != LegacyRenderer.ownerThread()) METRICS[17]++;
        if (samples < TIMES.length) TIMES[samples++] = nanos;
    }

    static synchronized boolean proofReady() { return proofStates != null; }

    static synchronized boolean rejectStaleProof() {
        if (proofStates == null) return false;
        if (Hook.ravelRendererAccept(proofGeneration))
            throw new IllegalStateException("Retired captured batch generation still accepted: " + proofGeneration);
        if (!BUSY.compareAndSet(false, true)) return false;
        try {
            long[] receipt = Hook.ravelRendererSnapshot(proofGeneration, proofStates, proofLight);
            if (receipt.length != 0)
                throw new IllegalStateException("Native accepted actual retired captured batch: generation="
                        + proofGeneration + ", receipt=" + Arrays.toString(receipt));
            METRICS[16]++;
            proofStates = null;
            proofLight = null;
            return true;
        } finally {
            BUSY.set(false);
        }
    }

    static synchronized long[] metrics() { return METRICS.clone(); }

    static synchronized long percentile(int percent) {
        if (samples == 0) return 0;
        long[] sorted = Arrays.copyOf(TIMES, samples);
        Arrays.sort(sorted);
        return sorted[(samples * percent + 99) / 100 - 1];
    }


    private static Field field(String role, String name) throws ReflectiveOperationException {
        return Members.field(LegacyAccess.type(role), LegacyNames.member(role, name));
    }

    private static final class Access {
        final Field lock = field("LEGACY_RENDER_CHUNK", "taskLock");
        final Field pending = field("LEGACY_RENDER_CHUNK", "pendingTask");
        final Field region = field("LEGACY_RENDER_CHUNK", "region");
        final Field origin = field("LEGACY_RENDER_CHUNK", "origin");
        final Method cancelled = method("LEGACY_COMPILE_TASK", "isCancelled");
        final Method compiled = method("LEGACY_COMPILE_TASK", "getCompiledChunk");
        final Method x = method("LEGACY_VEC3I", "getX");
        final Method y = method("LEGACY_VEC3I", "getY");
        final Method z = method("LEGACY_VEC3I", "getZ");
        final MethodHandle position = MethodHandles.lookup().unreflectConstructor(
                Members.constructor(LegacyAccess.type("LEGACY_MUTABLE_BLOCK_POS"),
                        LegacyNames.member("LEGACY_MUTABLE_BLOCK_POS", "<init>")))
                .asType(MethodType.methodType(Object.class, int.class, int.class, int.class));
        final MethodHandle set = handle("LEGACY_MUTABLE_BLOCK_POS", "set", void.class,
                Object.class, int.class, int.class, int.class);
        final MethodHandle state = handle("LEGACY_WORLD_VIEW", "getBlockState", Object.class, Object.class, Object.class);
        final MethodHandle block = handle("LEGACY_BLOCK_STATE", "getBlock", Object.class, Object.class);
        final MethodHandle id = handle("LEGACY_BLOCK", "getId", int.class, Object.class);
        final MethodHandle meta = handle("LEGACY_BLOCK", "getMetadataFromState", int.class, Object.class, Object.class);
        final MethodHandle light = handle("LEGACY_WORLD_REGION", "getLight", int.class,
                Object.class, Object.class, Object.class);
        final Object sky = LegacyAccess.field("LEGACY_LIGHT_TYPE", "SKY", null);
        final Object emitted = LegacyAccess.field("LEGACY_LIGHT_TYPE", "BLOCK", null);

        Access() throws ReflectiveOperationException { }

        private static MethodHandle handle(String role, String name, Class<?> result, Class<?>... parameters)
                throws ReflectiveOperationException {
            return MethodHandles.lookup().unreflect(method(role, name)).asType(MethodType.methodType(result, parameters));
        }
    }
}
