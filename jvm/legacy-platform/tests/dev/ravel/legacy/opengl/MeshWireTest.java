package dev.ravel.legacy.opengl;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicBoolean;

/** Pure Java executable checks: no context, GL calls or native host required. */
public final class MeshWireTest {
    private MeshWireTest() { }

    public static void main(String[] arguments) throws InterruptedException {
        parser();
        refusals();
        lifetime();
        capabilities();
        System.out.println("mesh-wire-parser-refusal-lifetime-ok");
    }

    private static void parser() {
        byte[] wire = quad();
        MeshWire mesh = MeshWire.parse(wire, 7, 11);
        require(mesh.drawable() && mesh.vertices == 4 && mesh.indices == 6 && mesh.materials == 1,
                "complete quad shape");
        staging(mesh);
        require(mesh.coordinate(2, 0) == 1 && mesh.coordinate(2, 1) == 1 && mesh.index(5) == 3,
                "little-endian payload");
        reject(null);
        reject(new byte[0]);
        reject(Arrays.copyOf(wire, 47));
        reject(Arrays.copyOf(wire, wire.length - 1));
        reject(Arrays.copyOf(wire, wire.length + 1));
        reject(new byte[MeshWire.MAX_BYTES + 1]);
        for (int offset : new int[]{0, 4, 8, 16, 24, 28, 32, 36, 44, 48, 51, 68, 96, 100}) {
            byte[] malformed = wire.clone();
            malformed[offset] = (byte) 255;
            reject(malformed);
        }
        byte[] air = wire.clone();
        ByteBuffer.wrap(air).order(ByteOrder.LITTLE_ENDIAN).putInt(92, 0);
        reject(air);
        expect(IllegalArgumentException.class, () -> MeshWire.parse(wire, 8, 11));
        expect(IllegalArgumentException.class, () -> MeshWire.parse(wire, 7, 12));
        maximum();
    }

    private static void staging(MeshWire mesh) {
        require(MeshWire.SOLID_VERTEX_STRIDE == 6 && MeshWire.MAX_SOLID_BYTES == 1_179_648,
                "bounded SHORT XYZ and INT indices");
        ByteBuffer packed = ByteBuffer.allocate(48).order(ByteOrder.nativeOrder());
        mesh.stageSolid(packed);
        require(mesh.solidVertexBytes() == 24 && packed.position() == 48, "quad GPU payload excludes face/light");
        packed.flip();
        for (int vertex = 0; vertex < mesh.vertices; vertex++)
            for (int axis = 0; axis < 3; axis++)
                require(packed.getShort() == mesh.coordinate(vertex, axis), "exact signed SHORT XYZ conversion");
        for (int index = 0; index < mesh.indices; index++)
            require(packed.getInt() == mesh.index(index), "exact native-order INT index conversion");
        require(!packed.hasRemaining(), "no wire face/light or material bytes leak into GPU layout");
        ByteBuffer small = ByteBuffer.allocate(47).order(ByteOrder.nativeOrder());
        expect(IllegalArgumentException.class, () -> mesh.stageSolid(small));
        require(small.position() == 0, "staging capacity refusal precedes writes");
        ByteOrder foreign = ByteOrder.nativeOrder() == ByteOrder.LITTLE_ENDIAN
                ? ByteOrder.BIG_ENDIAN : ByteOrder.LITTLE_ENDIAN;
        expect(IllegalArgumentException.class, () -> mesh.stageSolid(ByteBuffer.allocate(48).order(foreign)));
    }

    private static void refusals() {
        byte[] incomplete = quad();
        ByteBuffer.wrap(incomplete).order(ByteOrder.LITTLE_ENDIAN).putInt(24, 1);
        require(!MeshWire.parse(incomplete, 7, 11).drawable(), "incomplete never drawable");
        byte[] empty = header(0, 0, 0, 0).array();
        require(!MeshWire.parse(empty, 7, 11).drawable(), "empty never claims scene coverage");
        byte[] error = header(2, 0, 0, 0).array();
        require(!MeshWire.parse(error, 7, 11).drawable(), "worker error never drawable");
        byte[] contradictory = quad();
        ByteBuffer.wrap(contradictory).order(ByteOrder.LITTLE_ENDIAN).putInt(24, 2);
        reject(contradictory);
        long owner = Thread.currentThread().threadId();
        byte[] malformedComplete = quad();
        ByteBuffer.wrap(malformedComplete).order(ByteOrder.LITTLE_ENDIAN).putInt(92, 0);
        expect(IllegalArgumentException.class, () -> GeometryProof.upload(malformedComplete, 7, 11, owner, true));
        expect(IllegalArgumentException.class, () -> GeometryProof.upload(malformedComplete, 7, 11, owner, false));
        require(!GeometryProof.upload(incomplete, 7, 11, owner, true), "incomplete is an explicit no-draw disposition");
        require(!GeometryProof.upload(error, 7, 11, owner, true), "worker error is an explicit no-draw disposition");
        require(!GeometryProof.upload(empty, 7, 11, owner, true), "empty complete cannot prove a solid scene");
        require(!GeometryProof.upload(quad(), 7, 11, owner, false),
                "ineligible vanilla scope is counted without a GL upload or partial-section draw");
        long[] metrics = GeometryProof.metrics();
        require(metrics[3] == 0 && metrics[10] == 0 && metrics[12] == 6,
                "refusals perform no GL uploads/submissions; malformed complete output is counted but thrown");
        GeometryProof.retire(7, owner); // Also proves no-context cleanup does not make GL calls.
    }

    private static void maximum() {
        ByteBuffer bytes = header(0, MeshWire.MAX_VERTICES, MeshWire.MAX_INDICES, MeshWire.MAX_MATERIALS);
        for (int vertex = 0; vertex < MeshWire.MAX_VERTICES; vertex++) {
            int corner = vertex % 4;
            bytes.put((byte) (corner == 1 || corner == 2 ? 1 : 0));
            bytes.put((byte) (corner >= 2 ? 1 : 0));
            bytes.put((byte) 1).put((byte) 5).put((byte) 255);
        }
        for (int face = 0; face < MeshWire.MAX_MATERIALS; face++) {
            int first = face * 4;
            for (int index : new int[]{0, 1, 2, 0, 2, 3}) bytes.putInt(first + index);
        }
        for (int face = 0; face < MeshWire.MAX_MATERIALS; face++) bytes.putInt(1).putInt(face * 6).putInt(6);
        MeshWire mesh = MeshWire.parse(bytes.array(), 7, 11);
        require(mesh.byteSize() == MeshWire.MAX_BYTES && mesh.drawable(), "inclusive maximum bounds");
    }

    private static void lifetime() throws InterruptedException {
        ProofLease lease = new ProofLease();
        long owner = Thread.currentThread().threadId();
        expect(IllegalStateException.class, () -> lease.draw(7, owner));
        lease.uploaded(7, owner);
        lease.draw(7, owner);
        expect(IllegalStateException.class, () -> lease.draw(8, owner));
        lease.invalidate(7, owner);
        expect(IllegalStateException.class, () -> lease.draw(7, owner));
        lease.uploaded(7, owner);
        AtomicBoolean wrongThread = new AtomicBoolean();
        Thread worker = new Thread(() -> {
            expect(IllegalStateException.class, () -> lease.uploaded(7, owner));
            expect(IllegalStateException.class, () -> lease.draw(7, owner));
            expect(IllegalStateException.class, () -> lease.retire(7, owner));
            expect(IllegalStateException.class, () -> lease.invalidate(7, owner));
            wrongThread.set(true);
        });
        worker.start();
        worker.join();
        require(wrongThread.get(), "worker must refuse every context operation");
        lease.retire(7, owner);
        expect(IllegalStateException.class, () -> lease.draw(7, owner));
        expect(IllegalStateException.class, () -> lease.uploaded(7, owner));
        lease.retire(6, owner); // An older lifecycle cannot reopen generation seven.
        expect(IllegalStateException.class, () -> lease.uploaded(7, owner));
        lease.uploaded(8, owner);
        lease.draw(8, owner);
        lease.retire(8, owner);
        expect(IllegalStateException.class, () -> lease.draw(8, owner));
    }

    private static void capabilities() {
        require(GeometryProof.selectBackend(true, false, true, true).equals("OpenGL15"), "core VBO");
        require(GeometryProof.selectBackend(false, true, true, true).equals("ARB_vertex_buffer_object"), "ARB VBO");
        expect(UnsupportedOperationException.class, () -> GeometryProof.selectBackend(false, false, true, true));
        expect(UnsupportedOperationException.class, () -> GeometryProof.selectBackend(true, true, false, true));
        expect(UnsupportedOperationException.class, () -> GeometryProof.selectBackend(true, true, true, false));
        require(GeometryProof.selectTextureUnits(1) == 1 && GeometryProof.selectTextureUnits(32) == 32,
                "bounded compatibility texture-unit endpoints");
        expect(UnsupportedOperationException.class, () -> GeometryProof.selectTextureUnits(0));
        expect(UnsupportedOperationException.class, () -> GeometryProof.selectTextureUnits(33));
    }

    private static ByteBuffer header(int status, int vertices, int indices, int materials) {
        return ByteBuffer.allocate(MeshWire.HEADER + vertices * 5 + indices * 4 + materials * 12)
                .order(ByteOrder.LITTLE_ENDIAN).putInt(0x354d5652).putInt(1).putLong(7).putLong(11)
                .putInt(status).putInt(vertices).putInt(indices).putInt(materials).putLong(0);
    }

    private static byte[] quad() {
        ByteBuffer bytes = header(0, 4, 6, 1);
        for (byte[] vertex : new byte[][]{{0, 0, 1, 5, 0}, {1, 0, 1, 5, 0}, {1, 1, 1, 5, 0}, {0, 1, 1, 5, 0}})
            bytes.put(vertex);
        for (int index : new int[]{0, 1, 2, 0, 2, 3}) bytes.putInt(index);
        return bytes.putInt(1).putInt(0).putInt(6).array();
    }

    private static void reject(byte[] bytes) {
        expect(IllegalArgumentException.class, () -> MeshWire.parse(bytes, 7, 11));
    }

    private static void expect(Class<? extends Throwable> expected, Runnable operation) {
        try {
            operation.run();
        } catch (Throwable failure) {
            if (expected.isInstance(failure)) return;
            throw new AssertionError("Wrong refusal type", failure);
        }
        throw new AssertionError("Expected refusal: " + expected.getSimpleName());
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
