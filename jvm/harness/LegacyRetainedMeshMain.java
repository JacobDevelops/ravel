package dev.ravel.jvm;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;

/** Executable retained completion tests without a native host, game, LWJGL or a GL context. */
public final class LegacyRetainedMeshMain {
    private LegacyRetainedMeshMain() { }

    public static void main(String[] arguments) throws ReflectiveOperationException {
        transactions();
        headers();
        admissions();
        vanillaBufferBytes();
        layerProvenance();
        excludedTranslucentCallbacks();
        System.out.println("retained-mesh-copy-validation-upload-ack-ok");
    }

    private static void excludedTranslucentCallbacks() throws ReflectiveOperationException {
        var target = LegacyBakedResort.class.getDeclaredField("target");
        var uploaded = LegacyBakedResort.class.getDeclaredField("uploaded");
        target.setAccessible(true);
        uploaded.setAccessible(true);
        Object destination = new Object();
        target.set(null, destination);
        uploaded.set(null, new int[]{4, 0, 0, 0});
        try {
            LegacyBakedResort.sort(new Object(), 1, 2, 3);
            LegacyBakedResort.upload(destination, new Object());
            uploaded.set(null, new int[]{0, 0, 0, 4});
            expect(ExceptionInInitializerError.class, () -> LegacyBakedResort.sort(new Object(), 1, 2, 3));
            expect(NoClassDefFoundError.class, () -> LegacyBakedResort.upload(destination, new Object()));
        } finally {
            LegacyBakedResort.clear();
        }
    }

    private static void layerProvenance() {
        int record = LegacyRendererContract.BAKED_RECORD_BYTES;
        ByteBuffer source = ByteBuffer.allocate(32 + 4 * record).order(ByteOrder.LITTLE_ENDIAN);
        for (int layer = 0; layer < 4; layer++) source.putInt(32 + layer * record + 8, layer);
        int[] observed = new int[4];
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{4, 4, 4, 4}, 0, observed) == 4,
                "complete layers admitted");
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{8, 0, 4, 4}, 0, observed) == -1,
                "equal total vertices cannot hide layer mismatch");
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{4, 4, 8, 12}, 8, observed) == -1,
                "water does not excuse cutout vertices");
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{4, 4, 4, 12}, 8, observed) == 3,
                "water removes only translucent provenance");
        require(source.getInt(32 + 2 * record + 8) == 2, "resident source order retained");
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{12, 4, 4, 4}, 1, observed) == 3,
                "lava removes only solid provenance");
        require(source.getInt(40) == 1, "lava source compacts past solid quads");
        for (int layer = 0; layer < 4; layer++) source.putInt(32 + layer * record + 8, layer);
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{0, 4, 4, 4}, 1, observed) == -1,
                "fluid cannot hide fewer vanilla vertices than baked vertices");
        require(LegacyBakedCapture.provenLayers(source, 4, new int[]{12, 4, 4, 12}, 9, observed) == 2,
                "water and lava remove only their two layers");
        require(source.getInt(40) == 1 && source.getInt(40 + record) == 2,
                "two fluid layers preserve ordered cutout provenance");
        source.putInt(40, 4);
        require(LegacyBakedCapture.provenLayers(source, 1, new int[]{4, 4, 4, 4}, 9, observed) == -1,
                "unknown baked layer is refused");
        require(LegacyBakedCapture.provenLayers(source, 0, new int[4], 0, observed) == 0,
                "empty capture remains valid");
    }

    private static void vanillaBufferBytes() throws ReflectiveOperationException {
        Object block = new Object();
        for (String name : new String[]{"id", "format", "block"}) {
            var field = LegacyVanillaBuffers.class.getDeclaredField(name);
            field.setAccessible(true);
            Object value = block;
            if (!name.equals("block")) {
                var member = VanillaBuffer.class.getDeclaredField(name);
                member.setAccessible(true);
                value = member;
            }
            field.set(null, value);
        }
        VanillaBuffer first = new VanillaBuffer(block);
        VanillaBuffer second = new VanillaBuffer(block);
        LegacyVanillaBuffers.upload(first, ByteBuffer.allocate(100).position(20));
        LegacyVanillaBuffers.upload(second, ByteBuffer.allocate(40));
        LegacyVanillaBuffers.upload(new VanillaBuffer(new Object()), ByteBuffer.allocate(500));
        require(Arrays.equals(LegacyVanillaBuffers.metrics(), new long[]{120, 120, 2}), "section formats only");
        LegacyVanillaBuffers.upload(first, ByteBuffer.allocate(10));
        LegacyVanillaBuffers.deleted(first);
        require(Arrays.equals(LegacyVanillaBuffers.metrics(), new long[]{50, 120, 2}), "resize and live bind");
        first.id = -1;
        LegacyVanillaBuffers.deleted(first);
        LegacyVanillaBuffers.deleted(first);
        second.id = -1;
        LegacyVanillaBuffers.deleted(second);
        require(Arrays.equals(LegacyVanillaBuffers.metrics(), new long[]{0, 120, 0}), "delete is exact and idempotent");
    }

    private static final class VanillaBuffer {
        int id = 1;
        final Object format;
        VanillaBuffer(Object format) { this.format = format; }
    }

    private static void transactions() {
        Fake operations = new Fake();
        require(drain(operations), "complete disposition acknowledged");
        require(operations.trace.toString().equals("cguga"), "copy / live / upload / live / ack order");
        for (byte[] absent : new byte[][]{null, new byte[0]}) {
            operations = new Fake();
            operations.wire = absent;
            require(!drain(operations) && operations.trace.toString().equals("c"), "absent copy retained for retry");
        }
        operations = new Fake();
        operations.live = false;
        require(!drain(operations) && operations.trace.toString().equals("cg"), "stale copy never uploaded/acked");
        operations = new Fake();
        operations.staleAfterUpload = true;
        require(!drain(operations) && operations.trace.toString().equals("cgug"), "retired upload never acked");
        Fake failed = new Fake();
        failed.uploadFails = true;
        expect(IllegalStateException.class, () -> drain(failed));
        require(failed.trace.toString().equals("cgu"), "upload exception preserves retained ownership");
        operations = new Fake();
        operations.uploadReady = false;
        require(!drain(operations) && operations.trace.toString().equals("cgu"),
                "busy oracle keeps native ownership without acknowledgement");
        operations.trace.setLength(0);
        operations.uploadReady = true;
        require(drain(operations) && operations.trace.toString().equals("cguga"),
                "same completion retries and acknowledges once after oracle lock becomes available");
        for (int status : new int[]{1, 2}) {
            Fake refused = new Fake();
            refused.wire = wire(status, 0, 0);
            long[] metadata = metadata(11, status, 0, 0);
            require(LegacyRetainedMesh.drain(7, metadata, 4, refused), "copied incomplete/error outcome disposed");
            require(refused.trace.toString().equals("cguga"), "refused geometry still requires a real copy before ack");
        }
        operations = new Fake();
        operations.ack = new long[0];
        require(!drain(operations), "stale ack is not accepted");
        for (long[] ack : new long[][]{{0}, {1, 1}, {2}}) {
            Fake malformed = new Fake();
            malformed.ack = ack;
            expect(IllegalStateException.class, () -> drain(malformed));
        }
        long[] two = Arrays.copyOf(metadata(11, 0, 4, 6), 26);
        two[2] = 2;
        Fake one = new Fake();
        require(LegacyRetainedMesh.drain(7, two, 4, one), "first retained record disposed");
        require(one.trace.toString().equals("cguga"), "one copy/upload/ack per owner poll");
    }

    private static void headers() {
        for (int offset : new int[]{0, 4, 8, 16, 24, 28, 32, 40}) {
            Fake changed = new Fake();
            changed.wire[offset] ^= (byte) 255;
            expect(IllegalStateException.class, () -> drain(changed));
            require(changed.trace.toString().equals("cg"), "metadata/header mismatch must not upload or ack");
        }
        for (byte[] bounded : new byte[][]{new byte[47], new byte[1_376_305]}) {
            Fake invalid = new Fake();
            invalid.wire = bounded;
            expect(IllegalStateException.class, () -> drain(invalid));
            require(invalid.trace.toString().equals("cg"), "out-of-bounds copy never acked");
        }
        Fake missing = new Fake();
        expect(IllegalStateException.class, () -> LegacyRetainedMesh.drain(7, new long[0], 0, missing));
        require(missing.trace.isEmpty(), "invalid identity rejected before JNI copy");
        long[] changed = metadata(11, 0, 4, 6);
        changed[4] = 8;
        expect(IllegalStateException.class, () -> LegacyRetainedMesh.drain(7, changed, 4, missing));
        require(missing.trace.isEmpty(), "wrong generation rejected before JNI copy");
    }

    private static void admissions() {
        long first = LegacyMeshWork.reserve(7);
        long[] record = metadata(first, 0, 4, 6);
        require(LegacyMeshWork.retainedAdmission(record, 7), "reserved task permits early completion");
        synchronized (LegacyMeshWork.class) {
            require(LegacyMeshWork.accept(record, 4, 7), "early acknowledged completion recorded");
            require(!LegacyMeshWork.accept(record, 4, 7), "duplicate completion refused");
        }
        require(LegacyMeshWork.occupied() == 1, "reservation retained until native receipt");
        LegacyMeshWork.commit(first, new long[]{1, 0, 0, 0, 0, 0, 1, first}, 0);
        require(LegacyMeshWork.occupied() == 0, "early completion receipt releases slot exactly once");
        expect(IllegalStateException.class, () -> LegacyMeshWork.retainedAdmission(record, 7));
        long admitted = LegacyMeshWork.reserve(7);
        LegacyMeshWork.commit(admitted, new long[]{1, 0, 0, 0, 0, 0, 1, admitted}, 0);
        long[] complete = metadata(admitted, 0, 4, 6);
        require(LegacyMeshWork.retainedAdmission(complete, 7), "admitted task permits retained completion");
        synchronized (LegacyMeshWork.class) {
            require(!LegacyMeshWork.accept(complete, 4, 8), "wrong generation cannot settle a slot");
            require(LegacyMeshWork.accept(complete, 4, 7), "acknowledged admitted completion settles slot");
        }
        require(LegacyMeshWork.occupied() == 0, "admitted completion releases slot");
        require(!LegacyMeshWork.retainedAdmission(new long[]{1, 7, 0, 11}, 7), "empty peek does no work");
        expect(IllegalStateException.class, () -> LegacyMeshWork.retainedAdmission(new long[]{1, 7, 9, 11}, 7));
        long[] wrongWidth = metadata(11, 0, 4, 6);
        wrongWidth[3] = 10;
        expect(IllegalStateException.class, () -> LegacyMeshWork.retainedAdmission(wrongWidth, 7));
        long[] wrongGeneration = metadata(11, 0, 4, 6);
        expect(IllegalStateException.class, () -> LegacyMeshWork.retainedAdmission(wrongGeneration, 8));
    }

    private static boolean drain(Fake operations) {
        return LegacyRetainedMesh.drain(7, metadata(11, 0, 4, 6), 4, operations);
    }

    private static long[] metadata(long key, int status, int vertices, int indices) {
        return new long[]{1, 7, 1, 11, 7, key, status, 0, 0, 104, 1, vertices, indices, status == 1 ? 1 : 0, 0};
    }

    private static byte[] wire(int status, int vertices, int indices) {
        int materials = vertices == 0 ? 0 : 1;
        ByteBuffer bytes = ByteBuffer.allocate(48 + vertices * 5 + indices * 4 + materials * 12)
                .order(ByteOrder.LITTLE_ENDIAN).putInt(0x354d5652).putInt(1)
                .putLong(7).putLong(11).putInt(status).putInt(vertices).putInt(indices).putInt(materials).putLong(0);
        if (vertices != 0) {
            for (byte[] corner : new byte[][]{{0, 0, 1}, {1, 0, 1}, {1, 1, 1}, {0, 1, 1}})
                bytes.put(corner).put((byte) 5).put((byte) 0);
            for (int index : new int[]{0, 1, 2, 0, 2, 3}) bytes.putInt(index);
            bytes.putInt(1).putInt(0).putInt(6);
        }
        return bytes.array();
    }

    private static final class Fake implements LegacyRetainedMesh.Operations {
        final StringBuilder trace = new StringBuilder();
        byte[] wire = wire(0, 4, 6);
        long[] ack = {1};
        boolean live = true;
        boolean staleAfterUpload;
        boolean uploadFails;
        boolean uploadReady = true;
        @Override public byte[] copy(long generation, long key) {
            require(generation == 7 && key == 11, "exact copy identity");
            trace.append('c');
            return wire;
        }
        @Override public boolean upload(byte[] copy, long generation, long key, long nanos) {
            require(copy == wire && generation == 7 && key == 11 && nanos >= 0, "owned copy upload identity");
            trace.append('u');
            if (uploadFails) throw new IllegalStateException("GL publication failed");
            if (staleAfterUpload) live = false;
            return uploadReady;
        }
        @Override public boolean accepts(long generation) { trace.append('g'); return live; }
        @Override public long[] acknowledge(long generation, long key) {
            require(generation == 7 && key == 11, "exact ack identity");
            trace.append('a');
            return ack;
        }
    }

    private static void expect(Class<? extends Throwable> expected, Runnable operation) {
        try { operation.run(); }
        catch (Throwable failure) {
            if (expected.isInstance(failure)) return;
            throw new AssertionError("Wrong refusal", failure);
        }
        throw new AssertionError("Expected " + expected.getSimpleName());
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
