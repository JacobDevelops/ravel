package dev.ravel.jvm;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/** Optional platform bridge; the default diagnostic path never loads or initializes GL. */
final class LegacyGeometryProof {
    static final String[] NAMES = {"copiedMeshes", "copiedBytes", "copyNanos", "uploads", "uploadBytes",
            "uploadNanos", "residentBytes", "maxResidentBytes", "stagingBytes", "maxStagingBytes",
            "submissions", "submissionNanos", "refusedMeshes", "cleanups", "liveBuffers", "liveFramebuffers",
            "referenceMismatchPixels", "vanillaComparisons", "vanillaRgbaMismatchPixels",
            "vanillaCoverageMismatchPixels", "vanillaDepthMismatchPixels", "generation", "key"};
    private static final String FIDELITY = "geometric-only-no-uv-sprite-tint-ao-baked-light-world-replacement-refused";
    private static final String SELECTION = selection();
    private static volatile Class<?> platform;
    private static Method upload;
    private static Method copied;
    private static final LegacyRetainedMesh.Operations RETAINED = new LegacyRetainedMesh.Operations() {
        @Override public byte[] copy(long generation, long key) { return Hook.ravelRendererMesh(generation, key); }
        @Override public boolean upload(byte[] copy, long generation, long key, long nanos) {
            return uploadCopy(copy, generation, key, nanos);
        }
        @Override public boolean accepts(long generation) { return LegacyRenderer.accepts(generation); }
        @Override public long[] acknowledge(long generation, long key) {
            return Hook.ravelRendererWork(LegacyMeshWork.Operation.ACKNOWLEDGE.code, generation, key);
        }
    };

    static {
        String destination = System.getProperty("ravel.renderer.metrics");
        if (destination != null) Runtime.getRuntime().addShutdownHook(new Thread(() -> export(destination),
                "ravel-geometry-metrics"));
    }

    private LegacyGeometryProof() { }

    private static String selection() {
        String value = System.getProperty("ravel.renderer", "vanilla");
        if (!value.equals("vanilla") && !value.equals("geometry-proof") && !value.equals("resident-world"))
            throw new IllegalArgumentException("Renderer fidelity is unavailable for ravel.renderer=" + value);
        if (value.equals("resident-world") && !Boolean.getBoolean("ravel.renderer.validation"))
            throw new IllegalArgumentException("Renderer fidelity is unavailable: resident-world normal-frame parity unproven");
        return value;
    }

    static boolean selected() { return !SELECTION.equals("vanilla"); }

    private static synchronized Class<?> platform() {
        if (!selected()) throw new IllegalStateException("Geometry proof was not explicitly selected");
        if (platform == null) {
            try {
                platform = Class.forName("dev.ravel.legacy.opengl.GeometryProof", true,
                        ClassLoader.getSystemClassLoader());
                upload = platform.getMethod("upload", byte[].class, long.class, long.class, long.class, boolean.class);
                copied = platform.getMethod("copied", int.class, long.class);
            } catch (ReflectiveOperationException failure) {
                throw new IllegalStateException("Selected geometry proof platform is unavailable", failure);
            }
        }
        return platform;
    }

    private static Object call(String name, Class<?>[] parameters, Object... arguments) {
        try {
            return platform().getMethod(name, parameters).invoke(null, arguments);
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof RuntimeException runtime) throw runtime;
            if (cause instanceof Error error) throw error;
            throw new IllegalStateException("Geometry proof failed", cause);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Geometry proof API is unavailable", failure);
        }
    }

    static boolean transfer(long generation, long[] metadata, int at) {
        requireOwner(generation);
        boolean settled = LegacyRetainedMesh.drain(generation, metadata, at, RETAINED);
        if (settled) LegacyVanillaGeometry.discard(metadata[at + 1], generation);
        return settled;
    }

    private static boolean uploadCopy(byte[] copy, long generation, long key, long nanos) {
        platform();
        try {
            copied.invoke(null, copy.length, nanos);
            requireOwner(generation);
            // Even ineligible scopes are parsed and counted as explicit whole-mesh refusals.
            long eligibility = LegacyVanillaGeometry.eligibility(generation, key);
            if (LegacyVanillaGeometry.retryEligibility(eligibility)) return false;
            upload.invoke(null, copy, generation, key, LegacyRenderer.ownerThread(), eligibility == 1);
            long[] resident = identity();
            if (resident.length == 2 && resident[0] == generation && resident[1] == key)
                LegacyVanillaGeometry.retainResident(generation, key);
            return true;
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof RuntimeException runtime) throw runtime;
            if (cause instanceof Error error) throw error;
            throw new IllegalStateException("Geometry upload failed", cause);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException("Geometry upload API unavailable", failure);
        }
    }

    private static void requireOwner(long generation) {
        if (Thread.currentThread().threadId() != LegacyRenderer.ownerThread() || !LegacyRenderer.accepts(generation))
            throw new IllegalStateException("Geometry proof requires a live generation and renderer owner thread");
    }

    static long[] metrics() {
        return platform == null ? new long[NAMES.length] : (long[]) call("metrics", new Class<?>[0]);
    }

    static String[] info() {
        String backend = platform == null ? (selected() ? "selected-not-initialized" : "not-selected")
                : (String) call("backend", new Class<?>[0]);
        return new String[]{SELECTION, backend, SELECTION.equals("resident-world")
                ? "one-section-opaque-world-replacement-parity-unproven" : FIDELITY};
    }

    static long[] identity() {
        return platform == null ? new long[2] : (long[]) call("identity", new Class<?>[0]);
    }

    static byte[] pixels(boolean reference) {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        return (byte[]) call(reference ? "referencePixels" : "pixels", new Class<?>[]{long.class, long.class},
                generation, LegacyRenderer.ownerThread());
    }

    static float[] depths() {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        return (float[]) call("depths", new Class<?>[]{long.class, long.class}, generation, LegacyRenderer.ownerThread());
    }

    static byte[] vanillaPixels() {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        return (byte[]) call("vanillaPixels", new Class<?>[]{long.class, long.class},
                generation, LegacyRenderer.ownerThread());
    }

    static long[] compareVanilla(Runnable draw) {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        return (long[]) call("compareVanilla", new Class<?>[]{long.class, long.class, Runnable.class},
                generation, LegacyRenderer.ownerThread(), draw);
    }

    static long[] compareBaked(int[] emitted, byte[] oracle, long[] identity, boolean retain) {
        requireOwner(identity[0]);
        return (long[]) call("compareBaked", new Class<?>[]{int[].class, byte[].class, long[].class, boolean.class},
                emitted, oracle, identity, retain);
    }

    static byte[][] bakedReadbacks() {
        requireOwner(LegacyRenderer.currentGeneration());
        return (byte[][]) call("bakedReadbacks", new Class<?>[0]);
    }

    static void drawBaked(long resource, long atlas) {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        call("drawBaked", new Class<?>[]{long.class, long.class, long.class, long.class},
                generation, resource, atlas, LegacyRenderer.ownerThread());
    }

    static void worldLayer(int layer, int[] ids, long generation, long key) {
        call("worldLayer", new Class<?>[]{int.class, int[].class, long.class, long.class,
                long.class, long.class}, layer, ids, generation, key,
                LegacyRuntime.completedResourceGeneration(), LegacyRuntime.atlasResourceGeneration());
    }

    static void resortBaked(int[] output, long generation, long key) {
        requireOwner(generation);
        call("resortBaked", new Class<?>[]{int[].class, long.class, long.class, long.class},
                output, generation, key, LegacyRenderer.ownerThread());
    }
    static byte[] compositeFrame(Runnable render, int[] ids, int scene, int designated, int width, int height,
            boolean fallback) {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        return (byte[]) call("compositeFrame", new Class<?>[]{Runnable.class, int[].class, int.class,
                int.class, int.class, int.class, long.class, long.class, long.class, long.class, boolean.class},
                render, ids, scene, designated, width, height, generation,
                LegacyRuntime.completedResourceGeneration(), LegacyRuntime.atlasResourceGeneration(),
                LegacyRenderer.ownerThread(), fallback);
    }

    static long[] shadedNativeResidency() {
        return selected() ? Hook.ravelRendererBakedLease(0, 0, 0) : new long[12];
    }

    static long[] shadedGlResidency() {
        return platform == null ? new long[16] : (long[]) call("bakedResidency", new Class<?>[0]);
    }

    static void bindVanillaBuffer(int buffer) {
        long generation = LegacyRenderer.currentGeneration();
        requireOwner(generation);
        call("bindVanillaBuffer", new Class<?>[]{long.class, long.class, int.class},
                generation, LegacyRenderer.ownerThread(), buffer);
    }

    static void retire(long generation) {
        if (platform != null) call("retire", new Class<?>[]{long.class, long.class}, generation, LegacyRenderer.ownerThread());
    }

    static long[] worldMetrics() {
        return platform == null ? new long[11] : (long[]) call("worldMetrics", new Class<?>[0]);
    }

    static void flushMetrics() {
        String destination = System.getProperty("ravel.renderer.metrics");
        if (destination != null) export(destination);
    }

    private static void export(String destination) {
        try {
            long[] values = metrics();
            String[] info = info();
            StringBuilder json = new StringBuilder("{\"schemaVersion\":1,\"selection\":\"").append(info[0])
                    .append("\",\"backend\":\"").append(info[1]).append("\",\"fidelity\":\"")
                    .append(info[2]).append("\",\"upload\":\"bounded-native-wire-java-owned-offscreen-gl\",\"metrics\":{");
            for (int index = 0; index < NAMES.length; index++) {
                if (index != 0) json.append(',');
                json.append('"').append(NAMES[index]).append("\":").append(values[index]);
            }
            json.append("},\"bakedMetrics\":");
            json.append(Arrays.toString(LegacyBakedCapture.metrics()).replace(" ", ""));
            json.append(",\"bakedWorld\":");
            json.append(Arrays.toString(LegacyBakedCapture.world()).replace(" ", ""));
            json.append(",\"shadedNativeResidency\":");
            json.append(Arrays.toString(shadedNativeResidency()).replace(" ", ""));
            json.append(",\"shadedGlResidency\":");
            json.append(Arrays.toString(shadedGlResidency()).replace(" ", ""));
            json.append(",\"productionWorldDraw\":");
            json.append(platform == null ? "[0,0,0,0,0,0,0,0,0,0,0]"
                    : Arrays.toString((long[]) call("worldMetrics", new Class<?>[0])).replace(" ", ""));
            json.append("}\n");
            Path path = Path.of(destination);
            if (path.toAbsolutePath().getParent() != null) Files.createDirectories(path.toAbsolutePath().getParent());
            Files.writeString(path, json);
        } catch (Exception failure) {
            System.err.println("[ravel] geometry metrics export failed: " + failure);
        }
    }
}
