package dev.ravel.legacy.opengl;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL15;

/** One bounded resident section, submitted at vanilla's own VBO draw site. */
public final class ResidentWorldDraw {
    private static final boolean SELECTED = System.getProperty("ravel.renderer", "vanilla").equals("resident-world");
    private static int layer = -1;
    private static int vanilla;
    private static int resident;
    private static int vertices;
    private static int offset;
    private static final long[] METRICS = new long[11];
    private static boolean replacedThisFrame;
    private static int frameDraws;
    private static int frameVertices;
    private static int admittedDraws;
    private static int admittedVertices;

    private ResidentWorldDraw() { }

    static void layer(int current, int source, int buffer, int count, int start) {
        layer = current; vanilla = source; resident = buffer; vertices = count; offset = start;
    }

    static boolean draw(int mode, int first, int count) {
        if (layer < 0 || count <= 0) return false;
        if (layer >= 3 || vanilla == 0 || resident == 0
                || mode != GL11.GL_QUADS || first != 0 || count != vertices) {
            METRICS[4 + layer]++;
            return false;
        }
        int bound = GL11.glGetInteger(GL15.GL_ARRAY_BUFFER_BINDING);
        boolean admitted = layer < 3 && vanilla != 0 && bound == vanilla && resident != 0
                && mode == GL11.GL_QUADS && first == 0 && count == vertices;
        if (admitted) { admittedDraws++; admittedVertices += vertices; }
        if (!admitted || ProductionFrames.baseline()) {
            METRICS[4 + layer]++;
            return false;
        }
        if (ProductionFrames.omitting()) return true;
        CompositeFrame.submit(mode, count, offset, resident, bound);
        METRICS[layer]++;
        replacedThisFrame = true;
        frameDraws++;
        frameVertices += count;
        return true;
    }

    static void presented(int width, int height) {
        if (!SELECTED) return;
        METRICS[8]++;
        if (replacedThisFrame) METRICS[9]++;
        METRICS[10] = Math.max(METRICS[10], replacedThisFrame ? 1 : 0);
        admittedDraws = 0; admittedVertices = 0;
        replacedThisFrame = false; frameDraws = 0; frameVertices = 0;
        layer = -1;
    }

    public static void worldRendered() {
        if (!SELECTED || CompositeFrame.replaying()) return;
        ProductionFrames.presented(frameDraws, frameVertices, admittedDraws, admittedVertices,
                Display.getWidth(), Display.getHeight());
    }

    public static long[] metrics() { return METRICS.clone(); }
}
