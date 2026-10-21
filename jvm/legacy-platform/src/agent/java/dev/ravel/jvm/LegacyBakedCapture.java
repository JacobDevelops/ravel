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
    private static final int LIMIT = 8192;
    private static final int RECORD = 232;
    private static final long[] METRICS = new long[20];
    private static volatile Capture queued;
    static { METRICS[10] = -1; }
    private static final long[][] GOLDENS = new long[7][7];
    private static long firstResource;
    private static Capture storage;
    private static volatile Capture active;

    private LegacyBakedCapture() { }

    static synchronized void begin(Object task, float x, float y, float z) {
        if (!LegacyGeometryProof.selected()) return;
        if (LegacyRenderer.taskGeneration(task) <= 0) return;
        if (active != null || queued != null) { METRICS[1]++; return; }
        try {
            if (storage == null) storage = new Capture();
            storage.begin(task, x, y, z);
            active = storage;
        } catch (Throwable failure) { throw new IllegalStateException("Baked capture initialization failed", failure); }
    }

    static synchronized void clear() {
        active = null;
        queued = null;
        if (storage != null) storage.sprites.clear();
        if (storage != null) storage.release();
    }

    static synchronized long[] metrics() { return METRICS.clone(); }
    static synchronized long[][] goldens() {
        return Arrays.stream(GOLDENS).map(long[]::clone).toArray(long[][]::new);
    }

    static void draw() {
        if (queued != null) drawQueued();
    }

    private static synchronized void drawQueued() {
        Capture capture = queued;
        if (capture == null) return;
        queued = null;
        if (!LegacyRenderer.accepts(capture.generation) || !capture.resourceCurrent()) {
            METRICS[14]++;
            return;
        }
        long[] result = LegacyGeometryProof.compareBaked(capture.output, capture.oracle.array());
        if (result.length != 32) { METRICS[15]++; return; }
        METRICS[13]++;
        METRICS[16] += result[28]; METRICS[17] = Math.max(METRICS[17], result[29]);
        METRICS[18] += result[30]; METRICS[19] += result[31];
        if (firstResource == 0) firstResource = capture.source.getLong(16);
        for (int layer = 0; layer < 4; layer++) {
            addGolden(layer, result, layer * 7);
            if (capture.animated > 0) addGolden(4, result, layer * 7);
            if (capture.source.getLong(16) > firstResource) addGolden(5, result, layer * 7);
        }
    }

    private static void addGolden(int scene, long[] result, int base) {
        for (int index = 0; index < 7; index++) GOLDENS[scene][index] += result[base + index];
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
        } catch (Throwable failure) { capture.failed = true; }
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
                        if (capture.sprites.size() == LIMIT) { capture.failed = true; return; }
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
        } catch (Throwable failure) { capture.failed = true; }
    }

    static void vertices(Object builder, int[] words) {
        if (current() != null) verticesCurrent(builder, words);
    }

    private static synchronized void verticesCurrent(Object builder, int[] words) {
        Capture capture = current();
        if (capture == null || capture.builder != builder) return;
        if (words.length != 28 || capture.pending) { capture.failed = true; return; }
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
        if (index < 1 || index > 4) { capture.failed = true; return; }
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
        catch (Throwable failure) { capture.failed = true; }
    }

    static synchronized void finish(Object task) {
        Capture capture = current();
        if (capture == null || capture.task != task) return;
        active = null;
        try {
            if (capture.failed || capture.pending || !LegacyRenderer.accepts(capture.generation)) {
                METRICS[2]++; return;
            }
            capture.compare();
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
        final int[] light = new int[4];
        final float[] ao = new float[4];
        final float[] rgb = new float[12];
        Object task;
        Object builder;
        long generation;
        long owner;
        int state;
        int sprite;
        int tint;
        int count;
        int animated;
        boolean pending;
        boolean failed;

        Capture() throws ReflectiveOperationException { }

        void begin(Object task, float x, float y, float z) throws Throwable {
            release();
            this.task = task;
            generation = LegacyRenderer.taskGeneration(task);
            owner = Thread.currentThread().threadId();
            Object buffers = access.buffers.invokeExact(task);
            for (int i = 0; i < 4; i++) builders[i] = access.get.invokeExact(buffers, i);
            camera[0] = x; camera[1] = y; camera[2] = z;
            source.clear().position(32);
            source.putLong(16, LegacyRuntime.completedResourceGeneration());
            source.putLong(24, LegacyRuntime.atlasResourceGeneration());
            count = 0; animated = 0; failed = false;
        }

        void append(Object builder, double x, double y, double z) throws Throwable {
            if (count == LIMIT || access.uncolored.getBoolean(builder)) { failed = true; return; }
            int layer = -1;
            for (int i = 0; i < 4; i++) if (builders[i] == builder) layer = i;
            if (layer < 0) { failed = true; return; }
            source.putInt(state).putInt(sprite).putInt(layer).putInt(tint);
            source.putDouble(x + access.x.getDouble(builder));
            source.putDouble(y + access.y.getDouble(builder));
            source.putDouble(z + access.z.getDouble(builder));
            for (int word : words) source.putInt(word);
            for (float value : ao) source.putFloat(value);
            for (float value : rgb) source.putFloat(value);
            for (int value : light) source.putInt(value);
            count++;
        }

        boolean resourceCurrent() {
            return source.getLong(16) > 0
                    && source.getLong(16) == LegacyRuntime.completedResourceGeneration()
                    && source.getLong(16) == LegacyRuntime.resourceGeneration()
                    && source.getLong(24) == LegacyRuntime.atlasResourceGeneration();
        }

        void compare() throws Throwable {
            Object translucent = builders[3];
            source.putFloat(4, (float) (camera[0] + access.x.getDouble(translucent)));
            source.putFloat(8, (float) (camera[1] + access.y.getDouble(translucent)));
            source.putFloat(12, (float) (camera[2] + access.z.getDouble(translucent)));
            oracle.clear().position(16);
            int vertices = 0;
            for (int i = 0; i < 4; i++) {
                int length = (int) access.count.invokeExact(builders[i]);
                if (length < 0 || length % 4 != 0 || length > (LIMIT * 4) - vertices) {
                    METRICS[2]++; return;
                }
                vertices += length;
                oracle.putInt(i * 4, length / 4);
                ByteBuffer buffer = (ByteBuffer) access.buffer.invokeExact(builders[i]);
                ByteBuffer view = buffer.duplicate();
                view.clear().limit(length * 28);
                oracle.put(view);
            }
            if (vertices != count * 4) { METRICS[2]++; return; }
            if (!resourceCurrent()) { METRICS[3]++; return; }
            source.putInt(0, count);
            long[] result = Hook.ravelRendererBaked(generation, source.array(), oracle.array(), output);
            if (!resourceCurrent() || !LegacyRenderer.accepts(generation)) { METRICS[3]++; return; }
            if (result.length == 0) { METRICS[3]++; return; }
            if (result.length != 6 || result[0] < 0) { METRICS[2]++; return; }
            METRICS[0]++;
            METRICS[4] += result[0]; METRICS[5] += result[1]; METRICS[6] += result[3];
            if (result[2] >= 0 && METRICS[10] < 0) METRICS[10] = result[2];
            if (count > 0) queued = this;
            METRICS[7] = Math.max(METRICS[7], result[4]);
            METRICS[8] = Math.max(METRICS[8], result[5]); METRICS[9] += animated;
            METRICS[11] = source.getLong(16);
            METRICS[12] = source.getLong(24);
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
