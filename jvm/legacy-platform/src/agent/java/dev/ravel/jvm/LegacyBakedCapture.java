package dev.ravel.jvm;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import java.util.IdentityHashMap;

/** Selected-only owning-thread source capture; final vanilla buffers are an independent oracle. */
final class LegacyBakedCapture {
    private static final int LIMIT = LegacyRendererContract.BAKED_MAX_QUADS;
    private static final int RECORD = LegacyRendererContract.BAKED_RECORD_BYTES;
    static final int SCENES = 8;
    static final int DESIGNATED = 8;
    static final int SAMPLES = 8;
    static final int READBACK_BYTES = 16 + 64 * 64 * 16;
    private static final int QUAD_BUDGET = 20;
    private static final int UNCOLORED = 21;
    private static final int FOREIGN_BUILDER = 22;
    private static final int OBSERVATION = 23;
    private static final int INCOMPLETE_QUAD = 24;
    private static final int UNACCOUNTED_VERTICES = 25;
    private static final int RETIRED_AT_FINISH = 26;
    private static final int RUST_DECODE = 27;
    private static final int RUST_OTHER = 28;
    private static final long[] METRICS = new long[30];
    private static volatile Capture queued;
    static { METRICS[10] = -1; }
    private static final long[][] GOLDENS = new long[SCENES][7];
    private static final long[] WORLD = new long[7];
    private static final byte[][][] RAW = new byte[SCENES][SAMPLES][];
    private static final int[] RETAINED = new int[SCENES];
    private static final int[][] ORIGINS = new int[DESIGNATED][];
    private static final int[] COVERAGE = new int[3];
    private static final int[] RETAINED_COVERAGE = new int[3];
    private static int phase;
    private static long designatedResource;
    private static long phaseRenderer;
    private static Capture storage;
    private static volatile Capture active;
    private static volatile boolean resident;

    private LegacyBakedCapture() { }

    static void sort(Object builder, float x, float y, float z) {
        if (LegacyGeometryProof.selected()) LegacyBakedResort.sort(builder, x, y, z);
    }

    static void upload(Object buffer, Object bytes) {
        if (LegacyGeometryProof.selected()) LegacyBakedResort.upload(buffer, bytes);
    }
    static void invalidateResident() { resident = false; }

    static synchronized void begin(Object task, float x, float y, float z) {
        if (!LegacyGeometryProof.selected()) return;
        if (LegacyRenderer.taskGeneration(task) <= 0) return;
        if (active != null || queued != null) { METRICS[1]++; return; }
        try {
            if (LegacyCompositeCapture.pending() && !LegacyCompositeCapture.replacePending(task)) {
                METRICS[1]++;
                return;
            }
            if (storage == null) storage = new Capture();
            storage.begin(task, x, y, z);
        } catch (Throwable failure) { throw new IllegalStateException("Baked capture initialization failed", failure); }
        if (controlledWindow() && !storage.needed()) {
            storage.release();
            METRICS[29]++;
            return;
        }
        active = storage;
    }

    static synchronized void clear() {
        resident = false;
        LegacyWorldDraw.clear();
        LegacyBakedResort.clear();
        active = null;
        queued = null;
        LegacyCompositeCapture.clear();
        if (storage != null) storage.sprites.clear();
        if (storage != null) storage.release();
    }

    static synchronized long[] metrics() {
        long[] result = Arrays.copyOf(METRICS, 33);
        System.arraycopy(LegacyBakedResort.metrics(), 0, result, 30, 3);
        return result;
    }
    static synchronized long[] world() { return WORLD.clone(); }
    static synchronized void composited(int designated) { COVERAGE[phase] |= 1 << designated; }
    static synchronized byte[] composite(Runnable render, int width, int height, int expected, boolean finish) {
        if (expected != LegacyCompositeCapture.designated()) return null;
        if (expected < 5 && !resident) return null;
        return LegacyCompositeCapture.capture(render, width, height, finish);
    }
    static synchronized int compositeDesignated() { return LegacyCompositeCapture.designated(); }
    static synchronized long[][] goldens() {
        return Arrays.stream(GOLDENS).map(long[]::clone).toArray(long[][]::new);
    }

    static synchronized byte[][] readbacks(int scene) {
        return Arrays.stream(RAW[scene], 0, RETAINED[scene]).map(byte[]::clone).toArray(byte[][]::new);
    }

    static synchronized void designate(int scene, int x, int y, int z) {
        if (phase != 0 || scene < 0 || scene >= DESIGNATED || ((x | y | z) & 15) != 0)
            throw new IllegalArgumentException("Golden sections are designated once, section-aligned, before reload");
        ORIGINS[scene] = new int[]{x, y, z};
        designatedResource = LegacyRuntime.completedResourceGeneration();
    }

    static synchronized void phase(int next) {
        if (next != phase + 1 || next > 2) throw new IllegalStateException("Golden phases advance in order");
        phase = next;
        phaseRenderer = LegacyRenderer.currentGeneration();
    }

    static synchronized int coverage(int target) { return COVERAGE[target]; }

    // While a designated phase is incomplete, only designated sections wait for the next GL drain.
    private static boolean controlledWindow() {
        return ORIGINS[0] != null && COVERAGE[phase] != (1 << DESIGNATED) - 1;
    }

    static void draw() {
        if (!LegacyGeometryProof.selected() || LegacyCompositeCapture.replaying()) return;
        if (queued != null) drawQueued();
        else if (resident && System.getProperty("ravel.renderer", "vanilla").equals("geometry-proof")
                && LegacyRuntime.resourceGeneration() == LegacyRuntime.completedResourceGeneration())
            LegacyGeometryProof.drawBaked(LegacyRuntime.completedResourceGeneration(),
                    LegacyRuntime.atlasResourceGeneration());
    }

    private static synchronized void drawQueued() {
        Capture capture = queued;
        if (capture == null) return;
        if (!LegacyRenderer.accepts(capture.generation) || !capture.resourceCurrent()) {
            abandon(capture);
            queued = null;
            LegacyCompositeCapture.clear();
            METRICS[14]++;
            return;
        }
        int designated = capture.designated();
        int scene = designated < 0 ? -1 : capture.scene(designated);
        boolean retain = scene >= 0 && RETAINED[scene] < SAMPLES
                && designated < 5 && (RETAINED_COVERAGE[phase] & 1 << designated) == 0;
        long[] identity = {capture.generation, capture.key, capture.source.getLong(16),
                capture.source.getLong(24), capture.origin[0], capture.origin[1], capture.origin[2],
                LegacyRenderer.ownerThread()};
        long[] result;
        try {
            result = LegacyGeometryProof.compareBaked(capture.output, capture.oracle.array(), identity, retain);
        } catch (RuntimeException failure) {
            abandon(capture);
            queued = null;
            resident = false;
            LegacyCompositeCapture.clear();
            throw failure;
        }
        if (result.length != 32) {
            abandon(capture);
            queued = null;
            LegacyCompositeCapture.clear();
            METRICS[15]++;
            return;
        }
        synchronized (LegacyBakedResort.class) {
            if (!LegacyBakedResort.acknowledge(capture.generation, capture.key)) {
                abandon(capture);
                queued = null;
                LegacyCompositeCapture.clear();
                METRICS[15]++;
                return;
            }
            capture.settle(true);
            boolean exact = true;
            for (int layer = 0; layer < 3; layer++)
                for (int mismatch = 1; mismatch <= 3; mismatch++) exact &= result[layer * 7 + mismatch] == 0;
            if (exact) LegacyWorldDraw.acknowledge(capture.generation, capture.key);
            resident = true;
        }
        queued = null;
        METRICS[13]++;
        METRICS[16] += result[28]; METRICS[17] = Math.max(METRICS[17], result[29]);
        METRICS[18] += result[30]; METRICS[19] += result[31];
        byte[][] raw = retain ? LegacyGeometryProof.bakedReadbacks() : null;
        for (int layer = 0; layer < 4; layer++) {
            if (result[layer * 7] == 0) continue;
            add(WORLD, result, layer * 7);
            if (raw != null && raw[layer] != null && RETAINED[scene] < SAMPLES)
                retain(scene, designated, layer, result, raw[layer]);
        }
        if (LegacyCompositeCapture.pending()) LegacyCompositeCapture.ready(false);
    }

    private static void retain(int scene, int designated, int layer, long[] result, byte[] raw) {
        ByteBuffer sample = ByteBuffer.allocate(READBACK_BYTES).order(ByteOrder.LITTLE_ENDIAN);
        sample.putInt(scene).putInt(layer).putInt((int) result[layer * 7 + 6]).putInt(designated).put(raw);
        RAW[scene][RETAINED[scene]++] = sample.array();
        add(GOLDENS[scene], result, layer * 7);
        RETAINED_COVERAGE[phase] |= 1 << designated;
    }

    private static void add(long[] target, long[] result, int base) {
        for (int index = 0; index < 7; index++) target[index] += result[base + index];
    }

    private static void abandon(Capture capture) {
        synchronized (LegacyBakedResort.class) {
            capture.settle(false);
            LegacyBakedResort.clear();
            resident = false;
        }
    }

    private static void refuse(int reason) {
        METRICS[2]++;
        METRICS[reason]++;
    }

    private static Capture current() {
        Capture capture = active;
        return capture != null && capture.owner == Thread.currentThread().threadId() ? capture : null;
    }

    static void model(Object state, Object builder) {
        if (current() != null) modelCurrent(state, builder);
    }

    private static synchronized void modelCurrent(Object state, Object builder) {
        Capture capture = current();
        if (capture == null) return;
        try {
            Object block = capture.access.block.invokeExact(state);
            int id = (int) capture.access.id.invokeExact(block);
            int meta = (int) capture.access.meta.invokeExact(block, state);
            capture.state = (id << 4) | meta;
            capture.builder = builder;
            capture.pending = false;
            Arrays.fill(capture.ao, 1.0f);
        } catch (Throwable failure) { capture.fail(OBSERVATION); }
    }

    static void observe(String id, Object receiver, Object result, int number) {
        if (current() != null) observeCurrent(id, receiver, number);
    }

    private static synchronized void observeCurrent(String id, Object receiver, int number) {
        Capture capture = current();
        if (capture == null) return;
        try {
            switch (id) {
                case "legacyBakedQuad" -> {
                    Object sprite = capture.access.sprite.invokeExact(receiver);
                    Integer identity = capture.sprites.get(sprite);
                    if (identity == null) {
                        if (capture.sprites.size() == LIMIT) { capture.fail(OBSERVATION); return; }
                        identity = capture.sprites.size();
                        capture.sprites.put(sprite, identity);
                    }
                    capture.sprite = identity;
                    if ((boolean) capture.access.animated.invokeExact(sprite)) capture.animated++;
                }
                case "legacyBakedAo" -> {
                    float[] values = (float[]) capture.access.brightness.get(receiver);
                    System.arraycopy(values, 0, capture.ao, 0, 4);
                }
                case "legacyBakedTint" -> capture.tint = number;
                default -> { }
            }
        } catch (Throwable failure) { capture.fail(OBSERVATION); }
    }

    static void vertices(Object builder, int[] words) {
        if (current() != null) verticesCurrent(builder, words);
    }

    private static synchronized void verticesCurrent(Object builder, int[] words) {
        Capture capture = current();
        if (capture == null || capture.builder != builder) return;
        if (words.length != 28 || capture.pending) { capture.fail(OBSERVATION); return; }
        System.arraycopy(words, 0, capture.words, 0, 28);
        for (int i = 0; i < 4; i++) capture.light[i] = words[i * 7 + 6];
        Arrays.fill(capture.rgb, 1.0f);
        capture.tint = -1;
        capture.pending = true;
    }

    static void light(Object builder, int a, int b, int c, int d) {
        if (current() != null) lightCurrent(builder, a, b, c, d);
    }

    private static synchronized void lightCurrent(Object builder, int a, int b, int c, int d) {
        Capture capture = current();
        if (capture == null || capture.builder != builder || !capture.pending) return;
        capture.light[0] = a; capture.light[1] = b; capture.light[2] = c; capture.light[3] = d;
    }

    static void color(Object builder, float r, float g, float b, int index) {
        if (current() != null) colorCurrent(builder, r, g, b, index);
    }

    private static synchronized void colorCurrent(Object builder, float r, float g, float b, int index) {
        Capture capture = current();
        if (capture == null || capture.builder != builder || !capture.pending) return;
        if (index < 1 || index > 4) { capture.fail(OBSERVATION); return; }
        int offset = (4 - index) * 3;
        capture.rgb[offset] = r; capture.rgb[offset + 1] = g; capture.rgb[offset + 2] = b;
    }

    static void position(Object builder, double x, double y, double z) {
        if (current() != null) positionCurrent(builder, x, y, z);
    }

    private static synchronized void positionCurrent(Object builder, double x, double y, double z) {
        Capture capture = current();
        if (capture == null || capture.builder != builder || !capture.pending) return;
        capture.pending = false;
        try { capture.append(builder, x, y, z); }
        catch (Throwable failure) { capture.fail(OBSERVATION); }
    }

    static synchronized void finish(Object task) {
        Capture capture = current();
        if (capture == null || capture.task != task) return;
        active = null;
        try {
            int reason = capture.refusal != 0 ? capture.refusal
                    : capture.pending ? INCOMPLETE_QUAD
                    : !LegacyRenderer.accepts(capture.generation) ? RETIRED_AT_FINISH : 0;
            if (reason != 0) refuse(reason);
            else if (!capture.fluidFallback()) {
                synchronized (LegacyBakedResort.class) { capture.compare(); }
            }
        } catch (Throwable failure) {
            throw new IllegalStateException("Baked comparison failed", failure);
        } finally { capture.release(); }
    }

    private static final class Capture {
        final Access access = new Access();
        final IdentityHashMap<Object, Integer> sprites = new IdentityHashMap<>();
        final float[] camera = new float[3];
        final ByteBuffer source = ByteBuffer.allocate(32 + LIMIT * RECORD).order(ByteOrder.LITTLE_ENDIAN);
        final ByteBuffer oracle = ByteBuffer.allocate(16 + LIMIT * 112).order(ByteOrder.LITTLE_ENDIAN);
        final int[] output = new int[4 + LIMIT * 28];
        final Object[] builders = new Object[4];
        final int[] words = new int[28];
        final int[] fluids = new int[4];
        final int[] light = new int[4];
        final float[] ao = new float[4];
        final float[] rgb = new float[12];
        final int[] origin = new int[3];
        final long[] section = new long[3];
        long key;
        Object task;
        Object builder;
        long generation;
        long owner;
        int state;
        int sprite;
        int tint;
        int count;
        int animated;
        int refusal;
        boolean pending;

        Capture() throws ReflectiveOperationException { }

        void fail(int reason) {
            if (refusal == 0) refusal = reason;
        }

        void begin(Object task, float x, float y, float z) throws Throwable {
            release();
            this.task = task;
            generation = LegacyRenderer.taskGeneration(task);
            owner = Thread.currentThread().threadId();
            Object buffers = access.buffers.invokeExact(task);
            for (int i = 0; i < 4; i++) builders[i] = access.get.invokeExact(buffers, i);
            camera[0] = x; camera[1] = y; camera[2] = z;
            Object position = access.origin.get((Object) access.chunk.invokeExact(task));
            origin[0] = (int) access.originX.invokeExact(position);
            origin[1] = (int) access.originY.invokeExact(position);
            origin[2] = (int) access.originZ.invokeExact(position);
            for (int axis = 0; axis < 3; axis++) section[axis] = origin[axis];
            source.clear().position(32);
            source.putLong(16, LegacyRuntime.completedResourceGeneration());
            source.putLong(24, LegacyRuntime.atlasResourceGeneration());
            count = 0; animated = 0; refusal = 0;
        }

        void append(Object builder, double x, double y, double z) throws Throwable {
            if (count == LIMIT) { fail(QUAD_BUDGET); return; }
            if (access.uncolored.getBoolean(builder)) { fail(UNCOLORED); return; }
            int layer = -1;
            for (int i = 0; i < 4; i++) if (builders[i] == builder) layer = i;
            if (layer < 0) { fail(FOREIGN_BUILDER); return; }
            double offsetX = access.x.getDouble(builder);
            double offsetY = access.y.getDouble(builder);
            double offsetZ = access.z.getDouble(builder);
            source.putInt(state).putInt(sprite).putInt(layer).putInt(tint);
            source.putDouble(x + offsetX).putDouble(y + offsetY).putDouble(z + offsetZ);
            for (int word : words) source.putInt(word);
            for (float value : ao) source.putFloat(value);
            for (float value : rgb) source.putFloat(value);
            for (int value : light) source.putInt(value);
            count++;
        }

        int designated() {
            for (int index = 0; index < DESIGNATED; index++)
                if (ORIGINS[index] != null && Arrays.equals(ORIGINS[index], origin)) return index;
            return -1;
        }

        boolean needed() {
            int designated = designated();
            return designated >= 0 && (COVERAGE[phase] & 1 << designated) == 0;
        }

        int scene(int designated) {
            long resource = source.getLong(16);
            return switch (phase) {
                case 0 -> resource != designatedResource || designated == 4 && animated == 0 ? -1 : designated;
                case 1 -> resource > designatedResource && generation >= phaseRenderer ? 5 : -1;
                default -> generation >= phaseRenderer ? 6 : -1;
            };
        }

        boolean resourceCurrent() {
            return source.getLong(16) > 0
                    && source.getLong(16) == LegacyRuntime.completedResourceGeneration()
                    && source.getLong(16) == LegacyRuntime.resourceGeneration()
                    && source.getLong(24) == LegacyRuntime.atlasResourceGeneration();
        }

        boolean fluidFallback() throws Throwable {
            Object chunk = access.chunk.invokeExact(task);
            LegacySectionSnapshot.fluids(chunk, fluids);
            if (fluids[0] == 0) return false;
            refuse(UNACCOUNTED_VERTICES);
            int designated = designated();
            if (designated == 6 && fluids[2] == 0) return true;
            if (needed() && resourceCurrent() && LegacyRenderer.accepts(generation)) {
                LegacyCompositeCapture.offer(chunk, task, 7, designated, origin);
                LegacyCompositeCapture.fluids(fluids);
                LegacyCompositeCapture.ready(true);
            }
            return true;
        }

        void compare() throws Throwable {
            if (designated() >= 5) { refuse(UNACCOUNTED_VERTICES); return; }
            Object translucent = builders[3];
            source.putFloat(4, (float) (camera[0] + access.x.getDouble(translucent)));
            source.putFloat(8, (float) (camera[1] + access.y.getDouble(translucent)));
            source.putFloat(12, (float) (camera[2] + access.z.getDouble(translucent)));
            oracle.clear().position(16);
            int vertices = 0;
            for (int i = 0; i < 4; i++) {
                int length = (int) access.count.invokeExact(builders[i]);
                if (length < 0 || length % 4 != 0) { refuse(UNACCOUNTED_VERTICES); return; }
                if (length > (LIMIT * 4) - vertices) { refuse(QUAD_BUDGET); return; }
                vertices += length;
                oracle.putInt(i * 4, length / 4);
                ByteBuffer buffer = (ByteBuffer) access.buffer.invokeExact(builders[i]);
                ByteBuffer view = buffer.duplicate();
                view.clear().limit(length * 28);
                oracle.put(view);
            }
            if (vertices != count * 4) { refuse(UNACCOUNTED_VERTICES); return; }
            if (!resourceCurrent()) { METRICS[3]++; return; }
            source.putInt(0, count);
            long[] result = Hook.ravelRendererBaked(generation, section, source.array(), oracle.array(), output);
            if (result.length == 7) key = result[6];
            if (!resourceCurrent() || !LegacyRenderer.accepts(generation)) {
                if (result.length == 7) settle(false);
                METRICS[3]++;
                return;
            }
            if (result.length == 0) { METRICS[3]++; return; }
            if (result.length != 7 || result[0] < 0) {
                refuse(result.length == 6
                        && LegacyRendererContract.BakedFailure.decode(result[1]) == LegacyRendererContract.BakedFailure.DECODE
                        ? RUST_DECODE : RUST_OTHER);
                return;
            }
            publish(result);
        }

        private void publish(long[] result) throws Throwable {
            METRICS[0]++;
            METRICS[4] += result[0]; METRICS[5] += result[1]; METRICS[6] += result[3];
            if (result[2] >= 0 && METRICS[10] < 0) METRICS[10] = result[2];
            if (count > 0 && (!controlledWindow() || needed())) {
                queued = this;
                Object chunk = access.chunk.invokeExact(task);
                LegacyWorldDraw.offer(chunk, task, origin, generation, key);
                LegacyBakedResort.offer(generation, key, origin, output, chunk,
                        (ByteBuffer) access.buffer.invokeExact(builders[3]));
                resident = false;
                int designated = designated();
                int scene = scene(designated);
                if (scene >= 0 && needed())
                    LegacyCompositeCapture.offer(chunk, task, scene, designated, origin);
            } else settle(false);
            METRICS[7] = Math.max(METRICS[7], result[4]);
            METRICS[8] = Math.max(METRICS[8], result[5]); METRICS[9] += animated;
            METRICS[11] = source.getLong(16);
            METRICS[12] = source.getLong(24);
        }

        void settle(boolean accepted) {
            long[] result = Hook.ravelRendererBakedLease(generation, key, accepted ? 1 : 2);
            if (LegacyRenderer.accepts(generation) && result.length != 12)
                throw new IllegalStateException("Shaded native lease lost before exact acknowledgement");
        }

        void release() {
            task = null; builder = null; owner = 0; pending = false;
            Arrays.fill(builders, null);
        }
    }

    private static final class Access {
        final MethodHandle buffers = handle("LEGACY_COMPILE_TASK", "getBuffers", Object.class, Object.class);
        final MethodHandle get = handle("LEGACY_CHUNK_BUFFERS", "get", Object.class, Object.class, int.class);
        final MethodHandle buffer = handle("LEGACY_BUFFER_BUILDER", "getBuffer", ByteBuffer.class, Object.class);
        final MethodHandle count = handle("LEGACY_BUFFER_BUILDER", "getVertexCount", int.class, Object.class);
        final MethodHandle block = handle("LEGACY_BLOCK_STATE", "getBlock", Object.class, Object.class);
        final MethodHandle id = handle("LEGACY_BLOCK", "getId", int.class, Object.class);
        final MethodHandle meta = handle("LEGACY_BLOCK", "getMetadataFromState", int.class, Object.class, Object.class);
        final MethodHandle sprite = handle("LEGACY_BAKED_QUAD", "getSprite", Object.class, Object.class);
        final MethodHandle animated = handle("LEGACY_ATLAS_SPRITE", "isAnimated", boolean.class, Object.class);
        final Field x = field("LEGACY_BUFFER_BUILDER", "offsetX");
        final Field y = field("LEGACY_BUFFER_BUILDER", "offsetY");
        final Field z = field("LEGACY_BUFFER_BUILDER", "offsetZ");
        final Field uncolored = field("LEGACY_BUFFER_BUILDER", "uncolored");
        final Field brightness = field("LEGACY_BAKED_AO", "brightness");
        final MethodHandle chunk = handle("LEGACY_COMPILE_TASK", "getChunk", Object.class, Object.class);
        final Field origin = field("LEGACY_RENDER_CHUNK", "origin");
        final MethodHandle originX = handle("LEGACY_VEC3I", "getX", int.class, Object.class);
        final MethodHandle originY = handle("LEGACY_VEC3I", "getY", int.class, Object.class);
        final MethodHandle originZ = handle("LEGACY_VEC3I", "getZ", int.class, Object.class);

        Access() throws ReflectiveOperationException { }

        private static Field field(String role, String member) throws ReflectiveOperationException {
            return Members.field(LegacyAccess.type(role), LegacyNames.member(role, member));
        }

        private static MethodHandle handle(String role, String member, Class<?> result, Class<?>... parameters) {
            try {
                return MethodHandles.lookup().unreflect(LegacyAccess.method(role, member))
                        .asType(MethodType.methodType(result, parameters));
            } catch (ReflectiveOperationException failure) { throw new IllegalStateException("Baked mapping resolution failed", failure); }
        }
    }
}
