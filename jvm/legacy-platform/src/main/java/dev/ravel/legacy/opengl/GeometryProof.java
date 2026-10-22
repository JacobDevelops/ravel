package dev.ravel.legacy.opengl;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import org.lwjgl.opengl.ARBFramebufferObject;
import org.lwjgl.opengl.ARBShaderObjects;
import org.lwjgl.opengl.ARBVertexBufferObject;
import org.lwjgl.opengl.ARBVertexArrayObject;
import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL14;
import org.lwjgl.opengl.GL15;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GLCapabilities;
import org.lwjgl.system.MemoryUtil;

/**
 * Offscreen geometric submission only. Never cancels, replaces or overlays a vanilla draw.
 * A successful solid mask is NOT model/texture/tint/AO/light fidelity or a world renderer.
 */
public final class GeometryProof {
    public static final int SIZE = 64;
    private static final int PIXELS = SIZE * SIZE;
    private static final int UPLOAD_CAPACITY = MeshWire.MAX_SOLID_BYTES;
    private static final int READ_CAPACITY = PIXELS * 8;
    private static final ProofLease LEASE = new ProofLease();
    private static final long[] METRICS = new long[23];
    private static ByteBuffer staging;
    private static ByteBuffer readback;
    private static byte[] lastVanilla;
    private static MeshWire mesh;
    private static GLCapabilities context;
    private static boolean coreBuffers;
    private static String backend = "selected-not-initialized";
    private static int vertexBuffer;
    private static int indexBuffer;
    private static int framebuffer;
    private static int color;
    private static int vertexArray;
    private static boolean comparing;
    private static boolean capabilityRefused;
    private static int textureUnits;
    private static int depth;
    private static byte[] bakedPixels;
    private static float[] bakedDepth;
    private static byte[][] bakedRetained;
    private static final ProofLease BAKED_LEASE = new ProofLease();
    private static final long[] BAKED_RESIDENCY = new long[16];
    private static final int[] bakedLayers = new int[4];
    private static final int[] bakedReplacementLayers = new int[4];
    private static int bakedBuffer;
    private static int bakedPending;
    private static int bakedOracle;
    private static int bakedAtlas;
    private static int bakedLightmap;
    public static final int MAX_BAKED_QUADS = 8192;
    private static final int BAKED_CAPACITY = MAX_BAKED_QUADS * 112;

    public static synchronized long[] bakedResidency() { return BAKED_RESIDENCY.clone(); }

    private GeometryProof() { }

    private static final class CapabilityUnsupported extends UnsupportedOperationException {
        private static final long serialVersionUID = 1L;
        CapabilityUnsupported(String message) { super(message); }
    }

    public static synchronized String backend() { return backend; }
    public static synchronized long[] metrics() { return METRICS.clone(); }
    public static synchronized long[] identity() { return new long[]{METRICS[21], METRICS[22]}; }

    /** Exactly one complete, nonempty mesh can be resident; staging never grows with world size. */
    public static synchronized boolean upload(byte[] copy, long generation, long key, long owner, boolean eligible) {
        LEASE.check(generation, owner);
        MeshWire candidate;
        try {
            candidate = MeshWire.parse(copy, generation, key);
        } catch (IllegalArgumentException invalid) {
            METRICS[12]++;
            throw invalid; // Corrupt copied output is not a completed disposition: native ownership remains retained.
        }
        if (!candidate.drawable() || !eligible) {
            METRICS[12]++;
            return false;
        }
        if (capabilityRefused) {
            METRICS[12]++;
            return false;
        }
        try {
            initialize();
        } catch (CapabilityUnsupported unsupported) {
            capabilityRefused = true;
            backend = "unsupported";
            METRICS[12]++;
            return false; // Explicit unsupported configuration, not a successful upload or submission.
        }
        long started = System.nanoTime();
        int previousVertices = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        int previousIndices = GL11.glGetInteger(GL15.GL_ELEMENT_ARRAY_BUFFER_BINDING);
        try {
            LEASE.invalidate(generation, owner); // A partially failed upload can never draw the previous identity.
            mesh = null;
            lastVanilla = null;
            METRICS[21] = METRICS[22] = 0;
            staging.clear();
            candidate.stageSolid(staging);
            staging.flip();
            bind(GL15.GL_ARRAY_BUFFER, vertexBuffer);
            data(GL15.GL_ARRAY_BUFFER, staging.slice(0, candidate.solidVertexBytes()));
            bind(GL15.GL_ELEMENT_ARRAY_BUFFER, indexBuffer);
            data(GL15.GL_ELEMENT_ARRAY_BUFFER, staging.slice(candidate.solidVertexBytes(), candidate.indexBytes()));
            mesh = candidate;
            LEASE.uploaded(generation, owner);
            METRICS[3]++;
            METRICS[4] += candidate.solidVertexBytes() + candidate.indexBytes();
            METRICS[6] = candidate.solidVertexBytes() + candidate.indexBytes(); // VBO payload only; CPU staging is metric 8.
            METRICS[7] = Math.max(METRICS[7], METRICS[6]);
            METRICS[21] = generation;
            METRICS[22] = key;
            METRICS[5] += System.nanoTime() - started;
            submit(generation, owner, null, false);
            return true;
        } finally {
            bind(GL15.GL_ARRAY_BUFFER, previousVertices);
            bind(GL15.GL_ELEMENT_ARRAY_BUFFER, previousIndices);
        }
    }

    /** Native-to-Java copy timing is measured by the agent around its one JNI bulk copy. */
    public static synchronized void copied(int bytes, long nanos) {
        METRICS[0]++;
        METRICS[1] += bytes;
        METRICS[2] += nanos;
    }

    private static void initialize() {
        if (bakedPixels == null) {
            bakedPixels = new byte[PIXELS * 4];
            bakedDepth = new float[PIXELS];
        }
        GLCapabilities caps = GL.getCapabilities();
        if (context != null) {
            if (context != caps) throw new IllegalStateException("Geometry GL context changed without cleanup");
            return;
        }
        coreBuffers = caps.OpenGL15;
        backend = "unsupported";
        backend = selectBackend(coreBuffers, caps.GL_ARB_vertex_buffer_object,
                caps.OpenGL30 || caps.GL_ARB_framebuffer_object, caps.OpenGL13
                && caps.glPushAttrib != 0 && caps.glPushClientAttrib != 0
                && caps.glVertexPointer != 0 && caps.glMatrixMode != 0);
        textureUnits = selectTextureUnits(GL11.glGetInteger(GL13.GL_MAX_TEXTURE_UNITS));
        context = caps;
        try {
            staging = MemoryUtil.memAlloc(UPLOAD_CAPACITY).order(ByteOrder.nativeOrder());
            readback = MemoryUtil.memAlloc(READ_CAPACITY).order(ByteOrder.nativeOrder());
            vertexBuffer = generate();
            indexBuffer = generate();
            METRICS[14] = 2;
            if (caps.OpenGL30 || caps.GL_ARB_vertex_array_object) {
                vertexArray = ARBVertexArrayObject.glGenVertexArrays();
                if (vertexArray == 0) throw new IllegalStateException("Geometry VAO allocation failed");
            }
            createTarget();
            METRICS[8] = UPLOAD_CAPACITY + READ_CAPACITY;
            METRICS[9] = Math.max(METRICS[9], METRICS[8]);
        } catch (RuntimeException | Error failure) {
            destroy();
            throw failure;
        }
    }

    static String selectBackend(boolean core, boolean arb, boolean framebuffer, boolean compatibility) {
        if ((!core && !arb) || !framebuffer || !compatibility)
            throw new CapabilityUnsupported("Geometry proof needs compatibility VBO and framebuffer capabilities");
        return core ? "OpenGL15" : "ARB_vertex_buffer_object";
    }

    static int selectTextureUnits(int units) {
        if (units < 1 || units > 32)
            throw new CapabilityUnsupported("Geometry proof needs between 1 and 32 compatibility texture units");
        return units;
    }

    private static void createTarget() {
        int draw = GL11.glGetInteger(ARBFramebufferObject.GL_DRAW_FRAMEBUFFER_BINDING);
        int read = GL11.glGetInteger(ARBFramebufferObject.GL_READ_FRAMEBUFFER_BINDING);
        int renderbuffer = GL11.glGetInteger(ARBFramebufferObject.GL_RENDERBUFFER_BINDING);
        try {
            framebuffer = ARBFramebufferObject.glGenFramebuffers();
            if (framebuffer == 0) throw new IllegalStateException("Geometry framebuffer allocation failed");
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_FRAMEBUFFER, framebuffer);
            color = attachment(ARBFramebufferObject.GL_COLOR_ATTACHMENT0, GL11.GL_RGBA8);
            depth = attachment(ARBFramebufferObject.GL_DEPTH_ATTACHMENT, GL14.GL_DEPTH_COMPONENT24);
            GL11.glDrawBuffer(ARBFramebufferObject.GL_COLOR_ATTACHMENT0);
            GL11.glReadBuffer(ARBFramebufferObject.GL_COLOR_ATTACHMENT0);
            if (ARBFramebufferObject.glCheckFramebufferStatus(ARBFramebufferObject.GL_FRAMEBUFFER)
                    != ARBFramebufferObject.GL_FRAMEBUFFER_COMPLETE)
                throw new IllegalStateException("Geometry offscreen framebuffer is incomplete");
            METRICS[15] = 1;
        } finally {
            ARBFramebufferObject.glBindRenderbuffer(ARBFramebufferObject.GL_RENDERBUFFER, renderbuffer);
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_DRAW_FRAMEBUFFER, draw);
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_READ_FRAMEBUFFER, read);
        }
    }

    private static int attachment(int point, int format) {
        int buffer = ARBFramebufferObject.glGenRenderbuffers();
        if (buffer == 0) throw new IllegalStateException("Geometry renderbuffer allocation failed");
        try {
            ARBFramebufferObject.glBindRenderbuffer(ARBFramebufferObject.GL_RENDERBUFFER, buffer);
            ARBFramebufferObject.glRenderbufferStorage(ARBFramebufferObject.GL_RENDERBUFFER, format, SIZE, SIZE);
            ARBFramebufferObject.glFramebufferRenderbuffer(ARBFramebufferObject.GL_FRAMEBUFFER, point,
                    ARBFramebufferObject.GL_RENDERBUFFER, buffer);
            return buffer;
        } catch (RuntimeException | Error failure) {
            ARBFramebufferObject.glDeleteRenderbuffers(buffer);
            throw failure;
        }
    }

    public static synchronized void bindVanillaBuffer(long generation, long owner, int buffer) {
        LEASE.draw(generation, owner);
        if (!comparing || buffer <= 0 || context != GL.getCapabilities())
            throw new IllegalStateException("Vanilla VBO binding is restricted to the active offscreen oracle");
        bind(GL15.GL_ARRAY_BUFFER, buffer);
    }

    public static synchronized byte[] pixels(long generation, long owner) {
        submit(generation, owner, null, true);
        byte[] result = new byte[PIXELS * 4];
        readback.get(0, result);
        byte[] reference = referencePixels(generation, owner);
        METRICS[16] = mismatch(result, reference, false);
        return result;
    }

    public static synchronized float[] depths(long generation, long owner) {
        submit(generation, owner, null, true);
        return readDepth();
    }

    /** Caller supplies VANILLA's own compiled geometry, not the native mesh or Java reference. */
    public static synchronized long[] compareVanilla(long generation, long owner, Runnable vanillaDraw) {
        if (vanillaDraw == null) throw new IllegalArgumentException("Vanilla draw callback is required");
        submit(generation, owner, null, true);
        byte[] actual = new byte[PIXELS * 4];
        readback.get(0, actual);
        float[] actualDepth = readDepth();
        submit(generation, owner, vanillaDraw, true);
        byte[] vanilla = new byte[PIXELS * 4];
        readback.get(0, vanilla);
        lastVanilla = vanilla;
        float[] vanillaDepth = readDepth();
        long differentDepth = 0;
        for (int pixel = 0; pixel < PIXELS; pixel++)
            if (Math.abs(actualDepth[pixel] - vanillaDepth[pixel]) > 2.0 / 0xffffff) differentDepth++;
        long rgba = mismatch(actual, vanilla, false), coverage = mismatch(actual, vanilla, true);
        METRICS[17]++;
        METRICS[18] = rgba;
        METRICS[19] = coverage;
        METRICS[20] = differentDepth;
        return new long[]{rgba, coverage, differentDepth, PIXELS};
    }

    public static synchronized byte[] vanillaPixels(long generation, long owner) {
        LEASE.draw(generation, owner);
        if (lastVanilla == null) throw new IllegalStateException("No published vanilla comparison pixels");
        return lastVanilla.clone();
    }

    private static long mismatch(byte[] left, byte[] right, boolean coverage) {
        long count = 0;
        for (int pixel = 0; pixel < PIXELS; pixel++) {
            int at = pixel * 4;
            if (coverage ? (left[at] != 0) != (right[at] != 0)
                    : !Arrays.equals(left, at, at + 4, right, at, at + 4)) count++;
        }
        return count;
    }

    public static synchronized long[] compareBaked(int[] emitted, byte[] oracle, long[] identity, boolean retain) {
        if (emitted.length != 4 + MAX_BAKED_QUADS * 28 || oracle.length != 16 + MAX_BAKED_QUADS * 112)
            throw new IllegalArgumentException("Invalid bounded baked output");
        checkBakedIdentity(identity, BAKED_RESIDENCY[7]);
        BAKED_LEASE.check(identity[0], identity[7]);
        checkBakedCounts(emitted, oracle);
        bakedRetained = null;
        if (capabilityRefused) return new long[]{-1};
        try {
            initialize();
        } catch (CapabilityUnsupported unsupported) {
            capabilityRefused = true;
            backend = "unsupported";
            return new long[]{-1};
        }
        if (textureUnits < 2) return new long[]{-1};
        int active = GL11.glGetInteger(GL13.GL_ACTIVE_TEXTURE);
        GL13.glActiveTexture(GL13.GL_TEXTURE0);
        int atlas = GL11.glGetInteger(GL11.GL_TEXTURE_BINDING_2D);
        GL13.glActiveTexture(GL13.GL_TEXTURE1);
        int lightmap = GL11.glGetInteger(GL11.GL_TEXTURE_BINDING_2D);
        GL13.glActiveTexture(active);
        if (atlas == 0 || lightmap == 0) return new long[]{-1};
        int previous = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        long[] result = new long[32];
        byte[][] retained = retain ? new byte[4][] : null;
        long started = System.nanoTime();
        int offset = 0;
        try {
            reserveBaked();
            for (int layer = 0; layer < 4; layer++) {
                int quads = emitted[layer];
                if (quads > 0) bakedLayer(emitted, oracle, offset, quads, layer, bakedPending, atlas, lightmap, result);
                bakedReplacementLayers[layer] = quads * 4;
                if (quads > 0 && retained != null) retained[layer] = rawSample();
                offset += quads * 112;
            }
            result[31] = System.nanoTime() - started;
            BAKED_LEASE.uploaded(identity[0], identity[7]);
            int previousResident = bakedBuffer;
            bakedBuffer = bakedPending;
            bakedPending = previousResident;
            System.arraycopy(bakedReplacementLayers, 0, bakedLayers, 0, 4);
            BAKED_RESIDENCY[0] = 3L * BAKED_CAPACITY;
            BAKED_RESIDENCY[1] = Math.max(BAKED_RESIDENCY[1], BAKED_RESIDENCY[0]);
            BAKED_RESIDENCY[3]++;
            System.arraycopy(identity, 0, BAKED_RESIDENCY, 6, identity.length);
            bakedAtlas = atlas;
            bakedLightmap = lightmap;
            bakedRetained = retained;
            return result;
        } finally {
            bind(GL15.GL_ARRAY_BUFFER, previous);
        }
    }

    public static synchronized void resortBaked(int[] emitted, long generation, long key, long owner) {
        BAKED_LEASE.check(generation, owner);
        if (BAKED_RESIDENCY[6] != generation || BAKED_RESIDENCY[7] != key
                || context != GL.getCapabilities() || emitted.length < 4
                || emitted.length != 4 + emitted[3] * 28)
            throw new IllegalStateException("Stale resident transparency upload");
        int offset = 0;
        for (int layer = 0; layer < 4; layer++) {
            if (emitted[layer] * 4 != bakedLayers[layer])
                throw new IllegalArgumentException("Resident transparency layer count changed");
            if (layer < 3) offset += emitted[layer] * 112;
        }
        int bytes = emitted[3] * 112;
        if (bytes == 0) return;
        int previous = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        try {
            staging.clear().limit(bytes);
            staging.asIntBuffer().put(emitted, 4, bytes / 4);
            bind(GL15.GL_ARRAY_BUFFER, bakedBuffer);
            updateBaked(offset, staging);
        } finally {
            bind(GL15.GL_ARRAY_BUFFER, previous);
        }
    }

    static void checkBakedCounts(int[] emitted, byte[] oracle) {
        int total = 0;
        for (int layer = 0; layer < 4; layer++) {
            int count = emitted[layer];
            if (count < 0 || count > MAX_BAKED_QUADS - total || count != littleInt(oracle, layer * 4))
                throw new IllegalArgumentException("Baked layer counts differ from final buffers");
            total += count;
        }
    }

    static void checkBakedIdentity(long[] identity, long lastKey) {
        if (identity.length != 8 || identity[0] <= 0 || identity[1] <= lastKey
                || identity[2] <= 0 || identity[2] != identity[3])
            throw new IllegalArgumentException("Invalid or superseded shaded identity");
        for (int axis = 4; axis < 7; axis++)
            if ((identity[axis] & 15) != 0) throw new IllegalArgumentException("Unaligned shaded section");
    }

    private static void reserveBaked() {
        if (bakedBuffer != 0) return;
        int[] buffers = new int[3];
        try {
            for (int index = 0; index < buffers.length; index++) buffers[index] = reserveBakedBuffer();
        } catch (RuntimeException failure) {
            for (int buffer : buffers) if (buffer != 0) delete(buffer);
            throw failure;
        }
        bakedBuffer = buffers[0];
        bakedPending = buffers[1];
        bakedOracle = buffers[2];
        BAKED_RESIDENCY[2] += 3;
        BAKED_RESIDENCY[5] = 3;
    }

    private static int reserveBakedBuffer() {
        int buffer = generate();
        try {
            bind(GL15.GL_ARRAY_BUFFER, buffer);
            if (coreBuffers) GL15.glBufferData(GL15.GL_ARRAY_BUFFER, BAKED_CAPACITY, GL15.GL_DYNAMIC_DRAW);
            else ARBVertexBufferObject.glBufferDataARB(GL15.GL_ARRAY_BUFFER, BAKED_CAPACITY,
                    ARBVertexBufferObject.GL_DYNAMIC_DRAW_ARB);
            int size = coreBuffers ? GL15.glGetBufferParameteri(GL15.GL_ARRAY_BUFFER, GL15.GL_BUFFER_SIZE)
                    : ARBVertexBufferObject.glGetBufferParameteriARB(GL15.GL_ARRAY_BUFFER,
                            ARBVertexBufferObject.GL_BUFFER_SIZE_ARB);
            if (size != BAKED_CAPACITY) throw new IllegalStateException("Shaded VBO reservation failed");
            return buffer;
        } catch (RuntimeException failure) {
            delete(buffer);
            throw failure;
        }
    }

    public static synchronized void drawBaked(long generation, long resource, long atlas, long owner) {
        if (BAKED_RESIDENCY[0] == 0) return;
        if (BAKED_RESIDENCY[6] != generation || BAKED_RESIDENCY[8] != resource || BAKED_RESIDENCY[9] != atlas) {
            BAKED_RESIDENCY[15]++;
            return;
        }
        BAKED_LEASE.draw(generation, owner);
        if (context != GL.getCapabilities()) throw new IllegalStateException("Wrong shaded GL context");
        int offset = 0;
        for (int layer = 0; layer < 4; layer++) {
            if (bakedLayers[layer] != 0)
                bakedSubmit(bakedBuffer, bakedLayers[layer], layer, bakedAtlas, bakedLightmap, offset, false);
            offset += bakedLayers[layer] * 28;
        }
        BAKED_RESIDENCY[14]++;
    }
    public static synchronized byte[] compositeFrame(Runnable render, int[] ids, int scene, int designated,
            int width, int height, long generation, long resource, long atlas, long owner, boolean fallback) {
        if (fallback && designated >= 5 && context == null) initialize();
        if (!fallback || designated < 5) {
            if (BAKED_RESIDENCY[0] == 0 || BAKED_RESIDENCY[6] != generation
                    || BAKED_RESIDENCY[8] != resource || BAKED_RESIDENCY[9] != atlas)
                throw new IllegalStateException("Composited frame has no current resident section");
            BAKED_LEASE.draw(generation, owner);
        }
        if (context != GL.getCapabilities()) throw new IllegalStateException("Wrong shaded GL context");
        return CompositeFrame.capture(render, bakedBuffer, ids, bakedLayers.clone(), scene, designated,
                width, height, fallback);
    }

    private static int littleInt(byte[] bytes, int offset) {
        return (bytes[offset] & 255) | (bytes[offset + 1] & 255) << 8
                | (bytes[offset + 2] & 255) << 16 | bytes[offset + 3] << 24;
    }

    private static void bakedLayer(int[] emitted, byte[] oracle, int offset, int quads, int layer,
            int buffer, int atlas, int lightmap, long[] result) {
        int bytes = quads * 112;
        result[28] += 2L * bytes;
        result[29] = BAKED_CAPACITY;
        staging.clear().limit(bytes);
        staging.asIntBuffer().put(emitted, 4 + offset / 4, bytes / 4);
        bind(GL15.GL_ARRAY_BUFFER, buffer);
        long uploadStarted = System.nanoTime();
        updateBaked(offset, staging);
        result[30] += System.nanoTime() - uploadStarted;
        bakedSubmit(buffer, quads * 4, layer, atlas, lightmap, offset, true);
        readback.get(0, bakedPixels);
        readback.asFloatBuffer().get(PIXELS, bakedDepth);
        staging.clear().put(oracle, 16 + offset, bytes).flip();
        uploadStarted = System.nanoTime();
        bind(GL15.GL_ARRAY_BUFFER, bakedOracle);
        updateBaked(offset, staging);
        result[30] += System.nanoTime() - uploadStarted;
        bakedSubmit(bakedOracle, quads * 4, layer, atlas, lightmap, offset, true);
        bakedDifference(result, layer * 7, quads * 4);
    }

    private static void updateBaked(int offset, ByteBuffer bytes) {
        if (coreBuffers) GL15.glBufferSubData(GL15.GL_ARRAY_BUFFER, offset, bytes);
        else ARBVertexBufferObject.glBufferSubDataARB(GL15.GL_ARRAY_BUFFER, offset, bytes);
    }

    /** Last retained comparison's per-layer raw arms: emitted RGBA, depth, then oracle RGBA, depth. */
    public static synchronized byte[][] bakedReadbacks() {
        byte[][] retained = bakedRetained;
        bakedRetained = null;
        return retained;
    }

    private static byte[] rawSample() {
        ByteBuffer raw = ByteBuffer.allocate(PIXELS * 16).order(ByteOrder.LITTLE_ENDIAN);
        raw.put(bakedPixels);
        for (float value : bakedDepth) raw.putFloat(value);
        for (int index = 0; index < PIXELS * 4; index++) raw.put(readback.get(index));
        for (int pixel = 0; pixel < PIXELS; pixel++) raw.putFloat(readback.getFloat(PIXELS * 4 + pixel * 4));
        return raw.array();
    }

    private static void bakedDifference(long[] result, int base, int vertices) {
        result[base] = 1;
        result[base + 6] = vertices;
        for (int pixel = 0; pixel < PIXELS; pixel++) {
            boolean differs = false, actual = false, reference = false;
            for (int channel = 0; channel < 4; channel++) {
                int index = pixel * 4 + channel;
                byte value = readback.get(index);
                differs |= bakedPixels[index] != value;
                if (channel == 3) {
                    actual |= bakedPixels[index] != 0;
                    reference |= value != 0;
                }
            }
            if (differs) result[base + 1]++;
            if (actual != reference) result[base + 2]++;
            if (Math.abs(bakedDepth[pixel] - readback.getFloat(PIXELS * 4 + pixel * 4)) > 2.0 / 0xffffff)
                result[base + 3]++;
            if (actual) result[base + 4]++;
            if (reference) result[base + 5]++;
        }
    }

    private static void bakedSubmit(int buffer, int vertices, int layer, int atlas, int lightmap,
            int offset, boolean capture) {
        try (State state = new State()) {
            state.prepare();
            GL11.glClearColor(0, 0, 0, 0);
            GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);
            bind(GL15.GL_ARRAY_BUFFER, buffer);
            GL11.glEnableClientState(GL11.GL_VERTEX_ARRAY);
            GL11.glEnableClientState(GL11.GL_COLOR_ARRAY);
            GL11.glVertexPointer(3, GL11.GL_FLOAT, 28, offset);
            GL11.glColorPointer(4, GL11.GL_UNSIGNED_BYTE, 28, 12L + offset);
            bakedTexture(GL13.GL_TEXTURE0, atlas, GL11.GL_FLOAT, 16L + offset);
            bakedTexture(GL13.GL_TEXTURE1, lightmap, GL11.GL_SHORT, 24L + offset);
            GL11.glEnable(GL11.GL_ALPHA_TEST);
            GL11.glAlphaFunc(GL11.GL_GREATER, 0.1f);
            if (layer == 3) {
                GL11.glEnable(GL11.GL_BLEND);
                GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
                GL11.glDepthMask(false);
            }
            try {
                GL11.glDrawArrays(GL11.GL_QUADS, 0, vertices);
                if (capture) {
                    readback.clear();
                    GL11.glReadPixels(0, 0, SIZE, SIZE, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE,
                            readback.slice(0, PIXELS * 4));
                    GL11.glReadPixels(0, 0, SIZE, SIZE, GL11.GL_DEPTH_COMPONENT, GL11.GL_FLOAT,
                            readback.slice(PIXELS * 4, PIXELS * 4));
                }
            } finally {
                resetBakedArrays();
            }
        }
    }

    private static void resetBakedArrays() {
        GL11.glDisableClientState(GL11.GL_COLOR_ARRAY);
        bind(GL15.GL_ARRAY_BUFFER, vertexBuffer);
        GL11.glVertexPointer(3, GL11.GL_SHORT, MeshWire.SOLID_VERTEX_STRIDE, 0L);
        GL11.glColorPointer(4, GL11.GL_UNSIGNED_BYTE, 28, 12L);
        GL11.glDisableClientState(GL11.GL_VERTEX_ARRAY);
        GL13.glClientActiveTexture(GL13.GL_TEXTURE0);
        GL11.glTexCoordPointer(2, GL11.GL_FLOAT, 28, 16L);
        GL11.glDisableClientState(GL11.GL_TEXTURE_COORD_ARRAY);
        GL13.glClientActiveTexture(GL13.GL_TEXTURE1);
        GL11.glTexCoordPointer(2, GL11.GL_SHORT, 28, 24L);
        GL11.glDisableClientState(GL11.GL_TEXTURE_COORD_ARRAY);
    }

    private static void bakedTexture(int unit, int texture, int type, long offset) {
        GL13.glActiveTexture(unit);
        GL13.glClientActiveTexture(unit);
        GL11.glEnable(GL11.GL_TEXTURE_2D);
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, texture);
        GL11.glEnableClientState(GL11.GL_TEXTURE_COORD_ARRAY);
        GL11.glTexCoordPointer(2, type, 28, offset);
    }

    private static float[] readDepth() {
        float[] result = new float[PIXELS];
        readback.asFloatBuffer().get(PIXELS, result);
        return result;
    }

    private static void submit(long generation, long owner, Runnable vanillaDraw, boolean capture) {
        LEASE.draw(generation, owner);
        if (context != GL.getCapabilities()) throw new IllegalStateException("Wrong geometry GL context");
        long started = System.nanoTime();
        try (State state = new State()) {
            state.prepare();
            LEASE.draw(generation, owner);
            GL11.glEnableClientState(GL11.GL_VERTEX_ARRAY);
            if (vanillaDraw == null) {
                bind(GL15.GL_ARRAY_BUFFER, vertexBuffer);
                bind(GL15.GL_ELEMENT_ARRAY_BUFFER, indexBuffer);
                GL11.glVertexPointer(3, GL11.GL_SHORT, MeshWire.SOLID_VERTEX_STRIDE, 0L);
                GL11.glDrawElements(GL11.GL_TRIANGLES, mesh.indices, GL11.GL_UNSIGNED_INT, 0L);
                METRICS[10]++;
                METRICS[11] += System.nanoTime() - started; // Native setup + draw, excludes readback and oracle.
            } else {
                comparing = true;
                try { vanillaDraw.run(); }
                finally {
                    comparing = false;
                    // Do not retain vanilla buffer storage through our long-lived VAO's vertex pointer.
                    bind(GL15.GL_ARRAY_BUFFER, vertexBuffer);
                    GL11.glVertexPointer(3, GL11.GL_SHORT, MeshWire.SOLID_VERTEX_STRIDE, 0L);
                }
            }
            if (capture) {
                readback.clear();
                GL11.glReadPixels(0, 0, SIZE, SIZE, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE,
                        readback.slice(0, PIXELS * 4));
                GL11.glReadPixels(0, 0, SIZE, SIZE, GL11.GL_DEPTH_COMPONENT, GL11.GL_FLOAT,
                        readback.slice(PIXELS * 4, PIXELS * 4));
            }
        }
    }

    /** Bottom-up white solid reference, independently rasterized at pixel centres. Not vanilla. */
    public static synchronized byte[] referencePixels(long generation, long owner) {
        LEASE.draw(generation, owner);
        float[] z = new float[PIXELS];
        Arrays.fill(z, Float.NEGATIVE_INFINITY);
        for (int index = 0; index < mesh.indices; index += 3) raster(z, index);
        byte[] result = new byte[PIXELS * 4];
        for (int pixel = 0; pixel < PIXELS; pixel++) {
            if (z[pixel] != Float.NEGATIVE_INFINITY) Arrays.fill(result, pixel * 4, pixel * 4 + 3, (byte) 255);
            result[pixel * 4 + 3] = (byte) 255;
        }
        return result;
    }

    private static void raster(float[] depthValues, int index) {
        int a = mesh.index(index), b = mesh.index(index + 1), c = mesh.index(index + 2);
        float ax = mesh.coordinate(a, 0) * 4, ay = mesh.coordinate(a, 1) * 4;
        float bx = mesh.coordinate(b, 0) * 4, by = mesh.coordinate(b, 1) * 4;
        float cx = mesh.coordinate(c, 0) * 4, cy = mesh.coordinate(c, 1) * 4;
        float area = edge(ax, ay, bx, by, cx, cy);
        if (area == 0) return;
        int minX = (int) Math.min(ax, Math.min(bx, cx)), maxX = (int) Math.max(ax, Math.max(bx, cx));
        int minY = (int) Math.min(ay, Math.min(by, cy)), maxY = (int) Math.max(ay, Math.max(by, cy));
        for (int y = minY; y < maxY; y++) for (int x = minX; x < maxX; x++) {
            float wa = edge(bx, by, cx, cy, x + 0.5f, y + 0.5f) / area;
            float wb = edge(cx, cy, ax, ay, x + 0.5f, y + 0.5f) / area;
            float wc = 1 - wa - wb;
            if (wa >= 0 && wb >= 0 && wc >= 0) {
                float z = wa * mesh.coordinate(a, 2) + wb * mesh.coordinate(b, 2) + wc * mesh.coordinate(c, 2);
                int at = y * SIZE + x;
                depthValues[at] = Math.max(depthValues[at], z);
            }
        }
    }

    private static float edge(float ax, float ay, float bx, float by, float x, float y) {
        return (x - ax) * (by - ay) - (y - ay) * (bx - ax);
    }

    public static synchronized void retire(long generation, long owner) {
        LEASE.retire(generation, owner);
        BAKED_LEASE.retire(generation, owner);
        if (context != null && context != GL.getCapabilities())
            throw new IllegalStateException("Geometry cleanup requires its GL context");
        destroy();
        METRICS[13]++;
    }

    private static void destroy() {
        if (bakedBuffer != 0) delete(bakedBuffer);
        if (bakedPending != 0) delete(bakedPending);
        if (bakedOracle != 0) delete(bakedOracle);
        BAKED_RESIDENCY[4] += bakedBuffer != 0 ? 1 : 0;
        BAKED_RESIDENCY[0] = 0;
        BAKED_RESIDENCY[5] = 0;
        Arrays.fill(BAKED_RESIDENCY, 6, 14, 0);
        Arrays.fill(bakedLayers, 0);
        bakedBuffer = bakedPending = bakedOracle = bakedAtlas = bakedLightmap = 0;
        if (vertexBuffer != 0) delete(vertexBuffer);
        if (indexBuffer != 0) delete(indexBuffer);
        if (vertexArray != 0) ARBVertexArrayObject.glDeleteVertexArrays(vertexArray);
        if (framebuffer != 0) ARBFramebufferObject.glDeleteFramebuffers(framebuffer);
        if (color != 0) ARBFramebufferObject.glDeleteRenderbuffers(color);
        if (depth != 0) ARBFramebufferObject.glDeleteRenderbuffers(depth);
        if (staging != null) MemoryUtil.memFree(staging);
        if (readback != null) MemoryUtil.memFree(readback);
        vertexBuffer = indexBuffer = framebuffer = color = depth = 0;
        vertexArray = 0;
        staging = readback = null;
        mesh = null;
        bakedPixels = null;
        bakedDepth = null;
        bakedRetained = null;
        lastVanilla = null;
        context = null;
        capabilityRefused = false;
        textureUnits = 0;
        METRICS[6] = METRICS[8] = METRICS[14] = METRICS[15] = METRICS[21] = METRICS[22] = 0;
    }

    private static int generate() {
        int buffer = coreBuffers ? GL15.glGenBuffers() : ARBVertexBufferObject.glGenBuffersARB();
        if (buffer == 0) throw new IllegalStateException("Geometry VBO allocation failed");
        return buffer;
    }
    private static void bind(int target, int buffer) {
        if (coreBuffers) GL15.glBindBuffer(target, buffer);
        else ARBVertexBufferObject.glBindBufferARB(target, buffer);
    }
    private static void data(int target, ByteBuffer bytes) {
        if (coreBuffers) GL15.glBufferData(target, bytes, GL15.GL_STATIC_DRAW);
        else ARBVertexBufferObject.glBufferDataARB(target, bytes, ARBVertexBufferObject.GL_STATIC_DRAW_ARB);
        int size = coreBuffers ? GL15.glGetBufferParameteri(target, GL15.GL_BUFFER_SIZE)
                : ARBVertexBufferObject.glGetBufferParameteriARB(target, ARBVertexBufferObject.GL_BUFFER_SIZE_ARB);
        if (size != bytes.remaining()) throw new IllegalStateException("Geometry VBO allocation failed");
    }
    private static void delete(int buffer) {
        if (coreBuffers) GL15.glDeleteBuffers(buffer);
        else ARBVertexBufferObject.glDeleteBuffersARB(buffer);
    }

    /** Save real GL state, never mutate Minecraft's cached state or its draw ordering. */
    private static final class State implements AutoCloseable {
        private final int draw = GL11.glGetInteger(ARBFramebufferObject.GL_DRAW_FRAMEBUFFER_BINDING);
        private final int read = GL11.glGetInteger(ARBFramebufferObject.GL_READ_FRAMEBUFFER_BINDING);
        private final int matrix = GL11.glGetInteger(GL11.GL_MATRIX_MODE);
        private final int array = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        private final int elements = GL11.glGetInteger(GL15.GL_ELEMENT_ARRAY_BUFFER_BINDING);
        private final int vao = vertexArray != 0 ? GL11.glGetInteger(ARBVertexArrayObject.GL_VERTEX_ARRAY_BINDING) : 0;
        private final int program = context.OpenGL20 ? GL11.glGetInteger(GL20.GL_CURRENT_PROGRAM)
                : context.GL_ARB_shader_objects ? ARBShaderObjects.glGetHandleARB(ARBShaderObjects.GL_PROGRAM_OBJECT_ARB) : 0;
        private final boolean pixelBuffers = context.OpenGL21 || context.GL_ARB_pixel_buffer_object;
        private final int pack = pixelBuffers ? GL11.glGetInteger(0x88ed) : 0;

        State() {
            stackRoom(GL11.GL_ATTRIB_STACK_DEPTH, GL11.GL_MAX_ATTRIB_STACK_DEPTH);
            stackRoom(GL11.GL_CLIENT_ATTRIB_STACK_DEPTH, GL11.GL_MAX_CLIENT_ATTRIB_STACK_DEPTH);
            stackRoom(GL11.GL_PROJECTION_STACK_DEPTH, GL11.GL_MAX_PROJECTION_STACK_DEPTH);
            stackRoom(GL11.GL_MODELVIEW_STACK_DEPTH, GL11.GL_MAX_MODELVIEW_STACK_DEPTH);
            GL11.glPushAttrib(GL11.GL_ALL_ATTRIB_BITS);
            GL11.glPushClientAttrib(GL11.GL_CLIENT_ALL_ATTRIB_BITS);
            if (vertexArray != 0) ARBVertexArrayObject.glBindVertexArray(vertexArray);
            GL11.glMatrixMode(GL11.GL_PROJECTION);
            GL11.glPushMatrix();
            GL11.glMatrixMode(GL11.GL_MODELVIEW);
            GL11.glPushMatrix();
        }

        private static void stackRoom(int depth, int maximum) {
            if (GL11.glGetInteger(depth) >= GL11.glGetInteger(maximum))
                throw new IllegalStateException("Geometry proof cannot safely save full compatibility state");
        }

        void prepare() {
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_FRAMEBUFFER, framebuffer);
            GL11.glDrawBuffer(ARBFramebufferObject.GL_COLOR_ATTACHMENT0);
            GL11.glReadBuffer(ARBFramebufferObject.GL_COLOR_ATTACHMENT0);
            if (context.OpenGL20) GL20.glUseProgram(0);
            else if (context.GL_ARB_shader_objects) ARBShaderObjects.glUseProgramObjectARB(0);
            for (int unit = 0; unit < textureUnits; unit++) {
                GL13.glActiveTexture(GL13.GL_TEXTURE0 + unit);
                GL13.glClientActiveTexture(GL13.GL_TEXTURE0 + unit);
                GL11.glDisable(GL11.GL_TEXTURE_1D);
                GL11.glDisable(GL11.GL_TEXTURE_2D);
                GL11.glDisable(GL13.GL_TEXTURE_CUBE_MAP);
                GL11.glDisable(0x806f); // GL_TEXTURE_3D, core since OpenGL 1.2.
                GL11.glDisableClientState(GL11.GL_TEXTURE_COORD_ARRAY);
            }
            GL13.glActiveTexture(GL13.GL_TEXTURE0);
            GL13.glClientActiveTexture(GL13.GL_TEXTURE0);
            GL11.glDisableClientState(GL11.GL_COLOR_ARRAY);
            GL11.glDisableClientState(GL11.GL_NORMAL_ARRAY);
            GL11.glDisableClientState(GL11.GL_VERTEX_ARRAY);
            for (int capability : new int[]{GL11.GL_BLEND, GL11.GL_ALPHA_TEST, GL11.GL_LIGHTING,
                    GL11.GL_FOG, GL11.GL_CULL_FACE, GL11.GL_SCISSOR_TEST, GL11.GL_STENCIL_TEST,
                    GL11.GL_COLOR_LOGIC_OP, GL11.GL_DITHER, GL11.GL_POLYGON_OFFSET_FILL,
                    GL11.GL_POLYGON_SMOOTH, GL13.GL_MULTISAMPLE, GL13.GL_SAMPLE_ALPHA_TO_COVERAGE,
                    GL13.GL_SAMPLE_ALPHA_TO_ONE, GL13.GL_SAMPLE_COVERAGE}) GL11.glDisable(capability);
            if (context.OpenGL30 || context.GL_ARB_framebuffer_sRGB) GL11.glDisable(0x8db9);
            GL11.glDisable(GL11.GL_POLYGON_STIPPLE);
            if (context.OpenGL30) GL11.glDisable(0x8c89); // GL_RASTERIZER_DISCARD
            if (context.OpenGL31) GL11.glDisable(0x8f9d); // GL_PRIMITIVE_RESTART
            if (context.OpenGL43 || context.GL_ARB_ES3_compatibility) GL11.glDisable(0x8d69);
            if (context.GL_ARB_vertex_program) GL11.glDisable(0x8620);
            if (context.GL_ARB_fragment_program) GL11.glDisable(0x8804);
            GL11.glViewport(0, 0, SIZE, SIZE);
            GL11.glPolygonMode(GL11.GL_FRONT_AND_BACK, GL11.GL_FILL);
            GL11.glColorMask(true, true, true, true);
            GL11.glDepthMask(true);
            GL11.glEnable(GL11.GL_DEPTH_TEST);
            GL11.glDepthFunc(GL11.GL_LESS);
            GL11.glDepthRange(0, 1);
            GL11.glColor4f(1, 1, 1, 1);
            GL11.glClearColor(0, 0, 0, 1);
            GL11.glClearDepth(1);
            GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);
            GL11.glMatrixMode(GL11.GL_PROJECTION);
            GL11.glLoadIdentity();
            GL11.glOrtho(0, 16, 0, 16, -32, 32);
            GL11.glMatrixMode(GL11.GL_MODELVIEW);
            GL11.glLoadIdentity();
            for (int plane = 0; plane < GL11.glGetInteger(GL11.GL_MAX_CLIP_PLANES); plane++)
                GL11.glDisable(GL11.GL_CLIP_PLANE0 + plane);
            if (pixelBuffers) bind(0x88eb, 0); // GL_PIXEL_PACK_BUFFER
            GL11.glPixelStorei(GL11.GL_PACK_ALIGNMENT, 1);
            GL11.glPixelStorei(GL11.GL_PACK_ROW_LENGTH, 0);
            GL11.glPixelStorei(GL11.GL_PACK_SKIP_ROWS, 0);
            GL11.glPixelStorei(GL11.GL_PACK_SKIP_PIXELS, 0);
            GL11.glPixelStorei(GL11.GL_PACK_SWAP_BYTES, GL11.GL_FALSE);
        }

        @Override public void close() {
            GL11.glMatrixMode(GL11.GL_MODELVIEW);
            GL11.glPopMatrix();
            GL11.glMatrixMode(GL11.GL_PROJECTION);
            GL11.glPopMatrix();
            GL11.glMatrixMode(matrix);
            if (vertexArray != 0) ARBVertexArrayObject.glBindVertexArray(vao);
            GL11.glPopClientAttrib();
            bind(GL15.GL_ARRAY_BUFFER, array);
            bind(GL15.GL_ELEMENT_ARRAY_BUFFER, elements);
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_DRAW_FRAMEBUFFER, draw);
            ARBFramebufferObject.glBindFramebuffer(ARBFramebufferObject.GL_READ_FRAMEBUFFER, read);
            GL11.glPopAttrib();
            if (context.OpenGL20) GL20.glUseProgram(program);
            else if (context.GL_ARB_shader_objects) ARBShaderObjects.glUseProgramObjectARB(program);
            if (pixelBuffers) bind(0x88eb, pack);
        }
    }
}
