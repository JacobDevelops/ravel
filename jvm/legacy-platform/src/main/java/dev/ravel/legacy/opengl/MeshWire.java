package dev.ravel.legacy.opengl;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/** Owned JNI copy of geometric cubes only: no UV, sprite, tint, baked light or AO fidelity. */
public final class MeshWire {
    public static final int HEADER = 48;
    public static final int MAX_VERTICES = 16 * 16 * 16 * 6 * 4;
    public static final int MAX_INDICES = MAX_VERTICES / 4 * 6;
    public static final int MAX_MATERIALS = MAX_VERTICES / 4;
    public static final int MAX_BYTES = HEADER + MAX_VERTICES * 5 + MAX_INDICES * 4 + MAX_MATERIALS * 12;
    public static final int SOLID_VERTEX_STRIDE = 6; // GL_SHORT XYZ; fixed-function GL_BYTE positions are invalid.
    public static final int MAX_SOLID_BYTES = MAX_VERTICES * SOLID_VERTEX_STRIDE + MAX_INDICES * 4;
    // Source-exact geometric face order and winding from ravel-renderer/src/mesh.rs.
    private static final int[][][] CORNERS = {
        {{0,0,0}, {0,0,1}, {0,1,1}, {0,1,0}}, {{1,0,1}, {1,0,0}, {1,1,0}, {1,1,1}},
        {{0,0,1}, {0,0,0}, {1,0,0}, {1,0,1}}, {{0,1,0}, {0,1,1}, {1,1,1}, {1,1,0}},
        {{1,0,0}, {0,0,0}, {0,1,0}, {1,1,0}}, {{0,0,1}, {1,0,1}, {1,1,1}, {0,1,1}}
    };
    private static final int[] QUAD_INDICES = {0, 1, 2, 0, 2, 3};
    public final long generation;
    public final long key;
    public final int status;
    public final int vertices;
    public final int indices;
    public final int materials;
    public final long visibility;
    private final ByteBuffer bytes;

    private MeshWire(ByteBuffer bytes, long generation, long key, int status, int vertices,
            int indices, int materials, long visibility) {
        this.bytes = bytes;
        this.generation = generation;
        this.key = key;
        this.status = status;
        this.vertices = vertices;
        this.indices = indices;
        this.materials = materials;
        this.visibility = visibility;
    }

    /** The caller transfers ownership of the array and must not mutate it after parsing. */
    public static MeshWire parse(byte[] copy, long generation, long key) {
        require(copy != null && copy.length >= HEADER && copy.length <= MAX_BYTES, "wire length");
        ByteBuffer bytes = ByteBuffer.wrap(copy).order(ByteOrder.LITTLE_ENDIAN);
        require(bytes.getInt() == 0x354d5652 && bytes.getInt() == 1, "wire magic/version");
        require(generation > 0 && key > 0 && bytes.getLong() == generation && bytes.getLong() == key,
                "wire generation/key");
        int status = bytes.getInt(), vertices = bytes.getInt(), indices = bytes.getInt();
        int materials = bytes.getInt();
        long visibility = bytes.getLong();
        require(status >= 0 && status <= 2 && (visibility & ~((1L << 36) - 1)) == 0, "wire outcome");
        require(vertices >= 0 && vertices <= MAX_VERTICES && vertices % 4 == 0
                && indices == vertices / 4 * 6 && materials >= 0 && materials <= MAX_MATERIALS,
                "wire counts");
        require(copy.length == HEADER + (long) vertices * 5 + (long) indices * 4 + (long) materials * 12,
                "wire exact length");
        require((indices == 0) == (materials == 0), "wire material coverage");
        require(status != 2 || (vertices == 0 && visibility == 0), "error mesh payload");
        for (int vertex = 0; vertex < vertices; vertex++) {
            require(Byte.toUnsignedInt(bytes.get()) <= 16 && Byte.toUnsignedInt(bytes.get()) <= 16
                    && Byte.toUnsignedInt(bytes.get()) <= 16 && Byte.toUnsignedInt(bytes.get()) < 6,
                    "wire vertex");
            bytes.get(); // Raw packed light: every byte value is valid, but not baked lighting.
        }
        for (int vertex = 0; vertex < vertices; vertex += 4) validateQuad(bytes, vertex);
        for (int index = 0; index < indices; index++)
            require(bytes.getInt() == index / 6 * 4 + QUAD_INDICES[index % 6], "wire quad index");
        long first = 0;
        for (int material = 0; material < materials; material++) {
            require(bytes.getInt() != 0, "air material");
            long start = Integer.toUnsignedLong(bytes.getInt());
            long count = Integer.toUnsignedLong(bytes.getInt());
            require(start == first && count > 0 && count % 6 == 0 && count <= indices - first,
                    "wire material range");
            first += count;
        }
        require(first == indices, "wire uncovered indices");
        return new MeshWire(bytes.asReadOnlyBuffer().order(ByteOrder.LITTLE_ENDIAN), generation, key,
                status, vertices, indices, materials, visibility);
    }

    public boolean drawable() { return status == 0 && vertices > 0; }
    private int vertexBytes() { return vertices * 5; }
    public int indexBytes() { return indices * 4; }
    public int byteSize() { return bytes.capacity(); }
    public int solidVertexBytes() { return vertices * SOLID_VERTEX_STRIDE; }

    /** Packs native-order signed SHORT XYZ followed by unsigned INT indices; face/light stay CPU-only. */
    public void stageSolid(ByteBuffer destination) {
        require(destination.order() == ByteOrder.nativeOrder(), "solid staging must use native byte order");
        require(destination.remaining() >= solidVertexBytes() + indexBytes(), "solid staging capacity");
        for (int vertex = 0; vertex < vertices; vertex++)
            for (int axis = 0; axis < 3; axis++) destination.putShort((short) coordinate(vertex, axis));
        ByteBuffer source = bytes.slice(HEADER + vertexBytes(), indexBytes()).order(ByteOrder.LITTLE_ENDIAN);
        if (ByteOrder.nativeOrder() == ByteOrder.LITTLE_ENDIAN) destination.put(source);
        else while (source.hasRemaining()) destination.putInt(source.getInt());
    }
    public int coordinate(int vertex, int axis) {
        return Byte.toUnsignedInt(bytes.get(HEADER + vertex * 5 + axis));
    }
    public int index(int index) { return bytes.getInt(HEADER + vertexBytes() + index * 4); }

    private static void validateQuad(ByteBuffer bytes, int vertex) {
        int at = HEADER + vertex * 5;
        int face = Byte.toUnsignedInt(bytes.get(at + 3));
        byte light = bytes.get(at + 4);
        for (int corner = 0; corner < 4; corner++) {
            require(bytes.get(at + corner * 5 + 3) == face && bytes.get(at + corner * 5 + 4) == light,
                    "wire quad face/light");
            for (int axis = 0; axis < 3; axis++) {
                int origin = Byte.toUnsignedInt(bytes.get(at + axis)) - CORNERS[face][0][axis];
                require(origin >= 0 && origin < 16 && Byte.toUnsignedInt(bytes.get(at + corner * 5 + axis))
                        == origin + CORNERS[face][corner][axis], "wire cube corner");
            }
        }
    }

    private static void require(boolean valid, String reason) {
        if (!valid) throw new IllegalArgumentException(reason);
    }
}
